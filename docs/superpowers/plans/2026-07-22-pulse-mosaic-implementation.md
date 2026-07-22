# Pulse and Mosaic Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Preserve the existing T0 engine under `pulse`, add the zero-IPC cross-sectional `mosaic` Polars plugin, and validate it behind alpha-lab's existing ptrader object protocol.

**Architecture:** `pulse` and `mosaic` are separate Python and Rust modules that share only the extension package and allocator. `mosaic` receives a prepared dense panel as expression inputs, scans fixed-width daily slices with a Rust `PortfolioState`, and returns one Struct row per day; alpha-lab's `LofiEx.run()` adapts that daily result back into `position.history`.

**Tech Stack:** Rust nightly, Polars Rust 0.54.4, pyo3-polars 0.27.0, PyO3 0.28, Python Polars 1.43.x, Maturin, pytest, uv.

---

## Repository Roots

- Plugin: `/home/huangbogeng/github.com/huangbogeng/polars_bt_extension`
- Consumer: `/home/huangbogeng/github.com/Aequiludium/alpha-lab`

Do not modify unrelated dirty files in alpha-lab. The alpha-lab checkout already contains unrelated datacenter and alphaboard work.

## File Map

### polars_bt

- Modify `Cargo.toml`: align the Rust plugin ABI with Python Polars 1.43 and bump the package version.
- Modify `pyproject.toml`: constrain the Python Polars ABI range and expose supported Python versions.
- Modify `.github/workflows/ci.yml`: build the renamed package and run the new tests.
- Modify `polars_bt/__init__.py`: export only `pulse` and `mosaic` as engine functions.
- Create `polars_bt/pulse.py`: Python expression wrapper for the existing T0 engine.
- Create `polars_bt/mosaic.py`: Python expression wrapper for the cross-sectional engine.
- Delete `polars_bt/backtest.py`: replaced by `pulse.py`.
- Create `src/pulse/mod.rs`: existing T0 plugin entry and strategy.
- Create `src/pulse/exchange.rs`: existing `LofiEx` T0 exchange state.
- Create `src/pulse/matcher.rs`: existing matchers.
- Create `src/pulse/order.rs`: existing T0 order types.
- Create `src/pulse/position.rs`: existing T0 position types.
- Delete `src/exchange/mod.rs`, `src/exchange/matcher.rs`, `src/order.rs`, and `src/position.rs` after the code is moved without semantic changes.
- Modify `src/lib.rs`: expose the isolated `pulse` and `mosaic` modules.
- Create `src/mosaic/portfolio.rs`: pure Rust portfolio state and arithmetic.
- Create `src/mosaic/engine.rs`: fixed-width daily cross-sectional state machine.
- Create `src/mosaic/output.rs`: daily Struct output construction.
- Create `src/mosaic/mod.rs`: Polars expression entry point and structural checks.
- Create `tests/test_pulse.py`: T0 regression tests.
- Create `tests/reference_ptrader.py`: small pure-Python parity oracle.
- Create `tests/test_mosaic.py`: plugin schema, parity, and error tests.
- Create `benchmarks/benchmark_mosaic.py`: accepted 12.5-million-row benchmark.
- Modify `README.md`: document the two named engines and their different contracts.

### alpha-lab

- Modify `alphamaster/ptrader/core/lofi.py`: route `run()` through locally installed `polars_bt.mosaic` while retaining the object protocol and reference private methods.
- Modify `tests/alphamaster/test_ptrader_engine.py`: add compatibility and Rust parity coverage.
- Do not add an unreleased `polars_bt` lower bound to `pyproject.toml` or `uv.lock`. Build and install the local wheel for integration verification. Dependency declaration is a release operation and requires separate publication authority.

---

### Task 1: Align the Polars plugin ABI

**Files:**
- Modify: `Cargo.toml`
- Modify: `pyproject.toml`
- Create: `tests/test_import.py`

- [ ] **Step 1: Write the ABI smoke test**

```python
import polars as pl


def test_plugin_imports_with_supported_polars():
    import polars_bt

    assert tuple(map(int, pl.__version__.split(".")[:2])) == (1, 43)
    assert polars_bt.__version__ == "0.2.0"
```

- [ ] **Step 2: Create a clean development environment and verify the test fails on the old package metadata**

Run:

```bash
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python 'polars>=1.43,<1.44' maturin pytest ruff mypy
.venv/bin/python -m pytest tests/test_import.py -q
```

Expected: FAIL because the source distribution still reports version `0.1.2` or is not installed.

- [ ] **Step 3: Align Cargo dependencies with the Polars 1.43 source tag**

Change the package and dependency section to:

