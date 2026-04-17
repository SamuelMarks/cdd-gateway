# cdd-ctl

> This document is the main entry point for the project, providing a high-level overview, feature summary, and links to further documentation.


[![CI](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Test Coverage](https://img.shields.io/badge/coverage-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)
[![Doc Coverage](https://img.shields.io/badge/docs-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)

Central daemon manager, API Gateway, and SDK management backend for the `cdd-*` toolchain.

`cdd-ctl` serves as the robust Rust backend orchestrating the multi-language JSON-RPC ecosystem. It features an integrated daemon-manager to supervise child processes, coupled with a comprehensive OpenAPI-driven web backend for managing Projects (Organizations), SDKs (Repositories), and Releases with Role-Based Access Control (RBAC).

## Features

- **Daemon Manager:** Built-in supervisor managing the lifecycle, standard-streams (logging), and auto-restart backoff of up to 13 distinct `cdd-*` JSON-RPC language servers.
- **REST API Gateway:** High-performance RESTful API built on `actix-web`.
- **Database & ORM:** Fully implemented PostgreSQL data models for Organizations, Users, Repositories, and Releases using `diesel`.
- **GitHub Integrations:** Integrated GitHub OAuth, webhooks, and automated secrets management (via Libsodium-sealed Action Secrets).
- **Authentication & Security:** Secure JWT-based `Bearer` auth, featuring an OAuth2 password grant flow with **Argon2** password hashing.
- **Role-Based Access Control (RBAC):** Organization ownership models, ensuring secure management of underlying SDKs and sync processes.
- **OpenAPI Integration:** Fully self-documenting. Exposes a live Swagger UI at `/swagger-ui/` using `utoipa` out-of-the-box.
- **100% Coverage:** Guaranteed high quality via strict 100% Rustdoc and Test coverage requirements (`cargo tarpaulin`).

## Supported Ecosystems

`cdd-ctl` daemonizes and interfaces with the following language SDKs:

| Repository                                                     | Language                        | Client; Client CLI; Server | Extra features                                       | OpenAPI Standard                | CI Status                                                                                                                                                        | Browser/WASI   | WASM Notes                                                    |
| -------------------------------------------------------------- | ------------------------------- | -------------------------- | ---------------------------------------------------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------- | ------------------------------------------------------------- |
| [`cdd-c`](https://github.com/SamuelMarks/cdd-c)                | C (C89)                         | Client; Client CLI; Server | FFI                                                  | OpenAPI 3.2.0                   | [![CI/CD](https://github.com/offscale/cdd-c/workflows/cross-OS/badge.svg)](https://github.com/offscale/cdd-c/actions)                                            | ✅ Supported   | 0.83MB - Executes via pure WASI                               |
| [`cdd-cpp`](https://github.com/SamuelMarks/cdd-cpp)            | C++                             | Client; Client CLI; Server | Upgrades Swagger & Google Discovery to OpenAPI 3.2.0 | Swagger 2.0 until OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml)        | ✅ Supported   | 0.62MB - Executes via pure WASI                               |
| [`cdd-csharp`](https://github.com/SamuelMarks/cdd-csharp)      | C#                              | Client; Client CLI; Server | CLR                                                  | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml)        | ✅ Supported   | TBA - Executes via pure WASI (Wasi.Sdk)                       |
| [`cdd-go`](https://github.com/SamuelMarks/cdd-go)              | Go                              | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-go/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-go/actions/workflows/ci.yml)                | ✅ Supported   | 13.31MB - Executes via pure WASI                              |
| [`cdd-java`](https://github.com/SamuelMarks/cdd-java)          | Java                            | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-java/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-java/actions/workflows/ci.yml)            | ✅ Supported   | TBA - Executes via pure WASI (GraalVM native-image)           |
| [`cdd-kotlin`](https://github.com/offscale/cdd-kotlin)         | Kotlin (ktor for Multiplatform) | Client; Client CLI; Server | Auto-Admin UI                                        | OpenAPI 3.2.0                   | [![CI](https://github.com/offscale/cdd-kotlin/actions/workflows/ci.yml/badge.svg)](https://github.com/offscale/cdd-kotlin/actions/workflows/ci.yml)              | ✅ Supported   | 0.01MB - Executes via pure WASI                               |
| [`cdd-php`](https://github.com/SamuelMarks/cdd-php)            | PHP                             | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-php/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-php/actions/workflows/ci.yml)              | ✅ Supported   | 5.96MB - Executes via pure WASI                               |
| [`cdd-python-all`](https://github.com/offscale/cdd-python-all) | Python                          | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/offscale/cdd-python-client/actions/workflows/ci.yml/badge.svg)](https://github.com/offscale/cdd-python-all/actions/workflows/ci.yml)   | ✅ Supported   | 48.05MB - Executes via WASI (py2wasm)                         |
| [`cdd-ruby`](https://github.com/SamuelMarks/cdd-ruby)          | Ruby                            | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-ruby/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-ruby/actions/workflows/ci.yml)            | ✅ Supported   | 51.19MB - Executes via WASI (rbwasm)                          |
| [`cdd-rust`](https://github.com/SamuelMarks/cdd-rust)          | Rust                            | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/offscale/cdd-rust/actions/workflows/ci-cargo.yml/badge.svg)](https://github.com/offscale/cdd-rust/actions/workflows/ci-cargo.yml)      | ✅ Supported   | 6.70MB - Executes via pure WASI                               |
| [`cdd-sh`](https://github.com/SamuelMarks/cdd-sh)              | Shell (/bin/sh)                 | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![CI](https://github.com/SamuelMarks/cdd-sh/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-sh/actions/workflows/ci.yml)                | 🔴 N/A         | Not applicable for Shell                                      |
| [`cdd-swift`](https://github.com/SamuelMarks/cdd-swift)        | Swift                           | Client; Client CLI; Server |                                                      | OpenAPI 3.2.0                   | [![Swift](https://github.com/SamuelMarks/cdd-swift/actions/workflows/swift.yml/badge.svg)](https://github.com/SamuelMarks/cdd-swift/actions/workflows/swift.yml) | ✅ Supported   | 91.33MB - Executes via pure WASI                              |
| [`cdd-ts`](https://github.com/offscale/cdd-ts)                 | TypeScript                      | Client; Client CLI; Server | Auto-Admin UI; Angular; fetch; Axios; Node.js        | OpenAPI 3.2.0 & Swagger 2       | [![Tests and coverage](https://github.com/offscale/cdd-ts/actions/workflows/ci.yml/badge.svg)](https://github.com/offscale/cdd-ts/actions/workflows/ci.yml)      | ✅ Supported   | 138.36MB - Executes via WASI (Node.js polyfilled)             |

_See `cdd_docs_prompt.md` and `TO_DOCS_JSON.md` in this repository for the system prompts used to unify documentation and CLI interfaces across the entire `cdd-_` ecosystem.\*

### Browser-Native Execution

Alongside the CLI and server modes, the project ships with a pure-JavaScript WASI-compatible execution environment: **`cdd-ctl-wasm-sdk`**. This allows you to evaluate your OpenAPI generation schemas directly within the browser (client-side) against the 12 fully supported compiled `cdd-*` WASM binaries (C, C++, C#, Go, Java, Kotlin, PHP, Python, Ruby, Rust, Swift, TypeScript) without requiring backend communication. Heavy-VM runtimes are intentionally unsupported in this mode and rely on graceful degradation to JSON-RPC over HTTP.

## Documentation

For detailed guides on utilizing the `cdd-*` architecture and configuring this tool, refer to our comprehensive documentation:

- [**Architecture Guide**](ARCHITECTURE.md)
- [**Usage Guide**](USAGE.md)
- [**Daemon & Service Deployment Guide**](DEPLOYMENT.md)
- [**Developing & Contributing**](DEVELOPING.md)

## Quick Start

### Starting the Server

You can launch the API gateway and daemon-manager via Cargo:

```bash
# Native dependencies mode (REST interface)
cargo run --bin cdd-ctl --release -- --bind 0.0.0.0:8080 --config ./config.json

# WASM mode (runs compiled WASM binaries via wasmtime, REST interface)
cargo run --bin cdd-ctl-wasm --release -- --bind 0.0.0.0:8081 --config ./config.json

# JSON-RPC Native dependencies mode (JSON RPC over HTTP)
cargo run --bin cdd-rpc --release -- --bind 0.0.0.0:8082 --config ./config.json

# JSON-RPC WASM mode (runs compiled WASM binaries via wasmtime, JSON RPC over HTTP)
cargo run --bin cdd-rpc-wasm --release -- --bind 0.0.0.0:8083 --config ./config.json
```

### Accessing the Interactive API

Once running, the interactive OpenAPI standard documentation and sandbox is available at:
`http://localhost:8080/swagger-ui/`

## Test and Documentation Coverage

This repository is maintained with strict 100% test coverage and 100% rustdoc coverage requirements enforced in CI.

---

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <https://opensource.org/licenses/MIT>)

at your option.

### Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
