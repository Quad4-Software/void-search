# Void Search

Void Search is a Quad4-style SearXNG fork. Private metasearch, no tracking,
ranked for the far edge, with RavenGuard in front.

Upstream is [SearXNG](https://github.com/searxng/searxng). Brand tokens come
from [quad4.io/branding](https://quad4.io/branding). Edge WAF is
[RavenGuard](https://github.com/Quad4-Software/ravenguard). Process sandbox is
[landlockpy](https://github.com/Quad4-Software/landlockpy).

## Defaults

- Public instance mode, limiter, and hashed result cache (180s)
- No access logs, no query logs, no search history
- Outgoing requests are anonymized: engines see the server only
- POST searches, Wikipedia autocomplete, image proxy
- English results by default. Other-script pages are dropped on `en` queries
- Host blocklist plus SEO-farm demotion
- CAPTCHA and proof-of-work engines off
- Wiby and YaCy are wired and stay off until you enable them
- Download formats, timings, version strings, and `/stats` hidden
- Landlock after startup on Linux (skipped in debug)
- Four Granian workers, HTTP/2 pool of 256, eight redirects max

## Run locally

Python 3.12+, then:

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

`SEARXNG_PUBLIC_INSTANCE` and the limiter need Valkey. Without it, leave them
false for a single-user run. Debug mode skips Landlock so the reloader works.

## Run with Docker

```sh
mkdir -p config
export RG_CHALLENGE_SECRET="$(openssl rand -hex 24)"
docker compose up --build
```

Traffic path:

```text
client -> RavenGuard :48731 -> Void Search :8080 -> engines
```

Void is not published on the host. Change `RG_CHALLENGE_SECRET` before any
public bind. Raise `GRANIAN_WORKERS` if the box has spare RAM.

## Wiby and YaCy

Both engines ship disabled. In `/etc/searxng/settings.yml`:

```yaml
engines:
  - name: wiby
    disabled: false
  - name: yacy
    disabled: false
    base_url:
      - https://yacy.searchlab.eu
```

Prefer a YaCy instance you run. Public peers rate-limit quickly.

## Blocklist

Edit `searx/void_blocklist.txt` or set `void.block_hosts` in settings.
Matching hosts are removed from the result list.

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

Override with `/etc/searxng/settings.yml` and `use_default_settings: true`.

## License

GNU Affero General Public License v3.0 or later, same as SearXNG. See
`LICENSE` and `NOTICE`. RavenGuard is QSL and is not vendored, only configured.
landlockpy is 0BSD.
