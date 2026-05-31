# WASM Native Runtime Execution Plan

## Background & Objective
Currently, `cdd-ctl` delegates WASM execution to the `wasmtime` CLI subprocess for `cdd-*` ecosystems. Most of our SDK generators run cleanly as standard WebAssembly System Interface (WASI) binaries (e.g., `cdd-go`, `cdd-rust`, `cdd-cpp`). 

However, `cdd-java`, `cdd-python`, `cdd-python-all`, and `cdd-sh` are currently disabled in the backend because they require complex, custom host environments (like GraalVM JS interop or Pyodide) to run correctly. The `cdd-web-ui` supports these via browser-native JavaScript environments that bridge the execution seamlessly. When attempting to run these natively inside `cdd-ctl-wasm` via the standard `wasmtime` CLI, they fail because the CLI does not provide these custom JavaScript bridge imports.

Our objective is to completely replace the `wasmtime` CLI subprocess with an embedded, natively orchestrated `wasmtime` engine within `cdd-ctl`. This will allow us to surgically inject the required host imports, mock the file systems, and seamlessly execute these complex SDKs via our backend exactly as they function in the browser.

---

## Tasks & Implementation Steps

### Phase 1: Native `wasmtime` Embedding & Architecture
Replace the subprocess CLI call with a natively embedded WASM engine, allowing fine-grained control over execution environments.

- [ ] Add `wasmtime`, `wasmtime-wasi`, and `wasi-common` as direct dependencies in `cdd-ctl`'s `Cargo.toml`.
- [ ] Remove the `std::process::Command::new("wasmtime")` usage across `src/bin/cdd-ctl-wasm.rs` and `src/api/rpc.rs`.
- [ ] Define a `WasmExecutor` trait to standardize execution: `fn execute(&self, target: &str, input: &str, args: &[String]) -> Result<Vec<GeneratedFile>>`.
- [ ] Implement `wasmtime::Config` instantiation, explicitly enabling required WASM proposals (e.g., `--wasm-features=gc` for `cdd-kotlin`).
- [ ] Implement `wasmtime::Engine` and `wasmtime::Module` caching (using `Module::serialize`/`deserialize`) to prevent recompiling the WASM payloads on every RPC request, ensuring fast API response times.
- [ ] Create a `WasiContextBuilder` factory to standardize mounting the virtual filesystem (e.g., `/workspace`, `/out`) across all language targets.
- [ ] Inject standard environment variables (`CDD_COMMAND`, `INPUT`, `OUTPUT_DIR`) programmatically into the WASI context.
- [ ] Set up in-memory piped buffers for stdout/stderr to cleanly capture execution logs without relying on OS-level file descriptors.

### Phase 2: Pyodide Implementation (`cdd-python`, `cdd-python-all`)
Since Pyodide relies heavily on a JavaScript host to manage the CPython WASM binary and `micropip` installations, we must simulate a JS runtime in Rust.

- [ ] Add a lightweight embedded JavaScript engine dependency (e.g., `rquickjs` or `v8`) to `Cargo.toml` to act as the host for Pyodide.
- [ ] Initialize an `rquickjs::Context` inside the `cdd-python` execution flow.
- [ ] Inject the Pyodide WebAssembly module and JS glue code (`pyodide.mjs`) into the embedded JS context.
- [ ] Write a JS wrapper script inside the Rust binary that evaluates the same `micropip.install(["pydantic<2.0", "libcst", "urllib3"])` logic used in the UI's `wasm-worker.worker.ts`.
- [ ] Implement a bridge between `rquickjs`'s Pyodide virtual filesystem (`pyodide.FS`) and Rust's native memory so the `spec.yaml` can be mounted in memory.
- [ ] Handle asynchronous JS execution (`runPythonAsync`) within Rust's Tokio runtime, ensuring it does not block the main thread.
- [ ] Extract the generated SDK files by recursively reading the Pyodide `/out` directory via `pyodide.FS.readdir` and piping the byte arrays back to Rust.
- [ ] Catch Pyodide execution exceptions and gracefully extract Python traceback strings into detailed Rust `anyhow::Error` types for the API response.

