#!/bin/sh
# Build the forest to WebAssembly, build the page, and serve it at http://localhost:8000
set -e
cd "$(dirname "$0")/.."
cargo build -p forest --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/forest.wasm docs/forest.wasm
python3 tools/build.py
python3 -m http.server 8000 -d docs
