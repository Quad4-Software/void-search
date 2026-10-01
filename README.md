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
- CAPTCHA and proof-of-work engines are off. Wiby and YaCy are wired in but
  disabled until you turn them on.
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
docker compose up --build
```

```text
client -> RavenGuard :48731 -> Void Search :8080 -> engines
```

Void itself is not published on the host. Change `RG_CHALLENGE_SECRET` before
binding anything publicly. Raise `GRANIAN_WORKERS` if the box has RAM to
spare.

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
| `void.result_cache_ttl` | 180 |
| `outgoing.request_timeout` | 2.0s |
| `outgoing.pool_connections` | 256 |
| `outgoing.max_redirects` | 8 |

Override in `/etc/searxng/settings.yml` with `use_default_settings: true`.

## License

AGPL-3.0-or-later, same as SearXNG. See `LICENSE` and `NOTICE`.
