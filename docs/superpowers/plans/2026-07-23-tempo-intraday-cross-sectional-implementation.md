# Tempo Intraday Cross-Sectional Backtest Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a Rust-backed `tempo` Polars expression for long-only cross-sectional portfolios that can mark and rebalance at arbitrary intraday or cross-day timestamps with optional A-share T+1 settlement.

**Architecture:** Keep `mosaic` unchanged and add an isolated `src/tempo/` module. The Python wrapper normalizes types and derives a private trade-date column; Rust performs a complete layout validation pass, then runs a row-offset state machine over fixed-width datetime blocks and emits one Struct row per datetime.

**Tech Stack:** Rust 2021, Polars 0.54.4, pyo3-polars expression plugins, Python 3.10-3.12, Polars 1.43.x, pytest.

---

## File Structure

- Create `src/tempo/portfolio.rs`: cash, total holding, sellable holding, returns, T+1 release, and deal application.
- Create `src/tempo/engine.rs`: timestamp-slice state machine and snapshots.
- Create `src/tempo/validation.rs`: strict datetime-block and canonical asset-order validation.
- Create `src/tempo/diagnostics.rs`: bounded NaN and overweight counters for verbose logging.
- Create `src/tempo/output.rs`: Struct output construction.
- Create `src/tempo/mod.rs`: Polars plugin entry point and orchestration.
- Modify `src/lib.rs`: register the Rust module.
- Create `polars_bt/tempo.py`: public expression wrapper and type normalization.
- Modify `polars_bt/__init__.py`: export `tempo`.
- Create `tests/reference_tempo.py`: independent readable reference implementation.
- Create `tests/test_tempo.py`: public API, validation, parity, and T+1 integration tests.
- Create `tests/test_tempo_diagnostics.py`: Polars verbose diagnostics integration tests.
- Create `benchmarks/benchmark_tempo.py`: dense multi-timestamp performance guard.
- Modify `README.md`: describe the third engine and its input/output contract.

### Task 1: Portfolio State and Timestamp Engine

