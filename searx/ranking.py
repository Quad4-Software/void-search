# SPDX-License-Identifier: AGPL-3.0-or-later
"""Void ranking for SearXNG result lists.

The upstream score only multiplies engine weights by inverse rank. This module
keeps that consensus signal and then reweights by query match, URL quality,
authority, and known SEO farms so the first page is more useful out of the box.
"""

from __future__ import annotations

import datetime
import math
import re
import typing as t
from urllib.parse import ParseResult, urlparse

if t.TYPE_CHECKING:
    from searx.result_types import LegacyResult, MainResult

from searx.void_blocklist import is_blocked_url  # pylint: disable=wrong-import-position

_TOKEN_RE = re.compile(r"[a-z0-9][a-z0-9+._-]{1,}", re.I)
_FOREIGN_SCRIPT = re.compile(
    r"[\u0400-\u04FF\u0500-\u052F\u0590-\u05FF\u0600-\u06FF\u0900-\u097F"
    r"\u0E00-\u0E7F\u3040-\u30FF\u3400-\u9FFF\uAC00-\uD7AF]"
)
_FOREIGN_TLDS = (
    ".ru",
    ".su",
    ".cn",
    ".jp",
    ".kr",
    ".tw",
    ".hk",
    ".ir",
    ".vn",
    ".th",
    ".sa",
    ".ae",
    ".il",
    ".ua",
    ".by",
    ".kz",
)
_STOPWORDS = frozenset(
    {
        "a",
        "an",
        "and",
        "are",
        "as",
        "at",
        "be",
        "by",
        "for",
        "from",
        "how",
        "i",
        "in",
        "is",
        "it",
        "of",
        "on",
        "or",
        "that",
        "the",
        "this",
        "to",
        "was",
        "what",
        "when",
        "where",
        "who",
        "why",
        "with",
        "www",
    }
)

_AUTHORITY_HOSTS = frozenset(
    {
        "wikipedia.org",
        "wikimedia.org",
        "wikidata.org",
        "mediawiki.org",
        "github.com",
        "gitlab.com",
        "codeberg.org",
        "sr.ht",
        "developer.mozilla.org",
        "mdn.dev",
        "arxiv.org",
        "ietf.org",
        "rfc-editor.org",
        "w3.org",
        "kernel.org",
        "docs.python.org",
        "pypi.org",
        "go.dev",
        "pkg.go.dev",
        "doc.rust-lang.org",
        "crates.io",
        "npmjs.com",
        "nodejs.org",
        "stackoverflow.com",
        "stackexchange.com",
        "superuser.com",
        "serverfault.com",
        "askubuntu.com",
        "gnu.org",
        "fsf.org",
        "apache.org",
        "kubernetes.io",
        "cncf.io",
        "docker.com",
        "archlinux.org",
        "wiki.archlinux.org",
        "debian.org",
        "ubuntu.com",
        "freebsd.org",
        "man7.org",
        "cppreference.com",
        "sqlite.org",
        "postgresql.org",
        "mariadb.org",
        "openssl.org",
        "letsencrypt.org",
        "lwn.net",
        "phoronix.com",
        "nih.gov",
        "ncbi.nlm.nih.gov",
        "pubmed.ncbi.nlm.nih.gov",
        "nasa.gov",
        "esa.int",
        "europa.eu",
        "who.int",
        "un.org",
        "nature.com",
        "science.org",
        "acm.org",
        "ieee.org",
        "doi.org",
        "qt.io",
        "llvm.org",
        "gcc.gnu.org",
        "docs.searxng.org",
        "searxng.org",
        "quad4.io",
        "git.quad4.io",
        "reticulum.network",
        "python.org",
        "rust-lang.org",
        "typescriptlang.org",
        "react.dev",
        "vuejs.org",
        "angular.dev",
        "developer.apple.com",
        "swift.org",
        "golang.org",
        "docs.rs",
        "readthedocs.io",
        "readthedocs.org",
        "developer.android.com",
        "learn.microsoft.com",
        "docs.microsoft.com",
        "cloud.google.com",
        "web.dev",
        "caniuse.com",
        "iana.org",
        "unicode.org",
    }
)

