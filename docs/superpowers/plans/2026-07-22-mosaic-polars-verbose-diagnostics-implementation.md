# Mosaic Polars Verbose Diagnostics Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add bounded Rust-side `mosaic` diagnostics controlled by Polars verbose mode while preserving the Python API, five-field result schema, valid-input semantics, and production benchmark.

**Architecture:** A shared `src/diagnostics.rs` module renders and emits deterministic stderr records. A `src/mosaic/diagnostics.rs` module performs the optional O(N) input inspection and stores bounded counters, while the existing portfolio path gains contextual finite-state guards and explicit halt metadata. The plugin boundary reads Polars verbose once, runs diagnostics only when enabled, executes the same state machine, and emits one compact report.

**Tech Stack:** Rust 2021, Polars 0.54.4, pyo3-polars 0.27, Python 3.10-3.12, pytest, maturin.

---

## File Map

- Create `src/diagnostics.rs`: shared verbose detection, log level, deterministic record rendering, and stderr emission.
- Modify `src/lib.rs`: expose the internal diagnostics module.
- Create `src/mosaic/diagnostics.rs`: bounded `mosaic` input counters, first locations, input inspection, and report rendering.
- Modify `src/mosaic/portfolio.rs`: return contextual errors when state arithmetic becomes non-finite.
- Modify `src/mosaic/engine.rs`: propagate checked portfolio operations and return snapshots plus negative-cash halt metadata.
- Modify `src/mosaic/mod.rs`: orchestrate optional inspection, execution, and verbose logging without changing output.
- Modify `tests/test_mosaic.py`: pin existing tolerated semantics and unchanged output schema.
- Create `tests/test_mosaic_diagnostics.py`: subprocess-level Polars verbose and stderr integration tests.
- Modify `README.md`: document diagnostics, log format, and asset-identity limitation.

### Task 1: Shared Polars verbose logger

**Files:**
- Create: `src/diagnostics.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing logger rendering tests**

Create `src/diagnostics.rs` with tests first:

```rust
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Level {
    Info,
    Warn,
}

impl Level {
    fn as_str(self) -> &'static str {
        match self {
            Self::Info => "INFO",
            Self::Warn => "WARN",
        }
    }
}

pub fn render_record(
    component: &str,
    level: Level,
    event: &str,
    fields: &[(&str, String)],
) -> String {
    let mut output = format!("[polars-bt][{component}][{}] event={event}", level.as_str());
    for (key, value) in fields {
        output.push(' ');
        output.push_str(key);
        output.push('=');
        output.push_str(value);
    }
    output
}

#[cfg(test)]
mod tests {
    use super::{render_record, Level};

    #[test]
    fn renders_deterministic_key_value_record() {
        assert_eq!(
            render_record(
                "mosaic",
                Level::Warn,
                "halt",
                &[("reason", "NEGATIVE_CASH".into()), ("day_index", "7".into())],
            ),
            "[polars-bt][mosaic][WARN] event=halt reason=NEGATIVE_CASH day_index=7"
        );
    }
}
```

- [ ] **Step 2: Run the focused test and verify the module is not compiled yet**

Run: `cargo test diagnostics::tests::renders_deterministic_key_value_record -- --exact`

Expected: FAIL because `src/lib.rs` does not declare `diagnostics`.

- [ ] **Step 3: Expose the module and add the Polars integration**

Add to `src/lib.rs`:

```rust
mod diagnostics;
```

Add to `src/diagnostics.rs`:

```rust
pub fn verbose_enabled() -> bool {
    let config = polars_config::config();
    config.reload_env_var("POLARS_VERBOSE");
    config.verbose()
}

pub fn emit(component: &str, level: Level, event: &str, fields: &[(&str, String)]) {
    eprintln!("{}", render_record(component, level, event, fields));
}
```

- [ ] **Step 4: Verify the logger**

Run: `cargo test diagnostics::tests -- --nocapture`

Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/diagnostics.rs src/lib.rs
git commit -m "feat: add Polars verbose diagnostics logger"
```

