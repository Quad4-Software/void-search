# SPDX-License-Identifier: AGPL-3.0-or-later
""".. _botdetection src:

Implementations used for bot detection.

"""

__all__ = ["ProxyFix", "dump_request", "get_network", "init", "too_many_requests"]


import valkey

from . import config, valkeydb
from ._helpers import dump_request, get_network, too_many_requests
from .trusted_proxies import ProxyFix


def init(cfg: config.Config, valkey_client: valkey.Valkey | None):
    config.set_global_cfg(cfg)
    if valkey_client:
        valkeydb.set_valkey_client(valkey_client)
