use pybevy_core::build_info::{BuildInfo, install_build_info};
use pyo3::prelude::*;

mod compiled {
    include!(concat!(env!("OUT_DIR"), "/build_info.rs"));
}

#[pymodule(gil_used = false)]
fn _pybevy(m: &Bound<'_, PyModule>) -> PyResult<()> {
    install_build_info(BuildInfo {
        git_sha: compiled::GIT_SHA,
        git_sha_short: compiled::GIT_SHA_SHORT,
        git_dirty: compiled::GIT_DIRTY,
        profile: compiled::PROFILE,
    });
    pybevy::init_module(m)
}
