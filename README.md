# cdd-gateway

[![CI](https://github.com/SamuelMarks/cdd-gateway/actions/workflows/ci.yml/badge.svg)](https://github.com/SamuelMarks/cdd-gateway/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/license-Apache--2.0%20OR%20MIT-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![Test Coverage](https://img.shields.io/badge/coverage-100%25-success.svg)](https://github.com/SamuelMarks/cdd-gateway/actions)
[![Doc Coverage](https://img.shields.io/badge/docs-100%25-success.svg)](https://github.com/SamuelMarks/cdd-gateway/actions)

API Gateway and management plane for the `cdd-*` toolchain.

This repository contains the unified ingress controller, reverse proxy, and Control Plane API powered by `actix-web`, alongside database migrations for PostgreSQL via `diesel` and Role-Based Access Control (RBAC) logic.

## Overview

`cdd-gateway` serves as the primary ingress and Control Plane backend for [`cdd-web-ui`](https://github.com/SamuelMarks/cdd-web-ui), the central graphical interface and frontend dashboard for the CDD ecosystem. 

Through this gateway, the web UI performs centralized actions:
- **State Management:** Manages organizations, users, authentication, and repository syncing.
- **Code Generation Offloading:** Delegates the heavy lifting of compiling and generating AST payloads to the underlying `cdd-engine` orchestrator.
- **Webhooks:** Listens to GitHub webhook payloads and coordinates repository synchronization.

While `cdd-web-ui` is capable of running fully offline utilizing in-browser WebAssembly execution, `cdd-gateway` comes into play for cloud, team, and "Served" execution modes—enabling multi-tenant generation pipelines and centralized storage.

For a deeper dive into the system design, see [ARCHITECTURE.md](ARCHITECTURE.md).

## License

This project is dual-licensed under either of the following, at your option:

- Apache License, Version 2.0 (LICENSE-APACHE or <https://www.apache.org/licenses/LICENSE-2.0>)
- MIT License (LICENSE-MIT or <https://opensource.org/licenses/MIT>)
