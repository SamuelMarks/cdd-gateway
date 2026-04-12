# WebAssembly (WASM) Integration

A core strength of the `cdd-*` ecosystem is its ability to compile language-specific generators into WebAssembly (WASM) and WebAssembly System Interface (WASI) modules. This allows instantaneous, zero-latency code generation across any platform, including directly within a user's web browser, without requiring a network connection or native toolchains.

## Execution Environments

`cdd-ctl` supports WASM execution through two primary channels:

### 1. Backend Sandboxing (`cdd-ctl-wasm` & `cdd-rpc-wasm`)
Using `wasmtime`, `cdd-ctl` can evaluate `.wasm` payloads on the backend instead of spawning native OS processes. This provides a high-security, heavily sandboxed execution environment ideal for multi-tenant SaaS deployments where untrusted OpenAPI specifications are processed.

### 2. Frontend / Embedded (`cdd-ctl-wasm-sdk`)
The `cdd-ctl-wasm-sdk` package is a dedicated TypeScript library that uses `@bjorn3/browser_wasi_shim` to execute `.wasm` files directly in the browser. It mounts virtual filesystem descriptors, captures `stdout`/`stderr`, and returns the generated code artifacts natively to the frontend JavaScript context.

## Acquiring WASM Binaries

WASM binaries can be acquired via the bundled helper script, which downloads the latest stable releases from GitHub or attempts a local build fallback if releases are unavailable:

```bash
./scripts/fetch_wasm.sh
```
This script downloads artifacts into `cdd-ctl-wasm-sdk/assets/wasm/` and generates a `wasm-support.json` matrix.

## Current WASM Support Matrix

Extensive testing via `wasmtime` has yielded the following support constraints and capabilities for the 13 `cdd-*` ecosystems:

| Language Target | WASM Status | Execution Notes & Requirements |
| :--- | :---: | :--- |
| **C** (`cdd-c`) | ✅ **Supported** | Executes cleanly via standard WASI. |
| **Go** (`cdd-go`) | ✅ **Supported** | Executes cleanly via standard WASI. |
| **PHP** (`cdd-php`) | ✅ **Supported** | Executes cleanly via standard WASI (requires proper input file context). |
| **Rust** (`cdd-rust`) | ✅ **Supported** | Executes cleanly via standard WASI. |
| **Swift** (`cdd-swift`) | ✅ **Supported** | Executes cleanly via standard WASI. |
| **C++** (`cdd-cpp`) | ✅ **Supported** | Executes cleanly via standard WASI. |
| **C#** (`cdd-csharp`) | 🔴 **Unsupported** | Fails in `wasmtime` without Mono JS bindings (`mono_wasm_bind_js_import_ST`). Triggers native fallback. |
| **Kotlin** (`cdd-kotlin`) | 🔴 **Unsupported** | Fails in `wasmtime` (missing `legacy_exceptions` / GC polyfills). Triggers native fallback. |
| **Ruby** (`cdd-ruby`) | 🔴 **Unsupported** | Fails in `wasmtime` without Ruby JS ABI host bindings. Triggers native fallback. |
| **TypeScript** (`cdd-ts`) | ✅ **Supported** | Executes cleanly via standard WASI (Node.js dependencies polyfilled). |
| **Java** (`cdd-java`) | 🔴 **Unsupported** | Fails due to heavy reliance on Reflection, `java.nio`, and Sockets. Requires JVM/Docker. |
| **Python** (`cdd-python`) | ✅ **Supported** | Executes via standard WASI (compiled using `py2wasm`). |
| **Shell** (`cdd-sh`) | 🔴 **N/A** | Shell scripts are interpreted natively and are not applicable for WebAssembly compilation. |

## Fallback Gracefulness

If a specific `cdd-*` tool lacks WASM support (e.g., `cdd-java` or `cdd-sh`), the architecture degrades gracefully:
- **Frontend**: The Web UI dynamically reads the `wasm-support.json` matrix at launch. Unsupported languages are explicitly flagged and greyed out to prevent execution errors.
- **Backend (`cdd-ctl-wasm`)**: If requested via the API, the backend will identify the missing WASM capability and gracefully fall back to the native binary equivalent (if configured) or return a descriptive `400 Bad Request` indicating that the target ecosystem requires a native runtime.