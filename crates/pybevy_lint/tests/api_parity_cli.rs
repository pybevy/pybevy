//! Public CLI regressions for return-type parity and signature gates.

use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT_FIXTURE: AtomicUsize = AtomicUsize::new(0);

struct Fixture(PathBuf);

impl Fixture {
    fn new(wrapper: &str, stub: &str, config: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "pybevy-api-parity-{}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(root.join("rust")).unwrap();
        fs::create_dir_all(root.join("pybevy")).unwrap();
        fs::write(root.join("rust/wrapper.rs"), wrapper).unwrap();
        fs::write(root.join("pybevy/math.pyi"), stub).unwrap();
        fs::write(root.join(".pybevy-lint.toml"), config).unwrap();
        Self(root)
    }

    fn run(&self, args: &[&str]) -> (i32, String) {
        let output = Command::new(env!("CARGO_BIN_EXE_pybevy-lint"))
            .current_dir(&self.0)
            .args(["--rust-path", "rust", "--python-path", "pybevy"])
            .args(args)
            .output()
            .unwrap();
        (
            output.status.code().unwrap_or(-1),
            format!(
                "{}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            ),
        )
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn pinned_bevy_source() -> PathBuf {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo = manifest.parent().and_then(Path::parent).unwrap();
    let mut candidates = vec![
        repo.join("target/bevy-api-audit/bevy-b56fc29d3016"),
        repo.join("target/bevy-api-audit/bevy-src"),
    ];
    if let Some(path) = std::env::var_os("PYBEVY_BEVY_SOURCE") {
        candidates.insert(0, path.into());
    }
    if let Some(home) = std::env::var_os("HOME") {
        candidates.push(PathBuf::from(home).join("Documents/rust/bevy"));
    }
    candidates
        .into_iter()
        .find(|path| path.join("Cargo.toml").exists())
        .expect("API parity CLI regressions require the pinned Bevy source")
}

fn validate_return(rust_return: &str, stub_return: &str) -> (i32, String) {
    let fixture = Fixture::new(
        &format!(
            r#"
#[pyclass(name = "Value", module = "pybevy.math", frozen)]
pub struct PyValue;
#[pymethods]
impl PyValue {{
    fn result(&self) -> {rust_return} {{ todo!() }}
}}
"#
        ),
        &format!("class Value:\n    def result(self) -> {stub_return}: ...\n"),
        "[bevy.crate_mappings]\n",
    );
    let bevy = pinned_bevy_source();
    fixture.run(&[
        "--deny-warnings",
        "validate",
        "--bevy-path",
        bevy.to_str().unwrap(),
    ])
}

#[test]
fn nested_tuple_returns_check_elements_and_arity() {
    let rust_return = "PyResult<(f32, (u32, bool))>";
    let (code, output) = validate_return(rust_return, "tuple[float, tuple[int, bool]]");
    assert_eq!(code, 0, "matching tuple must pass: {output}");

    for stub_return in [
        "tuple[float, tuple[str, bool]]",
        "tuple[float, tuple[int]]",
        "tuple[float, tuple[int, bool], int]",
    ] {
        let (code, output) = validate_return(rust_return, stub_return);
        assert_ne!(code, 0, "incorrect tuple must fail: {output}");
        assert!(output.contains("E006"), "{output}");
        assert!(output.contains("result"), "{output}");
    }
}

#[test]
fn unit_and_value_returns_are_distinct() {
    for (rust_return, stub_return, matches) in [
        ("PyResult<()>", "None", true),
        ("PyResult<()>", "int", false),
        ("PyResult<u32>", "None", false),
    ] {
        let (code, output) = validate_return(rust_return, stub_return);
        if matches {
            assert_eq!(code, 0, "{output}");
        } else {
            assert_ne!(code, 0, "{output}");
            assert!(output.contains("E006"), "{output}");
        }
    }
}

#[test]
fn self_returns_resolve_to_the_declaring_class() {
    for rust_return in ["Self", "PyResult<Self>", "PyRef<'_, Self>"] {
        let (code, output) = validate_return(rust_return, "Value");
        assert_eq!(code, 0, "owned and borrowed Self must resolve: {output}");
        let (code, output) = validate_return(rust_return, "int");
        assert_ne!(
            code, 0,
            "Self must not accept an unrelated result: {output}"
        );
        assert!(output.contains("E006"), "{output}");
    }
}

#[test]
fn arrays_lists_and_bytes_follow_python_conversion_shapes() {
    for (rust_return, stub_return, matches) in [
        ("PyResult<[f32; 2]>", "list[float]", true),
        ("PyResult<[f32; 2]>", "list[str]", false),
        (
            "PyResult<Vec<(f32, bool)>>",
            "list[tuple[float, bool]]",
            true,
        ),
        (
            "PyResult<Vec<(f32, bool)>>",
            "list[tuple[int, bool]]",
            false,
        ),
        ("PyResult<Vec<u8>>", "bytes", true),
        ("PyResult<Vec<u8>>", "list[int]", false),
    ] {
        let (code, output) = validate_return(rust_return, stub_return);
        if matches {
            assert_eq!(code, 0, "conversion shape must match: {output}");
        } else {
            assert_ne!(code, 0, "conversion shape must differ: {output}");
            assert!(output.contains("E006"), "{output}");
        }
    }
}

#[test]
fn compare_rejects_a_tuple_in_place_of_a_vector() {
    let bevy = pinned_bevy_source();
    for (return_type, matches) in [("PyVec2", true), ("(f32, f32)", false)] {
        let fixture = Fixture::new(
            &format!(
                r#"
#[pyclass(name = "Vec2", module = "pybevy.math", frozen)]
pub struct PyVec2;
#[pymethods]
impl PyVec2 {{
    fn normalize(&self) -> {return_type} {{ todo!() }}
}}
"#
            ),
            "class Vec2: ...\n",
            "[bevy.crate_mappings]\nmath = 'bevy_math'\n[bevy.crate_type_sources]\nbevy_math = ['glam']\n",
        );
        let (code, output) = fixture.run(&[
            "compare",
            "math",
            "--bevy-path",
            bevy.to_str().unwrap(),
            "--check-signatures",
            "--check-usage",
            "false",
        ]);
        assert!(
            output.contains("normalize"),
            "comparison must run: {output}"
        );
        if matches {
            assert_eq!(code, 0, "{output}");
        } else {
            assert_ne!(code, 0, "vector replaced by tuple must fail: {output}");
            assert!(output.contains("return"), "{output}");
        }
    }
}

#[test]
fn compare_rejects_discarding_timer_tick_self_return() {
    let bevy = pinned_bevy_source();
    for (return_type, matches) in [("PyResult<Self>", true), ("PyResult<()>", false)] {
        let fixture = Fixture::new(
            &format!(
                r#"
#[pyclass(name = "Timer", module = "pybevy.time")]
pub struct PyTimer;
#[pymethods]
impl PyTimer {{
    fn tick(&mut self, delta: Duration) -> {return_type} {{ todo!() }}
}}
"#
            ),
            "class Timer: ...\n",
            "[bevy.crate_mappings]\ntime = 'bevy_time'\n",
        );
        let (code, output) = fixture.run(&[
            "compare",
            "time",
            "--bevy-path",
            bevy.to_str().unwrap(),
            "--check-signatures",
            "--check-usage",
            "false",
        ]);
        assert!(output.contains("tick"), "comparison must run: {output}");
        if matches {
            assert_eq!(code, 0, "Self must match the wrapper: {output}");
        } else {
            assert_ne!(code, 0, "discarded Self must fail: {output}");
            assert!(output.contains("return"), "{output}");
        }
    }
}

#[test]
fn static_method_returns_are_validated() {
    let bevy = pinned_bevy_source();
    for (stub_return, matches) in [("int", true), ("None", false)] {
        let fixture = Fixture::new(
            r#"
#[pyclass(name = "Value", module = "pybevy.math", frozen)]
pub struct PyValue;
#[pymethods]
impl PyValue {
    #[staticmethod]
    fn result() -> PyResult<u32> { todo!() }
}
"#,
            &format!("class Value:\n    @staticmethod\n    def result() -> {stub_return}: ...\n"),
            "[bevy.crate_mappings]\n",
        );
        let (code, output) = fixture.run(&[
            "--deny-warnings",
            "validate",
            "--bevy-path",
            bevy.to_str().unwrap(),
        ]);
        if matches {
            assert_eq!(code, 0, "{output}");
        } else {
            assert_ne!(code, 0, "{output}");
            assert!(output.contains("E006"), "{output}");
            assert!(output.contains("result"), "{output}");
        }
    }
}

#[test]
fn every_stub_overload_return_is_checked() {
    let bevy = pinned_bevy_source();
    for (second_return, matches) in [("int", true), ("str", false)] {
        let fixture = Fixture::new(
            r#"
#[pyclass(name = "Value", module = "pybevy.math", frozen)]
pub struct PyValue;
#[pymethods]
impl PyValue {
    fn result(&self, input: &Bound<PyAny>) -> PyResult<u32> { todo!() }
}
"#,
            &format!(
                "from typing import overload\nclass Value:\n    @overload\n    def result(self, input: int) -> int: ...\n    @overload\n    def result(self, input: str) -> {second_return}: ...\n"
            ),
            "[bevy.crate_mappings]\n",
        );
        let (code, output) = fixture.run(&[
            "--deny-warnings",
            "validate",
            "--bevy-path",
            bevy.to_str().unwrap(),
        ]);
        if matches {
            assert_eq!(code, 0, "{output}");
        } else {
            assert_ne!(code, 0, "later overload must be checked: {output}");
            assert!(output.contains("E006"), "{output}");
        }
    }
}

#[test]
fn signature_exceptions_must_declare_the_complete_return_mapping() {
    let bevy = pinned_bevy_source();
    for (spec, matches) in [
        ("'tick'", false),
        (
            "{ name = 'tick', expected_params = { delta = 'Duration' } }",
            false,
        ),
        ("{ name = 'tick', expected_return = 'PyResult' }", false),
        ("{ name = 'tick', expected_return = 'PyResult<()>' }", true),
    ] {
        let fixture = Fixture::new(
            r#"
#[pyclass(name = "Timer", module = "pybevy.time")]
pub struct PyTimer;
#[pymethods]
impl PyTimer {
    fn tick(&mut self, delta: Duration) -> PyResult<()> { todo!() }
}
"#,
            "class Timer: ...\n",
            &format!(
                "[bevy.crate_mappings]\ntime = 'bevy_time'\n[bevy.type_ignores.Timer]\nintentional_signature_diffs = [{spec}]\n"
            ),
        );
        let (code, output) = fixture.run(&[
            "compare",
            "time",
            "--bevy-path",
            bevy.to_str().unwrap(),
            "--check-signatures",
            "--check-usage",
            "false",
        ]);
        assert!(output.contains("tick"), "comparison must run: {output}");
        if matches {
            assert_eq!(code, 0, "complete declared mapping must pass: {output}");
        } else {
            assert_ne!(code, 0, "incomplete exception must fail: {output}");
            assert!(output.contains("return"), "{output}");
        }
    }
}

#[test]
fn public_dunder_presence_is_validated_in_both_directions() {
    let bevy = pinned_bevy_source();
    for (native_method, stub_method, diagnostic, method) in [
        (
            "fn __eq__(&self, other: &Self) -> bool { todo!() }",
            "",
            "E002",
            "__eq__",
        ),
        (
            "",
            "    def __getitem__(self, index: int) -> float: ...\n",
            "E003",
            "__getitem__",
        ),
    ] {
        let fixture = Fixture::new(
            &format!(
                r#"
#[pyclass(name = "Value", module = "pybevy.math", frozen)]
pub struct PyValue;
#[pymethods]
impl PyValue {{
    {native_method}
}}
"#
            ),
            &format!("class Value:\n{stub_method}    ...\n"),
            "[bevy.crate_mappings]\n",
        );
        let (code, output) = fixture.run(&[
            "--deny-warnings",
            "validate",
            "--bevy-path",
            bevy.to_str().unwrap(),
        ]);
        assert_ne!(code, 0, "dunder drift must fail: {output}");
        assert!(output.contains(diagnostic), "{output}");
        assert!(output.contains(method), "{output}");
    }
}

#[test]
fn compare_binds_generic_self_to_exact_adapter() {
    let bevy = pinned_bevy_source();
    for (method, parameter, parameter_type) in [
        ("from_seconds", "seconds", "f64"),
        ("from_hz", "hz", "f64"),
        ("from_duration", "timestep", "Duration"),
    ] {
        for (return_type, matches) in [
            ("PyResult<Py<PyTimeFixed>>", true),
            ("PyResult<Self>", true),
            ("PyResult<Py<PyTimeReal>>", false),
            ("PyResult<Py<PyTimeVirtual>>", false),
            ("PyResult<()>", false),
        ] {
            let fixture = Fixture::new(
                &format!(
                    r#"
#[pyclass(name = "_TimeFixed", module = "pybevy.time")]
pub struct PyTimeFixed;
#[pymethods]
impl PyTimeFixed {{
    #[staticmethod]
    fn {method}({parameter}: {parameter_type}) -> {return_type} {{ todo!() }}
}}
"#
                ),
                "class _TimeFixed: ...\n",
                "[bevy.crate_mappings]\ntime = 'bevy_time'\n[bevy.type_mappings]\nPyTimeFixed = 'Time<Fixed>'\n",
            );
            let (code, output) = fixture.run(&[
                "compare",
                "time",
                "--bevy-path",
                bevy.to_str().unwrap(),
                "--check-signatures",
                "--check-usage",
                "false",
            ]);
            assert!(
                output.contains("Time<Fixed>"),
                "generic comparison must run: {output}"
            );
            if matches {
                assert_eq!(code, 0, "exact adapter must match Self: {output}");
            } else {
                assert_ne!(code, 0, "unrelated or discarded Self must fail: {output}");
                assert!(output.contains(method), "{output}");
                assert!(output.contains("return"), "{output}");
            }
        }
    }
}
