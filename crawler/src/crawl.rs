use crate::blocklist::Blocklist;
use crate::challenge::{ChallengeKind, ChallengeSolver};
use crate::config::Config;
use crate::extract::extract;
use crate::fetcher::{FetchError, Fetcher};
use crate::frontier::Frontier;
use crate::index::{now_epoch, SearchIndex};
use crate::robots::RobotsCache;
use crate::store::{DocRecord, FrontierItem, Store};
use crate::urlnorm;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};

pub struct Crawler {
    cfg: Config,
    frontier: Arc<Frontier>,
    store: Arc<Store>,
    index: Arc<SearchIndex>,
    fetcher: Fetcher,
    robots: Arc<RobotsCache>,
    blocklist: Arc<Blocklist>,
    solver: ChallengeSolver,
    authority: Arc<dashmap::DashMap<String, u64>>,
    docs_done: Arc<AtomicU64>,
}

impl Crawler {
    pub fn new(cfg: Config) -> anyhow::Result<Self> {
        let store = Arc::new(Store::open(&cfg.store.path)?);
        let index = Arc::new(SearchIndex::open(
            &cfg.store.path.join("index"),
            cfg.index.writer_heap_mb,
        )?);
        let fetcher = Fetcher::new(&cfg.crawl, &cfg.identity.user_agent)?;
        let robots = Arc::new(RobotsCache::new(fetcher.client().clone(), cfg.identity.user_agent.clone()));
        let blocklist = Arc::new(Blocklist::load(&cfg.store.path, &cfg.blocklists.extra_files));
        let frontier = Arc::new(Frontier::new(store.clone(), cfg.crawl.min_delay_ms));
        let solver = ChallengeSolver::new(cfg.challenge.clone(), fetcher.client().clone());

        Ok(Self {
            cfg,
            frontier,
            store,
            index,
            fetcher,
            robots,
            blocklist,
            solver,
            authority: Arc::new(dashmap::DashMap::new()),
            docs_done: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn index(&self) -> Arc<SearchIndex> {
        self.index.clone()
    }

    pub fn store_doc_count(&self) -> u64 {
        self.store.doc_count()
    }

    pub fn docs_done(&self) -> Arc<AtomicU64> {
        self.docs_done.clone()
    }

    /// seed the frontier; persists unseen seeds to redb so restarts resume
    pub fn seed(&self, urls: &[String]) -> usize {
        let mut n = 0;
        for raw in urls {
            let Some(u) = urlnorm::normalize(raw) else { continue };
            let key = urlnorm::url_key(&u);
            if self.store.seen(key) {
                continue;
            }
            let item = FrontierItem {
                url: u.to_string(),
                depth: 0,
                score: 1.0,
            };
            if self.store.push_frontier(&item, key).is_ok() {
                n += 1;
            }
        }
        n
    }

    /// main crawl loop - spawns workers that lease urls under politeness rules
    pub async fn run(self: Arc<Self>) {
        info!(workers = self.cfg.crawl.workers, "crawler starting");
        let idle_guard = Arc::new(Semaphore::new(1));

        // periodic commit so an untimely kill does not lose indexed docs,
        // plus a frontier flush so queued urls survive a restart
        let idx = self.index.clone();
        let fr = self.frontier.clone();
        let maint = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let _ = idx.commit();
                fr.persist_snapshot();
            }
        });

        let mut handles = Vec::new();
        for wid in 0..self.cfg.crawl.workers {
            let this = self.clone();
            let idle = idle_guard.clone();
            handles.push(tokio::spawn(async move {
                this.worker(wid, idle).await;
            }));
        }
        for h in handles {
            let _ = h.await;
        }
        maint.abort();
        self.frontier.flush();
        let _ = self.index.commit();
        info!("crawler stopped");
    }

