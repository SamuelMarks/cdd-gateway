//! Daemon manager for external JSON-RPC servers (cdd-* projects).
#![allow(clippy::needless_return)]

use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::{watch, Mutex};
use tokio::task::JoinHandle;

fn default_max_retries() -> usize {
    5
}
fn default_restart_delay_ms() -> u64 {
    2000
}

/// Configuration for a single managed process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProcessConfig {
    /// Command to run (e.g., `node`, `./cdd-go`). If omitted, assumes an external address.
    pub command: Option<String>,
    /// Arguments to pass to the command.
    pub args: Option<Vec<String>>,
    /// External address (e.g., `https://remote.server` or `/tmp/socket.sock`) overriding the local spawn.
    pub external_address: Option<String>,
    /// Maximum number of consecutive retries before giving up.
    #[serde(default = "default_max_retries")]
    pub max_retries: usize,
    /// Delay between restarts in milliseconds.
    #[serde(default = "default_restart_delay_ms")]
    pub restart_delay_ms: u64,
}

/// Daemon manager that keeps track of the processes.
pub struct ProcessManager {
    /// Configurations.
    pub configs: HashMap<String, ProcessConfig>,
    /// Active monitor tasks keyed by their logical name.
    handles: Arc<Mutex<HashMap<String, JoinHandle<()>>>>,
    /// Channel to signal shutdown to all monitors.
    shutdown_tx: watch::Sender<bool>,
}

impl ProcessManager {
    /// Create a new ProcessManager from configurations.
    pub fn new(configs: HashMap<String, ProcessConfig>) -> Self {
        let (shutdown_tx, _) = watch::channel(false);
        Self {
            configs,
            handles: Arc::new(Mutex::new(HashMap::new())),
            shutdown_tx,
        }
    }

    /// Start all configured local processes.
    pub async fn start_all(&self) -> Result<(), String> {
        let mut handles = self.handles.lock().await;

        for (name, config) in &self.configs {
            if let Some(ref external) = config.external_address {
                info!(
                    "[{}] Configured to use external address: {}",
                    name, external
                );
                continue;
            }

            if config.command.is_none() {
                error!("[{}] No command or external address configured", name);
                continue;
            }

            let name_clone = name.clone();
            let config_clone = config.clone();
            let shutdown_rx = self.shutdown_tx.subscribe();

            let handle = tokio::spawn(async move {
                Self::monitor_process(name_clone, config_clone, shutdown_rx).await;
            });

            handles.insert(name.clone(), handle);
        }
        Ok(())
    }

    /// Stop all managed local processes and wait for them to exit gracefully.
    pub async fn stop_all(&self) {
        info!("Initiating graceful shutdown of all managed processes...");
        let _ = self.shutdown_tx.send(true);

        let mut handles = self.handles.lock().await;
        for (name, handle) in handles.drain() {
            info!("Waiting for process monitor '{}' to exit...", name);
            let _ = handle.await;
        }
        info!("All managed processes stopped.");
    }

    /// The core monitor loop for a single process. Handles spawning, logging, and restarting.
    async fn monitor_process(
        name: String,
        config: ProcessConfig,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let cmd_str = config.command.unwrap();
        let mut retries = 0;

        loop {
            info!("[{}] Starting process: {}", name, cmd_str);
            let mut cmd = Command::new(&cmd_str);
            if let Some(ref args) = config.args {
                cmd.args(args);
            }

            cmd.stdout(Stdio::piped()).stderr(Stdio::piped());

            let start_time = tokio::time::Instant::now();

            let mut child = match cmd.spawn() {
                Ok(c) => c,
                Err(e) => {
                    error!("[{}] Failed to spawn: {}", name, e);
                    if retries >= config.max_retries {
                        error!(
                            "[{}] Max retries ({}) reached. Giving up.",
                            name, config.max_retries
                        );
                        return;
                    }
                    retries += 1;
                    tokio::time::sleep(Duration::from_millis(config.restart_delay_ms)).await;
                    continue;
                }
            };

            // Capture and standardize standard streams
            let stdout = child.stdout.take().unwrap();
            let stderr = child.stderr.take().unwrap();

            let name_out = name.clone();
            let mut stdout_reader = BufReader::new(stdout).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stdout_reader.next_line().await {
                    info!("[{}] {}", name_out, line);
                }
            });

            let name_err = name.clone();
            let mut stderr_reader = BufReader::new(stderr).lines();
            tokio::spawn(async move {
                while let Ok(Some(line)) = stderr_reader.next_line().await {
                    warn!("[{}] ERR: {}", name_err, line);
                }
            });

            tokio::select! {
                status_res = child.wait() => {
                    match status_res {
                        Ok(status) => {
                            if status.success() {
                                info!("[{}] Exited successfully.", name);
                            } else {
                                warn!("[{}] Exited with status: {}", name, status);
                            }
                        }
                        Err(e) => {
                            error!("[{}] Error waiting for process: {}", name, e);
                        }
                    }

                    if *shutdown_rx.borrow() {
                        info!("[{}] Shutdown requested, not restarting.", name);
                        break;
                    }

                    // Reset retries if the process was stable for at least 10 seconds
                    if start_time.elapsed() > Duration::from_secs(10) {
                        info!("[{}] Process was stable. Resetting retry count.", name);
                        retries = 0;
                    }

                    if retries >= config.max_retries {
                        error!("[{}] Max retries ({}) reached after crash. Giving up.", name, config.max_retries);
                        break;
                    }

                    retries += 1;
                    warn!("[{}] Restarting in {} ms (Retry {}/{})", name, config.restart_delay_ms, retries, config.max_retries);
                    tokio::time::sleep(Duration::from_millis(config.restart_delay_ms)).await;
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        info!("[{}] Shutdown signaled. Killing process.", name);
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        break;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_process_manager_external() {
        let mut configs = HashMap::new();
        configs.insert(
            "cdd-go".to_string(),
            ProcessConfig {
                command: None,
                args: None,
                external_address: Some("http://localhost:8080".to_string()),
                max_retries: 3,
                restart_delay_ms: 100,
            },
        );
        let manager = ProcessManager::new(configs);
        manager.start_all().await.unwrap();

        let handles = manager.handles.lock().await;
        assert!(handles.is_empty());
    }

    #[tokio::test]
    async fn test_process_manager_local() {
        let mut configs = HashMap::new();
        configs.insert(
            "test-echo".to_string(),
            ProcessConfig {
                command: Some("echo".to_string()),
                args: Some(vec!["hello".to_string()]),
                external_address: None,
                max_retries: 1,
                restart_delay_ms: 100,
            },
        );
        let manager = ProcessManager::new(configs);
        manager.start_all().await.unwrap();

        {
            let handles = manager.handles.lock().await;
            assert_eq!(handles.len(), 1);
            assert!(handles.contains_key("test-echo"));
        }

        // Let it start and then stop
        tokio::time::sleep(Duration::from_millis(50)).await;
        manager.stop_all().await;
    }
}
