use crate::models::TransactionManifest;
use serde::{Deserialize, Serialize};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};

const CMD_FILE_DIR: &str = "/var/tmp";
const CMD_FILE_PREFIX: &str = "monarch-cmd-";
const PRODUCTION_HELPER: &str = "/usr/lib/monarch-store/monarch-helper";
const CANCEL_FILE: &str = "/var/tmp/monarch-cancel";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", content = "payload")]
enum HelperCommand {
    ExecuteBatch { manifest: TransactionManifest },
    /// Legacy: older helpers accept these instead of ExecuteBatch.
    Refresh,
    InstallTargets { targets: Vec<String> },
    UninstallTargets { targets: Vec<String> },
    PrepareChaoticComponents,
    RefreshKeyring,
    AlpmInstallFiles { paths: Vec<String> },
    AlpmCleanCache { keep_versions: u32 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecoveryAction {
    UnlockDatabase,
    RepairKeyring,
    RefreshMirrors,
    ForceRefreshDb,
    CleanCache,
    Retry,
    Manual,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassifiedError {
    pub kind: String,
    pub title: String,
    pub description: String,
    pub recovery_action: Option<String>,
    pub raw_message: String,
}

#[derive(Debug, Clone)]
pub enum HelperProgress {
    Message {
        message: String,
        percent: Option<u8>,
    },
    ClassifiedError(ClassifiedError),
    Finished(Result<String, String>),
}

#[derive(Debug, Deserialize)]
struct HelperProgressLine {
    #[serde(default)]
    event_type: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    percent: Option<u8>,
}

#[derive(Debug, Clone)]
pub struct PrivilegedClient;

impl PrivilegedClient {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrivilegedClient {
    fn default() -> Self {
        Self::new()
    }
}

impl PrivilegedClient {
    pub async fn execute_manifest(&self, manifest: TransactionManifest) -> Result<String, String> {
        self.execute_manifest_with_password(manifest, None).await
    }

    pub async fn execute_manifest_with_password(
        &self,
        manifest: TransactionManifest,
        password: Option<String>,
    ) -> Result<String, String> {
        tokio::task::spawn_blocking(move || execute_manifest_blocking(manifest, password))
            .await
            .map_err(|e| e.to_string())?
    }

    pub async fn execute_manifest_stream(
        &self,
        manifest: TransactionManifest,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.execute_manifest_stream_with_password(manifest, None)
            .await
    }

    pub async fn execute_manifest_stream_with_password(
        &self,
        manifest: TransactionManifest,
        password: Option<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        let helper_bin = resolve_helper_bin()?;
        let command_path = write_command_file(&HelperCommand::ExecuteBatch {
            manifest: manifest.clone(),
        })?;
        spawn_helper_stream(
            helper_bin,
            command_path,
            password.clone(),
            Some((manifest, password)),
        )
        .await
    }

    pub async fn cancel_active_operation(&self) -> Result<(), String> {
        tokio::task::spawn_blocking(|| {
            std::fs::write(CANCEL_FILE, "1").map_err(|e| format!("Could not request cancel: {e}"))
        })
        .await
        .map_err(|e| e.to_string())??;

        tokio::time::sleep(std::time::Duration::from_millis(1500)).await;
        self.execute_manifest(TransactionManifest {
            remove_lock: true,
            ..Default::default()
        })
        .await
        .map(|_| ())
    }

    pub async fn prepare_chaotic_components(&self) -> Result<String, String> {
        self.prepare_chaotic_components_with_password(None).await
    }

    pub async fn prepare_chaotic_components_with_password(
        &self,
        password: Option<String>,
    ) -> Result<String, String> {
        self.execute_helper_command_with_password(HelperCommand::PrepareChaoticComponents, password)
            .await
    }

    pub async fn prepare_chaotic_components_stream(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.prepare_chaotic_components_stream_with_password(None)
            .await
    }

    pub async fn prepare_chaotic_components_stream_with_password(
        &self,
        password: Option<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.execute_helper_command_stream_with_password(
            HelperCommand::PrepareChaoticComponents,
            password,
        )
        .await
    }

    pub async fn refresh_keyring(&self) -> Result<String, String> {
        self.refresh_keyring_with_password(None).await
    }

    pub async fn refresh_keyring_with_password(
        &self,
        password: Option<String>,
    ) -> Result<String, String> {
        self.execute_helper_command_with_password(HelperCommand::RefreshKeyring, password)
            .await
    }

    pub async fn refresh_keyring_stream(
        &self,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.refresh_keyring_stream_with_password(None).await
    }

    pub async fn refresh_keyring_stream_with_password(
        &self,
        password: Option<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.execute_helper_command_stream_with_password(HelperCommand::RefreshKeyring, password)
            .await
    }

    pub async fn clear_cache_keep(&self, keep_versions: u32) -> Result<String, String> {
        self.clear_cache_keep_with_password(keep_versions, None)
            .await
    }

    pub async fn clear_cache_keep_with_password(
        &self,
        keep_versions: u32,
        password: Option<String>,
    ) -> Result<String, String> {
        self.execute_helper_command_with_password(
            HelperCommand::AlpmCleanCache { keep_versions },
            password,
        )
        .await
    }

    pub async fn alpm_install_files_stream(
        &self,
        paths: Vec<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.alpm_install_files_stream_with_password(paths, None)
            .await
    }

    pub async fn alpm_install_files_stream_with_password(
        &self,
        paths: Vec<String>,
        password: Option<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        self.execute_helper_command_stream_with_password(
            HelperCommand::AlpmInstallFiles { paths },
            password,
        )
        .await
    }

    async fn execute_helper_command_with_password(
        &self,
        command: HelperCommand,
        password: Option<String>,
    ) -> Result<String, String> {
        tokio::task::spawn_blocking(move || execute_command_blocking(command, password))
            .await
            .map_err(|e| e.to_string())?
    }

    async fn execute_helper_command_stream_with_password(
        &self,
        command: HelperCommand,
        password: Option<String>,
    ) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
        let helper_bin = resolve_helper_bin()?;
        let command_path = write_command_file(&command)?;
        spawn_helper_stream(helper_bin, command_path, password, None).await
    }
}

fn is_execute_batch_unknown_variant(e: &str) -> bool {
    e.contains("unknown variant") && e.contains("ExecuteBatch")
}

/// Run manifest as a sequence of legacy commands for helpers that don't support ExecuteBatch.
fn execute_manifest_legacy_blocking(
    manifest: TransactionManifest,
    password: Option<String>,
) -> Result<String, String> {
    if manifest.refresh_db {
        execute_command_blocking(HelperCommand::Refresh, password.clone())?;
    }
    if !manifest.remove_targets.is_empty() {
        execute_command_blocking(
            HelperCommand::UninstallTargets {
                targets: manifest.remove_targets,
            },
            password.clone(),
        )?;
    }
    if !manifest.install_targets.is_empty() {
        execute_command_blocking(
            HelperCommand::InstallTargets {
                targets: manifest.install_targets,
            },
            password,
        )?;
    }
    Ok("Legacy transaction completed.".to_string())
}

fn execute_manifest_blocking(
    manifest: TransactionManifest,
    password: Option<String>,
) -> Result<String, String> {
    match execute_command_blocking(HelperCommand::ExecuteBatch { manifest: manifest.clone() }, password.clone()) {
        Err(e) if is_execute_batch_unknown_variant(&e) => {
            execute_manifest_legacy_blocking(manifest, password)
        }
        other => other,
    }
}

fn execute_command_blocking(
    command: HelperCommand,
    password: Option<String>,
) -> Result<String, String> {
    let helper_bin = resolve_helper_bin()?;
    let command_path = write_command_file(&command)?;
    let output = run_helper_output(&helper_bin, &command_path, password)?;
    let _ = std::fs::remove_file(&command_path);

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
        if let Some(classified) = classify_error_text(&stderr) {
            Err(classified.description)
        } else if stderr.is_empty() {
            Err(format!(
                "Helper exited with status {}",
                output.status.code().unwrap_or_default()
            ))
        } else {
            Err(stderr)
        }
    }
}

async fn spawn_helper_stream(
    helper_bin: String,
    command_path: PathBuf,
    password: Option<String>,
    fallback: Option<(TransactionManifest, Option<String>)>,
) -> Result<tokio::sync::mpsc::Receiver<HelperProgress>, String> {
    let mut command = if password.is_some() {
        let mut command = tokio::process::Command::new("sudo");
        command
            .arg("-E")
            .arg("-S")
            .arg(&helper_bin)
            .arg(command_path.to_string_lossy().as_ref());
        command
    } else {
        let mut command = tokio::process::Command::new("pkexec");
        command
            .arg("--disable-internal-agent")
            .arg(&helper_bin)
            .arg(command_path.to_string_lossy().as_ref());
        command
    };

    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if password.is_some() {
                format!("Failed to spawn monarch-helper via sudo: {e}")
            } else {
                format!("Failed to spawn monarch-helper via pkexec: {e}")
            }
        })?;

