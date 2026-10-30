# polars_bt

`polars_bt` is a Rust-backed Polars expression plugin with three deliberately
separate backtesting engines.

| Engine | Model | State axis | Output |
| --- | --- | --- | --- |
| `pulse` | T0 quote/signal matching | time rows | scalar summary |
| `mosaic` | cross-sectional portfolio | dense daily panels | daily portfolio rows |
| `tempo` | multi-time cross-sectional portfolio | dense datetime panels | timestamp portfolio rows |

All three engines execute inside the Polars process. They do not serialize a
DataFrame through Arrow IPC to call Rust.

## Requirements and installation

- CPython 3.10, 3.11, or 3.12
- Python 3.10: Polars >=2.0.0,<2.1
- Python 3.11–3.12: Polars >=2.0.0,<2.2 (target release: 2.1.0)
- Prebuilt wheels: Linux x86_64; other platforms require a source build and
  are not covered by the release test matrix

```bash
pip install --upgrade polars_bt
```

| polars_bt version | Python Polars |
| --- | --- |
| 0.2.4 | Python 3.10: >=2.0.0,<2.1; Python 3.11–3.12: >=2.0.0,<2.2 |
| 0.2.3 | >=2.0.0,<2.1 |
| 0.2.2 | >=1.44.2,<1.45 |
| 0.2.1 | >=1.43,<1.44 |

Version 0.2.4 adds Polars 2.1 support without changing the backtest API.
Polars 2.1 requires Python >=3.11; dependency markers keep Python 3.10 on
Polars 2.0.x. Upgrade `polars_bt` and Polars together; environments retaining
Polars 1.44 should retain `polars_bt==0.2.2`.

For a local build, install the Rust toolchain specified in
`rust-toolchain.toml` (1.95), then run from the repository root:

```bash
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python -r requirements.txt
make install-release
```

The native plugin is built with Rust Polars 0.55.2, pyo3-polars 0.28, and
PyO3 0.29. Upgrading the Python Polars major or minor version requires
rebuilding and testing the plugin with the corresponding Rust dependencies.
CPython's `abi3` wheel tag does not guarantee compatibility with a different
Polars version.
See [the upgrade analysis](docs/polars-upgrade.md) for the 2.0/2.1 verification
results and historical 1.44 migration, and [ARCHITECTURE.md](ARCHITECTURE.md)
for current engine contracts.

