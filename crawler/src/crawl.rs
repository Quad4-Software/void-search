use crate::blocklist::Blocklist;
use crate::challenge::{ChallengeKind, ChallengeSolver};
use crate::config::Config;
use crate::extract::extract;
use crate::fetcher::{FetchError, Fetcher};
use crate::frontier::Frontier;
use crate::hister;
use crate::index::{SearchIndex, now_epoch};
use crate::robots::RobotsCache;
use crate::store::{DocRecord, FrontierItem, Store};
use crate::urlnorm;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Duration;
use tokio::sync::Semaphore;
use tracing::{debug, info, warn};
use url::Url;

pub struct Crawler {
    cfg: Config,
    frontier: Arc<Frontier>,
    store: Arc<Store>,
    index: Arc<SearchIndex>,
    fetcher: Fetcher,
    robots: Arc<RobotsCache>,
    blocklist: Arc<Blocklist>,
    solver: ChallengeSolver,
    hister: hister::HisterClient,
    authority: Arc<dashmap::DashMap<String, u64>>,
    /// pages stored per site - enforces the per-site crawl budget
    site_pages: Arc<dashmap::DashMap<String, u32>>,
    /// normalized-content hashes for near-dup skipping
    content_hashes: Arc<dashmap::DashSet<u64>>,
    /// hosts whose robots sitemaps were already enqueued
    sitemaps_seen: Arc<dashmap::DashSet<String>>,
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
        let robots = Arc::new(RobotsCache::new(
            fetcher.client().clone(),
            cfg.identity.user_agent.clone(),
        ));
        let blocklist = Arc::new(Blocklist::load(
            &cfg.store.path,
            &cfg.blocklists.extra_files,
        ));
        let frontier = Arc::new(Frontier::new(store.clone(), cfg.crawl.min_delay_ms));
        let solver = ChallengeSolver::new(cfg.challenge.clone(), fetcher.client().clone());
        let hister = hister::HisterClient::new(
            &cfg.hister.url,
            &cfg.hister.token,
            fetcher.client().clone(),
            cfg.hister.batch_size,
        );