    async fn worker(&self, _wid: usize, idle_guard: Arc<Semaphore>) {
        loop {
            if self.frontier.is_idle() {
                // acquire means "i saw it idle"; if it still is, exit
                let _permit = idle_guard.try_acquire();
                tokio::time::sleep(Duration::from_secs(2)).await;
                if self.frontier.is_idle() {
                    return;
                }
                continue;
            }

            let Some((key, item, site)) = self.frontier.lease() else {
                tokio::time::sleep(Duration::from_millis(200)).await;
                continue;
            };

            if item.depth > self.cfg.crawl.max_depth {
                self.frontier.done(&site, true, 0);
                continue;
            }

            let url_str = item.url.clone();
            let Some(url) = urlnorm::normalize(&url_str) else {
                self.frontier.done(&site, true, 0);
                continue;
            };

            let host = urlnorm::host_of(&url);

            // blocklist + ssrf guard before dns
            if self.blocklist.blocked(&host) {
                debug!(host, "blocked host");
                self.frontier.done(&site, true, 0);
                continue;
            }
            // refuse hosts that resolve to private/loopback space
            match tokio::net::lookup_host((host.as_str(), 443)).await {
                Ok(addrs) => {
                    let addrs: Vec<_> = addrs.collect();
                    if addrs.is_empty() || !addrs.iter().any(|a| Blocklist::ip_allowed(a.ip(), self.cfg.crawl.allow_private_ips)) {
                        debug!(host, "refusing private/unroutable resolution");
                        self.frontier.done(&site, true, 0);
                        continue;
                    }
                }
                Err(_) => {
                    self.frontier.done(&site, false, 0);
                    continue;
                }
            }
            if let Some(until) = self.store.host_banned_until(&site)
                && until > now_epoch() {
                    self.frontier.penalize(&site, until - now_epoch());
                    self.frontier.done(&site, true, 0);
                    continue;
                }

            // robots.txt
            let (allowed, delay_ms) = self.robots.check(&url).await;
            if !allowed {
                debug!(%url, "disallowed by robots.txt");
                self.frontier.done(&site, true, 0);
                continue;
            }

            match self.fetcher.get(url.as_str()).await {
                Ok(resp) => {
                    let body = resp.body;
                    let kind = ChallengeSolver::looks_challenged(&body, resp.status.as_u16());
                    let body = match kind {
                        ChallengeKind::Anubis => {
                            match self.solver.solve_anubis(&body, &resp.final_url).await {
                                Some(cleared) => cleared,
                                None => {
                                    self.frontier.done(&site, false, delay_ms);
                                    continue;
                                }
                            }
                        }
                        ChallengeKind::Cloudflare => {
                            match self.solver.solve_flaresolverr(url.as_str()).await {
                                Some(cleared) => cleared,
                                None => {
                                    self.frontier.done(&site, false, delay_ms);
                                    continue;
                                }
                            }
                        }
                        ChallengeKind::None => body,
                    };

                    let parsed = extract(&body, &resp.final_url);
                    if parsed.noindex {
                        self.frontier.done(&site, true, delay_ms);
                        continue;
                    }

                    // inlink-based authority, cheap pagerank-lite
                    for l in &parsed.links {
                        if let Some(nu) = urlnorm::normalize(l) {
                            *self.authority.entry(urlnorm::host_of(&nu)).or_insert(0) += 1;
                        }
                    }

                    let host_auth = self.authority.get(&host).map(|v| *v.value()).unwrap_or(0) as f64;
                    let authority = (1.0f64 + host_auth).ln();

                    let doc = DocRecord {
                        url: resp.final_url.to_string(),
                        title: parsed.title.clone(),
                        host: host.clone(),
                        fetched_at: now_epoch(),
                        text_z: zstd::encode_all(parsed.text.as_bytes(), 3).unwrap_or_default(),
                        links: parsed.links.clone(),
                    };
                    let _ = self.store.put_doc(&doc);
                    let _ = self.index.add_doc(
                        &doc.url, &host, &doc.title, &parsed.text, &parsed.description,
                        doc.fetched_at, authority, parsed.links.len() as u64,
                    );

                    let n = self.docs_done.fetch_add(1, Ordering::Relaxed) + 1;
                    if n.is_multiple_of(self.cfg.index.commit_every_docs as u64) {
                        let _ = self.index.commit();
                    }

                    // enqueue outlinks within depth
                    let depth = item.depth + 1;
                    if depth <= self.cfg.crawl.max_depth {
                        for l in parsed.links {
                            if let Some(nu) = urlnorm::normalize(&l) {
                                if self.blocklist.blocked(&urlnorm::host_of(&nu)) {
                                    continue;
                                }
                                // push() checks and marks seen itself
                                self.frontier.push(FrontierItem {
                                    url: nu.to_string(),
                                    depth,
                                    score: item.score * 0.9,
                                });
                            }
                        }
                    }
                    let _ = key;
                    self.frontier.done(&site, true, delay_ms);
                }
                Err(e) => {
                    match &e {
                        FetchError::Status(s) if s.as_u16() == 403 || s.as_u16() == 503 => {
                            // maybe a challenge page - try solvers once
                            debug!(%url, status = %s, "attempting challenge solvers");
                            if self.solver.solve_flaresolverr(url.as_str()).await.is_none() {
                                self.frontier.done(&site, false, delay_ms);
                                continue;
                            }
                            self.frontier.done(&site, true, delay_ms);
                        }
                        FetchError::Status(s) if s.as_u16() == 429 => {
                            warn!(host, "429 - backing off");
                            self.frontier.done(&site, false, delay_ms);
                            self.frontier.penalize(&site, 60);
                        }
                        FetchError::SlowDrip | FetchError::Timeout => {
                            warn!(host, error = %e, "possible tarpit");
                            let fails = self.frontier.failures(&site);
                            if fails > 3 {
                                let _ = self.store.ban_host(&site, now_epoch() + 3600);
                            }
                            self.frontier.done(&site, false, delay_ms);
                        }
                        _ => {
                            debug!(%url, error = %e, "fetch failed");
                            self.frontier.done(&site, false, delay_ms);
                        }
                    }
                }
            }
        }
    }
}
