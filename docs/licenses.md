# Dependency Licenses

## Rust (cargo-deny enforced)

All Rust dependencies are checked by `cargo deny check`. See `deny.toml` for the full configuration.

## Python Runtime Dependencies

| Package | License | Notes |
|---------|---------|-------|
| click ~=8.3 | BSD-3-Clause | CLI framework |
| watchfiles ~=1.1 | MIT | File watching for hot reload |
| numpy >=2.3.2 | BSD-3-Clause | Array operations |
| httpx >=0.27 | BSD-3-Clause | HTTP client |

## Python Optional Dependencies

| Package | License | Notes |
|---------|---------|-------|
| numba >=0.62.1 | BSD-2-Clause | JIT compilation (`pybevy[numba]`) |
| jax >=0.4.30 | Apache-2.0 | Accelerated computing (`pybevy[jax]`) |
| jaxlib >=0.4.30 | Apache-2.0 | JAX backend (`pybevy[jax]`) |

## Python Dev Dependencies

| Package | License |
|---------|---------|
| pytest | MIT |
| maturin | MIT/Apache-2.0 |
| mypy | MIT |
| ruff | MIT |
| pytest-benchmark | BSD-2-Clause |
| pytest-xdist | MIT |
| pillow | HPND |
| jax[cpu] | Apache-2.0 |

