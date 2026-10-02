use serde::Deserialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Config {
    pub identity: Identity,
    pub crawl: Crawl,
    pub frontier: FrontierCfg,
    pub challenge: Challenge,
    pub store: Store,
    pub index: IndexCfg,
    pub api: Api,
    pub blocklists: Blocklists,
    /// live document push into a hister server while crawling
    pub hister: Hister,
    pub seeds: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Hister {
    /// base url of the hister server, empty disables live push
    pub url: String,
    /// access token - matches hister's app.access_token
    pub token: String,
    /// docs per /api/batch call, server caps at 100
    pub batch_size: usize,
}

impl Default for Hister {
    fn default() -> Self {
        Self {
            url: String::new(),
            token: String::new(),
            batch_size: 50,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Identity {
    pub user_agent: String,
    pub contact: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Crawl {
    pub workers: usize,
    pub min_delay_ms: u64,
    pub max_retries: u32,
    pub fetch_timeout_secs: u64,
    pub connect_timeout_secs: u64,
    pub max_body_bytes: usize,
    pub max_redirects: usize,
    pub max_depth: u32,
    pub slow_drip_min_bytes: usize,
    pub slow_drip_floor_kbps: u64,
    /// allow private/loopback ips - off by default, tests turn it on
    pub allow_private_ips: bool,
    /// outbound proxy for fetches, e.g. socks5h://10.64.0.1:1080 for the
    /// mullvad in-tunnel socks5 proxy. empty means direct egress.
    pub proxy: String,
    /// crawl budget per site - keeps a giant site from eating the frontier
    pub max_pages_per_site: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct FrontierCfg {
    pub max_pending: usize,
    pub refill_batch: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Challenge {
    pub flaresolverr_url: String,
    pub flaresolverr_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Store {
    pub path: PathBuf,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct IndexCfg {
    pub writer_heap_mb: usize,
    pub commit_every_docs: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Api {
    pub listen: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
#[derive(Default)]
pub struct Blocklists {
    pub extra_files: Vec<PathBuf>,
}

impl Default for Identity {
    fn default() -> Self {
        Self {
            user_agent: format!(
                "VoidCrawler/{} (+https://void.quad4.io/bot)",
                env!("CARGO_PKG_VERSION")
            ),
            contact: "https://void.quad4.io/info/en/about".into(),
        }
    }
}

impl Default for Crawl {
    fn default() -> Self {
        Self {
            workers: 8,
            min_delay_ms: 2000,
            max_retries: 2,
            fetch_timeout_secs: 20,
            connect_timeout_secs: 8,
            max_body_bytes: 2 * 1024 * 1024,
            max_redirects: 5,
            max_depth: 3,
            slow_drip_min_bytes: 64 * 1024,
            slow_drip_floor_kbps: 8,
            allow_private_ips: false,
            proxy: String::new(),
            max_pages_per_site: 500,
        }
    }
}

impl Default for FrontierCfg {
    fn default() -> Self {
        Self {
            max_pending: 100_000,
            refill_batch: 256,
        }
    }
}

impl Default for Challenge {
    fn default() -> Self {
        Self {
            flaresolverr_url: String::new(),
            flaresolverr_timeout_ms: 60_000,
        }
    }
}

impl Default for Store {
    fn default() -> Self {
        Self {
            path: PathBuf::from("data"),
        }
    }
}

impl Default for IndexCfg {
    fn default() -> Self {
        Self {
            writer_heap_mb: 256,
            commit_every_docs: 500,
        }
    }
}

impl Default for Api {
    fn default() -> Self {
        Self {
            listen: "127.0.0.1:8088".into(),
        }
    }
}

impl Config {
    pub fn load(path: &std::path::Path) -> anyhow::Result<Self> {
        let text = std::fs::read_to_string(path)?;
        let mut cfg: Config = toml::from_str(&text)?;
        cfg.apply_env();
        Ok(cfg)
    }

    /// env overrides so containers can run without a mounted config file
    pub fn apply_env(&mut self) {
        if let Ok(v) = std::env::var("VC_LISTEN") {
            self.api.listen = v;
        }
        if let Ok(v) = std::env::var("VC_DATA_DIR") {
            self.store.path = std::path::PathBuf::from(v);
        }
        if let Ok(v) = std::env::var("VC_USER_AGENT") {
            self.identity.user_agent = v;
        }
        if let Ok(v) = std::env::var("VC_FLARESOLVERR") {
            self.challenge.flaresolverr_url = v;
        }
        if let Ok(v) = std::env::var("VC_PROXY") {
            self.crawl.proxy = v;
        }
        if let Ok(v) = std::env::var("VC_HISTER_URL") {
            self.hister.url = v;
        }
        if let Ok(v) = std::env::var("VC_HISTER_TOKEN") {
            self.hister.token = v;
        }
        if let Ok(v) = std::env::var("VC_MAX_PAGES_PER_SITE")
            && let Ok(n) = v.parse()
        {
            self.crawl.max_pages_per_site = n;
        }
        if let Ok(v) = std::env::var("VC_WORKERS")
            && let Ok(n) = v.parse()
        {
            self.crawl.workers = n;
        }
    }
}
