# Pulse and Mosaic Backtest Engines

## Status

Approved design for implementation planning on 2026-07-22.

## Context

`polars_bt` currently contains a Rust-backed Polars expression plugin for a T0 signal and order-matching workflow. The existing engine consumes quote and signal columns, advances an order book and position state along the time axis, and returns a scalar summary.

`alpha-lab` contains a separate cross-sectional portfolio backtester under `alphamaster/ptrader/core`. Its production path is Python-driven: it sorts and groups a prepared Polars DataFrame by date, advances a fixed-width holdings vector, executes a sell leg followed by a buy leg, and records one portfolio snapshot per trading day. At roughly 2,500 days by 5,000 assets, this Python orchestration performs thousands of small Polars operations and is too slow.

ATRS 1.1.0 is not the target integration model because its Python wrapper serializes Polars DataFrames into Arrow IPC bytes before calling Rust and serializes the results back to IPC. `mosaic` must instead execute as a native Polars expression plugin over in-process Series buffers.

## Goals

- Preserve the existing T0 engine as an isolated engine named `pulse`.
- Add an independent cross-sectional portfolio engine named `mosaic`.
- Keep `alpha-lab` preprocessing, reporting, and public `BacktestEngine` API unchanged.
- Preserve the current `LofiEx(df)`, external `Position` injection, `run() -> None`, and `position.history` compatibility protocol.
- Match the current Python ptrader result on normal, complete, dense cross-sectional inputs within `1e-12` for all daily numeric outputs.
- Process a prepared 12.5-million-row input in at most 5 seconds and at least 10 times faster than the Python core on the same machine. The stretch target is 2 seconds.
- Avoid Arrow IPC serialization, per-day DataFrames, per-day joins, and per-day sorting inside Rust.

## Non-goals

- Supporting a changing set or changing order of assets between dates.
- Guaranteeing compatibility for malformed, sparse, duplicated, or otherwise abnormal engine inputs.
- Adding strict front-door validation of nulls, NaNs, infinities, weights, or market data.
- Moving `alpha-lab` market-data loading, `prepare_engine_input`, performance analysis, or reports into Rust.
- Sharing trading state or execution types between the T0 and cross-sectional engines.
- Parallelizing across dates. Portfolio state and cash are sequential across dates.
- Changing ptrader's execution rules, cash rule, fee semantics, or output metrics.

## Public API

The package exposes two intentionally distinct names:

```python
from polars_bt import pulse
from polars_bt import mosaic
```

`pulse` is the renamed public entry point for the existing T0 plugin. Its behavior and arguments remain unchanged apart from the function name.

`mosaic` is a new expression constructor:

```python
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
    *,
    asset_num=5000,
    st_fee=6e-4,
    lg_fee=1e-4,
) -> pl.Expr
```

The Python wrapper converts column names and expressions to the following plugin input dtypes:

- `weight`, `ovn_ret`, `ind_ret`, `prev_close`, and `vwap`: `Float64`
- `buyable`, `sellable`, and `is_rebalance`: `Boolean`
- `date`: retained in its input dtype

Numeric casting, including conversion from Polars Decimal market-data columns, happens inside the package wrapper so callers are not required to pre-convert built-in market data.

The expression is registered with `is_elementwise=False` and `changes_length=True`. It is supported only in an eager, whole-table `select` call. Streaming, window, and grouped invocation are outside the contract.

Example:

```python
daily_df = (
    prepared_df.select(
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
            asset_num=prepared_df["asset"].n_unique(),
            st_fee=6e-4,
            lg_fee=1e-4,
        ).alias("daily")
    ).unnest("daily")
)
```

`mosaic` returns one Struct value per processed trading day with fields:

```text
date
cash
nav
turnover
holding_count
```

The `date` field uses the input date dtype. The other fields use `Float64`, except `holding_count`, which uses `Int64`.

## Source Layout and Isolation

The T0 code is logically renamed and kept behind `pulse`; the cross-sectional implementation lives in a separate module:

```text
polars_bt/
├── __init__.py
├── pulse.py
└── mosaic.py

src/
├── lib.rs
├── pulse/
│   ├── mod.rs
│   ├── exchange.rs
│   ├── matcher.rs
│   ├── order.rs
│   └── position.rs
└── mosaic/
    ├── mod.rs
    ├── engine.rs
    ├── portfolio.rs
    └── output.rs
```