Polars 2.0 defaults lazy collection to streaming. Prepare and explicitly sort
your complete panel before calling a backtest through eager `DataFrame.select`.
For example, use `panel.lazy().sort(["datetime", "asset"]).collect()` before
Tempo; use your asset key with `date` for Mosaic. Do not rely on join or
group-by output order. Direct lazy/streaming backtest execution is outside
the supported contract. See the [official 2.0 migration guide](https://docs.pola.rs/releases/upgrade/2/).

## Pulse: T0 quote matching

`pulse` retains the original quote-by-quote T0 matcher and returns one Struct
summary.

```python
import polars as pl
from polars_bt import pulse

quotes = pl.DataFrame(
    {
        "ask": [100.0, 101.0, 102.0],
        "bid": [99.5, 100.5, 101.5],
        "long": [1, 0, 0],
        "short": [0, 1, 0],
        "close_long": [0, 0, 0],
        "close_short": [0, 0, 0],
        "time": [1000, 2000, 3000],
        "limit_down": [90.0] * 3,
        "limit_up": [110.0] * 3,
    }
)

summary = quotes.select(
    pulse(
        "ask",
        "bid",
        "long",
        "short",
        "close_long",
        "close_short",
        "time",
        "limit_down",
        "limit_up",
    ).alias("pulse")
)
```

Set `LOFIEX_MATCHER=easy` to use the relaxed matcher; the default matcher keeps
the original limit-price checks.

## Mosaic: cross-sectional portfolios

`mosaic` scans a dense, date-major panel in fixed `asset_num` row blocks. It
returns one daily Struct row containing `date`, `cash`, `nav`, `turnover`, and
`holding_count`.

```python
import polars as pl
from polars_bt import mosaic

panel = pl.DataFrame(
    {
        "date": ["2024-01-02", "2024-01-02", "2024-01-03", "2024-01-03"],
        "weight": [0.4, 0.4, 0.0, 0.5],
        "ovn_ret": [0.0, 0.0, 0.01, -0.01],
        "ind_ret": [0.0, 0.0, 0.0, 0.0],
        "buyable": [True] * 4,
        "sellable": [True] * 4,
        "prev_close": [10.0] * 4,
        "vwap": [10.0] * 4,
        "is_rebalance": [True] * 4,
    }
)

daily = panel.select(
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
        asset_num=2,
    ).alias("daily")
).unnest("daily")
```

Mosaic's input contract is intentionally narrow:

- rows are already sorted by `(date, asset)` and every date has exactly
  `asset_num` rows;
- the asset row order is stable across dates, so the engine uses row offsets and
  performs no joins or asset hashing;
- callers materialize a complete panel before the call; the wrapper does not
  sort or fill missing assets;
- numeric nulls in `weight`, `ovn_ret`, `ind_ret`, `prev_close`, and `vwap`
  are preserved as NaN semantics rather than silently filled with zero;
- use it as an eager whole-table expression; it changes the output length;
- fees default to `st_fee=6e-4` and `lg_fee=1e-4`.

### Mosaic diagnostics

Enable Polars verbose mode to see bounded Rust-side diagnostics on stderr:

```python
with pl.Config(verbose=True):
    daily = panel.select(
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
            asset_num=2,
        ).alias("daily")
    ).unnest("daily")
```

`pl.Config.set_verbose(True)` and the process-level `POLARS_VERBOSE=1` switch
enable the same plugin diagnostics. Records use a stable prefix and compact
key/value format:

```text
[polars-bt][mosaic][INFO] event=start rows=12500000 days=2500 assets=5000
[polars-bt][mosaic][WARN] event=input_summary nan_weight=32 mixed_date_blocks=1
[polars-bt][mosaic][WARN] event=halt reason=NEGATIVE_CASH day_index=1902 cash=-0.0021
[polars-bt][mosaic][INFO] event=finish completed_days=1903 expected_days=2500
```

Verbose diagnostics add no result fields and do not change tolerated-input
semantics. Non-finite portfolio state is always a hard error with day, asset,
and calculation-stage context. Diagnostic reports retain only counts and the
first location for each category, so memory use does not grow with the number
of anomalies. Nullable returns remain visible as `NAN_OVN_RET` or `NAN_IND_RET`,
and nullable prices remain visible as `INVALID_PREV_CLOSE` or `INVALID_VWAP`.

Mosaic cannot detect cross-day asset-order changes because asset identifiers
are intentionally absent from its row-offset protocol. Callers must continue
to provide a stable asset order for every date.

## Tempo: intraday and cross-day portfolios

`tempo` extends the dense row-offset model to arbitrary timestamps. Every
datetime contains a complete target cross-section, while `is_rebalance`
controls whether that timestamp only marks the existing portfolio or also
trades toward the supplied weights.

```python
from datetime import datetime

import polars as pl
from polars_bt import tempo

panel = pl.DataFrame(
    {
        "datetime": [
            datetime(2024, 1, 2, 9, 31),
            datetime(2024, 1, 2, 9, 31),
            datetime(2024, 1, 2, 14, 30),
            datetime(2024, 1, 2, 14, 30),
        ],
        "asset": ["A", "B", "A", "B"],
        "weight": [0.4, 0.4, 0.0, 0.8],
        "period_ret": [0.0, 0.0, 0.01, -0.01],
        "buyable": [True] * 4,
        "sellable": [True] * 4,
        "is_rebalance": [True] * 4,
    }
)

path = panel.select(
    tempo(
        "datetime",
        "asset",
        "weight",
        "period_ret",
        "buyable",
        "sellable",
        "is_rebalance",
        t1=True,
    ).alias("path")
).unnest("path")
```

Tempo's contract is:

- rows are sorted by `(datetime, asset)` and every datetime contains the same
  assets in the same order;
- the first timestamp defines the canonical asset vector; Rust validates every
  later timestamp before running the backtest and never sorts or joins;
- `period_ret` is the return from the preceding timestamp to the current one,
  and is applied before the current rebalance;
- `is_rebalance=False` still marks holdings and emits a snapshot but does not
  trade;
- use it as an eager whole-table expression; it changes the output length;
- numeric nulls become NaN; NaN weights mean zero target and NaN returns mean
  zero return, with bounded warnings available through Polars verbose mode;
- `t1=True` freezes same-day purchases until the date derived from `datetime`
  changes, while `t1=False` allows same-day sales;
- output contains one row per datetime with `datetime`, `cash`, `nav`,
  timestamp turnover, and `holding_count`.

Tempo supports long-only weights. `buyable` and `sellable` describe market
constraints at the current timestamp; Rust separately tracks the partially
sellable quantity required by T+1.

## Development

```bash
make install-release
make fmt
make pre-commit
.venv/bin/python examples/basic_usage.py
.venv/bin/python benchmarks/benchmark_mosaic.py
.venv/bin/python benchmarks/benchmark_tempo.py
```

The accepted benchmark scale is 12.5 million rows. Mosaic uses 2,500 days by
5,000 assets; Tempo uses 500 days by five timestamps by 5,000 assets. Both have
a five-second hard limit measured only around the expression call.

`make fmt` formats Rust and Python sources; `make pre-commit` checks formatting,
Clippy, Rust tests, Python tests, and Ruff. CI repeats the quality checks and
tests the installed wheel outside the source checkout on Python 3.10–3.12.
See [CONTRIBUTING.md](CONTRIBUTING.md) for the release process and
[CHANGELOG.md](CHANGELOG.md) for version history.

## License

MIT. See [LICENSE](LICENSE).