    if let Some(password) = password {
        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(format!("{password}\n").as_bytes())
                .await
                .map_err(|e| format!("Failed to send session password to sudo: {e}"))?;
            let _ = stdin.shutdown().await;
        }
    }

    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let (tx, rx) = tokio::sync::mpsc::channel(128);

    if let Some(stdout) = stdout {
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let progress = parse_helper_line(&line);
                let _ = tx_stdout.send(progress).await;
            }
        });
    }

    if let Some(stderr) = stderr {
        let tx_stderr = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let text = line.trim().to_string();
                if !text.is_empty() {
                    if let Some(classified) = classify_error_text(&text) {
                        let _ = tx_stderr
                            .send(HelperProgress::ClassifiedError(classified))
                            .await;
                    } else {
                        let _ = tx_stderr
                            .send(HelperProgress::Message {
                                message: text,
                                percent: None,
                            })
                            .await;
                    }
                }
            }
        });
    }

    tokio::spawn(async move {
        let status = child.wait().await;
        let _ = std::fs::remove_file(&command_path);
        let mut result = match status {
            Ok(status) if status.success() => Ok("Helper transaction completed.".to_string()),
            Ok(status) => Err(format!(
                "Helper exited with status {}",
                status.code().unwrap_or_default()
            )),
            Err(error) => Err(format!("Failed to wait for helper: {error}")),
        };
        if result.is_err() && fallback.is_some() {
            let (manifest, password) = fallback.unwrap();
            match tokio::task::spawn_blocking(move || {
                execute_manifest_legacy_blocking(manifest, password)
            })
            .await
            {
                Ok(legacy_result) => result = legacy_result,
                Err(join_err) => result = Err(join_err.to_string()),
            }
        }
        let _ = tx.send(HelperProgress::Finished(result)).await;
    });

    Ok(rx)
}