### Phase 3: GraalVM Implementation (`cdd-java`)
GraalVM compiles Java to WASM but emits specific Javascript interop requirements that must be explicitly mocked in the `wasmtime::Linker`.

- [ ] Create a custom `GraalVmLinker` struct wrapping `wasmtime::Linker` to register the specific module imports GraalVM expects.
- [ ] Implement memory read/write helper functions (`read_string`, `write_string`) to translate WASM memory pointers into Rust `String` objects safely.
- [ ] Implement a simulated JS object heap (`HashMap<u32, Box<dyn Any>>`) in the `wasmtime::Store` to mock the JS object references GraalVM passes back and forth.
- [ ] Mock the `interop` namespace: Link `stdoutWriter.printChars` and `stderrWriter.printChars` to route directly into our Rust captured log buffers.
- [ ] Mock the `interop` namespace: Link `Date.now` and `performance.now`, returning accurate Unix timestamps via Rust's `std::time::SystemTime`.
- [ ] Mock the `interop` namespace: Link `runtime.setExitCode` to capture the internal GraalVM exit status for validation.
- [ ] Mock the `jsbody` namespace (Core Java<->JS translation): Link `_JSObject.stringValue___String` and `_JSNumber.javaDouble___Double`.
- [ ] Mock the `jsbody` namespace: Link `_JSConversion.extractJavaScriptString___String_Object` to parse Java strings natively.
- [ ] Mock the `compat` namespace: Link `f64rem`, `f64log`, `f64log10`, and `f64pow` by delegating directly to Rust's native `f64` mathematical methods.
- [ ] Handle the GraalVM instantiation phase accurately, ensuring `wasi.initialize(instance)` is called and the explicit `from_openapi` export is executed instead of relying solely on `_start`.

### Phase 4: Shell Implementation (`cdd-sh`)
Shell scripts do not compile natively to WASM. We must orchestrate a WASM-compatible shell interpreter to evaluate the `.sh` scripts.

- [ ] Identify and bundle a minimal `dash` or `busybox` WASI binary to act as the interpreter.
- [ ] Configure the `wasmtime-wasi` context to mount the `cdd-sh` script payload into a virtual `/bin/cdd-sh` path.
- [ ] Configure the WASI entrypoint parameters so that `argv[0]` points to the WASI-shell interpreter and `argv[1]` points to the script path.
- [ ] Map the in-memory OpenAPI specification (`spec.yaml`) directly into the WASI context's stdin descriptor, matching standard bash piping workflows.
- [ ] Extract the generated output artifacts from the WASI virtual `/out` directory back to the Rust host.

### Phase 5: Refactoring, Testing & CI/CD
Ensure stability, performance, and correctness of the new native integrations.

- [ ] Re-enable the blocked targets (`cdd-java`, `cdd-python`, `cdd-python-all`, and `cdd-sh`) in `src/api/rpc.rs` as natively supported execution targets.
- [ ] Write integration test `test_rpc_handler_to_docs_json_native_cdd_java` to explicitly test the complex GraalVM linkage logic.
- [ ] Write integration test `test_rpc_handler_to_docs_json_native_cdd_python` to verify the Pyodide/rquickjs engine initialization.
- [ ] Implement robust error handling by removing `.unwrap()` calls introduced during the rapid mocking phase, returning typed `actix_web::HttpResponse::BadRequest` on failure.
- [ ] Benchmark the overhead of embedding `wasmtime` and `rquickjs` against the previous CLI subprocess approach to ensure latency remains acceptable.
- [ ] Update `.github/workflows/ci.yml` to actively cache the Cargo registry and build artifacts, as adding `v8`/`rquickjs` and `wasmtime` will significantly increase the project's compile time.