```toml
[package]
name = "polars_bt"
version = "0.2.0"
edition = "2021"

[dependencies]
pyo3 = { version = "0.28", features = ["abi3-py39", "extension-module"] }
pyo3-polars = { version = "0.27.0", features = ["derive"] }
serde = { version = "1", features = ["derive"] }
polars = { version = "0.54.4", default-features = false, features = [
    "dtype-datetime",
    "dtype-struct",
] }
chrono = "0.4.42"
```

Remove unused lazy-expression feature flags rather than carrying the old broad Polars feature list forward.

- [ ] **Step 4: Constrain the Python ABI peer dependency**

Set:

```toml
[project]
name = "polars_bt"
requires-python = ">=3.9"
dynamic = ["version"]
dependencies = ["polars>=1.43.0,<1.44.0"]
```

- [ ] **Step 5: Build and install the extension**

Run:

```bash
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/test_import.py -q
```

Expected: `1 passed`.

- [ ] **Step 6: Commit the ABI alignment**

```bash
git add Cargo.toml pyproject.toml tests/test_import.py
git commit -m "build: align plugin with polars 1.43"
```

### Task 2: Isolate and rename the T0 engine as Pulse

**Files:**
- Create: `tests/test_pulse.py`
- Create: `polars_bt/pulse.py`
- Modify: `polars_bt/__init__.py`
- Delete: `polars_bt/backtest.py`
- Create: `src/pulse/mod.rs`
- Create: `src/pulse/exchange.rs`
- Create: `src/pulse/matcher.rs`
- Create: `src/pulse/order.rs`
- Create: `src/pulse/position.rs`
- Modify: `src/lib.rs`
- Delete: `src/exchange/mod.rs`
- Delete: `src/exchange/matcher.rs`
- Delete: `src/order.rs`
- Delete: `src/position.rs`

- [ ] **Step 1: Write a failing Pulse regression test**

Create the fixture and assertions below. The same file must also contain `test_pulse_empty_signals` with three zero-signal rows and `test_pulse_custom_limits` with the two-row `[95.0, 105.0]` limit fixture preserved in `git show eb745b5:test_backtest.py`; both assert a one-row Struct result.

```python
import polars as pl
from polars_bt import pulse


def test_pulse_returns_the_existing_t0_summary():
    data = pl.DataFrame(
        {
            "ask_price": [100.0, 101.0, 102.0, 101.5, 100.5],
            "bid_price": [99.5, 100.5, 101.5, 101.0, 100.0],
            "lg_signal": [1, 0, 0, 1, 0],
            "st_signal": [0, 1, 0, 0, 1],
            "cl_signal": [0, 0, 0, 0, 0],
            "cs_signal": [0, 0, 0, 0, 0],
            "time_idx": [1000, 2000, 3000, 4000, 5000],
            "limit_down": [90.0] * 5,
            "limit_up": [110.0] * 5,
        }
    )
    result = data.select(
        pulse(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=10,
            step_2_limit=5,
        ).alias("pulse")
    )

    assert result.shape == (1, 1)
    assert result.schema["pulse"] == pl.Struct(
        {
            "lg_pos": pl.UInt64,
            "st_pos": pl.UInt64,
            "lg_win": pl.UInt64,
            "st_win": pl.UInt64,
            "all_signal": pl.UInt64,
            "floating_pnl": pl.Float64,
            "realized_pnl": pl.Float64,
        }
    )
```

- [ ] **Step 2: Verify the new name fails before implementation**

Run:

```bash
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/test_pulse.py -q
```

Expected: collection FAIL because `polars_bt.pulse` is not exported.

- [ ] **Step 3: Move the existing T0 Rust files without changing behavior**

Use `git mv` for the mechanical moves, then update imports to `crate::pulse::{order, position}`. Split the current `src/exchange/mod.rs` so the `#[polars_expr]` function and `Strategy` live in `src/pulse/mod.rs`, while `LofiEx` lives in `src/pulse/exchange.rs`. Rename the existing `backtest` function directly and retain its full body:

```rust
#[polars_expr(output_type_func=output_type_pulse)]
pub fn pulse(inputs: &[Series], kwargs: PulseKwargs) -> PolarsResult<Series> {
    // Keep the existing input extraction, quote loop, summary extraction,
    // and StructChunked construction here without arithmetic changes.
}
```

Do not introduce a second helper or duplicate the T0 algorithm; this step is a move-and-rename of the existing function body.

- [ ] **Step 4: Add the Python Pulse wrapper**

