use crate::models::{Package, PackageSource, UpdateItem};
use crate::privileged::{ClassifiedError, HelperProgress};
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

const FLATHUB_REPO_URL: &str = "https://dl.flathub.org/repo/flathub.flatpakrepo";
const FLATHUB_BETA_REPO_URL: &str = "https://flathub.org/beta-repo/flathub-beta.flatpakrepo";

/// Parses a size string from flatpak remote-info (e.g. "3,4 MB" or "11.1 MB") into bytes.
fn parse_flatpak_size(s: &str) -> Option<u64> {
    let parts: Vec<&str> = s.trim().split_whitespace().collect();
    let (num_str, unit) = match parts.as_slice() {
        [num, unit] => ((*num).replace(',', "."), *unit),
        _ => return None,
    };
    let value: f64 = num_str.parse().ok()?;
    let factor = match unit.to_uppercase().as_str() {
        "B" => 1u64,
        "KB" | "K" => 1000,
        "MB" | "M" => 1_000_000,
        "GB" | "G" => 1_000_000_000,
        _ => return None,
    };
    Some((value * factor as f64) as u64)
}

/// Fetches download and installed size for a ref from a remote (e.g. flathub, app_id).
/// Returns (download_size_bytes, installed_size_bytes). Runs `flatpak remote-info <remote> <ref>`.
pub async fn remote_info_sizes(
    remote: &str,
    ref_or_id: &str,
) -> Result<(Option<u64>, Option<u64>), String> {
    let output = Command::new("flatpak")
        .args(["remote-info", remote, ref_or_id])
        .output()
        .await
        .map_err(|e| format!("flatpak remote-info: {e}"))?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut download = None;
    let mut installed = None;
    for line in stdout.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("Download size:") {
            download = parse_flatpak_size(rest);
        } else if let Some(rest) = line.strip_prefix("Installed size:") {
            installed = parse_flatpak_size(rest);
        }
    }
    Ok((download, installed))
}

pub fn is_flatpak_available() -> bool {
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).any(|path| path.join("flatpak").exists()))
        .unwrap_or(false)
}

pub async fn get_installed_packages() -> Result<Vec<Package>, String> {
    let output = Command::new("flatpak")
        .args([
            "list",
            "--app",
            "--columns=application,name,version,description,origin",
        ])
        .output()
        .await
        .map_err(|e| format!("Failed to run flatpak list: {e}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    let mut packages = Vec::new();
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let parts = line.split('\t').map(str::trim).collect::<Vec<_>>();
        if parts.len() < 5 {
            continue;
        }

        let app_id = parts[0].to_string();
        let display_name = non_empty(parts[1]);
        let version = parts[2].to_string();
        let description = non_empty(parts[3]).unwrap_or_else(|| "Flatpak application".to_string());
        let origin = non_empty(parts[4]).unwrap_or_else(|| "flathub".to_string());
        let label = if origin == "flathub-beta" {
            "Flatpak (Beta)"
        } else {
            "Flatpak (Sandboxed)"
        };

        packages.push(Package {
            name: app_id.clone(),
            display_name,
            display_title: None,
            description,
            version: version.clone(),
            source: PackageSource {
                source_type: "flatpak".to_string(),
                id: origin.clone(),
                version: version.clone(),
                label: label.to_string(),
                package_name: Some(app_id.clone()),
            },
            app_id: Some(app_id.clone()),
            canonical_id: app_id.clone(),
            installed: true,
            available_sources: Some(vec![PackageSource {
                source_type: "flatpak".to_string(),
                id: origin,
                version,
                label: label.to_string(),
                package_name: Some(app_id.clone()),
            }]),
            ..Package::default()
        });
    }

    Ok(packages)
}

pub async fn install_app(
    tx: Sender<HelperProgress>,
    app_id: String,
    remote: Option<String>,
) -> Result<String, String> {
    let remote = remote.unwrap_or_else(|| "flathub".to_string());
    ensure_remote(&tx, &remote).await?;
    run_flatpak_command(
        tx,
        vec!["install".to_string(), remote, app_id.clone(), "-y".to_string()],
        format!("Installing Flatpak app {app_id}..."),
    )
    .await?;
    Ok(format!("Flatpak app {app_id} installed."))
}

pub async fn remove_app(tx: Sender<HelperProgress>, app_id: String) -> Result<String, String> {
    run_flatpak_command(
        tx,
        vec!["uninstall".to_string(), app_id.clone(), "-y".to_string()],
        format!("Removing Flatpak app {app_id}..."),
    )
    .await?;
    Ok(format!("Flatpak app {app_id} removed."))
}

pub async fn update_app(tx: Sender<HelperProgress>, app_id: String) -> Result<String, String> {
    run_flatpak_command(
        tx,
        vec!["update".to_string(), app_id.clone(), "-y".to_string()],
        format!("Updating Flatpak app {app_id}..."),
    )
    .await?;
    Ok(format!("Flatpak app {app_id} updated."))
}

pub async fn ensure_flathub_ready() -> Result<String, String> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(8);
    tokio::spawn(async move {
        let result = ensure_remote(&tx, "flathub").await;
        let _ = tx
            .send(HelperProgress::Finished(
                result.map(|_| "Flathub remote is ready.".to_string()),
            ))
            .await;
    });

    while let Some(event) = rx.recv().await {
        if let HelperProgress::Finished(result) = event {
            return result;
        }
    }

    Err("Flathub preparation did not complete.".to_string())
}