fn run_helper_output(
    helper_bin: &str,
    command_path: &Path,
    password: Option<String>,
) -> Result<std::process::Output, String> {
    let mut command = if password.is_some() {
        let mut command = std::process::Command::new("sudo");
        command.arg("-E").arg("-S").arg(helper_bin);
        command
    } else {
        let mut command = std::process::Command::new("pkexec");
        command.arg("--disable-internal-agent").arg(helper_bin);
        command
    };
    command
        .arg(command_path.to_string_lossy().as_ref())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command.spawn().map_err(|e| {
        if password.is_some() {
            format!("Failed to spawn monarch-helper via sudo: {e}")
        } else {
            format!("Failed to spawn monarch-helper via pkexec: {e}")
        }
    })?;

    if let Some(password) = password {
        if let Some(mut stdin) = child.stdin.take() {
            stdin
                .write_all(format!("{password}\n").as_bytes())
                .map_err(|e| format!("Failed to send session password to sudo: {e}"))?;
        }
    }

    child
        .wait_with_output()
        .map_err(|e| format!("Failed to read helper output: {e}"))
}

fn resolve_helper_bin() -> Result<String, String> {
    if Path::new(PRODUCTION_HELPER).exists() {
        return Ok(PRODUCTION_HELPER.to_string());
    }

    if let Ok(path) = std::env::var("MONARCH_HELPER_PATH") {
        if Path::new(&path).exists() {
            return Ok(path);
        }
    }

    Err(format!(
        "monarch-helper not found at {}. Install the helper or set MONARCH_HELPER_PATH.",
        PRODUCTION_HELPER
    ))
}