### Task 2: Bounded Mosaic input diagnostics

**Files:**
- Create: `src/mosaic/diagnostics.rs`
- Modify: `src/mosaic/mod.rs`

- [ ] **Step 1: Write failing report tests**

Define the bounded report types and test fixtures in `src/mosaic/diagnostics.rs`:

```rust
use polars::prelude::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub day_index: usize,
    pub asset_index: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counter {
    pub count: usize,
    pub first: Option<Location>,
}

impl Counter {
    fn record(&mut self, day_index: usize, asset_index: Option<usize>) {
        self.count += 1;
        self.first.get_or_insert(Location {
            day_index,
            asset_index,
        });
    }
}

#[derive(Clone, Debug, Default)]
pub struct MosaicDiagnosticReport {
    pub nan_weight: Counter,
    pub nan_ovn_ret: Counter,
    pub nan_ind_ret: Counter,
    pub inf_weight: Counter,
    pub inf_ovn_ret: Counter,
    pub inf_ind_ret: Counter,
    pub invalid_prev_close: Counter,
    pub invalid_vwap: Counter,
    pub negative_weight: Counter,
    pub return_below_minus_one: Counter,
    pub mixed_date_blocks: Counter,
    pub mixed_rebalance_blocks: Counter,
    pub overweight_days: Counter,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counters_keep_only_the_first_location() {
        let mut counter = Counter::default();
        counter.record(3, Some(2));
        counter.record(8, Some(7));
        assert_eq!(counter.count, 2);
        assert_eq!(counter.first, Some(Location { day_index: 3, asset_index: Some(2) }));
    }
}
```

Declare `mod diagnostics;` inside `src/mosaic/mod.rs`.

- [ ] **Step 2: Run the focused report tests**

Run: `cargo test mosaic::diagnostics::tests -- --nocapture`

Expected: PASS for the bounded counter, before input inspection exists.

- [ ] **Step 3: Add failing table-driven inspection tests**

Add a `DiagnosticInputs<'a>` struct containing the five float slices and the
`is_rebalance` Boolean slice plus `&Series` dates. Add tests that construct two 2-asset days
and assert exact counts and first locations for all categories. Include:

```rust
#[test]
fn inspects_nan_infinity_prices_dates_rebalance_and_weights() {
    let dates = Series::new("date".into(), [1_i32, 2, 3, 3]);
    let report = inspect_inputs(
        &dates,
        DiagnosticInputs {
            weight: &[f64::NAN, -0.1, 0.7, 0.7],
            ovn_ret: &[f64::INFINITY, 0.0, -1.1, 0.0],
            ind_ret: &[0.0, f64::NAN, 0.0, f64::NEG_INFINITY],
            prev_close: &[10.0, 0.0, 10.0, f64::NAN],
            vwap: &[10.0, 10.0, -1.0, f64::INFINITY],
            is_rebalance: &[false, true, true, true],
        },
        2,
    )
    .unwrap();

    assert_eq!(report.nan_weight.count, 1);
    assert_eq!(report.inf_ovn_ret.count, 1);
    assert_eq!(report.nan_ind_ret.count, 1);
    assert_eq!(report.inf_ind_ret.count, 1);
    assert_eq!(report.invalid_prev_close.count, 2);
    assert_eq!(report.invalid_vwap.count, 2);
    assert_eq!(report.negative_weight.count, 1);
    assert_eq!(report.return_below_minus_one.count, 1);
    assert_eq!(report.mixed_date_blocks.count, 1);
    assert_eq!(report.mixed_rebalance_blocks.count, 1);
    assert_eq!(report.overweight_days.count, 1);
}
```

- [ ] **Step 4: Implement one-pass inspection**

Implement `inspect_inputs` with fixed-width loops. For each row, classify NaN
before infinity, count invalid prices as `!value.is_finite() || value <= 0.0`,
and count returns below `-1.0` only when finite. For each block:

