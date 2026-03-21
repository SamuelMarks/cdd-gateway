# Usage Guide for `cdd-ctl`

The `cdd-ctl` tool acts as the central interface, daemon manager, and API Gateway for all language-specific `cdd-*` toolchains. By starting `cdd-ctl`, you bring up a unified REST API layer that supervises underlying JSON-RPC servers and manages GitHub-synced metadata (Users, Repositories, SDKs, and Organizations) using a PostgreSQL backend.

## Starting the Server

The application is built in Rust using `actix-web`. To start the gateway and daemon manager:

```bash
# Native REST mode
cargo run --bin cdd-ctl --release -- --bind 0.0.0.0:8080 --config ./servers.json

# Native JSON-RPC mode
cargo run --bin cdd-rpc --release -- --bind 0.0.0.0:8082 --config ./servers.json
```

### CLI Arguments

*   `--bind <ADDRESS>`: (Optional) Override the interface and port where the API server listens. Defaults to `0.0.0.0:8080` if not set in configuration.
*   `--config <FILE_PATH>`: (Optional) Path to a configuration file containing database connection strings and child-process definitions.

---

## Configuration File

`cdd-ctl` uses the `config` crate, which supports JSON, YAML, and TOML. It maps out your PostgreSQL database, the bind address, and specifically defines how `cdd-ctl` should interact with the 13 distinct `cdd-*` language servers.

### Example `config.json`

```json
{
  "database_url": "postgres://postgres:password@localhost/cdd",
  "server_bind": "127.0.0.1:8080",
  "servers": {
    "cdd-rust": {
      "command": "./bin/cdd-rust-rpc",
      "args": ["--listen", "9091"],
      "max_retries": 5,
      "restart_delay_ms": 2000
    },
    "cdd-go": {
      "external_address": "http://remote.golang.server:9092"
    },
    "cdd-python": {
      "command": "python",
      "args": ["-m", "cdd_rpc"]
    }
  }
}
```

*   **`command`**: The executable to spawn. If provided, `cdd-ctl` acts as its daemon manager (handles restarts, logging, and graceful shutdown).
*   **`external_address`**: If a server is hosted externally (or runs its own container), providing an `external_address` bypasses local daemon spawning and routes API Gateway requests directly to that URL.

---

## Interactive API Documentation (OpenAPI)

Once the server is running, `cdd-ctl` dynamically hosts an interactive OpenAPI specification environment using `utoipa` and Swagger UI.

Navigate to:
```text
http://localhost:8080/swagger-ui/
```

Here, you can test all the REST API endpoints directly from your browser, including:
- **Authentication**: `POST /auth/register`, `POST /auth/login`, `POST /auth/github`
- **Data Synchronization**: `POST /github/sync`
- **Organizations & Projects**: `POST /orgs`
- **SDKs & Repositories**: `POST /repos`

---

## Example Interaction Flow (CLI / Curl)

### REST Interaction (cdd-ctl / cdd-ctl-wasm)

1.  **Register a New User:**
    ```bash
    curl -X POST http://localhost:8080/auth/register          -H "Content-Type: application/json"          -d '{"username": "dev1", "email": "dev1@example.com", "password": "mypassword"}'
    ```
    *Response:* `{"token": "eyJhbGciOiJIUz..."}`

2.  **Create an Organization (Using the JWT from Step 1):**
    ```bash
    curl -X POST http://localhost:8080/orgs          -H "Authorization: Bearer eyJhbGciOiJIUz..."          -H "Content-Type: application/json"          -d '{"login": "MyOrg", "description": "SDK Projects"}'
    ```
    *Response:* `{"id": 1, "login": "MyOrg" ...}`
    *(Note: This automatically grants you the `"owner"` RBAC role over `MyOrg`)*

### JSON-RPC Interaction (cdd-rpc / cdd-rpc-wasm)

When running the RPC variant, interact with the single JSON-RPC `/rpc` endpoint via HTTP POST.

1.  **Check API Gateway Version:**
    ```bash
    curl -X POST http://localhost:8082/rpc          -H "Content-Type: application/json"          -d '{"jsonrpc": "2.0", "method": "version", "id": 1}'
    ```
    *Response:* `{"jsonrpc":"2.0","result":{"version":"0.1.0"},"id":1}`
### Browser / JavaScript SDK Interaction
If you want to execute operations offline directly in the browser without deploying `cdd-ctl`, you can install the NPM package.

```bash
npm install cdd-ctl-wasm-sdk
```

```typescript
import { CddWasmSdk } from "cdd-ctl-wasm-sdk";

// Executes cdd-python locally using its WASM binary!
const generatedFiles = await CddWasmSdk.fromOpenApi({
    ecosystem: "cdd-python",
    target: "to_sdk",
    specContent: '{"openapi": "3.2.0"}',
    wasmBinary: myFetchedWasmArrayBuffer
});
```
