use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::sync::Arc;
use tracing_subscriber::EnvFilter;
use void_crawler::config::Config;
use void_crawler::{crawl, extract, fetcher, server};

#[derive(Parser)]
#[command(name = "void-crawler", about = "Independent crawler and index for Void Search")]
struct Cli {
    #[arg(short, long, default_value = "crawler.toml")]
    config: PathBuf,

    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// run the crawl loop
    Crawl {
        /// seed urls file, one per line, or repeat --seed
        #[arg(long)]
        seeds_file: Option<PathBuf>,
        #[arg(long)]
        seed: Vec<String>,
    },
    /// serve the search api
    Serve,
    /// crawl and serve the api in one process
    Run {
        #[arg(long)]
        seeds_file: Option<PathBuf>,
        #[arg(long)]
        seed: Vec<String>,
    },
    /// print index/frontier counters
    Stats,
    /// one-off fetch of a single url, prints extracted fields
    Fetch {
        url: String,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")))
        .init();

    let cli = Cli::parse();
    let cfg = if cli.config.exists() {
        Config::load(&cli.config)?
    } else {
        let mut c = Config::default();
        c.apply_env();
        c
    };

    match cli.command {
        Cmd::Crawl { seeds_file, seed } => {
            let mut seeds = crawler_seed_defaults();
            seeds.extend(cfg.seeds.iter().cloned());
            seeds.extend(seed);
            if let Some(f) = seeds_file {
                seeds.extend(
                    std::fs::read_to_string(f)?
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty() && !l.starts_with('#')),
                );
            }
            let crawler = Arc::new(crawl::Crawler::new(cfg)?);
            let n = crawler.seed(&seeds);
            tracing::info!(n, "seeded frontier");
            crawler.run().await;
        }
        Cmd::Serve => {
            let crawler = crawl::Crawler::new(cfg.clone())?;
            let state = server::AppState {
                index: crawler.index(),
                doc_count: crawler.docs_done(),
            };
            server::serve(state, &cfg.api.listen).await?;
        }
        Cmd::Run { seeds_file, seed } => {
            let mut seeds = crawler_seed_defaults();
            seeds.extend(cfg.seeds.iter().cloned());
            seeds.extend(seed);
            if let Some(f) = seeds_file {
                seeds.extend(
                    std::fs::read_to_string(f)?
                        .lines()
                        .map(|l| l.trim().to_string())
                        .filter(|l| !l.is_empty() && !l.starts_with('#')),
                );
            }
            let crawler = Arc::new(crawl::Crawler::new(cfg.clone())?);
            crawler.seed(&seeds);
            let state = server::AppState {
                index: crawler.index(),
                doc_count: crawler.docs_done(),
            };
            let listen = cfg.api.listen.clone();
            let api = tokio::spawn(async move { server::serve(state, &listen).await });
            let c2 = crawler.clone();
            let crawl_task = tokio::spawn(async move { c2.run().await });
            // api keeps serving the index after the frontier drains
            let _ = tokio::join!(api, crawl_task);
        }
        Cmd::Stats => {
            let crawler = crawl::Crawler::new(cfg)?;
            println!("docs stored: {}", crawler.store_doc_count());
            println!("docs fetched this session: {}", crawler.docs_done().load(std::sync::atomic::Ordering::Relaxed));
            println!("index docs: {}", crawler.index().num_docs());
        }
        Cmd::Fetch { url } => {
            let fetcher = fetcher::Fetcher::new(&cfg.crawl, &cfg.identity.user_agent)?;
            let resp = fetcher.get(&url).await?;
            println!("status: {}", resp.status);
            println!("bytes: {}", resp.bytes);
            println!("ms: {}", resp.elapsed_ms);
            let parsed = extract::extract(&resp.body, &resp.final_url);
            println!("title: {}", parsed.title);
            println!("desc: {}", parsed.description);
            println!("links: {}", parsed.links.len());
        }
    }
    Ok(())
}

fn crawler_seed_defaults() -> Vec<String> {
    vec![
        "https://en.wikipedia.org/wiki/Main_Page".into(),
        "https://news.ycombinator.com/".into(),
        "https://lobste.rs/".into(),
        "https://codeberg.org/".into(),
    ]
}
