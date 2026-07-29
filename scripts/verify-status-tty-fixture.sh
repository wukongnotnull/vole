#!/usr/bin/env bash
# 启动 status TUI 后被 SIGINT 终止，验证终端仍可交互。
set -euo pipefail
REPO=$(cd "$(dirname "$0")/.." && pwd)
export TERM=xterm-256color
exec /usr/bin/expect -f "$REPO/scripts/verify-status-tty.exp" "$REPO"
