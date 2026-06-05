# cdd-gateway

[![CI](https://github.com/SamuelMarks/cdd-gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-gateway/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Test Coverage](https://img.shields.io/badge/coverage-100%25-success.svg)](https://github.com/SamuelMarks/cdd-gateway/actions)
[![Doc Coverage](https://img.shields.io/badge/docs-100%25-success.svg)](https://github.com/SamuelMarks/cdd-gateway/actions)

API Gateway and management plane for the `cdd-*` toolchain.
This repository contains the REST API powered by `actix-web`, the database migrations for PostgreSQL via `diesel`, and Role-Based Access Control logic. 

## Overview

`cdd-gateway` delegates the heavy lifting of executing processes and WASM payloads to the `cdd-engine` crate. It focuses on HTTP serving, database persistence, synchronization with the GitHub API, and exposing an interactive OpenAPI Sandbox.

## License

This project is dual-licensed under either of the following, at your option:

- Apache License, Version 2.0 (LICENSE-APACHE or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License (LICENSE-MIT or <https://opensource.org/licenses/MIT>)
