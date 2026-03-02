# cdd-ctl

[![CI](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-ctl/actions/workflows/ci.yml)
[![Test Coverage](https://img.shields.io/badge/coverage-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)
[![Doc Coverage](https://img.shields.io/badge/docs-100%25-success.svg)](https://github.com/SamuelMarks/cdd-ctl/actions)

Central command-line interface, SDK, and JSON-RPC over HTTP server for the `cdd-*` toolchain.

## Features

- **CLI Interface:** Send requests to `cdd-*` tools via command line args.
- **SDK:** Pure Zig library for programmatic access.
- **JSON-RPC Server:** Extensible HTTP server serving JSON-RPC interfaces for multi-language components.
- **Process Management:** Built-in initd/systemd-like process management allowing zero-dependency deployments across Windows, Linux, macOS, and FreeBSD. Includes stats tracking (uptime, flakiness).
- **100% Coverage:** Guaranteed high quality via 100% doc coverage and test coverage requirements.

## Supported Ecosystems

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

## Usage

For detailed guides on utilizing the `cdd-*` architecture and configuring this tool, refer to our comprehensive documentation:
- [**Architecture Guide**](ARCHITECTURE.md)
- [**Usage Guide**](USAGE.md)
- [**Daemon & Service Deployment Guide**](DEPLOYMENT.md)
- [**Developing & Contributing**](DEVELOPING.md)

### CLI

Route a command to a specific language server/client:

```bash
cdd-ctl --language cpp --type client_cli -- [additional_args...]
```

### Config File

Alternatively, manage defaults via a `.json` configuration file:

```bash
cdd-ctl --config ./cdd-ctl.json -- [additional_args...]
```

### JSON-RPC Server

Start the cdd-ctl HTTP API:

```bash
cdd-ctl server --port 8080
```

Request format example:
```json
{
  "jsonrpc": "2.0",
  "method": "execute",
  "params": {
    "language": "python",
    "type": "client",
    "args": ["--input", "openapi.json"]
  },
  "id": 1
}
```
