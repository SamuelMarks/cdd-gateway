# Usage Guide for `cdd-ctl`

The `cdd-ctl` tool acts as the central interface for all language-specific `cdd-*` toolchains. It can be run in **CLI mode**, **Server mode**, or by providing an **External Configuration File**.

## 1. CLI Mode

In CLI mode, `cdd-ctl` spawns a managed process for the specified language implementation, acts as a pass-through layer for specific arguments, and monitors the background process for uptime, crash counts, and flakiness.

```bash
# General syntax
cdd-ctl --language <lang> --type <client|client_cli|server> -- [pass_through_args]
```

### Examples

**Running the Python Client:**
```bash
cdd-ctl --language python --type client -- --input openapi.json --output ./models
```

**Running the Rust Server:**
```bash
cdd-ctl --language rust --type server -- --port 9090 --verbose
```

---

## 2. Server Mode

Server Mode initiates a persistent HTTP JSON-RPC 2.0 API that listens on a port (default: 8080). This mode is designed for continuous integrations or IDE plugins that interact frequently with the `cdd-*` ecosystem without wanting to repeatedly spawn sub-processes.

```bash
cdd-ctl --server --port 8080
```

### JSON-RPC Request Example
```bash
curl -X POST -H "Content-Type: application/json" -d '{
  "jsonrpc": "2.0",
  "method": "execute",
  "params": {
    "language": "python",
    "type": "client",
    "args": ["--input", "openapi.json"]
  },
  "id": 1
}' http://localhost:8080
```

---

## 3. Configuration File Mode

If you do not want to pass repetitive CLI arguments, `cdd-ctl` can consume a JSON configuration file. **CLI flags take precedence over configuration file fields.**

### Syntax

```bash
cdd-ctl --config <path_to_config.json> -- [pass_through_args]
```

### Example Config File (`cdd-ctl.json`)

```json
{
  "language": "typescript",
  "component_type": "client",
  "start_server": false,
  "remote_server": "http://localhost:3000",
  "socket_path": "/tmp/cdd-typescript.sock",
  "port": 8080
}
```

If you start `cdd-ctl` with this configuration file:

```bash
cdd-ctl --config cdd-ctl.json -- --watch
```

It will execute equivalent to:
```bash
cdd-ctl --language typescript --type client --remote-server http://localhost:3000 -- --watch
```

### Overriding Config via CLI
You can override specific fields from the JSON config by simply supplying the CLI parameter:

```bash
cdd-ctl --config cdd-ctl.json --language python -- --verbose
```
*The resulting configuration will use Python instead of TypeScript, while preserving the other config attributes.*