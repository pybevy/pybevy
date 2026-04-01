//! Build helper for downstream pybevy wheels.
//!
//! Add `pybevy-build` as a `[build-dependency]` and call
//! [`copy_python_to_out_dir`] from your `build.rs`:
//!
//! ```rust,no_run
//! fn main() {
//!     pybevy_build::copy_python_to_out_dir();
//! }
//! ```
//!
//! This locates pybevy's Python source tree via `cargo_metadata` and copies
//! all `.py`, `.pyi`, and `py.typed` files into `OUT_DIR/pybevy/`, preserving
//! the directory structure. The downstream `pyproject.toml` can then include
//! them via maturin's `[tool.maturin] include` directive.

use std::{
    env, fs,
    path::{Path, PathBuf},
};

/// Copy pybevy's Python sources into `$OUT_DIR/pybevy/`.
///
/// # Panics
///
/// Panics if `cargo_metadata` cannot locate the `pybevy` package or if
/// the expected `pybevy/` directory does not exist within it.
pub fn copy_python_to_out_dir() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("OUT_DIR not set"));
    let pybevy_src = locate_pybevy_python_dir();
    let dest = out_dir.join("pybevy");

    copy_python_tree(&pybevy_src, &dest);

    // Tell Cargo to re-run if any Python source changes.
    println!("cargo:rerun-if-changed={}", pybevy_src.display());
}

/// Locate the `pybevy/` Python source directory inside the `pybevy` crate.
fn locate_pybevy_python_dir() -> PathBuf {
    let manifest_path = env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .expect("CARGO_MANIFEST_DIR not set");

    let metadata = cargo_metadata::MetadataCommand::new()
        .manifest_path(manifest_path.join("Cargo.toml"))
        .exec()
        .expect("failed to run `cargo metadata`");

    let pybevy_pkg = metadata
        .packages
        .iter()
        .find(|p| p.name == "pybevy")
        .expect("could not find `pybevy` package in cargo metadata");

    let pybevy_root = pybevy_pkg
        .manifest_path
        .parent()
        .expect("pybevy manifest has no parent directory");

    let python_dir = PathBuf::from(pybevy_root.as_std_path()).join("pybevy");
    assert!(
        python_dir.is_dir(),
        "expected Python source directory at {}",
        python_dir.display()
    );

    python_dir
}

/// Recursively copy `.py`, `.pyi`, and `py.typed` files from `src` to `dst`.
fn copy_python_tree(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).expect("failed to create destination directory");

    for entry in fs::read_dir(src).expect("failed to read source directory") {
        let entry = entry.expect("failed to read directory entry");
        let path = entry.path();
        let file_name = entry.file_name();
        let file_name_str = file_name.to_string_lossy();

        // Skip __pycache__ and hidden directories.
        if file_name_str.starts_with('.') || file_name_str == "__pycache__" {
            continue;
        }

        let dest_path = dst.join(&file_name);

        if path.is_dir() {
            copy_python_tree(&path, &dest_path);
        } else if file_name_str.ends_with(".py")
            || file_name_str.ends_with(".pyi")
            || file_name_str == "py.typed"
        {
            fs::copy(&path, &dest_path).unwrap_or_else(|e| {
                panic!("failed to copy {}: {e}", path.display());
            });
        }
    }
}
