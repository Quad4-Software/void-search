# SPDX-License-Identifier: AGPL-3.0-or-later
"""Best-effort Landlock sandbox via landlockpy."""

from __future__ import annotations

import contextlib
import os
import sys
from pathlib import Path

from searx import logger

log = logger.getChild("void_sandbox")


def _existing(paths: list[str]) -> list[Path]:
    found = []
    for raw in paths:
        if not raw:
            continue
        path = Path(raw)
        if path.exists():
            found.append(path)
    return found


def apply() -> bool:
    """Restrict this process with Landlock. Returns True when applied."""
    try:
        from searx import get_setting, sxng_debug

        if sxng_debug or not bool(get_setting("void.landlock")):
            return False
    except Exception:
        return False

    try:
        from landlockpy import AccessFS, AccessNet, Ruleset, supported
    except Exception as exc:
        log.info("landlockpy not available: %s", exc)
        return False

    if not supported():
        log.info("Landlock is not available on this kernel")
        return False

    read_fs = AccessFS.READ_FILE | AccessFS.READ_DIR | AccessFS.EXECUTE
    file_fs = AccessFS.READ_FILE
    if hasattr(AccessFS, "IOCTL_DEV"):
        file_fs |= AccessFS.IOCTL_DEV
    write_fs = (
        AccessFS.READ_FILE
        | AccessFS.READ_DIR
        | AccessFS.WRITE_FILE
        | AccessFS.MAKE_REG
        | AccessFS.REMOVE_FILE
        | AccessFS.MAKE_DIR
        | AccessFS.REMOVE_DIR
    )
    if hasattr(AccessFS, "TRUNCATE"):
        write_fs |= AccessFS.TRUNCATE

    read_paths = _existing(
        [
            "/usr",
            "/lib",
            "/lib64",
            "/bin",
            "/sbin",
            "/etc/ssl",
            "/etc/pki",
            "/usr/share/zoneinfo",
            "/usr/local/void",
            "/opt/void",
            "/workspace",
            sys.prefix,
            sys.base_prefix,
            # source checkout of this package (development installs and tests)
            str(Path(__file__).resolve().parent.parent),
            os.environ.get("__SEARXNG_CONFIG_PATH", "/etc/searxng"),
        ]
    )
    read_files = _existing(
        [
            "/etc/hosts",
            "/etc/resolv.conf",
            "/etc/nsswitch.conf",
            "/etc/gai.conf",
            "/etc/localtime",
            os.environ.get("SEARXNG_SETTINGS_PATH", ""),
            "/dev/null",
            "/dev/urandom",
            "/dev/random",
        ]
    )
    write_paths = _existing(
        [
            "/tmp",  # noqa: S108
            "/var/tmp",  # noqa: S108
            "/var/cache/searxng",
            os.environ.get("__SEARXNG_DATA_PATH", "/var/cache/searxng"),
        ]
    )

    # quiet rules are only valid when the ruleset declares the rights they may
    # carry quiet for, otherwise landlock_add_rule() fails with EINVAL
    quiet_fs_mask = read_fs | file_fs | write_fs
    quiet_net_mask = AccessNet.CONNECT_TCP | AccessNet.BIND_TCP

    try:
        with Ruleset(quiet_fs=quiet_fs_mask, quiet_net=quiet_net_mask) as ruleset:
            for path in read_paths:
                with contextlib.suppress(Exception):
                    ruleset.allow_path(str(path), read_fs, quiet=True)
            for path in read_files:
                with contextlib.suppress(Exception):
                    ruleset.allow_path(str(path), file_fs, quiet=True)
            for path in write_paths:
                with contextlib.suppress(Exception):
                    ruleset.allow_path(str(path), write_fs, quiet=True)
            try:
                listen = int(get_setting("server.port") or 8080)
            except Exception:
                listen = 8080
            for port in {80, 443, 6379, 8080, 8443, 8888, 48731, listen}:
                with contextlib.suppress(Exception):
                    ruleset.allow_port(int(port), AccessNet.CONNECT_TCP, quiet=True)
            with contextlib.suppress(Exception):
                ruleset.allow_port(listen, AccessNet.BIND_TCP, quiet=True)
            ruleset.restrict()
        log.info("Landlock applied")
        return True
    except Exception as exc:
        log.warning("Landlock skipped: %s", exc)
        return False
