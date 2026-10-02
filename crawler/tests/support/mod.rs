use std::path::Path;
use void_crawler::*;

pub fn test_config(dir: &Path) -> Config {
    let mut cfg = Config::default();
    cfg.crawl.allow_private_ips = true;
    cfg.crawl.min_delay_ms = 10;
    cfg.crawl.workers = 4;
    cfg.crawl.fetch_timeout_secs = 10;
    cfg.crawl.max_depth = 3;
    cfg.store.path = dir.join("data");
    cfg.api.listen = "127.0.0.1:0".into();
    cfg
}

pub fn make_crawler(cfg: Config) -> anyhow::Result<Crawler> {
    Crawler::new(cfg)
}

pub fn app_state(c: Crawler) -> AppState {
    AppState {
        index: c.index(),
        doc_count: c.docs_done(),
    }
}

pub use void_crawler::server::router;
