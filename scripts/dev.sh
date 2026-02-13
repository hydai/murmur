#!/bin/bash
# Dev server launcher with configurable port via LOCALTYPE_DEV_PORT.
# Usage:
#   ./scripts/dev.sh                              # default port 5173
#   LOCALTYPE_DEV_PORT=5174 ./scripts/dev.sh      # custom port

PORT="${LOCALTYPE_DEV_PORT:-5173}"
export LOCALTYPE_DEV_PORT="$PORT"

exec cargo tauri dev --config "{\"build\":{\"devUrl\":\"http://localhost:$PORT\"}}"