```python
from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def pulse(
    ask_price: str | pl.Expr,
    bid_price: str | pl.Expr,
    lg_signal: str | pl.Expr,
    st_signal: str | pl.Expr,
    cl_signal: str | pl.Expr,
    cs_signal: str | pl.Expr,
    time_idx: str | pl.Expr,
    limit_down: str | pl.Expr,
    limit_up: str | pl.Expr,
    step_1_limit: int = 10,
    step_2_limit: int = 5,
    verbose: bool = False,
) -> pl.Expr:
    def expr(value: str | pl.Expr) -> pl.Expr:
        return pl.col(value) if isinstance(value, str) else value

    return register_plugin_function(
        args=[
            expr(ask_price).cast(pl.Float64),
            expr(bid_price).cast(pl.Float64),
            expr(lg_signal).cast(pl.UInt8),
            expr(st_signal).cast(pl.UInt8),
            expr(cl_signal).cast(pl.UInt8),
            expr(cs_signal).cast(pl.UInt8),
            expr(time_idx).cast(pl.UInt64),
            expr(limit_down).cast(pl.Float64),
            expr(limit_up).cast(pl.Float64),
        ],
        plugin_path=LIB,
        function_name="pulse",
        kwargs={
            "step_1_limit": step_1_limit,
            "step_2_limit": step_2_limit,
            "verbose": verbose,
        },
        is_elementwise=False,
        returns_scalar=True,
    )
```

Export `pulse` from `polars_bt/__init__.py`. Do not retain a public `backtest` alias.

- [ ] **Step 5: Rebuild and verify Pulse**

Run:

```bash
cargo fmt --all
cargo test --all-targets
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/test_import.py tests/test_pulse.py -q
```

Expected: all tests PASS.

- [ ] **Step 6: Commit Pulse isolation**

```bash
git add polars_bt src tests/test_pulse.py
git commit -m "refactor: isolate t0 engine as pulse"
```

### Task 3: Build the pure Rust Mosaic portfolio state

**Files:**
- Create: `src/mosaic/portfolio.rs`
- Modify: `src/mosaic/mod.rs`

- [ ] **Step 1: Write failing unit tests for portfolio arithmetic**

```rust
#[cfg(test)]
mod tests {
    use super::PortfolioState;

    #[test]
    fn applies_returns_and_deals() {
        let mut state = PortfolioState::new(2);
        state.apply_deals(&[0.4, 0.6], 0.0001);
        state.apply_returns(&[0.10, -0.10]);
        assert!((state.cash() + 0.0001).abs() < 1e-15);
        assert!((state.holdings()[0] - 0.44).abs() < 1e-15);
        assert!((state.holdings()[1] - 0.54).abs() < 1e-15);
        assert!((state.nav() - 0.9799).abs() < 1e-15);
    }

    #[test]
    fn target_difference_uses_nav_after_initial_build() {
        let mut state = PortfolioState::new(2);
        assert_eq!(state.target_difference(&[0.4, 0.6]), vec![0.4, 0.6]);
        state.apply_deals(&[0.4, 0.6], 0.0);
        state.apply_returns(&[0.10, -0.10]);
        let diff = state.target_difference(&[0.5, 0.5]);
        assert!((diff[0] - 0.05).abs() < 1e-15);
        assert!((diff[1] + 0.05).abs() < 1e-15);
    }
}
```

- [ ] **Step 2: Run tests and verify the module is missing**

Run: `cargo test mosaic::portfolio -- --nocapture`

Expected: compile FAIL because `PortfolioState` is undefined.

- [ ] **Step 3: Implement `PortfolioState`**

```rust
#[derive(Debug, Clone)]
pub struct PortfolioState {
    hold: Vec<f64>,
    cash: f64,
    daily_trade_abs: f64,
    daily_turnover: f64,
}

impl PortfolioState {
    pub fn new(asset_num: usize) -> Self {
        Self {
            hold: vec![0.0; asset_num],
            cash: 1.0,
            daily_trade_abs: 0.0,
            daily_turnover: 0.0,
        }
    }

    pub fn begin_day(&mut self) {
        self.daily_trade_abs = 0.0;
        self.daily_turnover = 0.0;
    }

    pub fn apply_returns(&mut self, returns: &[f64]) {
        for (holding, value) in self.hold.iter_mut().zip(returns) {
            let clean = if value.is_nan() { 0.0 } else { *value };
            *holding *= 1.0 + clean;
        }
    }

    pub fn target_difference(&self, weights: &[f64]) -> Vec<f64> {
        let scale = if self.total_hold() > 0.0001 { self.nav() } else { 1.0 };
        weights
            .iter()
            .zip(&self.hold)
            .map(|(weight, holding)| {
                let clean = if weight.is_nan() { 0.0 } else { *weight };
                scale * clean - holding
            })
            .collect()
    }

    pub fn apply_deals(&mut self, amounts: &[f64], fee_rate: f64) {
        let traded_abs: f64 = amounts.iter().map(|value| value.abs()).sum();
        let traded_net: f64 = amounts.iter().sum();
        self.daily_trade_abs += traded_abs;
        for (holding, amount) in self.hold.iter_mut().zip(amounts) {
            *holding += amount;
        }
        self.cash -= traded_net;
        self.cash -= traded_abs * fee_rate;
    }

    pub fn finalize_turnover(&mut self) {
        let nav = self.nav();
        self.daily_turnover = if nav > 1e-12 {
            self.daily_trade_abs / nav
        } else {
            0.0
        };
    }

    pub fn total_hold(&self) -> f64 { self.hold.iter().sum() }
    pub fn nav(&self) -> f64 { self.cash + self.total_hold() }
    pub fn cash(&self) -> f64 { self.cash }
    pub fn turnover(&self) -> f64 { self.daily_turnover }
    pub fn holdings(&self) -> &[f64] { &self.hold }
    pub fn holding_count(&self) -> i64 {
        self.hold.iter().filter(|value| value.abs() > 1e-8).count() as i64
    }
}
```

