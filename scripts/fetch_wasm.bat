@echo off
setlocal enabledelayedexpansion

echo Fetching WASM files using the Rust fetch_wasm binary...
cargo run --bin fetch_wasm --release
if errorlevel 1 exit /b 1
