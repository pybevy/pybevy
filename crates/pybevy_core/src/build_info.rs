//! Interpreter-neutral build identity reported by every PyBevy surface.
//!
//! The values are compiled into the binary's entry point, not this crate, so a
//! new commit does not rebuild it or its dependents. The `pybevy_python` cdylib
//! installs them before `pybevy::init_module`; an embedder that links the
//! `pybevy` crate into its own binary may call [`install_build_info`] with its
//! own values, and otherwise reports [`BuildInfo::UNKNOWN`] with the correct
//! `version`.

use std::sync::OnceLock;

use crate::{BEVY_VERSION, PYBEVY_VERSION};

/// Placeholder for identity that the build could not determine.
pub const UNKNOWN: &str = "unknown";

/// Identity of the compiled PyBevy binary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuildInfo {
    /// Full 40-character commit hash, or "unknown".
    pub git_sha: &'static str,
    /// Abbreviated commit hash, or "unknown".
    pub git_sha_short: &'static str,
    /// Whether tracked Rust sources or manifests differed from `git_sha`.
    pub git_dirty: bool,
    /// Cargo profile the binary was compiled with ("debug" or "release").
    pub profile: &'static str,
}

impl BuildInfo {
    /// Identity used before an adapter installs the compiled values.
    pub const UNKNOWN: Self = Self {
        git_sha: UNKNOWN,
        git_sha_short: UNKNOWN,
        git_dirty: false,
        profile: UNKNOWN,
    };

    /// JSON object shared by the control API and interpreter adapters.
    pub fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "version": PYBEVY_VERSION,
            "bevy_version": BEVY_VERSION,
            "git_sha": self.git_sha,
            "git_sha_short": self.git_sha_short,
            "git_dirty": self.git_dirty,
            "profile": self.profile,
        })
    }
}

static INSTALLED: OnceLock<BuildInfo> = OnceLock::new();

/// Record the binary's identity; later calls keep the first value.
///
/// Call it before Python imports `pybevy`, which reads the value once into
/// `pybevy.BUILD_INFO`.
pub fn install_build_info(info: BuildInfo) {
    let _ = INSTALLED.set(info);
}

/// The installed identity, or [`BuildInfo::UNKNOWN`] before installation.
pub fn build_info() -> BuildInfo {
    INSTALLED.get().copied().unwrap_or(BuildInfo::UNKNOWN)
}

#[cfg(test)]
mod tests {
    use super::{BEVY_VERSION, BuildInfo, PYBEVY_VERSION, UNKNOWN};

    #[test]
    fn json_carries_every_field_and_the_version() {
        let info = BuildInfo {
            git_sha: "0123456789abcdef0123456789abcdef01234567",
            git_sha_short: "0123456789",
            git_dirty: true,
            profile: "release",
        };
        let json = info.to_json();
        assert_eq!(json["version"], PYBEVY_VERSION);
        assert_eq!(json["bevy_version"], BEVY_VERSION);
        assert_eq!(json["git_sha"], info.git_sha);
        assert_eq!(json["git_sha_short"], info.git_sha_short);
        assert_eq!(json["git_dirty"], true);
        assert_eq!(json["profile"], "release");
    }

    #[test]
    fn unknown_identity_is_clean() {
        let json = BuildInfo::UNKNOWN.to_json();
        assert_eq!(json["git_sha"], UNKNOWN);
        assert_eq!(json["git_dirty"], false);
    }
}