- [ ] **Step 4: Run and commit the portfolio tests**

Run: `cargo test mosaic::portfolio -- --nocapture`

Expected: all portfolio tests PASS.

```bash
git add src/mosaic
git commit -m "feat: add mosaic portfolio state"
```

### Task 4: Implement the Mosaic daily state machine

**Files:**
- Create: `src/mosaic/engine.rs`
- Modify: `src/mosaic/mod.rs`

- [ ] **Step 1: Write failing tests for sequential cross-sectional execution**

Define `DailySlice` with borrowed slices and test a two-day, two-asset panel:

```rust
#[test]
fn runs_sell_before_buy_and_preserves_row_order() {
    let days = vec![
        DailySlice {
            weight: &[0.5, 0.5],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[true, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[true, true],
        },
        DailySlice {
            weight: &[0.0, 1.0],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[true, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[true, true],
        },
    ];
    let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0)).unwrap();
    assert_eq!(snapshots.len(), 2);
    assert!((snapshots[0].nav - 1.0).abs() < 1e-15);
    assert!((snapshots[1].turnover - 1.0).abs() < 1e-15);
    assert_eq!(snapshots[1].holding_count, 1);
}
```

Add six table-driven cases to the same test module: all-false `is_rebalance` produces zero turnover; `sellable=false` retains an existing holding; `buyable=false` leaves cash unchanged; nonzero fees reduce NAV by traded absolute value times the configured rate; a buy whose cumulative cost equals cash is rejected; and processing stops immediately after the first snapshot whose cash is below `-0.00001`.

- [ ] **Step 2: Verify the engine tests fail**

Run: `cargo test mosaic::engine -- --nocapture`

Expected: compile FAIL because `DailySlice`, `Config`, and `run_slices` are undefined.

- [ ] **Step 3: Implement the engine types and loop**

```rust
pub struct DailySlice<'a> {
    pub weight: &'a [f64],
    pub ovn_ret: &'a [f64],
    pub ind_ret: &'a [f64],
    pub buyable: &'a [bool],
    pub sellable: &'a [bool],
    pub prev_close: &'a [f64],
    pub vwap: &'a [f64],
    pub is_rebalance: &'a [bool],
}

#[derive(Clone, Copy)]
pub struct Config {
    pub asset_num: usize,
    pub st_fee: f64,
    pub lg_fee: f64,
}

impl Config {
    pub fn new(asset_num: usize, st_fee: f64, lg_fee: f64) -> Self {
        Self { asset_num, st_fee, lg_fee }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DailySnapshot {
    pub cash: f64,
    pub nav: f64,
    pub turnover: f64,
    pub holding_count: i64,
}

pub fn run_slices(days: &[DailySlice<'_>], config: Config) -> PolarsResult<Vec<DailySnapshot>> {
    let mut state = PortfolioState::new(config.asset_num);
    let mut snapshots = Vec::with_capacity(days.len());
    for day in days {
        state.apply_returns(day.ovn_ret);
        state.begin_day();
        if day.is_rebalance.iter().any(|value| *value) {
            let sell_diff = state.target_difference(day.weight);
            let mut executed = vec![0.0; config.asset_num];
            for index in 0..config.asset_num {
                let amount = sell_diff[index];
                if amount < 0.0
                    && day.sellable[index]
                    && day.prev_close[index] > 0.0
                    && day.vwap[index] > 0.0
                {
                    executed[index] = amount;
                }
            }
            state.apply_deals(&executed, config.st_fee);

            let buy_diff = state.target_difference(day.weight);
            let available_cash = state.cash();
            let mut cumulative = 0.0;
            executed.fill(0.0);
            for index in 0..config.asset_num {
                let eligible = buy_diff[index] > 0.0
                    && day.buyable[index]
                    && day.prev_close[index] > 0.0
                    && day.vwap[index] > 0.0;
                let amount = if eligible { buy_diff[index] } else { 0.0 };
                cumulative += amount * (1.0 + config.lg_fee);
                if eligible && cumulative < available_cash {
                    executed[index] = amount;
                }
            }
            state.apply_deals(&executed, config.lg_fee);
        }
        state.apply_returns(day.ind_ret);
        state.finalize_turnover();
        snapshots.push(DailySnapshot {
            cash: state.cash(),
            nav: state.nav(),
            turnover: state.turnover(),
            holding_count: state.holding_count(),
        });
        if state.cash() < -0.00001 {
            break;
        }
    }
    Ok(snapshots)
}
```

