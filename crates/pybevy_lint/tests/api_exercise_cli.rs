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
fn nested_inherited_and_generic_calls_preserve_operation_evidence() {
    let root = TempDirectory::new();
    fs::create_dir(root.join("pybevy")).unwrap();
    fs::create_dir(root.join("tests")).unwrap();
    fs::write(
        root.join("pybevy/sample.pyi"),
        r#"
class State:
    def ready(self) -> bool: ...
    def overridden(self) -> bool: ...
    class Loaded(State):
        def __init__(self) -> None: ...
class Child(State):
    def __init__(self) -> None: ...
    def overridden(self) -> bool: ...
class Buttons:
    def __init__(self) -> None: ...
    def pressed(self) -> bool: ...
    def released(self) -> bool: ...
class Other:
    def ready(self) -> bool: ...
"#,
    )
    .unwrap();
    fs::write(
        root.join("tests/test_sample.py"),
        r#"
from pybevy.sample import State, Buttons, Child

def test_state():
    state = State.Loaded()
    assert state.ready()
    assert Child().overridden()

def check_resource(buttons: Res[Buttons[int]]):
    assert buttons.released()

def test_generic():
    buttons = Buttons[int]()
    assert buttons.pressed()
"#,
    )
    .unwrap();
    fs::write(
        root.join("exceptions.json"),
        r#"{"schema_version": 2, "exceptions": []}"#,
    )
    .unwrap();
    let output = run_linter(&root.0, "--update-baseline");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let baseline: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("baseline.json")).unwrap()).unwrap();
    for (path, expected) in [
        ("pybevy.sample.State.ready", "exercised"),
        ("pybevy.sample.State.overridden", "debt"),
        ("pybevy.sample.Child.overridden", "exercised"),
        ("pybevy.sample.Buttons", "exercised"),
        ("pybevy.sample.Buttons.pressed", "exercised"),
        ("pybevy.sample.Buttons.released", "exercised"),
        ("pybevy.sample.Other.ready", "debt"),
    ] {
        let operations = baseline["operations"].as_array().unwrap();
        let operation = operations
            .iter()
            .find(|op| op["path"] == path)
            .unwrap_or_else(|| panic!("missing {path}: {baseline}"));
        assert_eq!(operation["status"], expected, "{path}");
    }
    assert!(run_linter(&root.0, "--check-baseline").status.success());
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

#[test]
fn source_only_facades_allow_baseline_updates_without_masking_bad_names() {
    let root = TempDirectory::new();
    fs::create_dir_all(root.join("pybevy/contrib")).unwrap();
    fs::create_dir(root.join("tests")).unwrap();
    fs::write(root.join("pybevy/math.pyi"), "class Vec3: ...\n").unwrap();
    fs::write(
        root.join("pybevy/contrib/__init__.py"),
        "from .camera import FlyCamera\n__all__ = ['FlyCamera']\n",
    )
    .unwrap();
    fs::write(
        root.join("pybevy/contrib/camera.py"),
        "class FlyCamera: pass\n",
    )
    .unwrap();
    fs::write(
        root.join("exceptions.json"),
        r#"{"schema_version": 2, "exceptions": []}"#,
    )
    .unwrap();
    fs::write(root.join("tests/test_facade.py"), "import pybevy.contrib as contrib\n\ndef test_export():\n    assert contrib.FlyCamera is not None\n    contrib.FlyCamera()\n").unwrap();
    let update = run_linter(&root.0, "--update-baseline");
    assert!(
        update.status.success(),
        "{}",
        String::from_utf8_lossy(&update.stderr)
    );
    assert!(
        !fs::read_to_string(root.join("baseline.json"))
            .unwrap()
            .contains("contrib.FlyCamera")
    );
    for (module, name) in [
        ("math", "MissingVec3"),
        ("nonexistent", "Vec3"),
        ("contrib", "NotAnExport"),
    ] {
        fs::write(
            root.join("tests/test_facade.py"),
            format!(
                "import pybevy.{module} as module\n\ndef test_reference():\n    module.{name}()\n"
            ),
        )
        .unwrap();
        let update = run_linter(&root.0, "--update-baseline");
        assert!(!update.status.success());
        assert!(
            String::from_utf8_lossy(&update.stderr).contains(&format!("module.{name}")),
            "{}",
            String::from_utf8_lossy(&update.stderr)
        );
    }
}
