# WASM Native Runtime Execution Plan

## Background & Objective
Currently, `cdd-ctl` delegates WASM execution to `wasmtime` for `cdd-*` ecosystems. Most of our SDK generators run cleanly as WebAssembly System Interface (WASI) binaries (e.g. `cdd-go`, `cdd-rust`, `cdd-cpp`). 

However, `cdd-java`, `cdd-python`, `cdd-python-all`, and `cdd-sh` require complex environments (like GraalVM JS interop or Pyodide) to run correctly. The `cdd-web-ui` supports these because they use rich browser JavaScript runtimes that bridge the execution seamlessly. When attempting to run these natively inside `cdd-ctl-wasm` via `wasmtime`, they fail because `wasmtime` does not have access to these JavaScript bridges.

Our objective is to natively implement the required host imports and file system mocking within `cdd-ctl`'s Rust environment so these SDKs can be executed via our backend exactly as they are in the browser.

## Tasks & Implementation Steps

### Phase 1: Dependency Updates & `wasmtime` Integration
- [ ] Add `wasmtime` as a direct dependency in `cdd-ctl`'s `Cargo.toml`.
  - Use the `wasmtime` and `wasmtime-wasi` crates to embed the runtime directly instead of shelling out via `std::process::Command::new("wasmtime")`.
- [ ] Refactor `src/bin/cdd-ctl-wasm.rs` to parse arguments and boot a native `wasmtime::Engine` instead of a subprocess.
- [ ] Set up a unified trait or router for `WasmExecutor` that can dispatch execution based on the target ecosystem.

### Phase 2: Pyodide Implementation (`cdd-python`, `cdd-python-all`)
- [ ] Explore native implementations for running Python-WASM payloads. `wasmtime` cannot natively execute Pyodide (which requires a JS host). 
- [ ] Investigate if we can bundle a standalone WASI-CPython image (e.g., via `py2wasm` or `rustpython`) to embed inside `cdd-ctl`.
- [ ] Mount the `/out` directory and `spec.yaml`/`spec.json` directly into the WASI virtual filesystem using `wasmtime-wasi` context builders.
- [ ] Alternatively, integrate a lightweight V8 or QuickJS engine (`rquickjs` or `v8` crate) to wrap Pyodide natively in Rust.

### Phase 3: GraalVM Implementation (`cdd-java`)
- [ ] Create a custom `wasmtime::Linker` to mock the GraalVM javascript interop functions.
- [ ] Implement the `jsbody` imports expected by GraalVM:
  - `_JSObject.stringValue___String`
  - `_JSNumber.javaDouble___Double`
  - `_JSConversion.extractJavaScriptString___String_Object`
  - `_JSObject.get___Object_Object`
  - (and the other ~10 specific imports noted in `cdd-ctl-wasm-sdk`)
- [ ] Implement the `interop` imports expected by GraalVM:
  - `stdoutWriter.printChars`
  - `stderrWriter.printChars`
  - `Date.now` & `performance.now` (mapped to Rust `std::time`)
- [ ] Create a mock representation for GraalVM objects to manage pointers and conversions between WASM memory and Rust strings.

### Phase 4: `cdd-sh` Implementation
- [ ] Find a suitable bash/sh implementation that compiles cleanly to WASI, or compile bash-to-wasm directly.
- [ ] Link `cdd-sh.wasm` against standard WASI modules to allow standard FS mapping.

### Phase 5: Testing & CI/CD
- [ ] Re-enable `cdd-java`, `cdd-python`, `cdd-python-all`, and `cdd-sh` in `src/api/rpc.rs` as supported targets for `WASM_EXECUTION_MODE`.
- [ ] Write integration tests that invoke the Rust-embedded `wasmtime` instances for these ecosystems to ensure they accurately generate JSON or SDKs.
- [ ] Clean up `.unwrap()` calls introduced during the mock implementation, preferring strict `anyhow` error handling inside the WASM execution flow.