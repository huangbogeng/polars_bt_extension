# Mosaic Nullable Numeric Inputs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Accept nullable Mosaic numeric inputs without erasing market-data missingness or leaking `cont_slice()` errors.

**Architecture:** Normalize Polars nulls to NaN at the public Python expression boundary. Keep existing field-specific NaN behavior in Rust, and add a named residual-null guard immediately before contiguous slice extraction.

**Tech Stack:** Python 3.10–3.12, Polars 1.43, Rust, pyo3-polars, pytest, Cargo test, maturin, GitHub Actions, PyPI Trusted Publishing.

---

### Task 1: Lock the nullable-input behavior with failing tests

**Files:**
- Modify: `tests/test_mosaic.py`
- Modify: `src/mosaic/mod.rs`

- [ ] **Step 1: Add a Python regression test for all numeric inputs**

Parameterize `weight`, `ovn_ret`, `ind_ret`, `prev_close`, and `vwap`. For each field, compare a panel containing a Polars null with the same panel containing NaN and require identical Mosaic output.

- [ ] **Step 2: Run the Python regression test and verify RED**

Run: `.venv/bin/python -m pytest tests/test_mosaic.py -k nullable -vv`

Expected: each nullable case fails with the plugin's current `chunked array is not contiguous` error.

- [ ] **Step 3: Add a Rust unit test for the named residual-null guard**

Create a nullable `Float64` series and require the helper error to contain `mosaic numeric input weight cannot contain nulls`.

- [ ] **Step 4: Run the Rust test and verify RED**

Run: `PYO3_PYTHON=.venv/bin/python LD_LIBRARY_PATH=$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))') cargo test --locked rejects_nullable_numeric_input -- --exact`

Expected: compilation fails because the guard helper does not exist yet.

### Task 2: Implement lossless normalization and precise validation

**Files:**
- Modify: `polars_bt/mosaic.py`
- Modify: `src/mosaic/mod.rs`

- [ ] **Step 1: Normalize nulls to NaN in the Python wrapper**

Add a focused `_numeric_expr` helper equivalent to `_expr(value).cast(pl.Float64).fill_null(float("nan"))`, and use it for exactly the five numeric Mosaic inputs. Do not call `fill_nan`.

- [ ] **Step 2: Add the Rust named residual-null guard**

Check `null_count()` after dtype validation and before `cont_slice()`. Include the public argument name in the `ComputeError` message.

- [ ] **Step 3: Rebuild the extension and verify GREEN**

Run: `make install-release && .venv/bin/python -m pytest tests/test_mosaic.py -k nullable -vv`

Expected: all five nullable cases pass.

- [ ] **Step 4: Run the focused Rust test and verify GREEN**

Run the focused Cargo test from Task 1 and require exit code 0.

### Task 3: Prepare and verify release 0.2.1

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `tests/test_import.py`
- Modify: `CHANGELOG.md`

- [ ] **Step 1: Bump the package version**

Set the crate and expected installed version to `0.2.1`, update the lockfile, and add a `0.2.1` changelog entry describing nullable numeric support and precise Rust errors.

- [ ] **Step 2: Run full verification**

Run: `make pre-commit && env -u POLARS_VERBOSE .venv/bin/python benchmarks/benchmark_mosaic.py && git diff --check`

Expected: Rust tests, Python tests, formatting, Clippy, Ruff, and the 12.5-million-row benchmark all pass; benchmark remains below five seconds.

- [ ] **Step 3: Commit and push main**

Commit only this repository's changes and push `main` after checking a clean diff.

- [ ] **Step 4: Publish and verify PyPI**

After main CI succeeds, create GitHub Release `v0.2.1`. Require the Release workflow and public PyPI JSON/import verification to succeed before reporting completion.