_AUTHORITY_SUFFIXES = (
    ".edu",
    ".gov",
    ".mil",
    ".ac.uk",
    ".gov.uk",
)

_SEO_HOSTS = frozenset(
    {
        "pinterest.com",
        "pinterest.co.uk",
        "quora.com",
        "wikihow.com",
        "buzzfeed.com",
        "slideshare.net",
        "scribd.com",
        "fandom.com",
        "facebook.com",
        "instagram.com",
        "tiktok.com",
        "taboola.com",
        "outbrain.com",
        "msn.com",
        "yahoo.com",
        "answers.com",
        "ehow.com",
        "livestrong.com",
        "hubpages.com",
        "medium.com",
        "substack.com",
        "w3schools.com",
        "geeksforgeeks.org",
        "tutorialspoint.com",
        "javatpoint.com",
        "programiz.com",
        "digitaltrends.com",
        "makeuseof.com",
        "howtogeek.com",
        "forbes.com",
        "businessinsider.com",
        "dailymail.co.uk",
        "express.co.uk",
        "cnet.com",
        "zdnet.com",
        "lifehacker.com",
        "gizmodo.com",
        "mashable.com",
        "listverse.com",
        "ranker.com",
    }
)

_NAV_HOSTS = {
    "github": ("github.com",),
    "gitlab": ("gitlab.com",),
    "wikipedia": ("wikipedia.org",),
    "wiki": ("wikipedia.org",),
    "python": ("python.org", "docs.python.org"),
    "mdn": ("developer.mozilla.org", "mdn.dev"),
    "mozilla": ("mozilla.org", "developer.mozilla.org"),
    "stackoverflow": ("stackoverflow.com",),
    "arxiv": ("arxiv.org",),
    "youtube": ("youtube.com",),
    "reddit": ("reddit.com",),
    "kernel": ("kernel.org",),
    "npm": ("npmjs.com",),
    "pypi": ("pypi.org",),
    "rust": ("rust-lang.org", "doc.rust-lang.org", "crates.io"),
    "docker": ("docker.com", "docs.docker.com"),
    "ietf": ("ietf.org", "datatracker.ietf.org"),
}

_TRACKING_KEYS = frozenset(
    {
        "utm_source",
        "utm_medium",
        "utm_campaign",
        "utm_term",
        "utm_content",
        "fbclid",
        "gclid",
        "mc_cid",
        "mc_eid",
        "ref",
        "ref_src",
    }
)


def tokenize_query(query: str) -> list[str]:
    """Return lowercased query tokens with stopwords removed."""
    tokens = [tok.lower() for tok in _TOKEN_RE.findall(query or "")]
    kept = [tok for tok in tokens if tok not in _STOPWORDS and len(tok) > 1]
    return kept or tokens


def _field(result: t.Any, name: str, default: str = "") -> str:
    value = getattr(result, name, None)
    if value:
        return str(value)
    if hasattr(result, "get"):
        value = result.get(name)
        if value:
            return str(value)
    return default


def _host(parsed: ParseResult | None, url: str) -> str:
    if parsed is not None:
        host = (parsed.hostname or parsed.netloc or "").lower()
    else:
        host = (urlparse(url).hostname or "").lower()
    if host.startswith("www."):
        host = host[4:]
    return host


def _is_authority(host: str) -> bool:
    if host in _AUTHORITY_HOSTS:
        return True
    for suffix in _AUTHORITY_SUFFIXES:
        if host.endswith(suffix):
            return True
    return any(host.endswith("." + parent) for parent in _AUTHORITY_HOSTS)


def _is_seo_farm(host: str) -> bool:
    if host in _SEO_HOSTS:
        return True
    return any(host.endswith("." + parent) for parent in _SEO_HOSTS)