Factor repeated slice-length checks into one private function. Do not introduce asset IDs or hashing.

- [ ] **Step 4: Run the Rust engine suite**

Run: `cargo test mosaic -- --nocapture`

Expected: all Mosaic portfolio and engine tests PASS.

- [ ] **Step 5: Commit the engine**

```bash
git add src/mosaic
git commit -m "feat: add mosaic cross-sectional engine"
```

### Task 5: Add the Mosaic Polars expression boundary

**Files:**
- Create: `src/mosaic/output.rs`
- Modify: `src/mosaic/mod.rs`
- Create: `polars_bt/mosaic.py`
- Modify: `polars_bt/__init__.py`
- Create: `tests/test_mosaic.py`

- [ ] **Step 1: Write failing Python schema and structural tests**

```python
import polars as pl
import pytest
from polars_bt import mosaic


def dense_panel() -> pl.DataFrame:
    return pl.DataFrame(
        {
            "date": ["2024-01-02", "2024-01-02", "2024-01-03", "2024-01-03"],
            "weight": [0.5, 0.5, 0.0, 1.0],
            "ovn_ret": [0.0, 0.0, 0.0, 0.0],
            "ind_ret": [0.0, 0.0, 0.0, 0.0],
            "buyable": [True, True, True, True],
            "sellable": [True, True, True, True],
            "prev_close": [10.0, 10.0, 10.0, 10.0],
            "vwap": [10.0, 10.0, 10.0, 10.0],
            "is_rebalance": [True, True, True, True],
        }
    )


def run_mosaic(
    df: pl.DataFrame,
    asset_num: int = 2,
    st_fee: float = 0.0,
    lg_fee: float = 0.0,
) -> pl.DataFrame:
    return df.select(
        mosaic(
            date="date",
            weight="weight",
            ovn_ret="ovn_ret",
            ind_ret="ind_ret",
            buyable="buyable",
            sellable="sellable",
            prev_close="prev_close",
            vwap="vwap",
            is_rebalance="is_rebalance",
            asset_num=asset_num,
            st_fee=st_fee,
            lg_fee=lg_fee,
        ).alias("daily")
    ).unnest("daily")


def test_mosaic_returns_one_row_per_day():
    result = run_mosaic(dense_panel())
    assert result.columns == ["date", "cash", "nav", "turnover", "holding_count"]
    assert result.height == 2
    assert result["date"].to_list() == ["2024-01-02", "2024-01-03"]


def test_mosaic_rejects_non_rectangular_panel():
    with pytest.raises(Exception, match="divisible by asset_num"):
        run_mosaic(dense_panel().head(3))
```

- [ ] **Step 2: Verify the Mosaic import fails**

Run: `.venv/bin/python -m pytest tests/test_mosaic.py -q`

Expected: collection FAIL because `mosaic` is not exported.

- [ ] **Step 3: Implement the Python wrapper**

```python
from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def _expr(value: str | pl.Expr) -> pl.Expr:
    return pl.col(value) if isinstance(value, str) else value


def mosaic(
    date: str | pl.Expr,
    weight: str | pl.Expr,
    ovn_ret: str | pl.Expr,
    ind_ret: str | pl.Expr,
    buyable: str | pl.Expr,
    sellable: str | pl.Expr,
    prev_close: str | pl.Expr,
    vwap: str | pl.Expr,
    is_rebalance: str | pl.Expr,
    *,
    asset_num: int,
    st_fee: float = 6e-4,
    lg_fee: float = 1e-4,
) -> pl.Expr:
    return register_plugin_function(
        args=[
            _expr(date),
            _expr(weight).cast(pl.Float64),
            _expr(ovn_ret).cast(pl.Float64),
            _expr(ind_ret).cast(pl.Float64),
            _expr(buyable).cast(pl.Boolean),
            _expr(sellable).cast(pl.Boolean),
            _expr(prev_close).cast(pl.Float64),
            _expr(vwap).cast(pl.Float64),
            _expr(is_rebalance).cast(pl.Boolean),
        ],
        plugin_path=LIB,
        function_name="mosaic",
        kwargs={"asset_num": asset_num, "st_fee": st_fee, "lg_fee": lg_fee},
        is_elementwise=False,
        changes_length=True,
    )
```

- [ ] **Step 4: Implement the Rust plugin output type and validation**

Use a dynamic output type function whose date field clones `input_fields[0].dtype()`:

```rust
pub fn output_type_mosaic(input_fields: &[Field]) -> PolarsResult<Field> {
    polars_ensure!(!input_fields.is_empty(), ComputeError: "mosaic requires date input");
    Ok(Field::new(
        "mosaic".into(),
        DataType::Struct(vec![
            Field::new("date".into(), input_fields[0].dtype().clone()),
            Field::new("cash".into(), DataType::Float64),
            Field::new("nav".into(), DataType::Float64),
            Field::new("turnover".into(), DataType::Float64),
            Field::new("holding_count".into(), DataType::Int64),
        ]),
    ))
}

#[derive(Deserialize)]
pub struct MosaicKwargs {
    asset_num: usize,
    st_fee: f64,
    lg_fee: f64,
}
```

In `mosaic(inputs, kwargs)`, validate `asset_num`, fee finiteness/non-negativity, equal Series lengths, and divisibility. Rechunk numeric and Boolean Series once, borrow contiguous no-null slices, construct one `DailySlice` per fixed-width block, and call `run_slices`.

Build daily dates by taking indices `0, asset_num, 2 * asset_num, ...` from the input date Series and truncate them if negative cash caused early termination. Build the Struct from the date Series and four snapshot vectors with `StructChunked::from_series`.

- [ ] **Step 5: Build and run expression tests**

Run:

```bash
cargo fmt --all
cargo test mosaic -- --nocapture
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/test_mosaic.py -q
```

Expected: all Mosaic tests PASS.

- [ ] **Step 6: Commit the expression boundary**

```bash
git add polars_bt src/mosaic tests/test_mosaic.py
git commit -m "feat: expose mosaic polars expression"
```

### Task 6: Establish Python parity with ptrader semantics

**Files:**
- Create: `tests/reference_ptrader.py`
- Modify: `tests/test_mosaic.py`

- [ ] **Step 1: Add a deterministic Python reference implementation**

Implement `reference_ptrader(df, asset_num, st_fee, lg_fee)` using Python lists and the exact seven-stage semantics below. It returns the same five columns as `mosaic`; the buy loop captures cash before scanning and uses a strict `<` threshold.

```python
from __future__ import annotations

import math
import polars as pl


def _clean(value: float | None) -> float:
    return 0.0 if value is None or math.isnan(value) else value


def reference_ptrader(
    df: pl.DataFrame,
    asset_num: int,
    st_fee: float,
    lg_fee: float,
) -> pl.DataFrame:
    hold = [0.0] * asset_num
    cash = 1.0
    rows: list[dict[str, object]] = []
    columns = {name: df[name].to_list() for name in df.columns}

    for offset in range(0, df.height, asset_num):
        stop = offset + asset_num
        for index, value in enumerate(columns["ovn_ret"][offset:stop]):
            hold[index] *= 1.0 + _clean(value)

        daily_trade_abs = 0.0
        if any(columns["is_rebalance"][offset:stop]):
            total_hold = sum(hold)
            nav = cash + total_hold
            scale = nav if total_hold > 0.0001 else 1.0
            weights = [_clean(value) for value in columns["weight"][offset:stop]]
            diff = [scale * weight - value for weight, value in zip(weights, hold)]
            sells = [
                amount
                if amount < 0.0
                and columns["sellable"][offset + index]
                and columns["prev_close"][offset + index] > 0.0
                and columns["vwap"][offset + index] > 0.0
                else 0.0
                for index, amount in enumerate(diff)
            ]
            sold_abs = sum(abs(value) for value in sells)
            daily_trade_abs += sold_abs
            hold = [value + amount for value, amount in zip(hold, sells)]
            cash -= sum(sells)
            cash -= sold_abs * st_fee

            total_hold = sum(hold)
            nav = cash + total_hold
            scale = nav if total_hold > 0.0001 else 1.0
            diff = [scale * weight - value for weight, value in zip(weights, hold)]
            available_cash = cash
            cumulative = 0.0
            buys = [0.0] * asset_num
            for index, amount in enumerate(diff):
                eligible = (
                    amount > 0.0
                    and columns["buyable"][offset + index]
                    and columns["prev_close"][offset + index] > 0.0
                    and columns["vwap"][offset + index] > 0.0
                )
                candidate = amount if eligible else 0.0
                cumulative += candidate * (1.0 + lg_fee)
                if eligible and cumulative < available_cash:
                    buys[index] = amount
            bought_abs = sum(abs(value) for value in buys)
            daily_trade_abs += bought_abs
            hold = [value + amount for value, amount in zip(hold, buys)]
            cash -= sum(buys)
            cash -= bought_abs * lg_fee

        for index, value in enumerate(columns["ind_ret"][offset:stop]):
            hold[index] *= 1.0 + _clean(value)
        nav = cash + sum(hold)
        rows.append(
            {
                "date": columns["date"][offset],
                "cash": cash,
                "nav": nav,
                "turnover": daily_trade_abs / nav if nav > 1e-12 else 0.0,
                "holding_count": sum(abs(value) > 1e-8 for value in hold),
            }
        )
        if cash < -0.00001:
            break

    return pl.DataFrame(rows)
```

