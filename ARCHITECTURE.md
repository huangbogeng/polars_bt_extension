# Architecture

`polars_bt` contains two independent Polars expression plugins in one Rust
extension. They share only the Python package, the PyO3 module, and the Polars
allocator.

```text
polars_bt/
├── pulse.py              # T0 expression wrapper
└── mosaic.py             # cross-sectional expression wrapper
src/
├── pulse/                # quote matcher, orders, and T0 position state
└── mosaic/
    ├── portfolio.rs      # cash and holding arithmetic
    ├── engine.rs         # fixed-width daily state machine
    └── output.rs         # daily Struct construction
```

## Pulse

Pulse consumes quote and signal columns in time order. Its exchange, matcher,
order, and position types are isolated under `src/pulse`; it returns one Struct
summary for the complete input.

## Mosaic

Mosaic consumes a materialized date-major panel. Python preprocessing guarantees
that every date has `asset_num` rows in the same asset order. Rust therefore
uses fixed row slices and a holding vector, with no joins, asset IDs, hashing, or
Arrow IPC serialization.

For each day the engine applies:

1. overnight returns;
2. daily turnover reset;
3. eligible sells;
4. eligible buys in stable row order using captured cash and a strict `<` limit;
5. intraday returns;
6. turnover finalization and snapshot;
7. negative-cash stop after the snapshot.

The expression changes length from asset rows to daily rows and is intended for
eager whole-table execution.

## Compatibility boundary

`polars_bt` 0.2 targets CPython 3.10–3.12 and Python Polars 1.43.x. The Rust
plugin uses Polars 0.54.4 and pyo3-polars 0.27.0 to match that ABI.