def _token_present(text: str, token: str) -> bool:
    if not text or not token:
        return False
    return re.search(rf"(^|[^a-z0-9]){re.escape(token)}([^a-z0-9]|$)", text.lower()) is not None


def _coverage(text: str, tokens: list[str]) -> float:
    if not tokens or not text:
        return 0.0
    hits = sum(1 for tok in tokens if _token_present(text, tok))
    return hits / len(tokens)


def _phrase_bonus(text: str, query: str) -> float:
    if not query or not text:
        return 0.0
    haystack = text.lower()
    needle = query.strip().lower()
    if not needle:
        return 0.0
    if haystack == needle:
        return 0.42
    if haystack.startswith(needle):
        return 0.32
    if needle in haystack:
        return 0.16
    return 0.0


def _nav_bonus(host: str, query: str, parsed: ParseResult | None) -> float:
    tokens = tokenize_query(query)
    needle = " ".join(tokens)
    score = 0.0
    if not needle:
        return 0.0
    if needle in (host, host.rsplit(".", 1)[0]):
        score += 1.35
    for token in tokens:
        for dest in _NAV_HOSTS.get(token, ()):
            if host == dest or host.endswith("." + dest):
                score += 1.15 if len(tokens) == 1 else 0.45
                break
    path = "/"
    if parsed is not None:
        path = parsed.path or "/"
    if score and path in {"", "/"}:
        score += 0.28
    if path in {"/login", "/search", "/explore", "/signin", "/signup"}:
        score *= 0.35
    return score


def _freshness(result: t.Any) -> float:
    published = getattr(result, "publishedDate", None)
    if published is None and hasattr(result, "get"):
        published = result.get("publishedDate")
    if not isinstance(published, datetime.datetime):
        return 0.0
    if published.tzinfo is None:
        published = published.replace(tzinfo=datetime.UTC)
    age = datetime.datetime.now(datetime.UTC) - published
    days = max(age.total_seconds() / 86400.0, 0.0)
    if days <= 2:
        return 0.18
    if days <= 14:
        return 0.10
    if days <= 90:
        return 0.04
    if days >= 3650:
        return -0.04
    return 0.0


def _url_quality(parsed: ParseResult | None, url: str) -> float:
    if parsed is None:
        parsed = urlparse(url)
    score = 0.0
    if parsed.scheme == "https":
        score += 0.12
    elif parsed.scheme == "http":
        score -= 0.10
    path = parsed.path or "/"
    depth = len([part for part in path.split("/") if part])
    if depth <= 2:
        score += 0.04
    elif depth >= 7:
        score -= 0.06
    if len(url) > 180:
        score -= 0.05
    query_keys = {
        key.lower()
        for key, _ in [
            pair.split("=", 1) if "=" in pair else (pair, "") for pair in (parsed.query or "").split("&") if pair
        ]
    }
    tracking = len(query_keys & _TRACKING_KEYS)
    if tracking:
        score -= 0.03 * tracking
    if path.endswith((".pdf", ".html", ".htm", ".md", ".txt", ".rst")):
        score += 0.02
    return score


def foreign_script_ratio(text: str) -> float:
    """Share of letters that are not Latin when the query language is English."""
    letters = [char for char in text if char.isalpha()]
    if len(letters) < 8:
        return 0.0
    foreign = sum(1 for char in letters if _FOREIGN_SCRIPT.match(char))
    return foreign / len(letters)


def is_foreign_language(result: t.Any, lang: str) -> bool:
    """True when an English search pulled a clearly other-script page."""
    if not lang or lang in {"all", "auto"}:
        return False
    if not str(lang).lower().startswith("en"):
        return False
    try:
        from searx import get_setting  # pylint: disable=import-outside-toplevel

        if not get_setting("void.drop_foreign_script"):
            return False
    except Exception:  # noqa: S110  # pylint: disable=broad-exception-caught
        pass
    blob = f"{_field(result, 'title')} {_field(result, 'content')}"
    return foreign_script_ratio(blob) >= 0.18


