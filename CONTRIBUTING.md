# Contributing

## Development environment

Use CPython 3.10, 3.11, or 3.12 with Polars 1.43.x and the repository's Rust
nightly toolchain.

```bash
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python \
  'polars>=1.43,<1.44' maturin pytest ruff mypy
make install-release
```

## Checks

Run the complete local gate before submitting a change:

```bash
make pre-commit
```

For performance-sensitive Mosaic changes, also run:

```bash
.venv/bin/python benchmarks/benchmark_mosaic.py
```

The accepted scale is 2,500 days by 5,000 assets. The timed expression boundary
must remain at or below five seconds.

## Design rules

- Keep `pulse` and `mosaic` isolated; do not mix their state models.
- Preserve Mosaic's dense, date-major, fixed-width input contract.
- Do not add asset joins or hashing inside the Rust daily loop.
- Preserve sell-before-buy order and the strict cumulative cash threshold.
- Add a failing test before changing behavior, then verify it passes.
- Keep Python Polars and Rust Polars/pyo3-polars versions ABI-aligned.

## Pull requests

Describe the behavior change, include the exact verification commands, and
report the 12.5-million-row timing when the Mosaic hot path changes.
