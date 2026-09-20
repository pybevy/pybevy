use std::{fs, path::Path, process::Command};

#[test]
fn generic_bases_preserve_inheritance_diagnostics_in_full_and_scoped_validation() {
    let bevy = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target/bevy-api-audit/bevy-b56fc29d3016");
    assert!(bevy.join("Cargo.toml").exists());
    for (case, bases, enum_wrapper, expected) in [
        ("leading", "Generic[T], Resource", false, None),
        ("trailing", "Resource, Generic[T]", false, None),
        ("qualified", "typing.Generic[T], Resource", false, None),
        (
            "extensions",
            "typing_extensions.Generic[T], Resource",
            false,
            None,
        ),
        ("wrong-base", "Generic[T], Component", false, Some("E008")),
        ("generic-only", "Generic[T]", false, Some("E008")),
        (
            "real-generic-base",
            "Base[T], Resource",
            false,
            Some("E008"),
        ),
        (
            "unrelated-generic",
            "custom.Generic[T], Resource",
            false,
            Some("E008"),
        ),
        ("enum", "Generic[T], Enum", true, Some("E017")),
        ("enum-first", "Enum, Generic[T]", true, Some("E017")),
        (
            "qualified-enum",
            "typing.Generic[T], enum.IntEnum",
            true,
            Some("E017"),
        ),
    ] {
        let root =
            std::env::temp_dir().join(format!("pybevy-generic-base-{}-{case}", std::process::id()));
        fs::create_dir_all(root.join("src/ecs")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        let source = if enum_wrapper {
            "#[pyenum(Example)]\n#[pyclass(name = \"Example\", module = \"pybevy.ecs\")]\nenum PyExample { Value }"
        } else {
            "#[pyclass(name = \"Example\", module = \"pybevy.ecs\", extends = PyResource)]\nstruct PyExample;"
        };
        fs::write(root.join("src/ecs/example.rs"), source).unwrap();
        fs::write(
            root.join("pybevy/ecs.pyi"),
            format!("class Example({bases}): ...\n"),
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
                command.arg("Example");
            }
            let code = if enum_wrapper { "E017" } else { "E008" };
            let output = command
                .args([
                    "-c",
                    code,
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
                expected.is_none(),
                "{case}, scoped={scoped}: {text}"
            );
            if let Some(expected) = expected {
                assert!(text.contains(expected), "{case}, scoped={scoped}: {text}");
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
