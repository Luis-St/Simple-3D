#!/usr/bin/env bash
# Build the dev (debug) build and run it. Arguments are passed on to the app,
# so `./run.sh model.simple3d` opens that file.
set -euo pipefail

cd "$(dirname "$0")"
# The pinned toolchain lives in ~/.cargo/bin, which non-login shells miss.
export PATH="$HOME/.cargo/bin:$PATH"

cargo build -p simple3d-app
exec ./target/debug/simple-3d "$@"
