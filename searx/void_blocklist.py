# SPDX-License-Identifier: AGPL-3.0-or-later
"""Host blocklist used by ranking and result merge."""

from __future__ import annotations

from functools import lru_cache
from pathlib import Path
from urllib.parse import urlparse

_FILE = Path(__file__).with_name("void_blocklist.txt")


def _norm_host(host: str) -> str:
    host = (host or "").lower().strip().rstrip(".")
    if host.startswith("www."):
        host = host[4:]
    return host


@lru_cache(maxsize=1)
def blocked_hosts() -> frozenset[str]:
    """Return configured hosts plus the shipped blocklist file."""
    hosts: set[str] = set()
    if _FILE.is_file():
        for line in _FILE.read_text(encoding="utf-8").splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            hosts.add(_norm_host(line))
    try:
        from searx import get_setting

        extra = get_setting("void.block_hosts") or []
        if isinstance(extra, str):
            extra = [extra]
        for item in extra:
            hosts.add(_norm_host(str(item)))
    except Exception:
        pass
    hosts.discard("")
    return frozenset(hosts)


def host_of(url: str, parsed=None) -> str:
    """Return a normalized hostname from a URL or parsed result."""
    if parsed is not None:
        host = getattr(parsed, "hostname", None) or getattr(parsed, "netloc", "") or ""
        return _norm_host(str(host))
    return _norm_host(urlparse(url or "").hostname or "")


def is_blocked_host(host: str) -> bool:
    """True when host or a parent is on the blocklist."""
    host = _norm_host(host)
    if not host:
        return False
    blocked = blocked_hosts()
    if host in blocked:
        return True
    for listed in blocked:
        if host.endswith("." + listed):
            return True
    return False


def is_blocked_url(url: str, parsed=None) -> bool:
    """True when the result URL host is blocked."""
    return is_blocked_host(host_of(url, parsed))