pub async fn update_many(
    tx: Sender<HelperProgress>,
    updates: Vec<UpdateItem>,
) -> Result<String, String> {
    if updates.is_empty() {
        let _ = tx
            .send(HelperProgress::Message {
                message: "No Flatpak updates found.".to_string(),
                percent: Some(100),
            })
            .await;
        return Ok("No Flatpak updates found.".to_string());
    }

    let total = updates.len().max(1);
    let mut failed: Vec<(String, String)> = Vec::new();
    for (index, update) in updates.into_iter().enumerate() {
        let percent = ((index * 100) / total) as u8;
        let _ = tx
            .send(HelperProgress::Message {
                message: format!("Updating Flatpak {}...", update.name),
                percent: Some(percent),
            })
            .await;
        match update_app(tx.clone(), update.name.clone()).await {
            Ok(_) => {}
            Err(e) => {
                let _ = tx
                    .send(HelperProgress::Message {
                        message: format!("Flatpak update failed: {} — {}", update.name, e),
                        percent: None,
                    })
                    .await;
                failed.push((update.name, e));
            }
        }
    }

    if failed.is_empty() {
        Ok("Flatpak updates completed.".to_string())
    } else {
        let names: Vec<_> = failed.iter().map(|(n, _)| n.as_str()).collect();
        Ok(format!(
            "Flatpak: {} update(s) failed: {}.",
            failed.len(),
            names.join(", ")
        ))
    }
}

async fn ensure_remote(tx: &Sender<HelperProgress>, remote: &str) -> Result<(), String> {
    let repo_url = if remote == "flathub-beta" {
        FLATHUB_BETA_REPO_URL
    } else {
        FLATHUB_REPO_URL
    };

    run_flatpak_command(
        tx.clone(),
        vec![
            "remote-add".to_string(),
            "--if-not-exists".to_string(),
            remote.to_string(),
            repo_url.to_string(),
        ],
        format!("Ensuring Flatpak remote {remote} is configured..."),
    )
    .await
}

async fn run_flatpak_command(
    tx: Sender<HelperProgress>,
    args: Vec<String>,
    headline: String,
) -> Result<(), String> {
    let _ = tx
        .send(HelperProgress::Message {
            message: headline,
            percent: None,
        })
        .await;

    let mut child = Command::new("flatpak")
        .args(args.iter().map(String::as_str))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn flatpak: {e}"))?;

    let mut stderr_log = String::new();

    if let Some(stdout) = child.stdout.take() {
        let tx_stdout = tx.clone();
        tokio::spawn(async move {
            let mut lines = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                let line = line.trim().to_string();
                if !line.is_empty() {
                    let _ = tx_stdout
                        .send(HelperProgress::Message {
                            message: line,
                            percent: None,
                        })
                        .await;
                }
            }
        });
    }

    if let Some(stderr) = child.stderr.take() {
        let tx_stderr = tx.clone();
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            stderr_log.push_str(&line);
            stderr_log.push('\n');
            if let Some(classified) = classify_flatpak_error(&line) {
                let _ = tx_stderr.send(HelperProgress::ClassifiedError(classified)).await;
            } else {
                let _ = tx_stderr
                    .send(HelperProgress::Message {
                        message: line,
                        percent: None,
                    })
                    .await;
            }
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err(stderr_log.trim().to_string().if_empty_or_else(|| {
            format!("Flatpak exited with status {}", status.code().unwrap_or_default())
        }))
    }
}

fn classify_flatpak_error(text: &str) -> Option<ClassifiedError> {
    let lower = text.to_lowercase();
    if lower.contains("nothing matches") || lower.contains("no remote refs found") {
        Some(ClassifiedError {
            kind: "FlatpakNotFound".to_string(),
            title: "Flatpak App Not Found".to_string(),
            description:
                "The Flatpak app could not be found on the configured remote.".to_string(),
            recovery_action: Some("Manual".to_string()),
            raw_message: text.to_string(),
        })
    } else if lower.contains("refusing to operate on") || lower.contains("similar ref is already installed")
    {
        Some(ClassifiedError {
            kind: "FlatpakReinstall".to_string(),
            title: "Flatpak Installation Needs Repair".to_string(),
            description:
                "The Flatpak installation appears to be partial or inconsistent. Reinstalling it may fix the issue."
                    .to_string(),
            recovery_action: Some("Retry".to_string()),
            raw_message: text.to_string(),
        })
    } else if lower.contains("remote") && lower.contains("not found") {
        Some(ClassifiedError {
            kind: "FlatpakRemoteMissing".to_string(),
            title: "Flatpak Remote Not Configured".to_string(),
            description: "The requested Flatpak remote is not configured.".to_string(),
            recovery_action: Some("Retry".to_string()),
            raw_message: text.to_string(),
        })
    } else {
        None
    }
}

trait IfEmptyOrElse {
    fn if_empty_or_else<F: FnOnce() -> String>(self, fallback: F) -> String;
}

impl IfEmptyOrElse for String {
    fn if_empty_or_else<F: FnOnce() -> String>(self, fallback: F) -> String {
        if self.trim().is_empty() {
            fallback()
        } else {
            self
        }
    }
}

fn non_empty(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