**Files:**
- Create: `src/tempo/portfolio.rs`
- Create: `src/tempo/engine.rs`
- Create: `src/tempo/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: Write failing Rust tests for returns-before-trade, multiple same-day rebalances, and T+1**

Add engine tests with two assets and slices such as:

```rust
let slices = vec![
    TimestampSlice {
        trade_date: 20_000,
        weight: &[0.8, 0.0],
        period_ret: &[0.0, 0.0],
        buyable: &[true, true],
        sellable: &[true, true],
        is_rebalance: true,
    },
    TimestampSlice {
        trade_date: 20_000,
        weight: &[0.0, 1.0],
        period_ret: &[0.10, 0.0],
        buyable: &[true, true],
        sellable: &[true, true],
        is_rebalance: true,
    },
];
let outcome = run_slices(&slices, Config::new(2, 0.0, 0.0, true)).unwrap();
assert!((outcome.snapshots[1].nav - 1.08).abs() < 1e-12);
assert!(outcome.snapshots[1].holding_count >= 1);
```

The T+1 assertion must prove the first asset cannot be sold at the second same-day timestamp, while a third slice on a later `trade_date` can sell it.

- [ ] **Step 2: Run the focused test and verify it fails**

Run:

```bash
PYO3_PYTHON=.venv/bin/python cargo test tempo::engine --lib
```

Expected: compilation fails because `tempo::engine` does not yet exist.

- [ ] **Step 3: Implement the minimal portfolio state**

Define:

```rust
pub struct PortfolioState {
    hold: Vec<f64>,
    sellable_hold: Vec<f64>,
    cash: f64,
}
```

Implement:

- `release_t1()` as `sellable_hold.clone_from(&hold)`;
- `apply_returns()` that maps NaN to zero and marks both vectors by `1 + ret`;
- sell deals capped by `sellable_hold`;
- buy deals added only to `hold` for `t1=true`, and to both vectors for `t1=false`;
- finite-state checks that name `time_index`, `asset_index`, `stage`, and field.

- [ ] **Step 4: Implement the minimal timestamp engine**

Define:

```rust
pub struct TimestampSlice<'a> {
    pub trade_date: i32,
    pub weight: &'a [f64],
    pub period_ret: &'a [f64],
    pub buyable: &'a [bool],
    pub sellable: &'a [bool],
    pub is_rebalance: bool,
}
```

For every slice: release T+1 on date change, apply returns, calculate target differences from current NAV, execute sells before buys, enforce stable row-order cash allocation, and emit `Snapshot { cash, nav, turnover, holding_count }`.

- [ ] **Step 5: Run Rust tests and verify they pass**

Run:

```bash
PYO3_PYTHON=.venv/bin/python cargo test tempo::engine --lib
```

Expected: all `tempo::engine` tests pass.

- [ ] **Step 6: Commit the core state machine**

```bash
git add src/tempo src/lib.rs
git commit -m "feat: add tempo portfolio state machine"
```

### Task 2: Strict Layout Validation and Plugin Output

**Files:**
- Create: `src/tempo/validation.rs`
- Create: `src/tempo/output.rs`
- Modify: `src/tempo/mod.rs`

- [ ] **Step 1: Write failing validation tests**

Add tests that construct datetime, trade-date, asset, and rebalance columns and assert exact failures for:

```text
unsorted datetime
duplicate asset in first block
short later block
later block with changed asset order
mixed is_rebalance values inside one block
negative finite weight
infinite weight or period_ret
```

Each error assertion must include the public column name and the offending row or time index.

- [ ] **Step 2: Run the validation tests and verify they fail**

Run:

```bash
PYO3_PYTHON=.venv/bin/python cargo test tempo::validation --lib
```

Expected: compilation fails because the validator is missing.

- [ ] **Step 3: Implement canonical block validation**

Return a compact layout:

```rust
pub struct Layout {
    pub asset_num: usize,
    pub starts: Vec<usize>,
}
```

Infer the first block length from consecutive equal datetime values, reject duplicate first-block assets with a `HashSet<&str>`, then scan subsequent blocks once and compare every asset directly with the canonical first-block value. Require strictly increasing datetime values between blocks and a constant rebalance flag inside every block. Do not sort, hash-join, or repair input.

- [ ] **Step 4: Implement the Polars expression entry point**

The plugin receives eight internal inputs:

```text
datetime, trade_date, asset, weight, period_ret, buyable, sellable, is_rebalance
```

Deserialize:

```rust
pub struct TempoKwargs {
    t1: bool,
    st_fee: f64,
    lg_fee: f64,
}
```

Validate all lengths and null counts, rechunk numeric columns, run layout validation before creating `PortfolioState`, build timestamp slices from `Layout.starts`, and call the engine.

- [ ] **Step 5: Build the per-datetime Struct output**

The output schema must preserve the input datetime dtype:

```text
datetime: input_fields[0].dtype()
cash: Float64
nav: Float64
turnover: Float64
holding_count: Int64
```

Take the datetime value at every validated block start and combine it with snapshots through `StructChunked::from_series`.

- [ ] **Step 6: Run all Rust tests**

Run:

```bash
PYO3_PYTHON=.venv/bin/python cargo test --locked --all-targets
```

Expected: existing Mosaic/Pulse tests and new Tempo tests pass.

- [ ] **Step 7: Commit plugin validation and output**

```bash
git add src/tempo
git commit -m "feat: expose tempo Polars plugin"
```

### Task 3: Python Wrapper and End-to-End Parity

**Files:**
- Create: `polars_bt/tempo.py`
- Modify: `polars_bt/__init__.py`
- Create: `tests/reference_tempo.py`
- Create: `tests/test_tempo.py`

- [ ] **Step 1: Write failing public API tests**

Create a dense two-asset DataFrame with three timestamps on day one and one timestamp on day two. Assert:

```python
result = frame.select(
    tempo(
        "datetime",
        "asset",
        "weight",
        "period_ret",
        "buyable",
        "sellable",
        "is_rebalance",
        t1=True,
        st_fee=0.0,
        lg_fee=0.0,
    ).alias("result")
).unnest("result")

