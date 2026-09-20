use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const WRAPPERS: &str = r#"
#[cfg(feature = "gpu")]
#[pyclass(name = "Diagnostics", module = "pybevy.gpu", frozen)]
struct PyDiagnostics;
#[cfg(feature = "gpu")]
#[pymethods]
impl PyDiagnostics {
    #[getter]
    fn last_error(&self) -> String { String::new() }
}
#[cfg(not(feature = "gpu"))]
#[pyclass(name = "Diagnostics", module = "pybevy.gpu", frozen)]
struct PyDiagnostics;
#[cfg(not(feature = "gpu"))]
#[pymethods]
impl PyDiagnostics {
    #[getter]
    fn last_error(&self) -> String { String::new() }
}
#[pymethods]
impl PyDiagnostics { fn shared(&self) -> bool { true } }
"#;

const STUB: &str = r#"
class Diagnostics:
    @property
    def last_error(self) -> str: ...
    def shared(self) -> bool: ...
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
fn cfg_class_validation_checks_each_build_contract() {
    let bevy = pinned_bevy_source();
    for case in [
        "valid",
        "missing-native",
        "missing-fallback",
        "invented-property",
        "wrong-fallback-type",
        "native-only-method",
        "fallback-only-method",
        "unmatched-gate",
        "duplicate-gate",
        "conditional-member",
        "cfg-attr",
    ] {
        let root =
            std::env::temp_dir().join(format!("pybevy-cfg-classes-{}-{case}", std::process::id()));
        fs::create_dir_all(root.join("src/gpu")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        let mut wrappers = WRAPPERS.to_string();
        let mut stub = STUB.to_string();
        let expected = match case {
            "valid" => None,
            "missing-native" => {
                wrappers = wrappers.replacen(
                    "#[getter]\n    fn last_error(&self) -> String { String::new() }",
                    "",
                    1,
                );
                Some("E003")
            }
            "missing-fallback" => {
                let index = wrappers.rfind("#[getter]").unwrap();
                let end = wrappers[index..].find("String::new() }").unwrap()
                    + index
                    + "String::new() }".len();
                wrappers.replace_range(index..end, "");
                Some("E003")
            }
            "invented-property" => {
                stub.push_str("    @property\n    def invented(self) -> str: ...\n");
                Some("E003")
            }
            "wrong-fallback-type" => {
                let index = wrappers.rfind("fn last_error").unwrap();
                wrappers.replace_range(index.., &wrappers[index..].replacen("String", "bool", 1));
                Some("E006")
            }
            "native-only-method" | "fallback-only-method" => {
                let gate = if case == "native-only-method" {
                    "feature = \"gpu\""
                } else {
                    "not(feature = \"gpu\")"
                };
                wrappers.push_str(&format!("\n#[cfg({gate})]\n#[pymethods]\nimpl PyDiagnostics {{ fn branch_only(&self) {{}} }}"));
                stub.push_str("    def branch_only(self) -> None: ...\n");
                Some("E003")
            }
            "unmatched-gate" => {
                wrappers.push_str("#[cfg(feature = \"other\")] #[pymethods] impl PyDiagnostics { fn other(&self) {} }");
                Some("cfg gate must exactly match")
            }
            "duplicate-gate" => {
                wrappers = wrappers.replace("not(feature = \"gpu\")", "feature = \"gpu\"");
                Some("ambiguous cfg declarations")
            }
            "conditional-member" => {
                wrappers = wrappers.replace("fn shared", "#[cfg(feature = \"gpu\")] fn shared");
                Some("cfg-gated impl members")
            }
            "cfg-attr" => {
                wrappers = wrappers.replacen(
                    "#[pyclass",
                    "#[cfg_attr(feature = \"other\", cfg(feature = \"gpu\"))]\n#[pyclass",
                    1,
                );
                Some("cfg_attr-gated duplicate class")
            }
            _ => unreachable!(),
        };
        fs::write(root.join("src/gpu/mod.rs"), wrappers).unwrap();
        fs::write(root.join("pybevy/gpu.pyi"), stub).unwrap();
        fs::write(root.join(".pybevy-lint.toml"), "").unwrap();
        for scoped in [false, true] {
            let mut command = Command::new(env!("CARGO_BIN_EXE_pybevy-lint"));
            command.current_dir(&root).args([
                "--rust-path",
                ".",
                "--python-path",
                "pybevy",
                "validate",
            ]);
            if scoped {
                command.arg("Diagnostics");
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
            if let Some(expected) = expected {
                assert!(!output.status.success(), "{case}, scoped={scoped}: {text}");
                assert!(text.contains(expected), "{case}, scoped={scoped}: {text}");
            } else {
                assert!(output.status.success(), "{case}, scoped={scoped}: {text}");
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
