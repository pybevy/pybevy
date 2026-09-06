//! Cache for Bevy API parsing results.
//!
//! Caches `cargo public-api` output to avoid re-parsing unchanged Bevy versions.
//! The key combines the Bevy checkout's git commit with the feature set the
//! parse asked for, so widening the features does not reuse a narrower parse.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use anyhow::{Context, Result};

use super::types::BevyCrate;

/// Get the git commit hash for a Bevy checkout.
pub fn get_bevy_git_ref(bevy_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(bevy_path)
        .output()
        .with_context(|| format!("Failed to run git rev-parse in {}", bevy_path.display()))?;

    if !output.status.success() {
        anyhow::bail!(
            "git rev-parse failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(hash)
}

/// Get the cache directory path.
fn cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|d| d.join("pybevy-lint"))
}

/// Get the cache file path within a specific directory.
fn cache_path_in_dir(dir: PathBuf, git_ref: &str, crate_name: &str) -> Option<PathBuf> {
    // Use first 12 chars of commit hash for readability
    let short_ref = &git_ref[..git_ref.len().min(12)];
    let features = super::parser::feature_key(crate_name);
    Some(dir.join(format!("{}_{}_{}.json", short_ref, crate_name, features)))
}

/// Try to load a cached BevyCrate.
pub fn load_cached(git_ref: &str, crate_name: &str) -> Option<BevyCrate> {
    load_cached_from_dir(cache_dir()?, git_ref, crate_name)
}

/// Try to load a cached BevyCrate from a specific directory.
fn load_cached_from_dir(dir: PathBuf, git_ref: &str, crate_name: &str) -> Option<BevyCrate> {
    let path = cache_path_in_dir(dir, git_ref, crate_name)?;

    if !path.exists() {
        return None;
    }

    let content = fs::read_to_string(&path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Save a BevyCrate to the cache.
pub fn save_to_cache(git_ref: &str, crate_name: &str, crate_data: &BevyCrate) -> Result<()> {
    let dir = match cache_dir() {
        Some(d) => d,
        None => return Ok(()), // No cache dir available, skip silently
    };
    save_to_cache_in_dir(&dir, git_ref, crate_name, crate_data)
}

/// Save a BevyCrate to a specific cache directory.
fn save_to_cache_in_dir(
    dir: &Path,
    git_ref: &str,
    crate_name: &str,
    crate_data: &BevyCrate,
) -> Result<()> {
    let path = match cache_path_in_dir(dir.to_path_buf(), git_ref, crate_name) {
        Some(p) => p,
        None => return Ok(()),
    };

    // Ensure cache directory exists
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create cache directory: {}", parent.display()))?;
    }

    let content = serde_json::to_string_pretty(crate_data)
        .with_context(|| format!("Failed to serialize cache for {}", crate_name))?;

    fs::write(&path, content)
        .with_context(|| format!("Failed to write cache file: {}", path.display()))?;

    Ok(())
}