assert result.columns == ["datetime", "cash", "nav", "turnover", "holding_count"]
assert result.height == frame["datetime"].n_unique()
```

Also assert exact rejection messages for changed asset order, incomplete blocks, mixed rebalance flags, negative weights, Boolean nulls, and ±Inf.

- [ ] **Step 2: Run the focused pytest and verify it fails**

Run:

```bash
.venv/bin/python -m pytest tests/test_tempo.py -q
```

Expected: import fails because `polars_bt.tempo` is not exported.

- [ ] **Step 3: Implement the Python expression wrapper**

Create helpers equivalent to Mosaic and register:

```python
datetime_expr = _expr(datetime).cast(pl.Datetime("us"))
return register_plugin_function(
    args=[
        datetime_expr,
        datetime_expr.dt.date(),
        _expr(asset).cast(pl.String),
        _numeric_expr(weight),
        _numeric_expr(period_ret),
        _expr(buyable).cast(pl.Boolean),
        _expr(sellable).cast(pl.Boolean),
        _expr(is_rebalance).cast(pl.Boolean),
    ],
    plugin_path=LIB,
    function_name="tempo",
    kwargs={"t1": t1, "st_fee": st_fee, "lg_fee": lg_fee},
    is_elementwise=False,
    changes_length=True,
)
```

Export `tempo` from `polars_bt.__init__`.

- [ ] **Step 4: Add an independent Python reference implementation**

Use ordinary Python lists and fixed-width datetime blocks. Preserve the production order: date release, return marking, sell-before-buy, T+1 availability, stable row-order buys, then snapshot. Do not call production helpers from the reference.

- [ ] **Step 5: Add parity and null/NaN tests**

Compare Rust output with the reference for:

- T+1 and T+0;
- same-day multiple rebalances;
- non-rebalance marking timestamps;
- overnight date transition;
- blocked buys and sells;
- nonzero buy and sell fees;
- `weight` and `period_ret` null inputs versus explicit NaN inputs.

Use `polars.testing.assert_frame_equal` with a numerical tolerance.

- [ ] **Step 6: Build the extension and run focused integration tests**

Run:

```bash
make install-release
.venv/bin/python -m pytest tests/test_tempo.py -q
```

Expected: all Tempo integration tests pass.

- [ ] **Step 7: Commit the public API**

```bash
git add polars_bt/tempo.py polars_bt/__init__.py tests/reference_tempo.py tests/test_tempo.py
git commit -m "feat: add tempo Python expression API"
```

### Task 4: Verbose Diagnostics

**Files:**
- Create: `src/tempo/diagnostics.rs`
- Modify: `src/tempo/mod.rs`
- Create: `tests/test_tempo_diagnostics.py`

- [ ] **Step 1: Write failing subprocess diagnostics tests**

Run Tempo inside a child Python process with and without `pl.Config(verbose=True)`. The verbose case must contain:

```text
[polars-bt][tempo][INFO] event=start
[polars-bt][tempo][WARN] event=input_summary
code=NAN_WEIGHT
code=NAN_PERIOD_RET
[polars-bt][tempo][INFO] event=finish
```

The quiet case must not contain `[polars-bt][tempo]`.

- [ ] **Step 2: Run diagnostics tests and verify they fail**

Run:

```bash
.venv/bin/python -m pytest tests/test_tempo_diagnostics.py -q
```

Expected: assertions fail because Tempo emits no diagnostics yet.

- [ ] **Step 3: Implement bounded diagnostics**

Record counts and first locations only for:

```text
NAN_WEIGHT
NAN_PERIOD_RET
RETURN_BELOW_MINUS_ONE
OVERWEIGHT_TIMESTAMP
```

Emit start metadata with rows, timestamps, and assets; emit summaries and first locations only when verbose mode is enabled; always keep ±Inf, invalid state, and structural validation as hard errors.

- [ ] **Step 4: Run diagnostics and all Tempo tests**

Run:

```bash
make install-release
.venv/bin/python -m pytest tests/test_tempo.py tests/test_tempo_diagnostics.py -q
```

Expected: all tests pass and quiet-mode stderr remains empty.

- [ ] **Step 5: Commit diagnostics**

```bash
git add src/tempo tests/test_tempo_diagnostics.py
git commit -m "feat: add tempo verbose diagnostics"
```

### Task 5: Documentation, Benchmark, and Full Verification

**Files:**
- Create: `benchmarks/benchmark_tempo.py`
- Modify: `README.md`

- [ ] **Step 1: Add a representative benchmark**

Generate 2,500 timestamps across 500 trading days with five timestamps per day
and 5,000 assets, for 12.5 million total rows. Time only the eager
`frame.select(tempo(...))` call after one warm-up and print rows, timestamps,
elapsed seconds, and rows per second.

- [ ] **Step 2: Run the benchmark**

Run:

```bash
.venv/bin/python benchmarks/benchmark_tempo.py
```

Expected: one successful result with output height equal to the number of unique datetimes and no IPC or join in the benchmark path.

- [ ] **Step 3: Document Tempo**

Update the engine table, add a minimal same-day multi-rebalance example, and document:

- full `(datetime, asset)` dense blocks;
- strict canonical asset order;
- `period_ret` timing;
- `is_rebalance` semantics;
- NaN normalization;
- optional T+1;
- per-datetime output.

- [ ] **Step 4: Run formatting and the complete project gate**

Run:

```bash
make pre-commit
```

Expected: Rust fmt, Clippy with warnings denied, all Rust tests, all Python tests, and Ruff pass.

- [ ] **Step 5: Inspect the final diff and repository status**

Run:

```bash
git diff --check
git status --short
git log --oneline -6
```

Expected: no whitespace errors; only the intended final documentation/benchmark changes remain before the final commit.

- [ ] **Step 6: Commit documentation and benchmark**

```bash
git add README.md benchmarks/benchmark_tempo.py docs/superpowers/plans/2026-07-23-tempo-intraday-cross-sectional-implementation.md
git commit -m "docs: document tempo backtesting engine"
```
