# cdd-ctl

[![CI](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml)
[![Test Coverage](https://img.shields.io/badge/coverage-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)
[![Doc Coverage](https://img.shields.io/badge/docs-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)

Central daemon manager, API Gateway, and SDK management backend for the `cdd-*` toolchain.

`cdd-ctl` serves as the robust Rust backend orchestrating the multi-language JSON-RPC ecosystem. It features an integrated daemon-manager to supervise child processes, coupled with a comprehensive OpenAPI-driven web backend for managing Projects (Organizations), SDKs (Repositories), and Releases with Role-Based Access Control (RBAC).

## Features

- **Daemon Manager:** Built-in supervisor managing the lifecycle, standard-streams (logging), and auto-restart backoff of up to 13 distinct `cdd-*` JSON-RPC language servers.
- **REST API Gateway:** High-performance RESTful API built on `actix-web`.
- **Database & ORM:** Backed by PostgreSQL and `diesel`, managing complex relational data for Organizations, Repositories, and Releases.
- **Authentication & Security:** Secure JWT-based `Bearer` auth, featuring an OAuth2 password grant flow with **Argon2** password hashing, alongside stubs for GitHub OAuth integration.
- **Role-Based Access Control (RBAC):** Organization ownership models, ensuring secure management of underlying SDKs and sync processes.
- **OpenAPI Integration:** Fully self-documenting. Exposes a live Swagger UI at `/swagger-ui/` using `utoipa` out-of-the-box.
- **100% Coverage:** Guaranteed high quality via strict 100% Rustdoc and Test coverage requirements (`cargo tarpaulin`).

## Supported Ecosystems

`cdd-ctl` daemonizes and interfaces with the following language SDKs:

| Repository | Language | Client or Server | Extra features | OpenAPI Standard | CI Status |
|---|---|---|---|---|---|
| [`cdd-c`](https://github.com/SamuelMarks/cdd-c) | C (C89) | Client | FFI | OpenAPI 3.2.0 | [![CI/CD](https://github.com/offscale/cdd-c/workflows/cross-OS/badge.svg)](https://github.com/offscale/cdd-c/actions) |
| [`cdd-cpp`](https://github.com/SamuelMarks/cdd-cpp) | C++ | Client | Upgrades Swagger & Google Discovery to OpenAPI 3.2.0 | Swagger 2.0 until OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml) |
| [`cdd-csharp`](https://github.com/SamuelMarks/cdd-csharp) | C# | Client | CLR | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-csharp/actions/workflows/ci.yml) |
| [`cdd-go`](https://github.com/SamuelMarks/cdd-go) | Go | Client |  | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-go/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-go/actions/workflows/ci.yml) |
| [`cdd-java`](https://github.com/SamuelMarks/cdd-java) | Java | Client | | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-java/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-java/actions/workflows/ci.yml) |
| [`cdd-kotlin`](https://github.com/SamuelMarks/cdd-kotlin) | Kotlin (Multiplatform) | Client | Auto-Admin UI | OpenAPI 3.2.0 | [![CI](https://github.com/offscale/cdd-kotlin/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-kotlin/actions/workflows/ci.yml) |
| [`cdd-php`](https://github.com/SamuelMarks/cdd-php) | PHP | Client |  | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-php/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-php/actions/workflows/ci.yml) |
| [`cdd-python-client`](https://github.com/offscale/cdd-python-client) | Python | Client |  | OpenAPI 3.2.0 | [![uv](https://github.com/offscale/cdd-python-client/actions/workflows/uv.yml/badge.svg)](https://github.com/offscale/cdd-python-client/actions/workflows/uv.yml) |
| [`cdd-ruby`](https://github.com/SamuelMarks/cdd-ruby) | Ruby | Client |  | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-ruby/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-ruby/actions/workflows/ci.yml) |
| [`cdd-rust`](https://github.com/SamuelMarks/cdd-rust) | Rust | Client & Server | CLI frontend for SDK | OpenAPI 3.2.0 | [![CI](https://github.com/offscale/cdd-rust/actions/workflows/ci-cargo.yml/badge.svg)](https://github.com/offscale/cdd-rust/actions/workflows/ci-cargo.yml) |
| [`cdd-sh`](https://github.com/SamuelMarks/cdd-sh) | Shell (/bin/sh) | Client |  | OpenAPI 3.2.0 | [![CI](https://github.com/SamuelMarks/cdd-sh/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-sh/actions/workflows/ci.yml) |
| [`cdd-swift`](https://github.com/offscale/cdd-swift) | Swift | Client |  | OpenAPI 3.2.0 | [![Swift](https://github.com/SamuelMarks/cdd-swift/actions/workflows/swift.yml/badge.svg)](https://github.com/SamuelMarks/cdd-swift/actions/workflows/swift.yml) |
| [`cdd-web-ng`](https://github.com/offscale/cdd-web-ng) | TypeScript | Client | Auto-Admin UI; Angular; fetch; Axios; Node.js | OpenAPI 3.2.0 & Swagger 2 | [![Tests and coverage](https://github.com/offscale/cdd-web-ng/actions/workflows/tests_and_coverage.yml/badge.svg)](https://github.com/offscale/cdd-web-ng/actions/workflows/tests_and_coverage.yml) |

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
cargo run --release -- --bind 0.0.0.0:8080 --config ./config.json
```

### Accessing the Interactive API

Once running, the interactive OpenAPI standard documentation and sandbox is available at:
`http://localhost:8080/swagger-ui/`