```rust
let first_date = dates.get(start)?;
let first_different = (1..asset_num)
    .find(|offset| dates.get(start + offset).ok().as_ref() != Some(&first_date));
if let Some(asset_index) = first_different {
    report
        .mixed_date_blocks
        .record(day_index, Some(asset_index));
}

let rebalance = &inputs.is_rebalance[start..end];
if rebalance.iter().any(|value| *value) && rebalance.iter().any(|value| !*value) {
    let asset_index = rebalance
        .iter()
        .position(|value| *value != rebalance[0])
        .unwrap();
    report
        .mixed_rebalance_blocks
        .record(day_index, Some(asset_index));
}
```

On rebalance blocks, sum weights after applying the existing NaN-to-zero rule;
record `OVERWEIGHT_DAY` when the sum is finite and exceeds `1.0 + 1e-8`.

- [ ] **Step 5: Add deterministic summary rendering tests and implementation**

Add methods that return ordered fields and ordered first-location records:

```rust
pub struct FirstDiagnostic {
    pub code: &'static str,
    pub location: Location,
}

impl MosaicDiagnosticReport {
    fn entries(&self) -> [(&'static str, &'static str, &Counter); 13] {
        [
            ("nan_weight", "NAN_WEIGHT", &self.nan_weight),
            ("nan_ovn_ret", "NAN_OVN_RET", &self.nan_ovn_ret),
            ("nan_ind_ret", "NAN_IND_RET", &self.nan_ind_ret),
            ("inf_weight", "INF_WEIGHT", &self.inf_weight),
            ("inf_ovn_ret", "INF_OVN_RET", &self.inf_ovn_ret),
            ("inf_ind_ret", "INF_IND_RET", &self.inf_ind_ret),
            ("invalid_prev_close", "INVALID_PREV_CLOSE", &self.invalid_prev_close),
            ("invalid_vwap", "INVALID_VWAP", &self.invalid_vwap),
            ("negative_weight", "NEGATIVE_WEIGHT", &self.negative_weight),
            (
                "return_below_minus_one",
                "RETURN_BELOW_MINUS_ONE",
                &self.return_below_minus_one,
            ),
            ("mixed_date_blocks", "MIXED_DATE_BLOCK", &self.mixed_date_blocks),
            (
                "mixed_rebalance_blocks",
                "MIXED_REBALANCE_BLOCK",
                &self.mixed_rebalance_blocks,
            ),
            ("overweight_days", "OVERWEIGHT_DAY", &self.overweight_days),
        ]
    }

    pub fn summary_fields(&self) -> Vec<(&'static str, String)> {
        self.entries()
            .into_iter()
            .filter(|(_, _, counter)| counter.count > 0)
            .map(|(field, _, counter)| (field, counter.count.to_string()))
            .collect()
    }

    pub fn first_diagnostics(&self) -> Vec<FirstDiagnostic> {
        self.entries()
            .into_iter()
            .filter_map(|(_, code, counter)| {
                counter.first.map(|location| FirstDiagnostic { code, location })
            })
            .collect()
    }
}
```

Tests must assert zero counters are omitted and code order follows the design
table, independent of observation order.

- [ ] **Step 6: Verify and commit**

Run: `cargo test mosaic::diagnostics::tests -- --nocapture`

Expected: PASS.

```bash
git add src/mosaic/diagnostics.rs src/mosaic/mod.rs
git commit -m "feat: inspect mosaic inputs in verbose mode"
```

### Task 3: Contextual finite-state errors and halt metadata

**Files:**
- Modify: `src/mosaic/portfolio.rs`
- Modify: `src/mosaic/engine.rs`

- [ ] **Step 1: Write failing engine tests**

Add tests proving infinity reports day and stage, and negative cash remains a
successful truncated outcome:

```rust
#[test]
fn rejects_non_finite_overnight_state_with_context() {
    let days = [DailySlice {
        weight: &[0.4],
        ovn_ret: &[f64::INFINITY],
        ind_ret: &[0.0],
        buyable: &[true],
        sellable: &[true],
        prev_close: &[10.0],
        vwap: &[10.0],
        is_rebalance: &[false],
    }];
    let error = run_slices(&days, Config::new(1, 0.0, 0.0)).unwrap_err();
    let message = error.to_string();
    assert!(message.contains("day_index=0"));
    assert!(message.contains("stage=overnight_settlement"));
    assert!(message.contains("reason=NONFINITE_HOLDING"));
}

#[test]
fn reports_negative_cash_halt_without_dropping_the_snapshot() {
    let outcome = run_slices(&negative_cash_days(), Config::new(1, 3.0, 0.0)).unwrap();
    assert_eq!(outcome.snapshots.len(), 2);
    assert_eq!(outcome.halt.unwrap().day_index, 1);
}
```

- [ ] **Step 2: Run tests and verify compilation failure**

Run: `cargo test mosaic::engine::tests -- --nocapture`

Expected: FAIL because `run_slices` still returns `Vec<DailySnapshot>` and
portfolio operations do not report non-finite arithmetic.

- [ ] **Step 3: Add checked portfolio arithmetic**

Change portfolio methods to accept day/stage context and return
`PolarsResult`. Use one helper for the stable message:

```rust
fn ensure_finite(
    value: f64,
    day_index: usize,
    asset_index: Option<usize>,
    stage: &'static str,
    reason: &'static str,
    field: &'static str,
) -> PolarsResult<f64> {
    if value.is_finite() {
        Ok(value)
    } else {
        let asset = asset_index
            .map(|index| format!(" asset_index={index}"))
            .unwrap_or_default();
        polars_bail!(ComputeError:
            "polars-bt mosaic failed: day_index={day_index}{asset} stage={stage} reason={reason} {field}={value}"
        )
    }
}
```

Check updated holdings inside the existing return and deal loops, target
differences while they are built, cash after deals, and final NAV/turnover.
Do not add a separate holdings pass.

- [ ] **Step 4: Return explicit outcome metadata**

Define:

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Halt {
    pub day_index: usize,
    pub cash: f64,
}

pub struct RunOutcome {
    pub snapshots: Vec<DailySnapshot>,
    pub halt: Option<Halt>,
}
```

Change `run_slices` to enumerate days, propagate checked operations with `?`,
set `halt` immediately after retaining the negative-cash snapshot, and return
`RunOutcome`.

- [ ] **Step 5: Update existing Rust tests and verify**

Replace existing `run_slices(...).unwrap()` uses with
`run_slices(...).unwrap().snapshots`. Preserve every numeric assertion.

Run: `cargo test mosaic -- --nocapture`

Expected: all Mosaic Rust tests PASS.

- [ ] **Step 6: Commit**

```bash
git add src/mosaic/portfolio.rs src/mosaic/engine.rs
git commit -m "feat: guard mosaic portfolio state"
```

### Task 4: Wire diagnostics through the plugin

**Files:**
- Modify: `src/mosaic/mod.rs`
- Modify: `tests/test_mosaic.py`
- Create: `tests/test_mosaic_diagnostics.py`

- [ ] **Step 1: Pin tolerated semantics in Python tests**

Add explicit tests to `tests/test_mosaic.py` for NaN-to-zero, mixed rebalance,
invalid-price skipping, negative weights, negative-cash truncation, and exact
five-field schema. Expected frames must be computed from
`tests.reference_ptrader.reference_ptrader`, not duplicated formulas.

Run: `.venv/bin/python -m pytest tests/test_mosaic.py -q`

Expected: PASS before wiring diagnostics, proving the baseline.

- [ ] **Step 2: Write failing subprocess logging tests**

Create `tests/test_mosaic_diagnostics.py` with a helper that executes
`sys.executable -c <script>` from a temporary working directory while setting
`PYTHONPATH` only when testing a development install. Add tests asserting:

```python
assert "[polars-bt][mosaic]" not in quiet.stderr
assert "[polars-bt][mosaic][INFO] event=start" in verbose.stderr
assert "[polars-bt][mosaic][WARN] event=input_summary" in verbose.stderr
assert "code=MIXED_REBALANCE_BLOCK" in verbose.stderr
assert "[polars-bt][mosaic][INFO] event=finish" in verbose.stderr
```

Use one script with `pl.Config.set_verbose(True)` and a second with
`with pl.Config(verbose=True)`. Keep fixtures under ten rows.

Run: `.venv/bin/python -m pytest tests/test_mosaic_diagnostics.py -q`

Expected: FAIL because `mosaic` emits no package diagnostics.

- [ ] **Step 3: Wire start, input report, halt, and finish records**

In `src/mosaic/mod.rs`:

```rust
let verbose = crate::diagnostics::verbose_enabled();
if verbose {
    crate::diagnostics::emit(
        "mosaic",
        Level::Info,
        "start",
        &[
            ("rows", row_count.to_string()),
            ("days", day_count.to_string()),
            ("assets", kwargs.asset_num.to_string()),
        ],
    );
}
```

When verbose, call `inspect_inputs`, emit one `input_summary` only when it has
nonzero fields, then emit one `first` record per returned first diagnostic.
After `run_slices`, emit `halt` if present and always emit `finish` on success.
Use `outcome.snapshots` for dates and output construction.

- [ ] **Step 4: Build the development extension and verify integration**

Run: `make install-release`

Expected: release extension builds and installs into `.venv`.

Run: `.venv/bin/python -m pytest tests/test_mosaic.py tests/test_mosaic_diagnostics.py -q`

Expected: all tests PASS.

- [ ] **Step 5: Commit**

```bash
git add src/mosaic/mod.rs tests/test_mosaic.py tests/test_mosaic_diagnostics.py
git commit -m "feat: expose mosaic diagnostics through Polars verbose"
```

### Task 5: Documentation and full verification

**Files:**
- Modify: `README.md`

- [ ] **Step 1: Document diagnostics**

Add a `Mosaic diagnostics` subsection after the input contract with:

```python
with pl.Config(verbose=True):
    daily = panel.select(mosaic(...).alias("daily")).unnest("daily")