- [ ] **Step 2: Add parity tests before fixing numeric differences**

```python
from polars.testing import assert_frame_equal
from tests.reference_ptrader import reference_ptrader


def test_mosaic_matches_reference_on_complete_panel():
    df = make_seeded_panel(days=20, assets=30, seed=7)
    expected = reference_ptrader(df, 30, st_fee=6e-4, lg_fee=1e-4)
    actual = run_mosaic(df, asset_num=30, st_fee=6e-4, lg_fee=1e-4)
    assert_frame_equal(actual, expected, check_exact=False, rel_tol=0.0, abs_tol=1e-12)
```

Add separate cases for sparse rebalancing, blocked buys, blocked sells, and fee overrides.

- [ ] **Step 3: Run parity tests and inspect any first differing day**

Run: `.venv/bin/python -m pytest tests/test_mosaic.py -q -x`

Expected before numeric fixes: either PASS or a focused failure showing the first mismatched daily field.

- [ ] **Step 4: Make Rust evaluation order match the reference**

Adjust only arithmetic ordering and threshold conditions. Do not change the reference to make a Rust discrepancy disappear. Re-run:

```bash
cargo test mosaic
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests/test_mosaic.py -q
```

Expected: all parity tests PASS at `1e-12`.

- [ ] **Step 5: Commit parity coverage**

```bash
git add src/mosaic tests/reference_ptrader.py tests/test_mosaic.py
git commit -m "test: verify mosaic ptrader parity"
```

### Task 7: Add and run the accepted performance benchmark

**Files:**
- Create: `benchmarks/benchmark_mosaic.py`

- [ ] **Step 1: Build the materialized benchmark panel outside the timed section**

Use Polars ranges rather than Python lists:

```python
def make_panel(days: int = 2_500, assets: int = 5_000) -> pl.DataFrame:
    rows = days * assets
    return pl.DataFrame({"row": pl.arange(0, rows, eager=True)}).select(
        (pl.col("row") // assets).cast(pl.Int32).alias("date"),
        pl.lit(1.0 / assets).alias("weight"),
        pl.lit(0.0001).alias("ovn_ret"),
        pl.lit(0.0).alias("ind_ret"),
        pl.lit(True).alias("buyable"),
        pl.lit(True).alias("sellable"),
        pl.lit(10.0).alias("prev_close"),
        pl.lit(10.0).alias("vwap"),
        pl.lit(True).alias("is_rebalance"),
    ).rechunk()
```

- [ ] **Step 2: Time only the accepted boundary**

Warm up on a 20-day slice, then time `panel.select(mosaic(...)).unnest("daily")` with `time.perf_counter()`. Print rows, elapsed seconds, throughput, output days, and whether the 5-second hard target passed. Exit nonzero when elapsed exceeds 5 seconds.

- [ ] **Step 3: Run the release benchmark**

Run:

```bash
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python benchmarks/benchmark_mosaic.py
```

Expected: 12,500,000 input rows, 2,500 output rows, elapsed at most 5 seconds.

- [ ] **Step 4: Profile and optimize only if the hard target fails**

Measure separately: input rechunking, Rust scan, Struct construction, and Python unnest. Replace avoidable per-day allocations first, particularly `target_difference` vectors, with two reusable `Vec<f64>` buffers owned by `CrossSectionalEngine`. Do not add date parallelism or IPC.

- [ ] **Step 5: Commit the benchmark and any measured optimization**

```bash
git add benchmarks src/mosaic
git commit -m "perf: benchmark mosaic dense panels"
```

### Task 8: Update package tests, CI, and documentation

**Files:**
- Modify: `.github/workflows/ci.yml`
- Modify: `README.md`
- Modify: `Makefile`

- [ ] **Step 1: Make CI use the supported ABI and current package name**

Use Python 3.9 through 3.12, install `polars>=1.43,<1.44`, run `cargo fmt --check`, `cargo clippy --all-targets --all-features -- -D warnings`, build a release wheel, install it, and run:

```bash
python -m pytest tests -q
```

Change the clippy package selector from the obsolete `polars_bt_extension` name to `polars_bt`.

- [ ] **Step 2: Document both engines**

Replace the generic single-engine README introduction with a two-engine table:

```markdown
| Engine | Model | State axis | Output |
| --- | --- | --- | --- |
| `pulse` | T0 quote/signal matching | time rows | scalar summary |
| `mosaic` | cross-sectional portfolio | dense daily panels | daily portfolio rows |
```

Include one runnable example for each function and state the dense-panel, eager-only, no-IPC Mosaic contract.

