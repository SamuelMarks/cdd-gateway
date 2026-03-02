# Deploying `cdd-ctl` as a Background Service

`cdd-ctl` supports a built-in `--daemon` flag which triggers **JSONL (JSON Lines)** structured logging formatted with timestamps and log levels. This makes it heavily optimized for parsing by ingestion tools like ELK, Fluentd, DataDog, and Splunk.

To launch `cdd-ctl` as a continuous API interface or resilient local daemon, integrate it into your operating system's native process manager.

## 1. Systemd (Linux)

To run `cdd-ctl` on a modern Linux distribution (Ubuntu, Debian, RHEL, Arch), create a `systemd` unit file.

1. Create `/etc/systemd/system/cdd-ctl.service`:
```ini
[Unit]
Description=cdd-ctl Multi-Language JSON-RPC Service
After=network.target

[Service]
Type=simple
User=your_user
# Use the daemon flag to output ECS-compatible JSONL logs
ExecStart=/usr/local/bin/cdd-ctl --server --port 8080 --daemon
Restart=on-failure
RestartSec=5
StandardOutput=syslog
StandardError=syslog
SyslogIdentifier=cdd-ctl

[Install]
WantedBy=multi-user.target
```

2. Reload and enable the service:
```bash
sudo systemctl daemon-reload
sudo systemctl enable cdd-ctl
sudo systemctl start cdd-ctl
```

3. View JSON logs:
```bash
sudo journalctl -u cdd-ctl -f
```

## 2. Launchd (macOS)

For macOS, `launchd` manages persistent agents and daemons.

1. Create a Plist file in `~/Library/LaunchAgents/com.cdd.ctl.plist`:
```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.cdd.ctl</string>
    <key>ProgramArguments</key>
    <array>
        <string>/usr/local/bin/cdd-ctl</string>
        <string>--server</string>
        <string>--port</string>
        <string>8080</string>
        <string>--daemon</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/cdd-ctl.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/cdd-ctl.err</string>
</dict>
</plist>
```

2. Load and start the agent:
```bash
launchctl load ~/Library/LaunchAgents/com.cdd.ctl.plist
launchctl start com.cdd.ctl
```

## 3. Windows Service (Windows)

On Windows, you can wrap `cdd-ctl.exe` as a background Windows Service using the built-in `sc.exe` command or tools like NSSM / WinSW. Here is the native `sc.exe` approach.

1. Open an Administrator Command Prompt.
2. Create the service:
```cmd
sc create "cdd-ctl" binPath= "C:\path	o\cdd-ctl.exe --server --port 8080 --daemon" start= auto
```
*(Note the space after `binPath=` and `start=`, it is required by `sc.exe`)*

3. Start the service:
```cmd
sc start "cdd-ctl"
```

*Note: Windows standard stdout is discarded by default for services. If you need the JSONL logs saved, wrap the execution in a `.bat` file redirecting `> C:\logs\cdd.log` or use WinSW.*

## 4. Init.d (Legacy Linux / FreeBSD)

For systems lacking `systemd`, use a standard `init.d` LSB script relying on `start-stop-daemon`.

1. Create `/etc/init.d/cdd-ctl`:
```bash
#!/bin/sh
### BEGIN INIT INFO
# Provides:          cdd-ctl
# Required-Start:    $network $local_fs
# Required-Stop:     $network $local_fs
# Default-Start:     2 3 4 5
# Default-Stop:      0 1 6
# Short-Description: cdd-ctl JSON-RPC Server
### END INIT INFO

DAEMON=/usr/local/bin/cdd-ctl
# Ensure we run in daemon mode for parseable JSON logs
ARGS="--server --port 8080 --daemon"
PIDFILE=/var/run/cdd-ctl.pid
LOGFILE=/var/log/cdd-ctl.jsonl

case "$1" in
  start)
    echo "Starting cdd-ctl..."
    start-stop-daemon --start --background --make-pidfile --pidfile $PIDFILE 
      --exec /bin/sh -- -c "exec $DAEMON $ARGS >> $LOGFILE 2>&1"
    ;;
  stop)
    echo "Stopping cdd-ctl..."
    start-stop-daemon --stop --pidfile $PIDFILE
    ;;
  restart)
    $0 stop
    sleep 2
    $0 start
    ;;
  *)
    echo "Usage: $0 {start|stop|restart}"
    exit 1
    ;;
esac
exit 0
```

2. Make executable and start:
```bash
sudo chmod +x /etc/init.d/cdd-ctl
sudo /etc/init.d/cdd-ctl start
```