The engines share only the compiled extension module, Polars dependencies, allocator, and packaging infrastructure. `mosaic` must not reuse `pulse` order, matcher, or position types because their state models and execution semantics are different.

## Mosaic Input Contract

Python preprocessing owns panel construction. Before `mosaic` runs:

- Rows are sorted by `(date, asset)`.
- Every date has exactly `asset_num` rows.
- Every date contains the same assets in the same order.
- Row `i` within every date corresponds to the same asset and to holdings slot `i`.
- The input contains all columns listed in the public API.
- The input is materialized as an eager Polars DataFrame.

`asset` is deliberately not passed into Rust. The engine uses stable row positions and never hashes, joins, or compares asset identifiers.

The Rust plugin divides all input columns with `chunks_exact(asset_num)`. One chunk is one trading-day cross-section. This makes the inner loop independent of Polars DataFrame operations.

## Rust Components

### CrossSectionalEngine

`CrossSectionalEngine` owns the date loop. It receives borrowed column views, configuration, and a `PortfolioState`. It advances the portfolio one dense cross-section at a time and appends one result to `DailyOutputBuilder` after each completed day.

The engine is sequential across dates. It may use tight loops and contiguous slices within a date, but it must not reorder assets because the current buy cash rule is row-order-sensitive.

### PortfolioState

`PortfolioState` owns only cross-sectional portfolio state:

- `hold: Vec<f64>` with length `asset_num`
- `cash: f64`, initialized to `1.0`
- daily absolute traded value
- daily turnover

It provides small operations for applying returns, calculating NAV and total holdings, applying deals, counting holdings, and resetting/finalizing daily statistics. It has no Polars DataFrame dependency.

### DailyOutputBuilder

`DailyOutputBuilder` preallocates output vectors based on `row_count / asset_num`. It stores the first date value from each daily chunk and the final daily portfolio fields. It constructs a single Struct Series at the end of execution.

## Exact Daily Semantics

For each date chunk, the engine performs these steps in order.

### 1. Overnight settlement

For every asset row:

```text
hold[i] *= 1 + ovn_ret[i]
```

The current Python position code treats null and NaN numeric state inputs as zero. `mosaic` preserves this inexpensive normalization while reading values; it does not run a separate validation pass.

### 2. Start the trading day

Reset daily absolute traded value and daily turnover to zero.

### 3. Rebalance decision

If any `is_rebalance` value in the date chunk is true, execute both trading legs. If all are false, keep holdings unchanged until intraday settlement.

### 4. Sell leg

Calculate the target difference using the current ptrader rule:

```text
if sum(hold) > 0.0001:
    diff[i] = nav * weight[i] - hold[i]
else:
    diff[i] = weight[i] - hold[i]
```

A negative difference executes only when:

```text
sellable[i] and prev_close[i] > 0 and vwap[i] > 0
```

All eligible sells execute. The engine accumulates absolute traded value, updates holdings and cash, and deducts `abs(executed_sell).sum() * st_fee`.

### 5. Buy leg

Recalculate the target difference after the sell leg using the updated holdings, cash, and NAV.

A positive difference is initially eligible only when:

```text
buyable[i] and prev_close[i] > 0 and vwap[i] > 0
```

Then scan eligible buy differences in asset row order. Preserve the current cumulative cash rule exactly:

```text
cumulative += diff[i] * (1 + lg_fee)
execute diff[i] only while cumulative < cash
```

The row where cumulative reaches or exceeds cash and all later rows receive zero executed weight. The engine updates holdings and cash and deducts `abs(executed_buy).sum() * lg_fee`.

### 6. Intraday settlement

For every asset row:

```text
hold[i] *= 1 + ind_ret[i]
```

### 7. Snapshot

Calculate:

```text
nav = cash + sum(hold)
turnover = daily_abs_trade / nav if nav > 1e-12 else 0
holding_count = count(abs(hold[i]) > 1e-8)
```

Append the date and snapshot fields to the output builder.

After the snapshot, stop processing later dates if `cash < -0.00001`, matching the current Python engine's early-termination point.

## Alpha-lab Compatibility Adapter

`alpha-lab` retains its existing public interface:

