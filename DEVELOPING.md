# Developing `cdd-ctl`

Thank you for your interest in contributing to the `cdd-*` ecosystem's central controller!

This project is written purely in **Zig** and adheres strictly to a 100% test coverage and 100% documentation coverage constraint.

## Prerequisites

1. **Zig 0.13.0** (or whatever version is specified in `.github/workflows/ci.yml`).
2. **Git**
3. **pre-commit** (`pip install pre-commit` or `brew install pre-commit`)

## Getting Started

Clone the repository and install the pre-commit hooks:

```bash
git clone https://github.com/SamuelMarks/cdd-ctl
cd cdd-ctl
pre-commit install
```

## Build System

The project uses the standard Zig `build.zig` system. There are no Makefiles or complex shell scripts.

- **To build the project executable & static SDK:**
  ```bash
  zig build
  ```
  The resulting artifact will be located in `zig-out/bin/cdd-ctl`.

- **To run the executable:**
  ```bash
  zig build run -- [arguments...]
  # E.g., zig build run -- --language rust --type client_cli -- --verbose
  ```

- **To run tests (Required before opening a PR):**
  ```bash
  zig build test
  ```

- **To format code (Required by CI):**
  ```bash
  zig fmt .
  ```

- **To generate SDK documentation:**
  ```bash
  zig build docs
  ```
  The generated HTML will be located in `zig-out/docs/`.

## Architecture & Code Organization

We recommend reading [ARCHITECTURE.md](ARCHITECTURE.md) to understand how the components fit together.

- `src/main.zig`: The executable entry point. Routes CLI flags to respective systems.
- `src/root.zig`: The SDK library entry point. Exposes public modules to downstream code.
- `src/cli.zig`: Handles argument parsing and JSON configuration files.
- `src/server.zig`: Implements the JSON-RPC over HTTP stack.
- `src/process.zig`: Contains the cross-platform `ManagedProcess` daemon utility.

## Code Standards

- **100% Docs Coverage:** Every `pub` function, struct, and field must have `///` doc comments.
- **100% Test Coverage:** Any new logic added (e.g., config parsing, state management) must include a companion `test` block at the bottom of the file it modifies.
- **No external dependencies:** Stick to the Zig Standard Library whenever feasible. Cross-platform support is mandatory (Windows, macOS, Linux, FreeBSD).