//! Thread-safe application configuration.
//!
//! Replaces the deprecated `std::env::set_var` / `remove_var` pattern with an
//! `Arc<RwLock>` wrapper that can be shared safely across threads and async tasks.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::RwLock;

/// Shared, cloneable application configuration handle.
#[derive(Clone)]
pub struct AppConfig {
    inner: Arc<RwLock<ConfigInner>>,
}

#[derive(Debug)]
struct ConfigInner {
    /// User-configured download directory, if any.
    download_dir: Option<String>,
}

impl AppConfig {
    /// Create a new configuration with an optional initial download directory.
    pub fn new(download_dir: Option<String>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(ConfigInner { download_dir })),
        }
    }

    /// Get the currently configured download directory, if set.
    pub async fn download_dir(&self) -> Option<String> {
        self.inner.read().await.download_dir.clone()
    }

    /// Set the download directory.
    pub async fn set_download_dir(&self, dir: Option<String>) {
        self.inner.write().await.download_dir = dir;
    }

    /// Resolve the effective download directory, falling back to system defaults.
    pub async fn effective_download_dir(&self) -> PathBuf {
        if let Some(dir) = self.download_dir().await {
            return PathBuf::from(dir);
        }
        default_download_dir()
    }
}

/// Determine the system default download directory (no config override).
pub fn default_download_dir() -> PathBuf {
    if let Ok(home) = std::env::var("HOME") {
        return PathBuf::from(home).join("Downloads");
    }
    if let Ok(profile) = std::env::var("USERPROFILE") {
        return PathBuf::from(profile).join("Downloads");
    }
    PathBuf::from(".")
}
