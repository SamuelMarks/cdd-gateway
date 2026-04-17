# Deployment Guide (`cdd-ctl`)

> This document provides comprehensive guides and examples for deploying `cdd-ctl` as a background service across various operating systems and platforms.


This guide provides examples for deploying `cdd-ctl` as a background service across various operating systems and environments. Since `cdd-ctl` acts as a daemon manager for 13 external `cdd-*` JSON-RPC servers, running it reliably is critical.

The application is built to handle its own internal process lifecycle (retries, logging, graceful shutdown), but the host OS must ensure `cdd-ctl` itself stays alive.

---

## 1. systemd (Ubuntu, Debian, RHEL, CentOS, Arch)

`systemd` is the standard init system for most modern Linux distributions.

1. Create a new service file at `/etc/systemd/system/cdd-ctl.service`:

```ini
[Unit]
Description=cdd-ctl Daemon Manager (API Gateway for cdd-* processes)
After=network.target
Documentation=https://github.com/SamuelMarks/cdd-ctl

[Service]
Type=simple
User=cdd-user
Group=cdd-group
# Adjust the path to where your compiled binary and config live
ExecStart=/usr/local/bin/cdd-ctl --bind 0.0.0.0:8080 --config /etc/cdd-ctl/config.json
Restart=always
RestartSec=5
LimitNOFILE=65536
Environment="RUST_LOG=info"

[Install]
WantedBy=multi-user.target
```

2. Enable and start the service:

```bash
sudo systemctl daemon-reload
sudo systemctl enable cdd-ctl
sudo systemctl start cdd-ctl
sudo systemctl status cdd-ctl
```

3. View logs:

```bash
journalctl -u cdd-ctl -f
```

---

## 2. OpenRC (Alpine Linux, Gentoo)

For systems using OpenRC (like our `alpine.Dockerfile` base or Gentoo), use `start-stop-daemon`.

1. Create an init script at `/etc/init.d/cdd-ctl`:

```bash
#!/sbin/openrc-run

name="cdd-ctl"
description="cdd-ctl Daemon Manager"
command="/usr/local/bin/cdd-ctl (or /usr/local/bin/cdd-ctl-wasm, /usr/local/bin/cdd-rpc, /usr/local/bin/cdd-rpc-wasm)"
command_args="--bind 0.0.0.0:8080 --config /etc/cdd-ctl/config.json"
command_background="yes"
pidfile="/run/${RC_SVCNAME}.pid"
output_log="/var/log/cdd-ctl.log"
error_log="/var/log/cdd-ctl.err"

# Ensure the network is up before starting
depend() {
    need net
}

start_pre() {
    export RUST_LOG=info
    checkpath --directory --owner root:root --mode 0775 /var/log
}
```

2. Make it executable and add to the default runlevel:

```bash
sudo chmod +x /etc/init.d/cdd-ctl
sudo rc-update add cdd-ctl default
sudo rc-service cdd-ctl start
```

---

## 3. Windows Service

Because `cdd-ctl` is a standard CLI executable and does not natively implement the Windows Service API (`ServiceMain`), the most robust way to run it as a service is using **NSSM (Non-Sucking Service Manager)**.

