# Developing `cdd-ctl`

Thank you for your interest in contributing to the `cdd-*` ecosystem's central controller and API Gateway!

This project is written natively in **Rust** and adheres strictly to a 100% test coverage and 100% documentation coverage constraint.

## Prerequisites

1. **Rust (Stable)**: Install via [rustup.rs](https://rustup.rs/).
2. **PostgreSQL**: Used for local development and testing data models.
3. **Diesel CLI**: Installed via `cargo install diesel_cli --no-default-features --features postgres`.
4. **Cargo Tarpaulin**: Installed via `cargo install cargo-tarpaulin` (used for test coverage).
5. **Git** & **pre-commit** (`pip install pre-commit` or `brew install pre-commit`).

## Getting Started

Clone the repository and install the pre-commit hooks:

```bash
git clone https://github.com/SamuelMarks/cdd-ctl
cd cdd-ctl
pre-commit install
```

## Build System

The project uses standard Rust `cargo` commands wrapped by a helper `Makefile` (and `make.bat` for Windows) for convenience.

- **To install base dependencies:**
  ```bash
  make install_base
  ```

- **To build the project executable:**
  ```bash
  make build
  # Or simply: cargo build --release
  ```

- **To run the executable:**
  ```bash
  make run
  # Or simply: cargo run -- --bind 0.0.0.0:8080
  ```

- **To run tests (Required before opening a PR):**
  ```bash
  make test
  # Or simply: cargo test
  ```

- **To calculate coverage and update README shields:**
  ```bash
  cargo tarpaulin --out Lcov
  ./scripts/update_shields.sh
  ```

- **To format code and run the linter (Required by CI):**
  ```bash
  cargo fmt
  cargo clippy --all-targets --all-features -- -D warnings
  ```

- **To generate SDK documentation:**
  ```bash
  make build_docs
  ```

## Architecture & Code Organization

We recommend reading [ARCHITECTURE.md](ARCHITECTURE.md) to understand how the components fit together.

- `src/main.rs`: The executable entry point. Bootstraps the Actix web server and Tokio daemon manager.
- `src/api/`: Actix-web route definitions, DTO payloads, and OpenAPI integration (`utoipa`).
- `src/db/`: Diesel ORM mappings, Postgres schema generation, and the `CddRepository` data access traits.
- `src/daemon.rs`: The cross-platform, async `ProcessManager` daemon utility.
- `src/config.rs`: Configuration parsing logic for binding addresses and child server deployments.

## Code Standards

- **100% Docs Coverage:** The project is compiled with `#![warn(missing_docs)]`. Every `pub` function, struct, and field must be fully documented.
- **100% Test Coverage:** Any new logic added (e.g., config parsing, API routes) must include a companion `#[cfg(test)]` block. We use `mockall` to mock the Database layer, so there is no excuse to lack unit test coverage.
- **Asynchronous Execution:** Do not perform blocking operations in the `actix-web` request handlers or Tokio daemon manager loops. Use `web::block` or Tokio's asynchronous equivalents (`tokio::fs`, `tokio::process`).
