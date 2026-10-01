# syntax=docker/dockerfile:1.7
# Void hardened image. Multi-stage, rootless, no package managers in the
# final stage, and a generated secret on first boot.

FROM python:3.12-slim-bookworm AS builder

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PIP_DISABLE_PIP_VERSION_CHECK=1 \
    PIP_NO_CACHE_DIR=1 \
    UV_NO_MANAGED_PYTHON=true \
    UV_COMPILE_BYTECODE=1

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential \
        ca-certificates \
        curl \
    && rm -rf /var/lib/apt/lists/*

COPY --from=ghcr.io/astral-sh/uv:0.8.22 /uv /usr/local/bin/uv

WORKDIR /build
COPY requirements.txt requirements-server.txt ./
RUN uv venv /opt/void \
    && uv pip install --python /opt/void/bin/python \
        --requirements requirements.txt \
        --requirements requirements-server.txt \
    && find /opt/void -type d -name __pycache__ -exec rm -rf {} + \
    && find /opt/void -type f -name '*.pyc' -delete

COPY searx ./searx
COPY container/entrypoint.sh container/settings.template.yml ./
RUN printf '%s\n' \
        '# SPDX-License-Identifier: AGPL-3.0-or-later' \
        'VERSION_STRING = "1.0.0-void"' \
        'VERSION_TAG = "1.0.0-void"' \
        'DOCKER_TAG = "1.0.0-void"' \
        'GIT_URL = "https://quad4.io/"' \
        'GIT_BRANCH = "main"' \
        > searx/version_frozen.py \
    && /opt/void/bin/python -m compileall -q -f searx \
    && find searx -type d -name __pycache__ -prune -exec rm -rf {} +

FROM python:3.12-slim-bookworm AS runtime

LABEL org.opencontainers.image.title="Void" \
      org.opencontainers.image.description="Quad4-style SearXNG fork. Private metasearch, no tracking." \
      org.opencontainers.image.licenses="AGPL-3.0-or-later" \
      org.opencontainers.image.source="https://quad4.io/" \
      org.opencontainers.image.url="https://quad4.io/"

ENV PYTHONDONTWRITEBYTECODE=1 \
    PYTHONUNBUFFERED=1 \
    PYTHONSAFEPATH=1 \
    PATH="/opt/void/bin:${PATH}" \
    SEARXNG_SETTINGS_PATH="/etc/searxng/settings.yml" \
    __SEARXNG_CONFIG_PATH="/etc/searxng" \
    __SEARXNG_DATA_PATH="/var/cache/searxng" \
    GRANIAN_PROCESS_NAME="void" \
    GRANIAN_INTERFACE="wsgi" \
    GRANIAN_HOST="::" \
    GRANIAN_PORT="8080" \
    GRANIAN_WEBSOCKETS="false" \
    GRANIAN_WORKERS="4" \
    GRANIAN_RUNTIME_THREADS="2" \
    GRANIAN_BACKPRESSURE="128" \
    GRANIAN_BLOCKING_THREADS="8" \
    GRANIAN_ACCESS_LOG="false" \
    GRANIAN_LOG_LEVEL="error" \
    MALLOC_ARENA_MAX="2" \
    GRANIAN_WORKERS_KILL_TIMEOUT="30s" \
    GRANIAN_BLOCKING_THREADS_IDLE_TIMEOUT="5m"

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        ca-certificates \
        tini \
    && rm -rf /var/lib/apt/lists/* \
    && groupadd --gid 65532 void \
    && useradd --uid 65532 --gid 65532 --home-dir /usr/local/void --shell /usr/sbin/nologin void \
    && mkdir -p /etc/searxng /var/cache/searxng /usr/local/void \
    && chown -R 65532:65532 /etc/searxng /var/cache/searxng /usr/local/void

COPY --from=builder --chown=65532:65532 /opt/void /opt/void
COPY --from=builder --chown=65532:65532 /build/searx /usr/local/void/searx
COPY --from=builder --chown=65532:65532 /build/entrypoint.sh /usr/local/void/entrypoint.sh
COPY --from=builder --chown=65532:65532 /build/settings.template.yml /usr/local/void/settings.template.yml

WORKDIR /usr/local/void
ENV PYTHONPATH="/usr/local/void"

RUN chmod 0555 /usr/local/void/entrypoint.sh \
    && chmod -R a-s /opt/void /usr/local/void \
    && chmod 0770 /etc/searxng /var/cache/searxng

USER 65532:65532

EXPOSE 8080
VOLUME ["/etc/searxng", "/var/cache/searxng"]

HEALTHCHECK --interval=30s --timeout=5s --start-period=20s --retries=3 \
    CMD ["python", "-c", "import urllib.request; urllib.request.urlopen('http://127.0.0.1:8080/healthz', timeout=3).read()"]

ENTRYPOINT ["/usr/bin/tini", "--", "/usr/local/void/entrypoint.sh"]
