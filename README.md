# Void Search

A fork of [SearXNG](https://github.com/searxng/searxng), the metasearch
engine. I run this as my own instance with the defaults I actually want and a
few parts SearXNG does not have. All credit for the engine itself goes to the
SearXNG project.

## What is different

Most of the changes are defaults, not new features:

- `public_instance` and the limiter are on. Valkey is required.
- POST searches only. No access logs, no query logs.
- Engines never see your IP or browser: outgoing requests get a random
  browser user-agent and a generic header set.
- Results are cached for 180 seconds so repeat searches do not re-query
  engines.
- English by default. Results in other scripts are dropped on `en` queries.
- A host blocklist plus demotion for SEO-farm domains.
- Engines that ban or CAPTCHA instance IPs (Google, DuckDuckGo, Brave,
  Startpage, Qwant, Mojeek and friends) are force-disabled by the engine
  policy. Set `void.engine_policy: false` or `SEARXNG_ENGINE_POLICY=false`
  if you want them anyway.
- `/stats`, download formats, timings, and version strings are not exposed.
- On Linux, the process drops into a
  [Landlock](https://landlock.io) sandbox after startup. Skipped in debug
  mode.

Performance defaults: four Granian workers, an HTTP/2 pool of 256
connections, max eight redirects, two second request timeout.

The WAF in front is [RavenGuard](https://github.com/Quad4-Software/ravenguard).
The Landlock bindings are
[landlockpy](https://github.com/Quad4-Software/landlockpy).

## Run locally

Python 3.12 or newer:

```sh
python3 -m venv .venv
.venv/bin/pip install -r requirements.txt -r requirements-server.txt
export SEARXNG_SECRET="$(openssl rand -hex 32)"
export SEARXNG_DEBUG=1
export SEARXNG_PORT=48731
export SEARXNG_BIND_ADDRESS=0.0.0.0
export SEARXNG_LIMITER=false
export SEARXNG_PUBLIC_INSTANCE=false
PYTHONPATH=. .venv/bin/python -m searx.webapp
```

The limiter and `public_instance` need Valkey. For a single-user run without
it, leave both false. Debug mode skips Landlock so the reloader works.

## Docker with RavenGuard

```sh
mkdir -p config
export RG_CHALLENGE_SECRET="$(openssl rand -hex 24)"
docker compose up -d
```

```text
client -> RavenGuard :48731 -> Void Search :8080 -> engines
```

Void itself is not published on the host. Change `RG_CHALLENGE_SECRET` before
binding anything publicly. Raise `GRANIAN_WORKERS` if the box has RAM to
spare.

## Coolify

`docker-compose.coolify.yml` is the Coolify-ready definition: RavenGuard in
front, Void Search behind it, and a Valkey sidecar for the limiter and
result cache.

In Coolify create an Application from this repository, select the Docker
Compose build pack and set the Compose location to
`docker-compose.coolify.yml`. Put the public domain on the `ravenguard`
service's Domains field with the container port it exposes, for example
`https://search.example.com:8080`. Set `RG_CHALLENGE_SECRET` and any
`SEARXNG_*` or `RG_*` overrides in Coolify's environment editor; the secret
key is generated on first boot and kept in the `void-config` volume.

The image lives at `ghcr.io/quad4-software/void-search`. If the package is
private, add a registry credential in Coolify or mark the package public.

## Wiby and YaCy

Both ship disabled. In `/etc/searxng/settings.yml`:

```yaml
engines:
  - name: wiby
    disabled: false
  - name: yacy
    disabled: false
    base_url:
      - https://yacy.searchlab.eu
```

If you have your own YaCy, point at that. Public peers rate-limit quickly.

## Blocklist

Edit `searx/void_blocklist.txt` or set `void.block_hosts` in settings.
Matching hosts are removed from results.

## The crawler

`crawler/` is a Rust crawler that builds our own index. The `thevoid` engine
reads it. It respects robots.txt including crawl-delay, keeps one request in
flight per host, backs off on 429 and 5xx, refuses private IPs, skips
blocklisted hosts, detects tarpits (slow-drip responses get aborted), and
solves Anubis proof-of-work natively (codeberg, freedesktop and friends).
Cloudflare-type JS challenges go through FlareSolverr when
`VC_FLARESOLVERR` is set, and outbound traffic can ride a socks5 proxy
(Mullvad's in-tunnel proxy at `socks5h://10.64.0.1:1080` via `VC_PROXY`).
The process sandboxes itself with Landlock when the kernel supports it.

Documents are zstd-compressed in redb and indexed by tantivy. Ranking is BM25
on title and body with an authority boost from inlinks and a freshness decay.

```sh
cd crawler
cargo build --release
./target/release/void-crawler crawl --seeds-file seeds.txt
./target/release/void-crawler serve   # search api on 127.0.0.1:8088
```

`seeds.txt` ships ~1400 quality seeds: wikipedia topic pages, the curated
kagi small web domains and feeds, and community/reference sites. RSS and
Atom feeds in the seed list are mined for links rather than indexed.

Or `run` does both in one process. In compose, the `crawler` service is
profile-gated: `docker compose --profile crawler up`. In Coolify it builds
with `docker-compose.coolify.yml` automatically.

`export` dumps the document store - `--format jsonl` for our pipe format or
`--format hister` for the exact bracketed layout `hister import file` reads.
`import` accepts both shapes, so a hister export can seed or merge into our
index and vice versa.

## Settings

| Setting | Void default |
| --- | --- |
| `server.public_instance` | true |
| `server.limiter` | true |
| `server.method` | POST |
| `search.default_lang` | en |
| `void.anonymize_outgoing` | true |
| `void.drop_foreign_script` | true |
| `void.landlock` | true |
| `void.no_logs` | true |
| `void.engine_policy` | true |
| `void.result_cache_ttl` | 180 |
| `outgoing.request_timeout` | 2.0s |
| `outgoing.pool_connections` | 256 |
| `outgoing.max_redirects` | 8 |

Override in `/etc/searxng/settings.yml` with `use_default_settings: true`.

## License

AGPL-3.0-or-later, same as SearXNG. See `LICENSE` and `NOTICE`.
