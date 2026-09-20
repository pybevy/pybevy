use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const WRAPPERS: &str = r#"
#[pyclass(name = "Time", module = "pybevy.time")]
struct PyTime;
#[pymethods]
impl PyTime { fn elapsed(&self) -> f32 { 0.0 } }
#[pyclass(name = "_TimeVirtual", module = "pybevy.time")]
struct PyTimeVirtual;
#[pymethods]
impl PyTimeVirtual { fn pause(&self) {} }
#[pyclass(name = "_TimeFixed", module = "pybevy.time")]
struct PyTimeFixed;
#[pymethods]
impl PyTimeFixed {
    fn timestep(&self) -> f32 { 0.0 }
    #[staticmethod]
    fn from_hz(hz: f64) -> PyResult<Py<PyTimeFixed>> { todo!() }
}
"#;

const STUB: &str = r#"
class Time:
    def elapsed(self) -> float: ...
    def pause(self: Time[Virtual]) -> None: ...
    def timestep(self: Time[Fixed]) -> float: ...
    @classmethod
    def from_hz(cls: type[Time[Fixed]], hz: float) -> Time[Fixed]: ...
"#;

const CONFIG: &str = r#"
[[validation.specialized_receivers]]
path = "time.Time[Virtual]"
rust_type = "PyTimeVirtual"
reason = "fixture dispatches Virtual to PyTimeVirtual"
[[validation.specialized_receivers]]
path = "time.Time[Fixed]"
rust_type = "PyTimeFixed"
reason = "fixture dispatches Fixed to PyTimeFixed"
"#;

fn pinned_bevy_source() -> PathBuf {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap();
    let source = root.join("target/bevy-api-audit/bevy-b56fc29d3016");
    assert!(
        source.join("Cargo.toml").exists(),
        "pinned audit source is required"
    );
    source
}

#[test]
fn specialized_receiver_cli_preserves_missing_and_mismatched_method_checks() {
    let bevy = pinned_bevy_source();
    for case in [
        "valid",
        "scoped",
        "scoped-missing",
        "scoped-wrong-return",
        "quoted-receiver",
        "empty-reason",
        "duplicate-target",
        "base-native-leak",
        "missing-method",
        "wrong-specialization",
        "wrong-return",
        "wrong-argument",
        "wrong-factory-argument",
        "wrong-factory-return",
        "wrong-native-kind",
        "missing-target",
        "duplicate-mapping",
        "wrong-module",
        "stale-mapping",
        "base-leak",
        "unknown-receiver",
        "unrelated-filter",
    ] {
        let root =
            std::env::temp_dir().join(format!("pybevy-receivers-{}-{case}", std::process::id()));
        fs::create_dir_all(root.join("src/time")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        let mut wrappers = WRAPPERS.to_string();
        let mut stub = STUB.to_string();
        let mut config = CONFIG.to_string();
        let expected = match case {
            "valid" | "scoped" | "unrelated-filter" => None,
            "quoted-receiver" => {
                stub = stub
                    .replace("self: Time[Virtual]", "self: 'Time[Virtual]'")
                    .replace("cls: type[Time[Fixed]]", "cls: type [Time[Fixed]]");
                None
            }
            "empty-reason" => {
                config = config.replace("fixture dispatches Virtual to PyTimeVirtual", "");
                Some("E010")
            }
            "duplicate-target" => {
                wrappers = wrappers.replace(
                    "#[pyclass(name = \"_TimeVirtual\"",
                    "#[cfg(feature = \"alternative\")]\n#[pyclass(name = \"_TimeVirtual\"",
                );
                wrappers.push_str(
                    "#[cfg(not(feature = \"alternative\"))] #[pyclass(name = \"_Other\", module = \"pybevy.time\")] struct PyTimeVirtual;",
                );
                Some("E010")
            }
            "base-native-leak" => {
                wrappers = wrappers.replace("impl PyTime {", "impl PyTime { fn pause(&self) {}");
                Some("E002")
            }
            "missing-method" | "scoped-missing" => {
                wrappers = wrappers.replace("fn pause(&self)", "fn gone(&self)");
                Some("E003")
            }
            "wrong-specialization" => {
                stub = stub.replace("pause(self: Time[Virtual])", "pause(self: Time[Fixed])");
                Some("E003")
            }
            "wrong-return" | "scoped-wrong-return" => {
                stub = stub.replace(
                    "timestep(self: Time[Fixed]) -> float",
                    "timestep(self: Time[Fixed]) -> str",
                );
                Some("E006")
            }
            "wrong-argument" => {
                stub = stub.replace(
                    "pause(self: Time[Virtual])",
                    "pause(self: Time[Virtual], extra: int)",
                );
                Some("E008")
            }
            "wrong-factory-argument" => {
                stub = stub.replace("hz: float", "frequency: float");
                Some("E004")
            }
            "wrong-factory-return" => {
                wrappers = wrappers.replace(
                    "PyResult<Py<PyTimeFixed>>",
                    "PyResult<Option<Py<PyTimeFixed>>>",
                );
                Some("E006")
            }
            "wrong-native-kind" => {
                wrappers = wrappers.replace("fn pause(&self)", "#[staticmethod] fn pause()");
                Some("E003")
            }
            "missing-target" => {
                config =
                    config.replace("rust_type = \"PyTimeVirtual\"", "rust_type = \"PyMissing\"");
                Some("E010")
            }
            "duplicate-mapping" => {
                config.push_str(CONFIG);
                Some("E010")
            }
            "wrong-module" => {
                wrappers =
                    wrappers.replace("module = \"pybevy.time\"", "module = \"pybevy.other\"");
                Some("E010")
            }
            "stale-mapping" => {
                config = config.replace("time.Time[Virtual]", "time.Missing[Virtual]");
                Some("E010")
            }
            "base-leak" => {
                stub = stub.replace("pause(self: Time[Virtual])", "pause(self)");
                Some("E003")
            }
            "unknown-receiver" => {
                stub = stub.replace("pause(self: Time[Virtual])", "pause(self: Time[Real])");
                Some("E010")
            }
            _ => unreachable!(),
        };
        fs::write(root.join("src/time/mod.rs"), wrappers).unwrap();
        fs::write(root.join("pybevy/time.pyi"), stub).unwrap();
        fs::write(root.join(".pybevy-lint.toml"), config).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_pybevy-lint"));
        command.current_dir(&root).args([
            "--rust-path",
            ".",
            "--python-path",
            "pybevy",
            "validate",
        ]);
        if case.starts_with("scoped") {
            command.arg("Time");
        } else if case == "unrelated-filter" {
            command.arg("_TimeVirtual");
        }
        let output = command
            .args(["--bevy-path", bevy.to_str().unwrap(), "--deny-warnings"])
            .output()
            .unwrap();
        let text = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        if let Some(code) = expected {
            assert!(!output.status.success(), "{case}: {text}");
            assert!(text.contains(code), "{case}: {text}");
        } else {
            assert!(output.status.success(), "{case}: {text}");
        }
        fs::remove_dir_all(root).unwrap();
    }
}
