use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

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
fn iterator_returns_check_protocol_and_item_shape_in_full_and_scoped_validation() {
    let bevy = pinned_bevy_source();
    for (case, factory, next, item, valid) in [
        ("direct", "PyItems", "Option<PyEntity>", "Entity", true),
        (
            "wrapped",
            "PyResult<Py<PyItems>>",
            "PyResult<Option<Py<PyEntity>>>",
            "Entity",
            true,
        ),
        (
            "qualified",
            "PyResult<crate::items::PyItems>",
            "Option<crate::entity::PyEntity>",
            "Entity",
            true,
        ),
        (
            "direct-next",
            "PyItems",
            "PyResult<PyEntity>",
            "Entity",
            true,
        ),
        (
            "optional-item",
            "PyItems",
            "Option<Option<PyEntity>>",
            "Entity | None",
            true,
        ),
        (
            "lost-optional-item",
            "PyItems",
            "Option<Option<PyEntity>>",
            "Entity",
            false,
        ),
        (
            "tuple",
            "PyItems",
            "Option<(PyEntity, u32)>",
            "tuple[Entity, int]",
            true,
        ),
        (
            "wrong-tuple",
            "PyItems",
            "Option<(PyEntity, u32)>",
            "tuple[Entity, str]",
            false,
        ),
        (
            "list",
            "PyItems",
            "Option<Vec<PyEntity>>",
            "list[Entity]",
            true,
        ),
        ("wrong-item", "PyItems", "Option<PyEntity>", "str", false),
        (
            "optional-iterator",
            "Option<PyItems>",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
        (
            "erased-item",
            "PyItems",
            "PyResult<Py<PyAny>>",
            "Entity",
            false,
        ),
        (
            "erased-tuple-item",
            "PyItems",
            "Option<(PyEntity, Py<PyAny>)>",
            "tuple[Entity, int]",
            false,
        ),
        (
            "missing-iter",
            "PyItems",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
        (
            "missing-next",
            "PyItems",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
        ("wrong-iter", "PyItems", "Option<PyEntity>", "Entity", false),
        (
            "wrong-module",
            "PyItems",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
        (
            "next-argument",
            "PyItems",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
        (
            "ambiguous-next",
            "PyItems",
            "Option<PyEntity>",
            "Entity",
            false,
        ),
    ] {
        let root =
            std::env::temp_dir().join(format!("pybevy-iterator-{}-{case}", std::process::id()));
        fs::create_dir_all(root.join("src/ecs")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        let source = format!(
            r#"
#[pyclass(name = "Source", module = "pybevy.ecs")]
struct PySource;
#[pymethods]
impl PySource {{ fn items(&self) -> {factory} {{ todo!() }} }}
"#
        );
        let mut iterator = format!(
            r#"
#[pyclass(name = "_Items", module = "pybevy.ecs")]
struct PyItems;
#[pymethods]
impl PyItems {{
    fn __iter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {{ slf }}
    fn __next__(&mut self) -> {next} {{ todo!() }}
}}
"#
        );
        match case {
            "missing-iter" => iterator = iterator.replace("__iter__", "not_iter"),
            "missing-next" => iterator = iterator.replace("__next__", "not_next"),
            "wrong-iter" => iterator = iterator.replace("-> PyRef<'_, Self>", "-> PyOther"),
            "wrong-module" => iterator = iterator.replace("pybevy.ecs", "pybevy.other"),
            "next-argument" => {
                iterator =
                    iterator.replace("__next__(&mut self)", "__next__(&mut self, count: usize)")
            }
            "ambiguous-next" => {
                iterator =
                    iterator.replace("fn __next__", "#[cfg(feature = \"alternate\")] fn __next__");
                iterator.push_str("#[pymethods] impl PyItems { #[cfg(not(feature = \"alternate\"))] fn __next__(&self) -> bool { true } }");
            }
            _ => {}
        }
        fs::write(root.join("src/ecs/source.rs"), source).unwrap();
        fs::write(root.join("src/ecs/items.rs"), iterator).unwrap();
        fs::write(
            root.join("pybevy/ecs.pyi"),
            format!("class Source:\n    def items(self) -> Iterator[{item}]: ...\n"),
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
                .args(["--bevy-path", bevy.to_str().unwrap(), "--deny-warnings"])
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
            }
        }
        fs::remove_dir_all(root).unwrap();
    }
}
