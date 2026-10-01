# SPDX-License-Identifier: AGPL-3.0-or-later
"""Short-lived search result cache.

Repeated public queries hit the cache instead of every upstream engine. That
is faster and keeps the instance from looking like a scraper.
"""

from __future__ import annotations

import hashlib
import json
import threading
import time
import typing as t

from searx import get_setting, logger
from searx.result_types import LegacyResult
from searx.results import ResultContainer
from searx.valkeydb import client as valkey_client

if t.TYPE_CHECKING:
    from searx.search.models import SearchQuery

log = logger.getChild("result_cache")

_MEM: dict[str, tuple[float, dict[str, t.Any]]] = {}
_LOCK = threading.Lock()
_PREFIX = "void:res:"


def _ttl() -> int:
    try:
        return max(int(get_setting("void.result_cache_ttl")), 0)
    except Exception:  # pylint: disable=broad-exception-caught
        return 180


def _max_items() -> int:
    try:
        return max(int(get_setting("void.result_cache_max")), 16)
    except Exception:  # pylint: disable=broad-exception-caught
        return 4096


def cache_key(search_query: SearchQuery) -> str:
    """Return a stable key that does not include the client."""
    engines = ",".join(sorted(ref.name for ref in search_query.engineref_list))
    raw = "|".join(
        (
            search_query.query.strip().lower(),
            str(search_query.lang),
            str(search_query.pageno),
            str(search_query.safesearch),
            str(search_query.time_range or ""),
            engines,
        )
    )
    return hashlib.sha256(raw.encode("utf-8")).hexdigest()


def _pack(container: ResultContainer) -> dict[str, t.Any]:
    answers = []
    for answer in container.answers:
        if hasattr(answer, "as_dict"):
            answers.append(answer.as_dict())
        else:
            answers.append({"answer": str(answer)})
    return {
        "results": [item.as_dict() for item in container.get_ordered_results()],
        "infoboxes": list(container.infoboxes),
        "suggestions": list(container.suggestions),
        "corrections": list(container.corrections),
        "answers": answers,
        "query": "",
    }


def _unpack(payload: dict[str, t.Any]) -> ResultContainer:
    container = ResultContainer()
    container.query = str(payload.get("query") or "")
    results = payload.get("results") or []
    if results:
        container.extend(None, results)
    for box in payload.get("infoboxes") or []:
        container.infoboxes.append(box if isinstance(box, LegacyResult) else LegacyResult(box))
    container.suggestions.update(payload.get("suggestions") or [])
    container.corrections.update(payload.get("corrections") or [])
    for answer in payload.get("answers") or []:
        if isinstance(answer, dict) and answer.get("answer"):
            from searx.result_types.answer import Answer  # pylint: disable=import-outside-toplevel

            container.answers.add(Answer(answer=str(answer["answer"])))
    return container


def get(search_query: SearchQuery) -> ResultContainer | None:
    """Return a cached container or None."""
    ttl = _ttl()
    if ttl <= 0:
        return None
    key = cache_key(search_query)
    now = time.time()
    vk = valkey_client()
    if vk is not None:
        raw = vk.get(_PREFIX + key)
        if raw:
            try:
                payload = json.loads(raw)
                log.debug("valkey hit %s", key[:12])
                return _reopen(_unpack(payload), search_query)
            except Exception as exc:  # pylint: disable=broad-exception-caught
                log.debug("valkey decode failed: %s", exc)
        return None
    with _LOCK:
        item = _MEM.get(key)
        if not item:
            return None
        expires, payload = item
        if expires < now:
            _MEM.pop(key, None)
            return None
        log.debug("memory hit %s", key[:12])
        return _reopen(_unpack(payload), search_query)


def _reopen(container: ResultContainer, search_query: SearchQuery) -> ResultContainer:
    """Rescore a cached payload using the current query without storing it."""
    container.query = search_query.query
    container.lang = getattr(search_query, "lang", "") or ""
    container._closed = False  # pylint: disable=protected-access
    container._main_results_sorted = None  # type: ignore[assignment]  # pylint: disable=protected-access
    container.close()
    return container


def put(search_query: SearchQuery, container: ResultContainer) -> None:
    """Store a closed result container."""
    ttl = _ttl()
    if ttl <= 0:
        return
    if not container.get_ordered_results() and not container.answers:
        return
    if container.answers:
        # answers can embed request-local values (self_info exposes the client
        # IP and user-agent), so never serve them to a different client
        return
    key = cache_key(search_query)
    payload = _pack(container)
    blob = json.dumps(payload, default=str)
    vk = valkey_client()
    if vk is not None:
        vk.setex(_PREFIX + key, ttl, blob)
        return
    now = time.time()
    with _LOCK:
        if len(_MEM) >= _max_items():
            oldest = min(_MEM.items(), key=lambda pair: pair[1][0])[0]
            _MEM.pop(oldest, None)
        _MEM[key] = (now + ttl, payload)
