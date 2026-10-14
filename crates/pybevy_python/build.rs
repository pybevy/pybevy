//! Embeds the git commit and dirty state of the PyBevy checkout being compiled.
//!
//! It runs for the small cdylib crate, so a new commit recompiles only this
//! crate and relinks; the root `pybevy` crate does not depend on the values.
//!
//! Sources, in order: the enclosing PyBevy git checkout (this crate lives at
//! `crates/pybevy_python`, so the checkout root is two levels up), then the
//! `.cargo_vcs_info.json` that `cargo package` writes for crates.io builds, then
//! "unknown". No other directory is searched, so a vendored copy inside an
//! unrelated repository never reports that repository's commit.

use std::{
    env, fs,
    path::{Path, PathBuf},
    process::Command,
};

const UNKNOWN: &str = "unknown";

/// Paths whose tracked changes make the compiled binary differ from its commit.
const COMPILED_INPUTS: [&str; 4] = ["src", "crates", "Cargo.toml", "Cargo.lock"];

struct GitState {
    sha: String,
    sha_short: String,
    dirty: bool,
}

fn main() {
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("CARGO_MANIFEST_DIR"));
    let state = checkout_state(&manifest_dir)
        .or_else(|| packaged_state(&manifest_dir))
        .unwrap_or_else(|| GitState {
            sha: UNKNOWN.to_owned(),
            sha_short: UNKNOWN.to_owned(),
            dirty: false,
        });
    let profile = env::var("PROFILE").unwrap_or_else(|_| UNKNOWN.to_owned());

    let generated = format!(
        "pub const GIT_SHA: &str = {:?};\n\
         pub const GIT_SHA_SHORT: &str = {:?};\n\
         pub const GIT_DIRTY: bool = {};\n\
         pub const PROFILE: &str = {:?};\n",
        state.sha, state.sha_short, state.dirty, profile
    );
    let out = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR")).join("build_info.rs");
    fs::write(out, generated).expect("write build_info.rs");
}

fn checkout_state(manifest_dir: &Path) -> Option<GitState> {
    let root = manifest_dir.parent()?.parent()?;
    let dot_git = root.join(".git");
    let git_dir = resolve_git_dir(&dot_git)?;
    let common_dir = resolve_common_dir(&git_dir);

    watch_git_refs(&git_dir, &common_dir);
    for input in COMPILED_INPUTS {
        watch_if_exists(&root.join(input));
    }

    let sha = git(root, &["rev-parse", "--verify", "HEAD"])?;
    if !is_hex_sha(&sha) {
        return None;
    }
    let sha_short = git(root, &["rev-parse", "--short", "HEAD"])
        .filter(|short| is_hex_sha(short) && sha.starts_with(short.as_str()))
        .unwrap_or_else(|| sha[..12].to_owned());
    let mut status = vec!["status", "--porcelain", "--untracked-files=no", "--"];
    status.extend(COMPILED_INPUTS);
    let dirty = git(root, &status).is_some_and(|out| !out.is_empty());
    Some(GitState {
        sha,
        sha_short,
        dirty,
    })
}

/// `.git` is a directory in a normal clone and a `gitdir:` pointer file in a worktree.
fn resolve_git_dir(dot_git: &Path) -> Option<PathBuf> {
    if dot_git.is_dir() {
        return Some(dot_git.to_path_buf());
    }
    let pointer = fs::read_to_string(dot_git).ok()?;
    let target = pointer.trim().strip_prefix("gitdir:")?.trim();
    let target = dot_git.parent()?.join(target);
    target.is_dir().then_some(target)
}

/// Worktree git dirs keep refs in the shared directory named by `commondir`.
fn resolve_common_dir(git_dir: &Path) -> PathBuf {
    fs::read_to_string(git_dir.join("commondir"))
        .ok()
        .map(|relative| git_dir.join(relative.trim()))
        .filter(|dir| dir.is_dir())
        .unwrap_or_else(|| git_dir.to_path_buf())
}

fn watch_git_refs(git_dir: &Path, common_dir: &Path) {
    let head = git_dir.join("HEAD");
    watch_if_exists(&head);
    watch_if_exists(&common_dir.join("packed-refs"));
    let Some(reference) = fs::read_to_string(&head).ok().and_then(|head| {
        head.trim()
            .strip_prefix("ref:")
            .map(|r| r.trim().to_owned())
    }) else {
        return;
    };
    // A packed or unborn branch has no loose ref; watch its nearest existing ancestor.
    let mut path = common_dir.join(reference);
    while !path.exists() {
        match path.parent() {
            Some(parent) if parent.starts_with(common_dir) => path = parent.to_path_buf(),
            _ => return,
        }
    }
    watch_if_exists(&path);
}

/// Cargo reruns forever on a missing path, so only existing paths are watched.
fn watch_if_exists(path: &Path) {
    if path.exists() {
        println!("cargo:rerun-if-changed={}", path.display());
    }
}

fn git(root: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .env_remove("GIT_DIR")
        .env_remove("GIT_WORK_TREE")
        .env_remove("GIT_INDEX_FILE")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8(output.stdout).ok()?.trim().to_owned())
}

/// `cargo package` records the source commit in `.cargo_vcs_info.json`.
fn packaged_state(manifest_dir: &Path) -> Option<GitState> {
    let path = manifest_dir.join(".cargo_vcs_info.json");
    let text = fs::read_to_string(&path).ok()?;
    println!("cargo:rerun-if-changed={}", path.display());
    let after_key = &text[text.find("\"sha1\"")? + "\"sha1\"".len()..];
    let value = after_key.split('"').nth(1)?;
    if !is_hex_sha(value) || value.len() < 12 {
        return None;
    }
    let dirty = text
        .split("\"dirty\"")
        .nth(1)
        .is_some_and(|rest| rest.trim_start_matches([' ', ':']).starts_with("true"));
    Some(GitState {
        sha: value.to_owned(),
        sha_short: value[..12].to_owned(),
        dirty,
    })
}

fn is_hex_sha(value: &str) -> bool {
    value.len() >= 7 && value.bytes().all(|b| b.is_ascii_hexdigit())
}
