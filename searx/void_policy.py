# SPDX-License-Identifier: AGPL-3.0-or-later
"""Void engine and privacy policy applied when settings load.

These engines CAPTCHA, proof-of-work, or scrape in ways that get public
instances blocked. They stay in the catalog so an operator can turn them
on, but they are off by default.
"""

from __future__ import annotations

import os
import typing as t

if t.TYPE_CHECKING:
    from searx.settings_loader import SettingsType

_BLOCK_PRONE = frozenset(
    {
        "google",
        "google images",
        "google news",
        "google videos",
        "google cse",
        "google cse images",
        "google scholar",
        "google play apps",
        "google play movies",
        "duckduckgo",
        "duckduckgo web",
        "duckduckgo images",
        "duckduckgo videos",
        "duckduckgo news",
        "duckduckgo weather",
        "qwant",
        "qwant news",
        "qwant images",
        "qwant videos",
        "startpage",
        "startpage news",
        "startpage images",
        "mojeek",
        "mojeek images",
        "mojeek news",
        "brave",
        "braveapi",
        "brave.images",
        "brave.videos",
        "brave.news",
        "360search",
        "360search videos",
        "baidu",
        "baidu images",
        "baidu kaifa",
        "yandex",
        "yandex images",
        "yandex videos",
        "yandex music",
        "sogou",
        "sogou images",
        "sogou weixin",
        "quark",
        "quark images",
        "seznam",
        "yahoo",
        "yahoo news",
        "ahmia",
        "torch",
        "9gag",
        "bilibili",
        "acfun",
    }
)

_DROP_HEADERS = frozenset(
    {
        "referer",
        "referrer",
        "cookie",
        "x-forwarded-for",
        "x-forwarded-host",
        "x-forwarded-proto",
        "x-forwarded-port",
        "x-real-ip",
        "forwarded",
        "via",
        "true-client-ip",
        "cf-connecting-ip",
        "cf-ipcountry",
        "fastly-client-ip",
        "x-client-ip",
        "x-originating-ip",
    }
)


def apply_engine_policy(settings: SettingsType) -> None:
    """Disable engines that burn instance reputation with providers."""
    # env vars are not applied to settings yet at this stage
    if os.environ.get("SEARXNG_ENGINE_POLICY", "").lower() in ("0", "false", "off"):
        return
    void = settings.get("void")
    if isinstance(void, dict) and void.get("engine_policy") is False:
        return
    engines = settings.get("engines")
    if not isinstance(engines, list):
        return
    for engine in engines:
        if not isinstance(engine, dict):
            continue
        name = str(engine.get("name") or "")
        timeout = float(engine.get("timeout") or 0)
        if name in _BLOCK_PRONE or timeout >= 15:
            engine["disabled"] = True
            if name in {"braveapi", "ahmia", "torch"}:
                engine["inactive"] = True
            else:
                engine.pop("inactive", None)


def anonymize_outgoing_headers(headers: dict[str, str] | None) -> None:
    """Strip client identity from headers sent to search providers."""
    if not headers:
        return
    for key in list(headers):
        if key.lower() in _DROP_HEADERS:
            headers.pop(key, None)
            continue
        if key.lower() == "user-agent" and str(headers[key]).startswith("SearXNG/"):
            from searx.utils import gen_useragent  # pylint: disable=import-outside-toplevel

            headers[key] = gen_useragent()
    if not any(key.lower() == "accept-language" for key in headers):
        headers["Accept-Language"] = "en-US,en;q=0.9"