        Ok(Self {
            cfg,
            frontier,
            store,
            index,
            fetcher,
            robots,
            blocklist,
            solver,
            hister,
            authority: Arc::new(dashmap::DashMap::new()),
            site_pages: Arc::new(dashmap::DashMap::new()),
            content_hashes: Arc::new(dashmap::DashSet::new()),
            sitemaps_seen: Arc::new(dashmap::DashSet::new()),
            docs_done: Arc::new(AtomicU64::new(0)),
        })
    }

    pub fn index(&self) -> Arc<SearchIndex> {
        self.index.clone()
    }

    pub fn store(&self) -> Arc<Store> {
        self.store.clone()
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
            let Some(u) = urlnorm::normalize(raw) else {
                continue;
            };
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
        let hst = self.hister.clone();
        let maint = tokio::spawn(async move {
            loop {
                tokio::time::sleep(Duration::from_secs(30)).await;
                let _ = idx.commit();
                fr.persist_snapshot();
                let _ = hst.flush().await;
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
        if self.hister.enabled() {
            let _ = self.hister.flush().await;
        }
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
                    if addrs.is_empty()
                        || !addrs.iter().any(|a| {
                            Blocklist::ip_allowed(a.ip(), self.cfg.crawl.allow_private_ips)
                        })
                    {
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
                && until > now_epoch()
            {
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

            // first visit to a host: grab its advertised sitemaps so we
            // learn the url inventory instead of only link-following
            if self.sitemaps_seen.insert(site.clone()) {
                for sm in self.robots.sitemaps_for(&url).await {
                    if let Ok(su) = Url::parse(&sm) {
                        self.frontier.push(FrontierItem {
                            url: su.to_string(),
                            depth: item.depth.saturating_add(1),
                            score: item.score,
                        });
                    }
                }
            }

            // per-site crawl budget
            let site_pages = self.site_pages.get(&site).map(|v| *v.value()).unwrap_or(0);
            if site_pages >= self.cfg.crawl.max_pages_per_site {
                self.frontier.done(&site, true, delay_ms);
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

                    // sitemaps are mined for <loc> urls, not indexed
                    if crate::extract::is_sitemap(&body) {
                        for l in crate::extract::sitemap_urls(&body) {
                            if let Some(nu) = urlnorm::normalize(&l)
                                && !self.blocklist.blocked(&urlnorm::host_of(&nu))
                            {
                                self.frontier.push(FrontierItem {
                                    url: nu.to_string(),
                                    depth: item.depth + 1,
                                    score: item.score,
                                });
                            }
                        }
                        self.frontier.done(&site, true, delay_ms);
                        continue;
                    }

                    // feeds are mined for links, not indexed
                    let feed_links = crate::extract::feed_links(&body);
                    if !feed_links.is_empty() {
                        for l in feed_links {
                            if let Some(nu) = urlnorm::normalize(&l)
                                && !self.blocklist.blocked(&urlnorm::host_of(&nu))
                            {
                                self.frontier.push(FrontierItem {
                                    url: nu.to_string(),
                                    depth: item.depth + 1,
                                    score: item.score,
                                });
                            }
                        }
                        self.frontier.done(&site, true, delay_ms);
                        continue;
                    }

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

                    // near-dup content check - same normalized text seen
                    // before means mirror/parked content, skip indexing
                    let norm: String = parsed
                        .text
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .to_lowercase();
                    let chash =
                        xxhash_rust::xxh3::xxh3_64(norm.get(..4096).unwrap_or(&norm).as_bytes());
                    if !self.content_hashes.insert(chash) {
                        debug!(%url, "duplicate content skipped");
                        self.frontier.done(&site, true, delay_ms);
                        continue;
                    }

                    let host_auth =
                        self.authority.get(&host).map(|v| *v.value()).unwrap_or(0) as f64;
                    let authority = (1.0f64 + host_auth).ln();
                    *self.site_pages.entry(site.clone()).or_insert(0) += 1;

                    let doc = DocRecord {
                        url: resp.final_url.to_string(),
                        title: parsed.title.clone(),
                        host: host.clone(),
                        fetched_at: now_epoch(),
                        text_z: zstd::encode_all(parsed.text.as_bytes(), 3).unwrap_or_default(),
                        links: parsed.links.clone(),
                    };
                    let _ = self.store.put_doc(&doc);
                    if self.hister.enabled() {
                        self.hister
                            .queue(&doc.url, &host, &doc.title, &parsed.text, doc.fetched_at);
                    }
                    let _ = self.index.add_doc(
                        &doc.url,
                        &host,
                        &doc.title,
                        &parsed.text,
                        &parsed.description,
                        doc.fetched_at,
                        authority,
                        parsed.links.len() as u64,
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

impl Crawler {
    /// dump the doc store as jsonl - bridge format for hister and friends
    pub fn export_jsonl(&self, out: &std::path::Path) -> anyhow::Result<usize> {
        use std::io::BufWriter;
        let mut n = 0usize;
        let mut w = BufWriter::new(std::fs::File::create(out)?);
        for doc in self.store.iter_docs()? {
            let text = zstd::decode_all(doc.text_z.as_slice()).unwrap_or_default();
            let line = serde_json::json!({
                "url": doc.url,
                "title": doc.title,
                "host": doc.host,
                "fetched_at": doc.fetched_at,
                "text": String::from_utf8_lossy(&text),
            });
            serde_json::to_writer(&mut w, &line)?;
            std::io::Write::write_all(&mut w, b"\n")?;
            n += 1;
        }
        std::io::Write::flush(&mut w)?;
        Ok(n)
    }

    /// hister export layout: a json array where every document sits on one
    /// line starting with { and commas live on their own lines, matching what
    /// `hister import file` parses
    pub fn export_hister(&self, out: &std::path::Path) -> anyhow::Result<usize> {
        use std::io::Write;
        let mut n = 0usize;
        let mut w = std::io::BufWriter::new(std::fs::File::create(out)?);
        writeln!(w, "[")?;
        for doc in self.store.iter_docs()? {
            let text = zstd::decode_all(doc.text_z.as_slice()).unwrap_or_default();
            let line = serde_json::json!({
                "url": doc.url,
                "domain": doc.host,
                "title": doc.title,
                "text": String::from_utf8_lossy(&text),
                "added": doc.fetched_at,
                "updated": doc.fetched_at,
                "type": 0,
                "label": "void-crawler",
            });
            if n > 0 {
                writeln!(w, ",")?;
            }
            serde_json::to_writer(&mut w, &line)?;
            writeln!(w)?;
            n += 1;
        }
        writeln!(w, "]")?;
        w.flush()?;
        Ok(n)
    }

    /// import jsonl docs (url/title/host/fetched_at/text) into store+index.
    /// also accepts hister export files - non-{ lines are skipped and the
    /// hister field names domain/text/added map to ours
    pub fn import_jsonl(&self, input: &std::path::Path) -> anyhow::Result<usize> {
        use std::io::BufRead;
        let mut n = 0usize;
        for line in std::io::BufReader::new(std::fs::File::open(input)?).lines() {
            let line = line?;
            let t = line.trim();
            if !t.starts_with('{') {
                continue;
            }
            let v: serde_json::Value = serde_json::from_str(t)?;
            let url = v["url"].as_str().unwrap_or("");
            let title = v["title"].as_str().unwrap_or("");
            let host = v["host"].as_str().or(v["domain"].as_str()).unwrap_or("");
            let fetched = v["fetched_at"]
                .as_u64()
                .or(v["added"].as_u64())
                .or(v["updated"].as_u64())
                .unwrap_or_else(crate::index::now_epoch);
            let text = v["text"].as_str().unwrap_or("");
            if url.is_empty() || !url.starts_with("http") {
                continue;
            }
            let desc = text.get(..160).unwrap_or(text);
            let doc = crate::store::DocRecord {
                url: url.into(),
                title: title.into(),
                host: host.into(),
                fetched_at: fetched,
                text_z: zstd::encode_all(text.as_bytes(), 3)?,
                links: vec![],
            };
            self.store.put_doc(&doc)?;
            self.index
                .add_doc(url, host, title, text, desc, fetched, 0.0, 0)?;
            n += 1;
        }
        Ok(n)
    }
}
