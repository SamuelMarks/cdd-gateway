#!/bin/bash
set -e

echo "Fetching WASM files using the Rust fetch_wasm binary..."
cargo run --bin fetch_wasm --release
