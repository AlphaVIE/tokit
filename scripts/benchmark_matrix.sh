#!/usr/bin/env sh
set -eu

SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
if command -v python3.10 >/dev/null 2>&1; then
	exec python3.10 "$SCRIPT_DIR/benchmark_matrix.py" "$@"
fi

exec python3 "$SCRIPT_DIR/benchmark_matrix.py" "$@"
