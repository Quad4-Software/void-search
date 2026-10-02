use crate::store::{FrontierItem, Store};
use crate::urlnorm;
use dashmap::DashMap;
use std::collections::VecDeque;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};
use tracing::{debug, warn};
use url::Url;

/// per-site politeness bucket: enforces crawl-delay + in-flight limit
struct SiteState {
    next_allowed: Instant,
    failures: u32,
    in_flight: bool,
}

/// politeness-first frontier: urls grouped by site key, a site releases its
/// next url only when its delay has elapsed and nothing is in flight
pub struct Frontier {
    queues: DashMap<String, VecDeque<(u64, FrontierItem)>>,
    sites: DashMap<String, SiteState>,
    store: Arc<Store>,
    pending: AtomicUsize,
    default_delay_ms: u64,
}

impl Frontier {
    pub fn new(store: Arc<Store>, min_delay_ms: u64) -> Self {
        Self {
            queues: DashMap::new(),
            sites: DashMap::new(),
            store,
            pending: AtomicUsize::new(0),
            default_delay_ms: min_delay_ms.max(500),
        }
    }

    /// add a url after normalization, dedup and blocklist check happen in
    /// the caller; returns true when enqueued
    pub fn push(&self, item: FrontierItem) -> bool {
        let Ok(url) = Url::parse(&item.url) else {
            return false;
        };
        let site = urlnorm::site_key(&urlnorm::host_of(&url));
        let key = urlnorm::url_key(&url);
        if self.store.seen(key) {
            return false;
        }
        let _ = self.store.mark_seen(key);
        let mut q = self.queues.entry(site).or_default();
        q.push_back((key, item));
        self.pending.fetch_add(1, Ordering::Relaxed);
        true
    }

    /// lease the next url ready under politeness rules; the caller must call
    /// done() when finished so in_flight resets
    pub fn lease(&self) -> Option<(u64, FrontierItem, String)> {
        let now = Instant::now();
        for mut entry in self.queues.iter_mut() {
            let site = entry.key().clone();
            let mut state = self.sites.entry(site.clone()).or_insert_with(|| SiteState {
                next_allowed: Instant::now(),
                failures: 0,
                in_flight: false,
            });
            if state.in_flight || state.next_allowed > now {
                continue;
            }
            if let Some((key, item)) = entry.pop_front() {
                state.in_flight = true;
                self.pending.fetch_sub(1, Ordering::Relaxed);
                return Some((key, item, site));
            }
        }
        // nothing ready - pull persisted frontier items into queues
        self.refill();
        for mut entry in self.queues.iter_mut() {
            let site = entry.key().clone();
            let mut state = self.sites.entry(site.clone()).or_insert_with(|| SiteState {
                next_allowed: Instant::now(),
                failures: 0,
                in_flight: false,
            });
            if state.in_flight || state.next_allowed > now {
                continue;
            }
            if let Some((key, item)) = entry.pop_front() {
                state.in_flight = true;
                self.pending.fetch_sub(1, Ordering::Relaxed);
                return Some((key, item, site));
            }
        }
        None
    }

    fn refill(&self) {
        match self.store.pop_frontier(256) {
            Ok(items) => {
                for (key, item) in items {
                    let Ok(url) = Url::parse(&item.url) else {
                        continue;
                    };
                    let site = urlnorm::site_key(&urlnorm::host_of(&url));
                    self.queues.entry(site).or_default().push_back((key, item));
                    self.pending.fetch_add(1, Ordering::Relaxed);
                }
            }
            Err(e) => warn!(error = %e, "frontier refill failed"),
        }
    }

    /// mark site done - schedule next allowed fetch respecting crawl-delay
    /// and error backoff; success clears the failure streak
    pub fn done(&self, site: &str, ok: bool, crawl_delay_ms: u64) {
        let mut state = match self.sites.get_mut(site) {
            Some(s) => s,
            None => return,
        };
        state.in_flight = false;
        let base = crawl_delay_ms.max(self.default_delay_ms);
        let delay = if ok {
            state.failures = 0;
            Duration::from_millis(base)
        } else {
            state.failures = state.failures.saturating_add(1);
            let mult = 1u64 << state.failures.min(6);
            Duration::from_millis(base.saturating_mul(mult))
        };
        state.next_allowed = Instant::now() + delay;
    }

    /// consecutive failures beyond this mean the site is effectively banned
    /// for a while - backoff handled via done() plus a long cooldown
    pub fn failures(&self, site: &str) -> u32 {
        self.sites.get(site).map(|s| s.failures).unwrap_or(0)
    }

    pub fn penalize(&self, site: &str, secs: u64) {
        if let Some(mut s) = self.sites.get_mut(site) {
            s.next_allowed = Instant::now() + Duration::from_secs(secs);
        }
    }

    pub fn pending(&self) -> usize {
        self.pending.load(Ordering::Relaxed)
    }

    pub fn is_idle(&self) -> bool {
        self.pending() == 0 && self.store.frontier_len() == 0 && !self.any_in_flight()
    }

    fn any_in_flight(&self) -> bool {
        self.sites.iter().any(|s| s.in_flight)
    }

    /// persist all queued items to redb (call on shutdown)
    pub fn flush(&self) {
        let mut n = 0usize;
        for mut entry in self.queues.iter_mut() {
            while let Some((key, item)) = entry.pop_front() {
                if let Err(e) = self.store.push_frontier(&item, key) {
                    warn!(error = %e, "frontier flush failed");
                }
                n += 1;
            }
        }
        debug!(n, "flushed frontier to store");
    }

    /// snapshot queued items to redb without draining - a crash then resumes
    /// from the persisted frontier, duplicates are deduped by the seen set
    pub fn persist_snapshot(&self) {
        for entry in self.queues.iter() {
            for (key, item) in entry.iter() {
                let _ = self.store.push_frontier(item, *key);
            }
        }
    }
}
