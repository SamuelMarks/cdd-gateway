# WebAssembly (WASM) Support

This document outlines the state of WebAssembly generation capabilities in the CDD offline-first ecosystem.

## Strategy
The primary advantage of the CDD framework is providing instantaneous, zero-latency code generation via client-side WebAssembly. 

To acquire WASM engines for each language:
1. The build toolchain (`make build` or `make.bat`) attempts to dynamically download the pre-compiled `.wasm` binaries from the latest GitHub releases (e.g. `cdd-python`, `cdd-rust`).
2. If network conditions fail or a pre-compiled release is unavailable, it automatically falls back to cloning the repositories and attempting a local build via native toolchains (e.g. `cargo build --target wasm32-wasi`).
3. A JSON configuration matrix (`wasm-support.json`) is dynamically emitted documenting which binaries successfully loaded and which failed.

## Fallback Gracefulness
If a WASM generator fails to load or cannot be acquired:
- **Frontend Integration**: The Web UI dynamically reads the support matrix at launch. Languages missing a WASM generator will be gracefully greyed out (using `filter: grayscale(100%)`) and explicitly flagged as "Not available in WASM." The user cannot actively toggle generation for these unsupported tools. Should a backend invocation fail natively during evaluation, a resilient string noting that a "Fallback mock [has been] activated" prevents the entire UI state from crashing. 
- **Backend Fallback**: If called programmatically or directly via the `cdd-ctl` daemon without loaded WASM execution engines, the service logs the discrepancy to standard output, bypasses process instantiation, and returns safe generation placeholders indicating offline fallback states rather than throwing raw internal process exits.

## Current Support Matrix

*Note: This status corresponds to the simulated mock compilation environment capabilities at the time of writing.*

| Language Tool | Online Release | Local Build Fallback | Status | 
| :--- | :---: | :---: | :---: |
| **`cdd-typescript`** | ❌ Missing | ❌ Failed | 🔴 Unsupported |
| **`cdd-python`** | ❌ Missing | ❌ Failed | 🔴 Unsupported |
| **`cdd-rust`** | ❌ Missing | ❌ Failed | 🔴 Unsupported |
| **`cdd-go`** | ❌ Missing | ❌ Failed | 🔴 Unsupported |
| **`cdd-java`** | ❌ Missing | ❌ Failed | 🔴 Unsupported |

*In genuine usage environments, Rust and Python typically compile flawlessly under WASI (wasm32-wasi).*
