#!/bin/sh
# shellcheck shell=dash
set -eu

CONFIG_PATH="${__SEARXNG_CONFIG_PATH:-/etc/searxng}"
DATA_PATH="${__SEARXNG_DATA_PATH:-/var/cache/searxng}"
SETTINGS_PATH="${SEARXNG_SETTINGS_PATH:-$CONFIG_PATH/settings.yml}"
TEMPLATE="/usr/local/void/settings.template.yml"

if [ ! -d "$CONFIG_PATH" ]; then
    echo "config path is missing: $CONFIG_PATH" >&2
    exit 127
fi

if [ ! -d "$DATA_PATH" ]; then
    echo "data path is missing: $DATA_PATH" >&2
    exit 127
fi

if [ ! -f "$SETTINGS_PATH" ]; then
    echo "creating settings from template"
    cp -f "$TEMPLATE" "$SETTINGS_PATH"
fi

if grep -q 'ultrasecretkey' "$SETTINGS_PATH"; then
    secret="$(dd if=/dev/urandom bs=24 count=1 2>/dev/null | base64 | tr -dc 'a-zA-Z0-9' | head -c 32)"
    sed -i "s/ultrasecretkey/${secret}/g" "$SETTINGS_PATH"
fi

export SEARXNG_SETTINGS_PATH="$SETTINGS_PATH"

if [ -n "${SEARXNG_PORT:-}" ]; then
    case "$SEARXNG_PORT" in
        *[!0-9]*) ;;
        *) export GRANIAN_PORT="$SEARXNG_PORT" ;;
    esac
fi

echo "Void ${__SEARXNG_VERSION:-dev}"
exec /opt/void/bin/granian searx.webapp:app
