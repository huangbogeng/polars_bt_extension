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

## CI pipeline

Fast checks run Rustfmt, Ruff, actionlint, and release-tool unit tests before
starting expensive compilation. Rust checks and the manylinux2014 wheel build
then run in parallel. The same wheel is installed outside the source checkout
for all ten Python/Polars matrix entries. Rust debug symbols are disabled for
checks; the release wheel retains the existing release profile.

Distribution verification runs after Rust and wheel tests succeed. It checks
wheel/sdist filenames and versions, then records SHA256, repository, commit,
CI run ID, and run attempt in `release-manifest.json`. Both distributions and
the manifest are retained for 30 days as
`verified-distributions-COMMIT-ATTEMPT`. `CI gate` rejects failed, cancelled,
or skipped required stages; use this stable name for a required status check.

Runners are pinned to Ubuntu 24.04. The native dependency cache is populated
by main pushes and restored by PRs; manylinux builds retain sccache and its
hit/miss statistics. Each Python/Polars job has its own uv cache suffix to
avoid concurrent writes to the same cache key. A cold cache still requires a
full native build.

CI helpers use Python 3.11+ and only the standard library. Their tests always
run on Python 3.12 in the fast-check job. They are skipped by local pre-commit
on Python 3.10; native engine and wheel tests still run on that version.

```bash
.venv/bin/python -m pytest tools/tests -q
# Install actionlint 1.7.12, then:
actionlint
```

## Releases

1. Update the Cargo package version, lockfile, version test, and changelog.
2. Run the local checks, build the release wheel and sdist, and verify installation.
3. Merge the PR, then wait for the exact main push commit's complete `CI gate`.
4. Optionally rehearse artifact selection and verification without uploading:
   `gh workflow run release.yml --ref main`.
5. Create a `vX.Y.Z` GitHub Release targeting that exact tested commit.
6. Release selects a successful **push-to-main** run of `ci.yml` for that commit,
   requires its successful `CI gate`, and downloads its verified artifact by
   artifact ID. It verifies version, provenance, filenames, sizes and hashes,
   then uploads only those exact wheel/sdist bytes through PyPI Trusted Publishing.
7. Verify the wheel and sdist on PyPI and test an isolated installation.

Release does not rebuild or rerun the matrix. PR artifacts and manual CI runs
are not eligible publishing sources. If an artifact expires or a run is
cancelled, rerun the **entire** original main push CI (`gh run rerun RUN_ID`),
wait for success, then retry the entire Release workflow. Avoid failed-jobs-only
reruns: each attempt has separately named artifacts so attempts cannot mix.
Commits built before this pipeline have no verified manifest and cannot be
promoted through it; prepare a new release commit with the current workflow.

Manual release dispatches are rehearsals on main and never upload to PyPI.
Only `release.published` enables the separate publish job, which alone has
OIDC write permission and uses the existing `pypi` environment. Keep the
Trusted Publisher configured for `release.yml`; its filename has not changed.

Keep release notes in a file and pass it with `gh release create --notes-file`
to preserve Markdown and actual newlines. Do not publish a different commit
from the one that passed CI.
