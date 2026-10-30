# Contributing

## Development environment

Use CPython 3.10, 3.11, or 3.12 and the repository's Rust 1.95 toolchain.
Python 3.10 supports Polars >=2.0.0,<2.1; Python 3.11–3.12 supports
Polars >=2.0.0,<2.2. Polars 2.1 requires Python >=3.11.

```bash
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python \
  -r requirements.txt mypy
make install-release
```

## Checks

Format sources, then run the complete local gate before submitting a change:

```bash
make fmt
make pre-commit
```

For performance-sensitive engine changes, also run the corresponding benchmark:

```bash
.venv/bin/python benchmarks/benchmark_mosaic.py
.venv/bin/python benchmarks/benchmark_tempo.py
```

Both benchmarks use 12.5 million rows: Mosaic has 2,500 days by 5,000 assets;
Tempo has 500 days by five timestamps by 5,000 assets. The timed expression
boundary must remain at or below five seconds.

## Design rules

- Keep `pulse`, `mosaic`, and `tempo` isolated; do not mix their state models.
- Preserve Mosaic's dense, date-major, fixed-width input contract.
- Do not add asset joins or hashing inside the Rust daily loop.
- Preserve sell-before-buy order and the strict cumulative cash threshold.
- Add a failing test before changing behavior, then verify it passes.
- Keep Python Polars and Rust Polars/pyo3-polars versions ABI-aligned.

CI and release checks install the built wheel outside the source checkout on
each supported Python version, with the minimum Polars version (2.0.0)
and the newest allowed 2.0 patch. Python 3.11–3.12 also test Polars 2.1.0
and the newest allowed 2.1 patch. When upgrading Polars, update the Python
dependency range, verify Rust dependencies and the lockfile, and update
installation instructions and both workflow matrices together. Run the complete
Python suite against the rebuilt wheel; importing the package alone does not
exercise the plugin FFI.

## Pull requests

Describe the behavior change, include the exact verification commands, and
report the 12.5-million-row timing when the Mosaic hot path changes.

## Releases

1. Update the Cargo package version, lockfile, version test, and changelog.
2. Run the local checks, build the release wheel and sdist, and verify installation.
3. Push the release commit and wait for its complete CI run to succeed.
4. Create a `vX.Y.Z` GitHub Release targeting that exact tested commit.
5. Wait for the Release workflow to build and test the distributions, then
   publish through PyPI Trusted Publishing. Verify the wheel and sdist on PyPI.

Keep release notes in a file and pass it with `gh release create --notes-file`
to preserve Markdown and actual newlines. Do not publish a different commit
from the one that passed CI.
