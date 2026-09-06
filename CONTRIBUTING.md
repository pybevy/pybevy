# Contributing to pybevy

Thank you for your interest in contributing to `pybevy`! Let's make high-performance 3D development in Python more accessible.

## Getting Started

### Prerequisites

- Python 3.12+
- Rust toolchain (stable)
- Poetry for Python dependency management

**Ubuntu/Debian:**
```bash
sudo apt-get install libasound-dev libudev-dev mesa-vulkan-drivers vulkan-tools libwayland-dev
```

### Development Setup

1. **Clone the repository**

   ```bash
   git clone https://github.com/pybevy/pybevy.git
   cd pybevy
   ```

2. **Install dependencies**

   ```bash
   poetry install
   ```

3. **Build the project**

   ```bash
   poetry run maturin develop
   ```

   For optimized builds (slower to compile, faster to run):

   ```bash
   poetry run maturin develop --release
   ```

4. **Build and run tests**

   ```bash
   make test
   ```

### Running Checks Manually

**Linting:**
```bash
poetry run ruff check tests/
```

**Type checking:**
```bash
poetry run mypy tests/
```

**Rust formatting:**
```bash
cargo fmt --check
```

**Tests (single-threaded):**
```bash
poetry run pytest
```

**Tests (parallel):**
```bash
poetry run pytest -n auto
```

**All checks (build + test + lint):**
```bash
make test
```

## How to Contribute

### Code Standards

- **Python**: Follow PEP 8, use type hints everywhere, no `Any` types
- **Rust**: Follow standard Rust conventions, all imports at top of file
- Ensure `ruff check` and `mypy` pass with zero errors
- Ensure all tests pass with `poetry run pytest`

### Typings

When introducing new Python-exposed types or functions in Rust, update the relevant `.pyi` stub files to ensure type hints are available for users.

## Areas for Contribution

- **API Development**: Expanding Python bindings for Bevy features
- **Documentation**: Examples, tutorials, API documentation
- **Testing**: Unit tests, integration tests, benchmarks
- **Examples**: Showcase projects demonstrating pybevy capabilities

## Development Notes

- This is a hybrid Rust/Python project using PyO3 and Maturin
- Hot-reload functionality is a core feature - preserve it in changes
- Performance is critical - profile changes that affect the Python-Rust boundary

## Getting Help

- Join the [Pybevy Discord](https://discord.gg/hA4zUneA8f) to ask questions and chat
- Check existing issues and documentation

## License

By contributing, you agree that your contributions will be licensed under the same MIT/Apache-2.0 dual license as the project.
