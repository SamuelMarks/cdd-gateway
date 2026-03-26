# cdd-ctl Architecture

`cdd-ctl` serves as the central orchestration layer and API gateway for the multi-language `cdd-*` toolchain. Rewritten natively in Rust, it provides a highly concurrent, reliable foundation for managing the execution, synchronization, and authentication of 13+ distinct language SDKs and components.

It operates primarily across three distinct layers:
1. **The API Gateway (Actix Web)** - Exposing both REST and JSON-RPC interfaces.
2. **The Database & ORM (PostgreSQL & Diesel)**
3. **The Process & Lifecycle Daemon Manager (Tokio)**
4. **The WASM Execution Engine (`wasmtime`)** - Evaluates language-specific payloads directly in WASM execution modes.

## High-Level Diagram

```ascii
                      +-------------------+
                      |   Web UI / CLI    |
                      +---------+---------+
                                | (HTTP/REST / OpenAPI)
                                v
                      +---------+---------+
                      |  cdd-ctl Gateway  |
                      |   (actix-web)     |
                      +----+---------+----+
                           |         |
      +--------------------+         +---------------------+
      | (DB Queries via Diesel)                            | (Lifecycle Events / Wasmtime calls)
      v                                                    v
+-----+--------------+                             +-------+---------+
| PostgreSQL DB      |                             | Daemon Manager  |
| (Organizations,    |                             | (Tokio Tasks)   |
|  Users, Repos,     |                             +-------+---------+
|  Releases, RBAC)   |                                     |
+--------------------+                                     | (Spawns & Tracks)
                                                           v
                           +----------------------------------------------------+
                           |             cdd-* JSON-RPC Servers                 |
                           |   (cdd-python, cdd-rust, cdd-go, cdd-typescript)   |
                           +----------------------------------------------------+
```

## Core Subsystems

### 1. The REST API Gateway (`src/api/`)
Built upon `actix-web`, this component provides a secure, OpenAPI-compliant REST interface.
- **Routing & Sync:** Provides endpoints for managing Organizations, Users, Repositories (SDKs), and Releases. Future extensions handle secret management and direct syncing with the GitHub API.
- **Authentication:** Enforces JWT `Bearer` token auth (`src/api/auth_middleware.rs`). Issues tokens via an OAuth2 password grant flow hashed via **Argon2** and supports GitHub OAuth login stubs.
- **OpenAPI / Swagger:** Utilizes `utoipa` to generate live OpenAPI 3.x specifications automatically from the Rust codebase. A live sandbox is exposed at `/swagger-ui/`.

### 2. Database & Data Models (`src/db/`)
The database layer uses `diesel` (an asynchronous-friendly ORM in Rust) wrapping a `r2d2` PostgreSQL connection pool.
- **Entities:** Manages the relational mapping of `Users`, `Organizations`, `Repositories`, and `Releases`.
- **RBAC (Role-Based Access Control):** Uses many-to-many link tables (`organization_users`) storing explicit string-based roles (e.g., `"owner"`, `"member"`) to securely gate access to mutation APIs (like `POST /repos`).
- **Abstract Repository Pattern:** To ensure 100% test coverage and dependency inversion, `CddRepository` provides an async trait abstraction, allowing the business logic to be tested against a `mockall` mock repository without a live database.

### 3. The Daemon Manager (`src/daemon.rs`)
Because the ecosystem consists of diverse technology stacks (Python, Java, Go, Rust, Zig, C++, etc.), `cdd-ctl` must act as an agnostic process supervisor. Built fully on Tokio's async runtime, it acts as an embedded `initd` or `systemd`.
- **Concurrency:** Spawns distinct tasks for each monitored process, allowing non-blocking I/O handling.
- **I/O Standardizing:** Captures `stdout` and `stderr` from all 13 RPC servers, tagging and logging lines securely via the unified `log` crate.
- **Resilience:** Implements auto-restart backoffs, tracking uptime to distinguish between persistent crashes (which eventually halt retries) and sporadic failures (which reset retry counters upon stabilization).
- **Graceful Shutdown:** Subscribes all processes to a Tokio `watch` channel to cleanly cascade termination signals across the entire language-server fleet when the main gateway stops.


### 4. Binary Targets (`src/bin/`)
The architecture compiles down into distinct binaries to support various deployment strategies and interface preferences:
- **`cdd-ctl`**: The default manager. Provides a REST API gateway and spawns/supervises native `cdd-*` executables as background daemons.
- **`cdd-ctl-wasm`**: The WASM variant of the REST API gateway. Instead of spawning native daemon processes, it uses `wasmtime` to securely evaluate `.wasm` builds of the supported `cdd-*` ecosystems within a robust, multi-tenant sandbox. Unsupported targets (interpreted languages or heavy VMs) fallback to an HTTP 400 rejection.
- **`cdd-rpc`**: Provides a JSON-RPC 2.0 over HTTP interface instead of REST, managing native `cdd-*` background daemons.
- **`cdd-rpc-wasm`**: Provides a JSON-RPC 2.0 over HTTP interface, securely evaluating payloads via `wasmtime` against `.wasm` modules.

### 5. Git Submodules (`sdks/`)
Instead of relying strictly on network downloads or stale releases, the 13 `cdd-*` language SDKs are bundled as git submodules within the `sdks/` directory. This guarantees that `cdd-ctl` can reliably build and pin all child processes or WASM targets strictly to their latest `master` commits within a unified monorepo-like environment.

### 6. Client-Side WASM SDK (`cdd-ctl-wasm-sdk/`)
This is an isolated TypeScript/NPM package that wraps `@bjorn3/browser_wasi_shim`. It mounts virtual filesystem descriptors, parses WASM execution outputs, and allows executing 5 of the fully supported standalone `.wasm` payloads (C, Go, PHP, Rust, Swift) directly within a user's web browser, offline. Targets that rely on heavy JVM/CLR environments or NodeJS polyfills are unsupported in this purely client-side shim and must gracefully degrade to JSON-RPC HTTP calls back to a native `cdd-ctl` container environment.

## Configuration
Configurations are handled elegantly via the `config` crate, resolving environment variable overrides (`CDD__SERVER_BIND`) or falling back to defaults in a `config.json` file.