```

Document `POLARS_VERBOSE=1`, stderr, bounded summary behavior, unchanged result
schema, hard non-finite-state errors, and the inability to validate cross-day
asset order without an asset identifier.

- [ ] **Step 2: Run formatting and static checks**

Run:

```bash
cargo fmt --all -- --check
cargo clippy --locked --package polars_bt --all-targets --all-features -- -D warnings
.venv/bin/python -m ruff check polars_bt tests benchmarks
```

Expected: all commands exit 0.

- [ ] **Step 3: Run Rust and Python suites**

Run:

```bash
export POLARS_BT_PYTHON_LIBDIR="$(.venv/bin/python -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))')"
PYO3_PYTHON=.venv/bin/python LD_LIBRARY_PATH="$POLARS_BT_PYTHON_LIBDIR" cargo test --locked --all-targets
.venv/bin/python -m pytest tests -q
```

Expected: all tests PASS.

- [ ] **Step 4: Run the accepted benchmark**

Run: `.venv/bin/python benchmarks/benchmark_mosaic.py`

Expected: `rows=12,500,000`, `output_days=2500`, and
`target_passed=true` with verbose disabled.

- [ ] **Step 5: Confirm public compatibility surfaces**

Run:

```bash
.venv/bin/python - <<'PY'
import inspect
import polars_bt

assert list(inspect.signature(polars_bt.mosaic).parameters) == [
    "date", "weight", "ovn_ret", "ind_ret", "buyable", "sellable",
    "prev_close", "vwap", "is_rebalance", "asset_num", "st_fee", "lg_fee",
]
assert polars_bt.__all__ == ["mosaic", "pulse"]
PY
```

Expected: exit 0.

- [ ] **Step 6: Commit**

```bash
git add README.md
git commit -m "docs: explain mosaic verbose diagnostics"
```

- [ ] **Step 7: Final worktree audit**

Run: `git status --short && git log -6 --oneline`

Expected: clean worktree and the planned diagnostic commits in order.
