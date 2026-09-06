use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

struct TempDirectory(PathBuf);

impl TempDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "pybevy-api-exercise-cli-{}-{}",
            std::process::id(),
            NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn join(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn run_linter(root: &Path, mode: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_pybevy-lint"))
        .current_dir(root)
        .args([
            "--python-path",
            "pybevy",
            "test-coverage",
            "--test-path",
            "tests",
            mode,
            "--baseline-path",
            "baseline.json",
            "--exceptions-path",
            "exceptions.json",
        ])
        .output()
        .unwrap()
}

#[test]
fn check_mode_exits_nonzero_when_evidence_is_removed() {
    let root = TempDirectory::new();
    fs::create_dir(root.join("pybevy")).unwrap();
    fs::create_dir(root.join("tests")).unwrap();
    fs::write(
        root.join("pybevy/sample.pyi"),
        "class Value:\n    def method(self) -> None: ...\n",
    )
    .unwrap();
    fs::write(
        root.join("tests/test_sample.py"),
        "from pybevy.sample import Value\n\ndef test_method() -> None:\n    value = Value()\n    value.method()\n",
    )
    .unwrap();
    fs::write(
        root.join("exceptions.json"),
        "{\n  \"schema_version\": 2,\n  \"exceptions\": []\n}\n",
    )
    .unwrap();

    let update = run_linter(&root.0, "--update-baseline");
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    let initial_check = run_linter(&root.0, "--check-baseline");
    assert!(initial_check.status.success());

    fs::write(
        root.join("tests/test_sample.py"),
        "from pybevy.sample import Value\n\ndef test_method() -> None:\n    Value\n",
    )
    .unwrap();
    let regression = run_linter(&root.0, "--check-baseline");
    assert!(!regression.status.success());
    assert!(
        String::from_utf8_lossy(&regression.stderr).contains("lost all evidence"),
        "{}",
        String::from_utf8_lossy(&regression.stderr)
    );
}
