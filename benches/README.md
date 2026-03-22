# Benchmarks

```bash
poetry run maturin develop --release
```

- `ecs/`, `image/`, `math/`, `mesh/` — pytest-benchmark suites (`poetry run pytest benches/ --benchmark-only`)
- `paper/` — Reproducible benchmarks for the architecture paper (see [paper/README.md](paper/README.md))
