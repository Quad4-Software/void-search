# About Void Search

Void Search is a Quad4-style fork of [SearXNG]. It aggregates other
{{link('search engines', 'preferences')}} and does not store a profile of you.

Requests to this instance are not forwarded to search engines. The server
makes those requests itself, with generic headers and no client IP. Queries
are not written to logs. Access logs are off. The short result cache is
keyed by a hash and never stores the raw query.

## Why this fork

- No trackers, no analytics, no telemetry, no search logs
- Dark void theme and paper light theme out of the box
- Ranking that prefers official docs and primary sources
- Favicons, autocomplete, image proxy, and tracker stripping enabled
- English queries keep Latin results. Other-script pages are dropped
- Host blocklist for SEO farms and known junk
- Wiby and YaCy available, off until you turn them on
- Hardened container: multi-stage, rootless, read-only root, Landlock

Void Search keeps the SearXNG engine library and AGPL-3.0 license. Upstream
lives at [SearXNG sources].

## How does it work

Your query is sent to several engines at once. Duplicate URLs are merged.
Void Search then scores each URL by engine agreement, title and snippet
match, and host quality. Nothing about the query is stored.

Engine health metrics stay off.

## Make it yours

Take the code and run it. The image does not phone home. If it needs a
network, that is only to reach the search engines you enabled.

[SearXNG]: https://docs.searxng.org/
[SearXNG sources]: https://github.com/searxng/searxng
[metasearch engine]: https://en.wikipedia.org/wiki/Metasearch_engine
