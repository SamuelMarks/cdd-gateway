# cdd-ctl Architecture

`cdd-ctl` serves as the central orchestration layer for the multi-language `cdd-*` toolchain. Instead of asking developers to manage 13+ distinct binary SDKs and CLI tools written in various languages, `cdd-ctl` provides a single entry point.

It operates primarily in three distinct layers:
1. **The Command Line Interface (CLI) Frontend**
2. **The JSON-RPC Server**
3. **The Process & Lifecycle Manager**

## High-Level Diagram

```ascii
                      +-------------------+
                      |   User / Script   |
                      +---------+---------+
                                | (CLI Args / Config JSON)
                                v
                      +---------+---------+
                      |   cdd-ctl Entry   |
                      +----+---------+----+
                           |         |
      +--------------------+         +---------------------+
      | (Server Mode)                                      | (CLI Mode)
      v                                                    v
+-----+--------------+                             +-------+---------+
| RpcServer          |                             | Process Manager |
| (src/server.zig)   |                             | (src/process.zig|
+-----+--------------+                             +-------+---------+
      |                                                    |
      | (JSON-RPC requests)                                | (Spawns & Tracks)
      v                                                    v
+-----+----------------------------------------------------+---------+
|                  Language Sub-Processes / Sockets                  |
|          (cdd-python, cdd-rust, cdd-go, cdd-typescript)            |
+--------------------------------------------------------------------+
```

## Core Subsystems

### 1. The CLI Frontend (`src/cli.zig`)
The frontend is responsible for ingesting arguments and optionally a JSON configuration file. It merges the JSON payload and overrides it with explicitly passed CLI flags. It resolves the `Language` and `ComponentType` enum mappings.

### 2. The JSON-RPC Server (`src/server.zig`)
This subsystem launches a persistent HTTP server. It listens for `JSON-RPC 2.0` standard payloads on a configurable port. The server allows long-running systems (like IDE extensions or continuous integration pipelines) to issue commands to the `cdd-*` toolchain without incurring process-spawn overhead for every request.

### 3. Process Management (`src/process.zig`)
Because the ecosystem consists of diverse technology stacks (Python, Java, Go, Rust), `cdd-ctl` must act as an agnostic daemon manager. 
It uses a built-in cross-platform (Windows, macOS, Linux, FreeBSD) spawning utility (`ManagedProcess`). This system acts much like `systemd` or `init.d` but scoped exclusively to the `cdd-*` tooling.
It accurately tracks:
- Start counts.
- Crash counts.
- Uptime.
- **Flakiness metrics**, providing analytics on whether specific underlying language servers are unstable.

## Config & Override Precedence
Configurations are resolved in the following order (highest precedence to lowest):
1. Command-line flags (`--language`, `--port`, `--remote-server`).
2. Config file attributes (read via `--config <path>`).
3. Hardcoded CLI defaults (e.g., port `8080`).