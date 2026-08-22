use std::path::{Path, PathBuf};

use anyhow::anyhow;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

pub fn socket_path() -> anyhow::Result<PathBuf> {
    let runtime_dir =
        std::env::var("XDG_RUNTIME_DIR").map_err(|_| anyhow!("XDG_RUNTIME_DIR not set"))?;
    Ok(PathBuf::from(runtime_dir).join("snipexpand.sock"))
}

pub enum IpcCmd {
    Reload,
    Status,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct DaemonStatus {
    pub running: bool,
    pub version: String,
    pub pid: u32,
    pub match_groups: usize,
    pub triggers: usize,
    pub files: usize,
    pub config_valid: bool,
}

pub struct IpcServer {
    listener: UnixListener,
    path: std::path::PathBuf,
}

impl IpcServer {
    pub async fn new(path: &Path) -> anyhow::Result<Self> {
        let _ = std::fs::remove_file(path);
        let listener = UnixListener::bind(path)?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
        })
    }

    pub async fn accept(&self) -> anyhow::Result<(IpcCmd, UnixStream)> {
        loop {
            let (stream, _) = self.listener.accept().await?;
            let mut reader = BufReader::new(stream);
            let mut line = String::new();
            reader.read_line(&mut line).await?;
            match line.trim() {
                "reload" => return Ok((IpcCmd::Reload, reader.into_inner())),
                "status" => return Ok((IpcCmd::Status, reader.into_inner())),
                other => {
                    // Log and discard. Do not let a bad client kill the daemon.
                    tracing::warn!("IPC: ignoring unknown command (len={})", other.len());
                    continue;
                }
            }
        }
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

#[allow(dead_code)]
pub async fn send_cmd(path: &Path, cmd: &str) -> anyhow::Result<String> {
    let mut stream = UnixStream::connect(path).await?;
    stream.write_all(format!("{}\n", cmd).as_bytes()).await?;
    // Signal that we are done writing.
    stream.shutdown().await?;
    let mut reader = BufReader::new(stream);
    let mut response = String::new();
    reader.read_line(&mut response).await?;
    Ok(response.trim_end_matches('\n').to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Helper: create a temporary socket path inside a TempDir.
    fn tmp_sock(dir: &TempDir) -> PathBuf {
        dir.path().join("test.sock")
    }

    #[tokio::test]
    async fn test_server_receives_reload_command() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        let server = IpcServer::new(&path).await.unwrap();

        // Spawn a client task that sends "reload\n".
        let path_clone = path.clone();
        tokio::spawn(async move {
            send_cmd(&path_clone, "reload").await.unwrap();
        });

        let (cmd, _) = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Reload));
    }

    #[tokio::test]
    async fn test_server_receives_status_command() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        let server = IpcServer::new(&path).await.unwrap();

        let path_clone = path.clone();
        tokio::spawn(async move {
            send_cmd(&path_clone, "status").await.unwrap();
        });

        let (cmd, _) = server.accept().await.unwrap();
        assert!(matches!(cmd, IpcCmd::Status));
    }

    #[tokio::test]
    async fn test_stale_socket_is_removed_on_startup() {
        let dir = TempDir::new().unwrap();
        let path = tmp_sock(&dir);

        // Create a stale file at the socket path.
        std::fs::write(&path, b"stale").unwrap();
        assert!(path.exists());

        // IpcServer::new should remove the stale file and bind successfully.
        let _server = IpcServer::new(&path).await.unwrap();
        // If we get here without error the stale-removal logic worked.
    }

    #[test]
    fn daemon_status_round_trips_as_json() {
        let status = DaemonStatus {
            running: true,
            version: "0.1.1".to_string(),
            pid: 42,
            match_groups: 3,
            triggers: 4,
            files: 2,
            config_valid: true,
        };
        let encoded = serde_json::to_string(&status).unwrap();
        let decoded: DaemonStatus = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, status);
    }
}