```python
engine = LofiEx(prepared_df)
engine.position = Position(asset_num=num_assets)
engine._st_fee = st_fee
engine._lg_fee = lg_fee
engine.run()
history = engine.position.history
```

`LofiEx.run()` becomes a thin adapter:

1. Require an injected `Position` as it does today.
2. Call `polars_bt.mosaic` through an eager `select` on the already prepared DataFrame.
3. Expand the output Struct.
4. Convert the small daily result to the existing `position.history` list-of-dicts representation.
5. Return `None`.

`Position` remains exported and retains its existing Python methods and properties for compatibility. Its per-day Polars Series operations are no longer used by the production `LofiEx.run()` path. `BacktestEngine.run().analyze().result()` and the Python-side `net_ret` calculation remain unchanged.

## Error Handling

The first version performs only structural and configuration checks required to prevent unsafe or ambiguous execution:

- `asset_num` must be greater than zero.
- All plugin input Series must have the same length.
- Total row count must be divisible by `asset_num`.
- `st_fee` and `lg_fee` must be finite and non-negative.

Violations return a Polars compute error with the violated condition. Empty input remains handled by `BacktestEngine` before plugin invocation.

The engine does not validate asset identity, sort order, duplicate keys, weight sums, arbitrary missing-data patterns, or market-data realism. Those are outside the first-version contract.

## Testing Strategy

### Pulse regression

- Preserve or restore focused Python integration tests for the current T0 behavior before moving source files.
- Verify the renamed `pulse` entry point returns the same schema and values as the existing `backtest` implementation on identical fixtures.
- Run Rust unit tests for the existing matcher and position behavior.

### Mosaic unit tests

- Test return settlement, target-difference calculation, sell eligibility, buy eligibility, fees, cash threshold ordering, non-rebalance days, turnover, holding count, and negative-cash early termination with small fixed vectors.
- Test output schema and dynamic date dtype.
- Test each structural error independently.

### Python parity tests

- Generate deterministic, complete, dense panels with a fixed asset order.
- Run the current Python `LofiEx/Position` implementation as the reference.
- Run `mosaic` on the same prepared columns.
- Compare `cash`, `nav`, and `turnover` within `1e-12`; compare date and `holding_count` exactly.
- Include daily and sparse rebalancing, buy/sell restrictions, fees, and multiple sequential dates, while keeping inputs within the normal complete-data contract.

### Alpha-lab integration tests

- Verify `LofiEx(df)`, `Position(asset_num)`, external position injection, fee overrides, `run() -> None`, and `position.history` remain compatible.
- Verify `BacktestEngine.run().analyze().result()` keeps the existing result schema.
- Verify `net_ret` remains computed in Python from daily NAV.

## Performance Benchmark

The benchmark input is a materialized, prepared panel of approximately:

```text
2,500 trading days * 5,000 assets = 12,500,000 rows
```

Timing starts immediately before the eager Polars `select(mosaic(...))` call and ends after the daily Struct has been expanded into a DataFrame. It excludes market-data loading, `prepare_engine_input`, and performance reporting.

Benchmarks use a release build, run a warm-up iteration, and compare against the current Python core on the same machine and same prepared input.

Acceptance criteria:

- Hard wall-time target: at most 5 seconds.
- Hard relative target: at least 10 times faster than the Python core.
- Stretch wall-time target: at most 2 seconds.
- No Arrow IPC byte serialization in either direction.

## Delivery Sequence

1. Establish `pulse` regression coverage without changing behavior.
2. Rename and isolate the existing T0 package and Rust modules behind `pulse`.
3. Implement `mosaic` state and execution semantics with Rust unit tests.
4. Add the Polars expression boundary and Python wrapper.
5. Add Python parity and structural-error tests.
6. Benchmark and optimize `mosaic` against the accepted 12.5-million-row target.
7. Integrate `mosaic` behind the `alpha-lab` `LofiEx.run()` compatibility adapter.
8. Run focused `alpha-lab` ptrader regression and parity tests.

## Acceptance Summary

The design is complete when both named engines are independently usable, existing T0 behavior is available through `pulse`, normal dense cross-sectional ptrader behavior is available through `mosaic`, `alpha-lab` public APIs remain stable, daily results meet the parity tolerance, and the large prepared-panel benchmark meets both hard performance targets.