fn write_command_file(command: &HelperCommand) -> Result<PathBuf, String> {
    std::fs::create_dir_all(CMD_FILE_DIR)
        .map_err(|e| format!("Failed to create {CMD_FILE_DIR}: {e}"))?;

    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = Path::new(CMD_FILE_DIR).join(format!("{CMD_FILE_PREFIX}{stamp}.json"));
    let json = serde_json::to_string(command).map_err(|e| e.to_string())?;

    let mut file = std::fs::File::create(&path)
        .map_err(|e| format!("Failed to create command file {}: {e}", path.display()))?;
    file.write_all(json.as_bytes())
        .map_err(|e| format!("Failed to write command file: {e}"))?;
    file.sync_all()
        .map_err(|e| format!("Failed to sync command file: {e}"))?;

    Ok(path)
}

fn parse_helper_line(line: &str) -> HelperProgress {
    if let Ok(event) = serde_json::from_str::<HelperProgressLine>(line) {
        if event.event_type == "error" {
            if let Some(classified) = classify_error_text(&event.message) {
                return HelperProgress::ClassifiedError(classified);
            }
        }
        if !event.message.trim().is_empty() {
            return HelperProgress::Message {
                message: event.message,
                percent: event.percent,
            };
        }
    }
    HelperProgress::Message {
        message: line.trim().to_string(),
        percent: None,
    }
}

fn classify_error_text(text: &str) -> Option<ClassifiedError> {
    serde_json::from_str::<ClassifiedError>(text).ok().or_else(|| {
        let lower = text.to_lowercase();
        if lower.contains("database is locked") || lower.contains("db.lck") {
            Some(ClassifiedError {
                kind: "DatabaseLocked".to_string(),
                title: "Database Locked".to_string(),
                description:
                    "Another package manager is running or a previous operation was interrupted."
                        .to_string(),
                recovery_action: Some("UnlockDatabase".to_string()),
                raw_message: text.to_string(),
            })
        } else if lower.contains("pgp signature")
            || lower.contains("gpgme error")
            || lower.contains("unknown public key")
        {
            Some(ClassifiedError {
                kind: "KeyringError".to_string(),
                title: "Security Key Issue".to_string(),
                description:
                    "Package signatures could not be verified. Refresh the system keyrings and retry."
                        .to_string(),
                recovery_action: Some("RepairKeyring".to_string()),
                raw_message: text.to_string(),
            })
        } else if lower.contains("failed retrieving file")
            || lower.contains("failed to synchronize")
            || lower.contains("404")
        {
            Some(ClassifiedError {
                kind: "MirrorFailure".to_string(),
                title: "Download Failed".to_string(),
                description:
                    "Could not download package data. Refresh databases and try again."
                        .to_string(),
                recovery_action: Some("ForceRefreshDb".to_string()),
                raw_message: text.to_string(),
            })
        } else if lower.contains("no space left on device") {
            Some(ClassifiedError {
                kind: "DiskFull".to_string(),
                title: "Disk Full".to_string(),
                description: "Clear the package cache to free disk space and retry.".to_string(),
                recovery_action: Some("CleanCache".to_string()),
                raw_message: text.to_string(),
            })
        } else {
            None
        }
    })
}
