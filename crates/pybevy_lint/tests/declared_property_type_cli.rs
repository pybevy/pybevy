use std::{fs, path::Path, process::Command};

#[test]
fn declared_property_names_preserve_type_checks_in_full_and_scoped_validation() {
    let bevy = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/bevy-api-audit/bevy-b56fc29d3016");
    assert!(bevy.join("Cargo.toml").exists());
    for (case, rust_type, python_type, valid) in [
        ("direct", "PyToken", "token", true),
        ("wrapped", "PyResult<Py<PyToken>>", "token", true),
        ("borrowed", "PyResult<Bound<'_, PyToken>>", "token", true),
        ("reference", "&PyToken", "token", true),
        ("optional", "Option<PyToken>", "token | None", true),
        ("lost-option", "Option<PyToken>", "token", false),
        ("list", "Vec<PyToken>", "list[token]", true),
        ("wrong-list", "Vec<PyToken>", "list[str]", false),
        ("tuple", "(PyToken, u32)", "tuple[token, int]", true),
        ("wrong-tuple", "(PyToken, u32)", "tuple[token, str]", false),
        ("wrong-name", "PyToken", "Token", false),
        ("undeclared", "PyToken", "token", false),
        ("ambiguous", "PyToken", "token", false),
        ("qualified", "other::PyToken", "token", false),
        ("private", "PyToken", "token", false),
        ("missing-module", "PyToken", "token", false),
        ("foreign-module", "PyToken", "token", false),
        ("unrelated-leaf", "Token", "token", false),
        ("dynamic-name", "PyToken", "str", false),
        ("primitive-name", "PyToken", "str", false),
    ] {
        let root = std::env::temp_dir().join(format!(
            "pybevy-declared-property-{}-{case}",
            std::process::id()
        ));
        fs::create_dir_all(root.join("src/ecs")).unwrap();
        fs::create_dir_all(root.join("src/array")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        let caller = format!(
            r#"
#[pyclass(name = "Source", module = "pybevy.ecs", frozen)]
struct PySource;
#[pymethods]
impl PySource {{ #[getter] fn value(&self) -> {rust_type} {{ todo!() }} }}
"#
        );
        let mut declaration =
            r#"#[pyclass(name = "token", module = "pybevy.array", frozen)] struct PyToken;"#
                .to_string();
        match case {
            "dynamic-name" => declaration = declaration.replace("\"token\"", "\"Any\""),
            "primitive-name" => declaration = declaration.replace("\"token\"", "\"String\""),
            "undeclared" => declaration.clear(),
            "ambiguous" => {
                fs::write(
                    root.join("src/ecs/duplicate.rs"),
                    declaration.replace("pybevy.array", "pybevy.ecs"),
                )
                .unwrap();
            }
            "private" => declaration = declaration.replace("\"token\"", "\"_token\""),
            "missing-module" => {
                declaration = declaration.replace(", module = \"pybevy.array\"", "")
            }
            "foreign-module" => declaration = declaration.replace("pybevy.array", "other.array"),
            _ => {}
        }
        fs::write(root.join("src/ecs/source.rs"), caller).unwrap();
        fs::write(root.join("src/array/token.rs"), declaration).unwrap();
        fs::write(
            root.join("pybevy/ecs.pyi"),
            format!("class Source:\n    @property\n    def value(self) -> {python_type}: ...\n"),
        )
        .unwrap();
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
                command.arg("Source");
            }
            let output = command
                .args([
                    "-c",
                    "E006",
                    "--bevy-path",
                    bevy.to_str().unwrap(),
                    "--deny-warnings",
                ])
                .output()
                .unwrap();
            let text = format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(
                output.status.success(),
                valid,
                "{case}, scoped={scoped}: {text}"
            );
            if !valid {
                assert!(text.contains("E006"), "{case}, scoped={scoped}: {text}");
                assert!(
                    text.contains("PyToken") || case == "unrelated-leaf",
                    "original type must remain visible: {text}"
                );
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