def relevance_multiplier(result: t.Any, query: str, lang: str = "") -> float:
    """Return a multiplier centered near 1.0 for query and URL quality."""
    tokens = tokenize_query(query)
    title = _field(result, "title")
    content = _field(result, "content")
    url = _field(result, "url")
    parsed = getattr(result, "parsed_url", None)
    host = _host(parsed if isinstance(parsed, ParseResult) else None, url)

    if is_blocked_url(url, parsed if isinstance(parsed, ParseResult) else None):
        return 0.0
    if is_foreign_language(result, lang):
        return 0.0

    title_cov = _coverage(title, tokens)
    content_cov = _coverage(content, tokens)
    url_cov = _coverage(url, tokens)
    phrase = max(_phrase_bonus(title, query), _phrase_bonus(content, query) * 0.5)

    multiplier = 0.72
    multiplier += 1.45 * title_cov
    multiplier += 0.36 * content_cov
    multiplier += 0.18 * url_cov
    multiplier += phrase
    multiplier += _url_quality(parsed if isinstance(parsed, ParseResult) else None, url)
    multiplier += _freshness(result)

    if title_cov == 0.0 and content_cov == 0.0 and url_cov == 0.0:
        multiplier *= 0.20
    if lang and str(lang).lower().startswith("en"):
        for suffix in _FOREIGN_TLDS:
            if host.endswith(suffix):
                multiplier *= 0.35
                break

    if _is_authority(host):
        multiplier += 0.38
    multiplier += _nav_bonus(host, query, parsed if isinstance(parsed, ParseResult) else None)
    if _is_seo_farm(host):
        multiplier *= 0.38
    if host.endswith(".onion"):
        multiplier *= 0.85

    return max(0.0, multiplier)


def engine_weight_score(
    result: t.Any,
    priority: str | None,
    engines: dict[str, t.Any] | None = None,
) -> float:
    """Reproduce the SearXNG consensus score from engine weights and ranks."""
    import searx.engines  # pylint: disable=import-outside-toplevel

    weight = 1.0
    engine_map = engines if engines is not None else getattr(searx.engines, "engines", {})
    for result_engine in result["engines"]:
        engine = engine_map.get(result_engine)
        if engine is not None and hasattr(engine, "weight"):
            weight *= float(engine.weight)

    positions = result["positions"]
    weight *= len(positions)
    score = 0.0
    for position in positions:
        if priority == "low":
            continue
        if priority == "high":
            score += weight
        else:
            position = max(int(position), 1)
            score += weight / position
    return score


def calculate_score(
    result: MainResult | LegacyResult | t.Any,
    priority: str | None,
    query: str = "",
    engines: dict[str, t.Any] | None = None,
    lang: str = "",
) -> float:
    """Return the Void Search score for one merged result."""
    base = engine_weight_score(result, priority, engines=engines)
    if base <= 0:
        return 0.0
    refined = base * relevance_multiplier(result, query, lang=lang)
    if refined <= 0:
        return 0.0
    score = refined * (1.0 + math.log1p(len(result["engines"])))
    url = ""
    parsed = getattr(result, "parsed_url", None)
    url = str(result.get('url') or '') if hasattr(result, 'get') else str(getattr(result, 'url', '') or '')
    host = _host(parsed if isinstance(parsed, ParseResult) else None, url)
    nav = _nav_bonus(host, query, parsed if isinstance(parsed, ParseResult) else None)
    if nav >= 1.0:
        score += 80.0 * nav
    return score


def rank_results(
    results: list[t.Any],
    query: str = "",
    engines: dict[str, t.Any] | None = None,
    lang: str = "",
) -> list[t.Any]:
    """Score and sort results for tests and offline ranking."""
    for result in results:
        priority = getattr(result, "priority", None) or result.get("priority")
        result.score = calculate_score(result, priority, query=query, engines=engines, lang=lang)
    return sorted(results, key=lambda item: item.score, reverse=True)