- [ ] **Step 3: Run all package checks**

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests -q
.venv/bin/python -m ruff check polars_bt tests benchmarks
```

Expected: every command exits zero.

- [ ] **Step 4: Commit package completion**

```bash
git add .github/workflows/ci.yml README.md Makefile
git commit -m "docs: document pulse and mosaic engines"
```

### Task 9: Validate the alpha-lab compatibility adapter with a local wheel

**Files:**
- Modify: `/home/huangbogeng/github.com/Aequiludium/alpha-lab/alphamaster/ptrader/core/lofi.py`
- Modify: `/home/huangbogeng/github.com/Aequiludium/alpha-lab/tests/alphamaster/test_ptrader_engine.py`

- [ ] **Step 1: Build and install the local package without changing alpha-lab dependency metadata**

Run in the plugin repository:

```bash
unset CONDA_PREFIX
.venv/bin/maturin build --release --out dist
```

Run in alpha-lab:

```bash
.venv/bin/python -m pip install --force-reinstall --no-deps /home/huangbogeng/github.com/huangbogeng/polars_bt_extension/dist/polars_bt-0.2.0-*.whl
```

If the alpha-lab environment has no pip module, install the wheel with:

```bash
uv pip install --python .venv/bin/python --force-reinstall --no-deps /home/huangbogeng/github.com/huangbogeng/polars_bt_extension/dist/polars_bt-0.2.0-*.whl
```

- [ ] **Step 2: Write a failing compatibility test**

Add a test that builds a complete two-day panel, injects `Position(asset_num=2)`, calls `run()`, asserts the return value is `None`, asserts `position.history` remains a list of dictionaries, and compares it with the existing Python `_daily` reference at `1e-12`.

- [ ] **Step 3: Route `LofiEx.run()` through Mosaic**

Keep all existing helper methods for compatibility. Replace only `run()`:

```python
def run(self) -> None:
    from polars_bt import mosaic

    position = self._require_position()
    asset_num = len(position._hold)
    daily_df = self._raw_data.select(
        mosaic(
            date="date",
            weight="weight",
            ovn_ret="ovn_ret",
            ind_ret="ind_ret",
            buyable="buyable",
            sellable="sellable",
            prev_close="prev_close",
            vwap="vwap",
            is_rebalance="is_rebalance",
            asset_num=asset_num,
            st_fee=self._st_fee,
            lg_fee=self._lg_fee,
        ).alias("daily")
    ).unnest("daily")
    position.history = daily_df.to_dicts()
```

The import stays inside `run()` so importing alpha-lab remains possible before the independent wheel is installed. There is no silent fallback when `run()` is called without `polars_bt`; the missing accelerator produces the normal Python import error.

- [ ] **Step 4: Run focused alpha-lab tests**

```bash
.venv/bin/python -m pytest tests/alphamaster/test_ptrader_engine.py tests/alphamaster/test_ptrader_logging.py tests/alphamaster/test_ptrader_rack.py tests/alphamaster/test_ptrader_report.py -q
.venv/bin/python -m ruff check alphamaster/ptrader tests/alphamaster/test_ptrader_engine.py
```

Expected: all focused ptrader tests and Ruff checks PASS.

- [ ] **Step 5: Measure end-to-end core speed against the Python reference**

Use the same prepared 12.5-million-row panel for the old Python reference and the `LofiEx.run()` adapter. Exclude `prepare_engine_input`. Record both times and assert `python_seconds / mosaic_seconds >= 10.0` and `mosaic_seconds <= 5.0`.

- [ ] **Step 6: Commit only the adapter and its tests in alpha-lab**

Before staging, verify the diff contains no existing datacenter or alphaboard changes:

```bash
git diff -- alphamaster/ptrader/core/lofi.py tests/alphamaster/test_ptrader_engine.py
git add alphamaster/ptrader/core/lofi.py tests/alphamaster/test_ptrader_engine.py
git commit -m "perf(ptrader): run cross-sectional core in rust"
```

## Final Verification

Run in `polars_bt_extension`:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets
unset CONDA_PREFIX
.venv/bin/maturin develop --release
.venv/bin/python -m pytest tests -q
.venv/bin/python benchmarks/benchmark_mosaic.py
git status --short
```

Run in `alpha-lab`:

```bash
.venv/bin/python -m pytest tests/alphamaster/test_ptrader_engine.py tests/alphamaster/test_ptrader_logging.py tests/alphamaster/test_ptrader_rack.py tests/alphamaster/test_ptrader_report.py -q
.venv/bin/python -m ruff check alphamaster/ptrader tests/alphamaster/test_ptrader_engine.py
git status --short
```

Expected final state:

- Both repositories pass their focused checks.
- `pulse` retains the historical T0 behavior.
- `mosaic` matches the complete-panel reference within `1e-12`.
- The 12.5-million-row Mosaic boundary completes in at most 5 seconds and at least 10 times faster than the Python core.
- No unrelated alpha-lab files are staged or committed.
- Publishing `polars_bt` 0.2.0 and adding its released lower bound to alpha-lab remain separate externally visible release actions.