1. Download [NSSM](http://nssm.cc/).
2. Run the following commands in an elevated (Administrator) Command Prompt or PowerShell:

```powershell
# Install the service (Replace cdd-ctl with cdd-rpc or cdd-ctl-wasm if desired)
nssm install cdd-ctl "C:\path\to\cdd-ctl.exe"

# Set application arguments
nssm set cdd-ctl AppParameters "--bind 0.0.0.0:8080 --config C:\path\to\config.json"

# Set environment variables (e.g., logging level)
nssm set cdd-ctl AppEnvironmentExtra "RUST_LOG=info"

# Ensure it restarts on failure
nssm set cdd-ctl AppExit Default Restart

# Start the service
nssm start cdd-ctl
```

To view or edit the configuration via a GUI later, run: `nssm edit cdd-ctl`

---

## 4. Docker / Docker Compose

If you are running in a containerized environment, you can map your local config into the container. `cdd-ctl`'s internal manager will handle spawning the 13 `cdd-*` processes inside the container (or mock them via `external_address`).

1. Create a `docker-compose.yml`:

```yaml
version: "3.8"

services:
  cdd-ctl:
    build:
      context: .
      dockerfile: alpine.Dockerfile
    container_name: cdd-ctl-daemon
    ports:
      - "8080:8080"
    volumes:
      - ./config.json:/etc/cdd-ctl/config.json:ro
      # If your cdd-* processes are local binaries mounted into the container:
      - ./bin:/usr/local/bin/cdd-bins:ro
    environment:
      - RUST_LOG=info
    command: ["--bind", "0.0.0.0:8080", "--config", "/etc/cdd-ctl/config.json"]
    restart: unless-stopped
```

2. Run the stack:

```bash
docker-compose up -d
docker-compose logs -f
```

### 4.1. Deploying the WASM Variant via Docker

To deploy the highly secure WASM execution engine instead of the native daemon spawner, adjust the command in your `docker-compose.yml` to target the WASM binary and ensure `wasmtime` is available in your Docker image (or use a dedicated WASM-enabled Dockerfile):

```yaml
command:
  [
    "/usr/local/bin/cdd-ctl-wasm",
    "--bind",
    "0.0.0.0:8080",
    "--config",
    "/etc/cdd-ctl/config.json",
  ]
```

---

## 5. macOS (launchd)

To run `cdd-ctl` as a background daemon on macOS, you use `launchd`.

1. Create a plist file at `/Library/LaunchDaemons/com.offscale.cdd-ctl.plist` (for a system-wide daemon) or `~/Library/LaunchAgents/com.offscale.cdd-ctl.plist` (for a user-specific agent).

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.offscale.cdd-ctl</string>

    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/cdd-ctl (or /usr/local/bin/cdd-ctl-wasm, /usr/local/bin/cdd-rpc, /usr/local/bin/cdd-rpc-wasm)</string>
        <string>--bind</string>
        <string>0.0.0.0:8080</string>
        <string>--config</string>
        <string>/usr/local/etc/cdd-ctl/config.json</string>
    </array>

    <key>EnvironmentVariables</key>
    <dict>
        <key>RUST_LOG</key>
        <string>info</string>
    </dict>

    <key>RunAtLoad</key>
    <true/>

    <key>KeepAlive</key>
    <true/>

    <key>StandardOutPath</key>
    <string>/usr/local/var/log/cdd-ctl.log</string>
    <key>StandardErrorPath</key>
    <string>/usr/local/var/log/cdd-ctl.err</string>
</dict>
</plist>
```

2. Load and start the daemon:

```bash
sudo launchctl load -w /Library/LaunchDaemons/com.offscale.cdd-ctl.plist
```

---

## 6. Supervisor (Generic Linux / POSIX)

If you aren't using an init system directly, or prefer a generic process manager like `supervisord`:

1. Add a configuration block to `/etc/supervisor/conf.d/cdd-ctl.conf`:

```ini
[program:cdd-ctl]
command=/usr/local/bin/cdd-ctl (or /usr/local/bin/cdd-ctl-wasm, /usr/local/bin/cdd-rpc, /usr/local/bin/cdd-rpc-wasm) --bind 0.0.0.0:8080 --config /etc/cdd-ctl/config.json
autostart=true
autorestart=true
stderr_logfile=/var/log/cdd-ctl.err.log
stdout_logfile=/var/log/cdd-ctl.out.log
environment=RUST_LOG="info"
user=cdd-user
```

2. Update Supervisor:

```bash
sudo supervisorctl reread
sudo supervisorctl update
sudo supervisorctl start cdd-ctl
```
