# void-crawler

Independent web crawler and search index for Void Search. Feeds the `thevoid`
searxng engine. Written in Rust, designed to be respectful and disk-efficient.

## Principles

- Identifies itself honestly: `VoidCrawler/<version> (+<bot_url>)`
- Respects robots.txt fully, including crawl-delay, via the Google parser port
- Per-host politeness: one in-flight request per host, minimum delay between
  requests, exponential backoff on 429/5xx and hard ban detection on repeated
  errors
- Tarpit protection: hard timeouts, slow-drip detection (aborts when bandwidth
  drops below a floor), size caps, redirect limits
- SSRF protection: private, loopback, and link-local IPs are refused by
  default, blocklisted hosts are skipped before DNS
- Challenge handling: solves Anubis proof-of-work natively (sha256, no browser
  needed) and falls back to FlareSolverr for Cloudflare-type JS challenges
- Dedup on normalized URL (fragments and tracking params stripped) persisted
  across restarts
- Disk efficiency: documents compressed with zstd, index postings compressed
  by tantivy, no raw HTML retained by default

## Storage layout

- `data/state.redb` - frontier, seen-set, per-host state, documents
- `data/index/` - tantivy index (title, url, host, body, authority, freshness)
- `data/blocklists/*.txt` - host and suffix blocklists

The crawler is a single binary with three subcommands:

```
void-crawler crawl  --config crawler.toml
void-crawler serve  --config crawler.toml
void-crawler stats  --config crawler.toml
```

`crawl` runs the fetch pipeline, `serve` exposes the HTTP search API that
searxng talks to, `stats` prints index and frontier counters. They can run as
separate processes against the same data directory; redb and tantivy both
tolerate a single writer plus readers.

## Scaling path

Embedded today: redb for crawl state and docs, tantivy for the index. To go
HA later, point the store traits at Postgres (frontier via SELECT FOR UPDATE
SKIP LOCKED, docs as compressed bytea) and swap tantivy for ParadeDB or
Quickwit. The module boundaries (frontier, store, index, api) are drawn so
this is an implementation swap, not a rewrite.

## Blocklists

Files in `data/blocklists/` are merged with the built-in list. One hostname
or suffix per line, `#` comments allowed. A host matches when it equals or
ends with a listed suffix.

## Config

See `crawler.example.toml`.
