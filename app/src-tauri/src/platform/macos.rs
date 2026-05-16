use crate::config::AppConfig;
use std::fs;
use std::path::{Path, PathBuf};
use anyhow::{Context, Result};
use tokio::process::Command;
use std::process::Stdio;

pub async fn open_downloads_folder(config: &AppConfig) -> anyhow::Result<()> {
    let path = config.effective_download_dir().await;
    std::process::Command::new("open").arg(&path).spawn()?;
    Ok(())
}

pub async fn terminate_process(pid: u32) {
    if let Ok(mut child) = Command::new("kill")
        .arg("-TERM")
        .arg(pid.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
        let _ = child.wait().await;
    }
}

pub fn executable_alias(name: &str) -> String {
    name.to_string()
}

pub fn link_or_copy_sidecar(source: &Path, destination: &Path) -> Result<()> {
    use std::os::unix::fs as unix_fs;

    if destination.exists() {
        fs::remove_file(destination).with_context(|| {
            format!("failed to replace sidecar alias {}", destination.display())
        })?;
    }

    unix_fs::symlink(source, destination)
        .or_else(|_| {
            fs::copy(source, destination)
                .map(|_| ())
                .with_context(|| {
                    format!(
                        "failed to copy sidecar {} to {}",
                        source.display(),
                        destination.display()
                    )
                })
        })
        .with_context(|| {
            format!(
                "failed to prepare sidecar alias {} -> {}",
                destination.display(),
                source.display()
            )
        })
}

pub fn sidecar_candidates(name: &str) -> Vec<PathBuf> {
    let names = vec![format!("{name}-mac")];
    let mut candidates = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let resource_dir = parent.join("resources");
            for n in &names {
                candidates.push(resource_dir.join(n));
            }
        }
    }

    for n in &names {
        candidates.push(Path::new("resources").join(n));
        candidates.push(Path::new("app/src-tauri/resources").join(n));
    }

    candidates
}

pub fn reveal_in_folder(path: &Path) -> std::io::Result<std::process::ExitStatus> {
    std::process::Command::new("open")
        .arg("-R")
        .arg(path)
        .status()
}
