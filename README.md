# polars_bt

`polars_bt` is a Rust-backed Polars expression plugin with two deliberately
separate backtesting engines.

| Engine | Model | State axis | Output |
| --- | --- | --- | --- |
| `pulse` | T0 quote/signal matching | time rows | scalar summary |
| `mosaic` | cross-sectional portfolio | dense daily panels | daily portfolio rows |

Both engines execute inside the Polars process. They do not serialize a
DataFrame through Arrow IPC to call Rust.

## Requirements and installation

- CPython 3.10, 3.11, or 3.12
- Polars 1.43.x

```bash
pip install polars_bt
```

For a local release build:

```bash
uv venv --python 3.12 .venv
uv pip install --python .venv/bin/python 'polars>=1.43,<1.44' maturin pytest
.venv/bin/maturin develop --release
```

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
- preprocessing materializes a complete panel before the call;
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
of anomalies.

Mosaic cannot detect cross-day asset-order changes because asset identifiers
are intentionally absent from its row-offset protocol. Callers must continue
to provide a stable asset order for every date.

## Development

```bash
make install-release
make pre-commit
.venv/bin/python benchmarks/benchmark_mosaic.py
```

The accepted benchmark is 2,500 days by 5,000 assets (12.5 million rows), with
a five-second hard limit measured only around the Mosaic expression call.

## License

MIT. See [LICENSE](LICENSE).
