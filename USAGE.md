# Usage Guide for `cdd-ctl`

> This document is the end-user guide detailing how to run, configure, and embed `cdd-ctl` via native CLI, WASM, background server, or as a Rust crate.


The `cdd-ctl` ecosystem provides extremely flexible deployment models. Depending on your environment, you can use it as a native CLI tool, a WASM-backed CLI, a centralized background server, an embedded browser library, or a Rust SDK.

## 1. Via CLI (Native)

You can run the `cdd-ctl` daemon manager directly from your command line. In this mode, `cdd-ctl` runs natively on your host OS and spawns native `cdd-*` child processes (like `cdd-rust`, `cdd-python`, etc.).

```bash
# Run the native REST API Gateway and daemon manager
cargo run --bin cdd-ctl --release -- --bind 127.0.0.1:8080 --config ./servers.json

# Alternatively, run the JSON-RPC variant
cargo run --bin cdd-rpc --release -- --bind 127.0.0.1:8082 --config ./servers.json
```

**CLI Arguments:**
*   `--bind <ADDRESS>`: Override the interface and port where the API server listens.
*   `--config <FILE_PATH>`: Path to a configuration file containing database strings and child-process definitions.

## 2. Via CLI with WASM

If you prefer to avoid installing native dependencies for all 13 supported languages (Python, Go, etc.), you can run the WASM variant from the CLI. This mode utilizes `wasmtime` to execute pre-compiled WebAssembly binaries of the `cdd-*` toolchain safely sandboxed on your machine.

```bash
# Ensure wasmtime is installed
wasmtime --version

# Run the WASM-backed REST API Gateway
cargo run --bin cdd-ctl-wasm --release -- --bind 127.0.0.1:8081 --config ./servers.json

# Or the WASM-backed JSON-RPC variant
cargo run --bin cdd-rpc-wasm --release -- --bind 127.0.0.1:8083 --config ./servers.json
```
*Note: If your configuration does not define any servers, `cdd-ctl-wasm` will automatically populate the configuration to use `wasmtime` against the `.wasm` files located in `cdd-ctl-wasm-sdk/assets/wasm/`.*

## 3. As a Server (Native REST/RPC)

For production deployments, `cdd-ctl` is designed to be run as a persistent background server or inside a Docker container (see `alpine.Dockerfile` or `debian.Dockerfile`). It acts as a supervisor, restarting failed language servers and routing API Gateway traffic to them.

Create a `config.json` that defines your native servers:

```json
{
  "database_url": "postgres://postgres:password@localhost/cdd",
  "server_bind": "0.0.0.0:8080",
  "servers": {
    "cdd-rust": {
      "command": "./bin/cdd-rust-rpc",
      "args": ["--listen", "9091"],
      "max_retries": 5,
      "restart_delay_ms": 2000
    },
    "cdd-go": {
      "external_address": "http://remote.golang.server:9092"
    }
  }
}
```

Deploy the server using `systemd` or Docker, and interact with it via REST. Out of the box, you can access the interactive OpenAPI standard documentation and sandbox at `http://localhost:8080/swagger-ui/`.

```bash
# Register a user
curl -X POST http://localhost:8080/auth/register \
     -H "Content-Type: application/json" \
     -d '{"username": "dev1", "email": "dev1@example.com", "password": "mypassword"}'
```

## 4. As a Server with WASM

You can deploy `cdd-ctl-wasm` as your centralized server to ensure a highly secure, sandboxed execution environment. This is especially useful in multi-tenant architectures where you are processing untrusted OpenAPI specifications and executing dynamic generation jobs.

Configure your `config.json` to explicitly use the WASM runtime:

```json
{
  "database_url": "postgres://postgres:password@localhost/cdd",
  "server_bind": "0.0.0.0:8080",
  "servers": {
    "cdd-python": {
      "command": "wasmtime",
      "args": ["run", "/opt/wasm/cdd-python.wasm"],
      "max_retries": 3,
      "restart_delay_ms": 1000
    }
  }
}
```

Run the server persistently as you would the native version. The daemon manager will supervise the `wasmtime` processes instead of raw language binaries.

## 5. Embedded in WASM (Browser / Node.js)

If you want to execute operations completely offline, directly inside a user's web browser, or via a lightweight Node.js script, you can use the TypeScript/JavaScript SDK. This bypasses the Rust backend entirely and runs the `cdd-*` WASM binaries client-side.

```bash
npm install cdd-ctl-wasm-sdk
```

```typescript
import { CddWasmSdk } from "cdd-ctl-wasm-sdk";

// Executes the cdd-python generator locally using its WASM binary
const generatedFiles = await CddWasmSdk.fromOpenApi({
    ecosystem: "cdd-python",
    target: "to_sdk",
    specContent: '{"openapi": "3.2.0", "info": {"title": "Test"}}',
    wasmBinary: myFetchedWasmArrayBuffer
});

console.log(generatedFiles);
```

## 6. As an SDK (Rust Crate)

You can embed `cdd-ctl`'s powerful daemon management, database ORM, and GitHub integration logic directly into your own custom Rust applications by adding it as a library dependency.

Add it to your `Cargo.toml`:

```toml
[dependencies]
cdd-ctl = { path = "../cdd-ctl" } # Or fetch from crates.io if published
```

Use the `ProcessManager` or Database repositories directly in your Rust code:

```rust
use std::sync::Arc;
use cdd_ctl::{ProcessManager, ProcessConfig, PgRepository};
use std::collections::HashMap;

#[tokio::main]
async fn main() {
    // 1. Setup custom daemon orchestration
    let mut servers = HashMap::new();
    servers.insert("custom-worker".to_string(), ProcessConfig {
        command: Some("my-worker-binary".to_string()),
        args: Some(vec!["--start".to_string()]),
        external_address: None,
        max_retries: 3,
        restart_delay_ms: 1000,
    });

    let manager = ProcessManager::new(servers);
    manager.start_all().await.unwrap();

    // 2. Utilize the built-in Postgres repository logic
    let pool = cdd_ctl::db::establish_connection_pool("postgres://...");
    let repo = PgRepository { pool };
    
    // ... custom logic ...

    manager.stop_all().await;
}
```
## 7. Dumping the OpenAPI Specification

You can dynamically generate and dump the complete `openapi.json` specification from the source code via the `dump_openapi` utility binary.

```bash
cargo run --bin dump_openapi
```

This will output the `openapi.json` file to your current working directory.
