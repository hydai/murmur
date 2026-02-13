#!/bin/bash
# Dev server launcher with configurable port via MURMUR_DEV_PORT.
# Usage:
#   ./scripts/dev.sh                              # default port 5173
#   MURMUR_DEV_PORT=5174 ./scripts/dev.sh      # custom port

PORT="${MURMUR_DEV_PORT:-5173}"
export MURMUR_DEV_PORT="$PORT"

exec cargo tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$PORT\"}}"
