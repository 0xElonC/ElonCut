#!/usr/bin/env bash
# Installs what is missing and starts ElonCut. See scripts/dev.sh.
exec "$(cd "$(dirname "$0")" && pwd)/scripts/dev.sh" "$@"
