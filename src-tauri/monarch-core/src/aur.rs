use crate::privileged::{HelperProgress, PrivilegedClient};
use futures::future::BoxFuture;
use futures::FutureExt;
use raur::Raur;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;
use tokio::sync::mpsc::Sender;

const MAX_AUR_DEPTH: u32 = 64;
const SHARED_INSTALL_DIR: &str = "/tmp/monarch-install";

pub async fn install_package(
    privileged: Arc<PrivilegedClient>,
    tx: Sender<HelperProgress>,
    package_name: String,
    password: Option<String>,
) -> Result<String, String> {
    audit_builder_deps()?;

    let mut resolved = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack = std::collections::HashSet::new();
    resolve_aur_dependencies(
        tx.clone(),
        package_name.as_str(),
        &mut resolved,
        &mut visited,
        &mut stack,
        0,
    )
    .await?;

    let total = resolved.len().max(1);
    let mut built_paths = Vec::new();
    for (index, pkg_name) in resolved.into_iter().enumerate() {
        let percent = ((index * 65) / total) as u8;
        let _ = tx
            .send(HelperProgress::Message {
                message: format!("Building AUR package {}...", pkg_name),
                percent: Some(percent),
            })
            .await;
        let mut paths = build_aur_package_single(tx.clone(), &pkg_name).await?;
        built_paths.append(&mut paths);
    }

    let _ = tx
        .send(HelperProgress::Message {
            message: "Handing built AUR packages to monarch-helper...".to_string(),
            percent: Some(75),
        })
        .await;

    let helper_rx = privileged
        .alpm_install_files_stream_with_password(built_paths, password)
        .await?;
    forward_helper_stream(tx, helper_rx).await?;
    Ok(format!("AUR package {} built and installed.", package_name))
}

fn resolve_aur_dependencies<'a>(
    tx: Sender<HelperProgress>,
    name: &'a str,
    resolved: &'a mut Vec<String>,
    visited: &'a mut std::collections::HashSet<String>,
    stack: &'a mut std::collections::HashSet<String>,
    depth: u32,
) -> BoxFuture<'a, Result<(), String>> {
    async move {
        if depth > MAX_AUR_DEPTH {
            return Err("AUR dependency depth exceeded. Aborting build.".to_string());
        }
        if stack.contains(name) {
            return Err(format!("Cycle detected in AUR dependencies involving '{name}'."));
        }
        if visited.contains(name) {
            return Ok(());
        }

        visited.insert(name.to_string());
        stack.insert(name.to_string());
        let _ = tx
            .send(HelperProgress::Message {
                message: format!("Checking AUR dependencies for {}...", name),
                percent: None,
            })
            .await;

        let handle = raur::Handle::new();
        let packages = handle.info(&[name]).await.map_err(|e| e.to_string())?;
        let package = packages
            .first()
            .ok_or_else(|| format!("Package {name} not found in AUR."))?;

        let mut dependencies: Vec<String> = Vec::new();
        dependencies.extend(package.depends.clone());
        dependencies.extend(package.make_depends.clone());

        for dependency in dependencies {
            let dep_name = dependency
                .split(['=', '>', '<'])
                .next()
                .unwrap_or(dependency.as_str())
                .trim()
                .to_string();

            if dep_name.is_empty()
                || is_package_installed(&dep_name).await
                || is_in_sync_repos(&dep_name).await
            {
                continue;
            }

            resolve_aur_dependencies(
                tx.clone(),
                dep_name.as_str(),
                resolved,
                visited,
                stack,
                depth + 1,
            )
            .await?;
        }

        stack.remove(name);
        if !resolved.iter().any(|item| item == name) {
            resolved.push(name.to_string());
        }
        Ok(())
    }
    .boxed()
}

