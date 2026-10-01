# SPDX-License-Identifier: AGPL-3.0-or-later
"""Drop request, query, and client identity from process logs."""

from __future__ import annotations

import logging
import re

from werkzeug.serving import WSGIRequestHandler

_QUERY = re.compile(r"([?&](?:q|query|search)=)[^&\s]+", re.I)
_IP4 = re.compile(r"\b(?:\d{1,3}\.){3}\d{1,3}\b")
_IP6 = re.compile(r"\b(?:[0-9a-f]{0,4}:){2,7}[0-9a-f]{0,4}\b", re.I)


def _scrub(text: str) -> str:
    text = _QUERY.sub(r"\1[redacted]", text)
    text = _IP4.sub("[redacted]", text)
    text = _IP6.sub("[redacted]", text)
    return text


class NoQueryFilter(logging.Filter):
    """Rewrite log records so queries and client addresses never print."""

    def filter(self, record: logging.LogRecord) -> bool:
        try:
            message = record.getMessage()
        except Exception:
            return True
        record.msg = _scrub(str(message))
        record.args = ()
        return True


def _quiet_request_handler() -> None:
    WSGIRequestHandler.log_request = lambda self, *args, **kwargs: None  # type: ignore[method-assign]
    WSGIRequestHandler.log_error = lambda self, *args, **kwargs: None  # type: ignore[method-assign]
    WSGIRequestHandler.log_message = lambda self, *args, **kwargs: None  # type: ignore[method-assign]
    WSGIRequestHandler.log = lambda self, *args, **kwargs: None  # type: ignore[method-assign]


def install() -> None:
    """Attach the redaction filter and silence HTTP access logs."""
    filt = NoQueryFilter()
    logging.getLogger().addFilter(filt)
    for name in ("searx", "werkzeug", "granian", "wsgi"):
        logging.getLogger(name).addFilter(filt)
    logging.getLogger("werkzeug").setLevel(logging.ERROR)
    logging.getLogger("granian").setLevel(logging.ERROR)
    logging.getLogger("searx.botdetection").setLevel(logging.ERROR)
    _quiet_request_handler()
