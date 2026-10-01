# SPDX-License-Identifier: AGPL-3.0-or-later
"""Implement some checks in the active installation"""

import logging
import os
import warnings
from pathlib import Path

repo_root = Path(__file__).resolve().parent.parent

LOG_FORMAT_DEBUG = '%(levelname)-7s %(name)-30.30s: %(message)s'
logging.basicConfig(level=logging.getLevelName('DEBUG'), format=LOG_FORMAT_DEBUG)
os.environ['SEARXNG_DEBUG'] = '1'

# from here on implement the checks of the installation


OLD_SETTING = '/etc/searx/settings.yml'

if Path(OLD_SETTING).is_file():
    msg = (
        f"{OLD_SETTING} is no longer valid, move setting to"
        f" {os.environ.get('SEARXNG_SETTINGS_PATH', '/etc/searxng/settings.yml')}"
    )
    warnings.warn(msg, DeprecationWarning, stacklevel=2)

OLD_BRAND_ENV = repo_root / 'utils' / 'brand.env'

if OLD_BRAND_ENV.is_file():
    msg = f'{OLD_BRAND_ENV} is no longer needed, remove the file'
    warnings.warn(msg, DeprecationWarning, stacklevel=2)

from searx import get_setting, valkeydb  # noqa: E402

if get_setting('redis.url'):
    warnings.warn("setting redis.url is deprecated, use valkey.url", RuntimeWarning, stacklevel=2)

if not valkeydb.initialize():
    warnings.warn(f"can't connect to valkey DB at: {get_setting('valkey.url')}", RuntimeWarning, stacklevel=2)
    warnings.warn("--> no bot protection without valkey DB", RuntimeWarning, stacklevel=2)