async fn is_package_installed(name: &str) -> bool {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        std::process::Command::new("pacman")
            .args(["-Qq", &name])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

async fn is_in_sync_repos(name: &str) -> bool {
    let name = name.to_string();
    tokio::task::spawn_blocking(move || {
        std::process::Command::new("pacman")
            .args(["-Si", &name])
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

fn audit_builder_deps() -> Result<(), String> {
    for binary in ["git", "makepkg"] {
        let available = std::env::var_os("PATH")
            .map(|paths| std::env::split_paths(&paths).any(|path| path.join(binary).exists()))
            .unwrap_or(false);
        if !available {
            return Err(format!(
                "Required AUR build tool '{}' is missing. Install base-devel and git first.",
                binary
            ));
        }
    }
    Ok(())
}

async fn build_aur_package_single(
    tx: Sender<HelperProgress>,
    name: &str,
) -> Result<Vec<String>, String> {
    let temp_dir = tempfile::tempdir().map_err(|e| e.to_string())?;
    let clone_status = Command::new("git")
        .args([
            "clone",
            "--depth",
            "1",
            &format!("https://aur.archlinux.org/{}.git", name),
        ])
        .current_dir(temp_dir.path())
        .status()
        .await
        .map_err(|e| format!("Failed to clone AUR package {name}: {e}"))?;

    if !clone_status.success() {
        return Err(format!("Failed to clone {name} from AUR."));
    }

    let package_dir = temp_dir.path().join(name);
    let mut child = Command::new("makepkg")
        .args(["-s", "-r", "--noconfirm", "--needed"])
        .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
        .env("PKGEXT", ".pkg.tar.zst")
        .env("PACMAN", "pkexec pacman")
        .current_dir(&package_dir)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to run makepkg for {name}: {e}"))?;

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

    let mut stderr_log = String::new();
    let mut missing_key_ids: Vec<String> = Vec::new();
    if let Some(stderr) = child.stderr.take() {
        let mut lines = BufReader::new(stderr).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim().to_string();
            if line.is_empty() {
                continue;
            }
            stderr_log.push_str(&line);
            stderr_log.push('\n');
            if line.contains("unknown public key")
                || line.contains("not found in keychain")
                || line.contains("FAILED (unknown public key")
                || line.contains("could not be verified")
            {
                let words: Vec<&str> = line.split_whitespace().collect();
                for word in &words {
                    let clean = word
                        .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                        .to_string();
                    if clean.len() >= 8
                        && clean.chars().all(|c| c.is_ascii_hexdigit())
                        && !missing_key_ids.contains(&clean)
                    {
                        missing_key_ids.push(clean);
                    }
                }
                for (i, word) in words.iter().enumerate() {
                    if *word == "key" || word.ends_with("key") {
                        if let Some(next) = words.get(i + 1) {
                            let clean = next
                                .trim_matches(|c: char| !c.is_ascii_alphanumeric())
                                .to_string();
                            if clean.len() >= 8
                                && clean.chars().all(|c| c.is_ascii_hexdigit())
                                && !missing_key_ids.contains(&clean)
                            {
                                missing_key_ids.push(clean);
                            }
                        }
                    }
                }
            }
            let _ = tx
                .send(HelperProgress::Message {
                    message: line,
                    percent: None,
                })
                .await;
        }
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() && !missing_key_ids.is_empty() {
        let _ = tx
            .send(HelperProgress::Message {
                message: "--- PGP KEY RECOVERY ---".to_string(),
                percent: None,
            })
            .await;
        let _ = tx
            .send(HelperProgress::Message {
                message: format!(
                    "Detected {} missing PGP key(s). Attempting automatic import...",
                    missing_key_ids.len()
                ),
                percent: None,
            })
            .await;

        let keyservers = ["keyserver.ubuntu.com", "keys.openpgp.org", "pgp.mit.edu"];
        let mut imported_any = false;
        for key_id in &missing_key_ids {
            let _ = tx
                .send(HelperProgress::Message {
                    message: format!("Importing key: {}...", key_id),
                    percent: None,
                })
                .await;
            let mut key_imported = false;
            for server in keyservers {
                let output = Command::new("gpg")
                    .args(["--keyserver", server, "--recv-keys", key_id])
                    .output()
                    .await
                    .ok();
                if let Some(out) = output {
                    if out.status.success() {
                        let _ = tx
                            .send(HelperProgress::Message {
                                message: format!("✓ Key {} imported from {}", key_id, server),
                                percent: None,
                            })
                            .await;
                        key_imported = true;
                        imported_any = true;
                        break;
                    }
                }
            }
            if !key_imported {
                let _ = tx
                    .send(HelperProgress::Message {
                        message: format!(
                            "⚠ Could not import key {} from any keyserver",
                            key_id
                        ),
                        percent: None,
                    })
                    .await;
            }
        }

        if imported_any {
            let _ = tx
                .send(HelperProgress::Message {
                    message: "--- RETRYING BUILD WITH IMPORTED KEYS ---".to_string(),
                    percent: None,
                })
                .await;
            let rm_status = Command::new("rm")
                .args(["-rf", "src", "pkg"])
                .current_dir(&package_dir)
                .status()
                .await
                .map_err(|e| e.to_string())?;
            if !rm_status.success() {
                return Err("Failed to clean build dir before retry.".to_string());
            }
            let mut retry_child = Command::new("makepkg")
                .args(["-s", "-r", "--noconfirm", "--needed"])
                .env("MAKEFLAGS", format!("-j{}", num_cpus::get()))
                .env("PKGEXT", ".pkg.tar.zst")
                .env("PACMAN", "pkexec pacman")
                .current_dir(&package_dir)
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
                .map_err(|e| format!("Failed to retry makepkg for {name}: {e}"))?;
            if let Some(stdout) = retry_child.stdout.take() {
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
            if let Some(stderr) = retry_child.stderr.take() {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let line = line.trim().to_string();
                    if !line.is_empty() {
                        let _ = tx
                            .send(HelperProgress::Message {
                                message: line,
                                percent: None,
                            })
                            .await;
                    }
                }
            }
            let retry_status = retry_child.wait().await.map_err(|e| e.to_string())?;
            if retry_status.success() {
                let _ = tx
                    .send(HelperProgress::Message {
                        message: "✓ Build succeeded after key import!".to_string(),
                        percent: None,
                    })
                    .await;
                return copy_built_packages(&package_dir);
            }
            return Err(
                "Build failed after PGP key import. Check the log for makepkg errors.".to_string(),
            );
        }
        return Err(format!(
            "PGP verification failed. Could not import required keys: {}. You may need to import them manually (e.g. gpg --recv-keys KEYID).",
            missing_key_ids.join(", ")
        ));
    }
    if !status.success() {
        return Err(stderr_log.trim().to_string().if_empty_or_else(|| {
            format!("makepkg failed for {name}")
        }));
    }

    copy_built_packages(&package_dir)
}

fn copy_built_packages(package_dir: &Path) -> Result<Vec<String>, String> {
    std::fs::create_dir_all(SHARED_INSTALL_DIR).map_err(|e| e.to_string())?;
    let mut copied = Vec::new();
    for entry in std::fs::read_dir(package_dir).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let file_name = entry.file_name().to_string_lossy().to_string();
        if !path.is_file() || !file_name.contains(".pkg.tar.zst") || file_name.ends_with(".sig") {
            continue;
        }

        let destination = unique_install_path(&file_name);
        std::fs::copy(&path, &destination).map_err(|e| {
            format!(
                "Failed to copy built package {} to shared install dir: {}",
                path.display(),
                e
            )
        })?;
        copied.push(destination.to_string_lossy().to_string());
    }

    if copied.is_empty() {
        Err("makepkg completed but no .pkg.tar.zst artifacts were found.".to_string())
    } else {
        Ok(copied)
    }
}

fn unique_install_path(file_name: &str) -> PathBuf {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    Path::new(SHARED_INSTALL_DIR).join(format!("{stamp}-{file_name}"))
}

async fn forward_helper_stream(
    tx: Sender<HelperProgress>,
    mut rx: tokio::sync::mpsc::Receiver<HelperProgress>,
) -> Result<String, String> {
    while let Some(event) = rx.recv().await {
        match event {
            HelperProgress::Finished(result) => return result,
            other => {
                let _ = tx.send(other).await;
            }
        }
    }
    Err("AUR install stream ended unexpectedly.".to_string())
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
