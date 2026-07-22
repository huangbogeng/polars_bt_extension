# Mosaic Polars Verbose Diagnostics Design

**Date:** 2026-07-22

## Goal

Add low-overhead diagnostics to the Rust-backed `mosaic` cross-sectional
backtest. Diagnostics must use Polars' existing verbose switch, preserve the
current Python call signature and five-column output schema, and leave valid
input calculations unchanged.

The caller enables diagnostics with either interface below:

```python
pl.Config.set_verbose(True)

with pl.Config(verbose=True):
    result = panel.select(...)
```

`POLARS_VERBOSE=1` is the equivalent process-level switch. Rust diagnostics
are written to stderr with a stable `[polars-bt][mosaic]` prefix.

## Scope

This version covers `mosaic` only. It introduces a small shared logging helper
that `pulse` can reuse later, but does not change `pulse` in this work.

It does not:

- modify `alpha-lab`;
- change the nine `mosaic` expression inputs or keyword arguments;
- add status fields to the returned Struct;
- validate cross-day asset identity, because asset identifiers are not part of
  the plugin protocol;
- change NaN normalization, rebalance, execution, fee, cash, or early-stop
  semantics;
- emit one log line per input row.

## Compatibility Contract

`mosaic` continues to return exactly:

```text
date, cash, nav, turnover, holding_count
```

For all inputs that currently produce finite daily state, verbose mode and
non-verbose mode must produce identical output. Existing behavior remains:

- NaN `weight`, `ovn_ret`, and `ind_ret` values are treated as zero;
- a day rebalances when any `is_rebalance` value in its block is true;
- non-positive prices prevent the corresponding trade;
- processing stops after the first daily snapshot with `cash < -0.00001`.

Diagnostics describe these behaviors without redefining them.

## Considered Approaches

### Rust-native aggregated diagnostics

The selected approach performs an optional diagnostic scan in Rust when
Polars verbose mode is enabled, retains the existing calculation path, and
prints a compact summary. It keeps diagnostics adjacent to the engine state
and avoids Python-side scans or IPC.

### Python wrapper validation

Rejected because it would add another Polars query over the full panel, could
not observe Rust portfolio state, and would split the diagnostic contract
across languages.

### Additional result status fields

Deferred because it would change the public output schema. Structured result
status can be added later if persistent machine-readable audit data becomes a
requirement.

## Architecture

### Shared logging helper

`src/diagnostics.rs` owns the minimal shared integration with Polars:

- reload and read `POLARS_VERBOSE` through `polars_config::config()` once at
  the plugin boundary, so Python-side `pl.Config` changes reach the plugin's
  independently linked Rust configuration cache;
- emit deterministic stderr records;
- normalize the log prefix and levels;
- introduce no `log` or `tracing` dependency.

It must not contain `mosaic`-specific validation rules.

### Mosaic input diagnostics

`src/mosaic/diagnostics.rs` owns a pure `inspect_inputs` operation and the
`MosaicDiagnosticReport` data model. The report contains counters and the first
location for each diagnostic category. A location is expressed as
`day_index` and `asset_index`, because the engine does not receive an asset
identifier.

The diagnostic scan runs only when verbose mode is enabled. It may make one
additional O(N) pass over the contiguous input slices. It does not allocate
per-row messages or retain anomalous row collections.

### Portfolio engine

`src/mosaic/engine.rs` remains responsible for sequential portfolio
calculation. As overnight settlement, sells, buys, intraday settlement, and
snapshot finalization update state, the engine verifies the numeric values
touched by that stage are finite. Failure returns a `PolarsError` containing
the day index, calculation stage, reason code, and offending value. These
constant-time branches are the only new work on the verbose-disabled path;
the benchmark decides whether their implementation is acceptable.

The engine makes its existing negative-cash early stop explicit in its return
metadata so the plugin boundary can log the halt reason. This metadata is not
added to the output Struct.

### Plugin boundary

`src/mosaic/mod.rs` orchestrates the operation in this order:

1. Read the Polars verbose setting once.
2. Run the existing global structural checks.
3. Borrow and prepare the input slices.
4. If verbose is enabled, inspect inputs and hold one aggregate report.
5. Execute the existing portfolio state machine.
6. Reject non-finite daily state.
7. If verbose is enabled, emit the report and completion record.
8. Build the unchanged five-column Struct.

## Diagnostic Categories

The verbose input report records the following categories.

| Code | Definition | Existing calculation behavior |
| --- | --- | --- |
| `NAN_WEIGHT` | `weight.is_nan()` | Normalize to zero |
| `NAN_OVN_RET` | `ovn_ret.is_nan()` | Normalize to zero |
| `NAN_IND_RET` | `ind_ret.is_nan()` | Normalize to zero |
| `INF_WEIGHT` | Infinite weight | Calculation proceeds until a finite-state guard fails |
| `INF_OVN_RET` | Infinite overnight return | Calculation proceeds until a finite-state guard fails |
| `INF_IND_RET` | Infinite intraday return | Calculation proceeds until a finite-state guard fails |
| `INVALID_PREV_CLOSE` | Non-finite or `<= 0` previous close | Trade eligibility fails |
| `INVALID_VWAP` | Non-finite or `<= 0` VWAP | Trade eligibility fails |
| `NEGATIVE_WEIGHT` | Finite weight below zero | Existing target calculation is preserved |
| `RETURN_BELOW_MINUS_ONE` | Finite return below `-1.0` | Existing return settlement is preserved unless state becomes non-finite |
| `MIXED_DATE_BLOCK` | A fixed-width day block contains more than one date value | The block's first date remains the output date |
| `MIXED_REBALANCE_BLOCK` | A block contains both true and false | Existing `any()` rule is preserved |
| `OVERWEIGHT_DAY` | Clean finite weight sum exceeds `1.0 + 1e-8` on a rebalance block | Existing target and cash rules are preserved |

Each category stores only:

- total occurrence count;
- first `day_index`;
- first `asset_index`, when meaningful.

Boolean nulls and numeric nulls remain hard input errors in the existing slice
conversion path rather than diagnostic counters.

## Error Policy

Structural errors continue to return `PolarsError`, including invalid input
count, type, shape, `asset_num`, fees, and nullability.

The new finite-state guard always runs, regardless of verbose mode. A failure
uses this stable shape:

```text
polars-bt mosaic failed: day_index=846 stage=intraday_settlement reason=NONFINITE_NAV nav=NaN
```

This is a hard error because subsequent portfolio state cannot be trusted.
Diagnostic input observations do not raise by themselves. In particular, NaN
normalization, mixed rebalance flags, invalid prices, negative weights, and
negative-cash early termination retain their current behavior.

## Log Protocol

Logs are single-line key/value records so users can grep or redirect stderr.
The fixed prefix, component, level, and event appear first:

```text
[polars-bt][mosaic][INFO] event=start rows=12500000 days=2500 assets=5000
[polars-bt][mosaic][WARN] event=input_summary nan_weight=32 mixed_date_blocks=1
[polars-bt][mosaic][WARN] event=first code=MIXED_DATE_BLOCK day_index=846 asset_index=17
[polars-bt][mosaic][WARN] event=halt reason=NEGATIVE_CASH day_index=1902 cash=-0.0021
[polars-bt][mosaic][INFO] event=finish completed_days=1903 expected_days=2500
```

Rules:

- emit exactly one `start` and one `finish` record for a successful call;
- omit zero-valued categories from `input_summary`;
- emit at most one `first` record per nonzero category;
- emit one `halt` record only when negative cash triggers the existing stop;
- never print full rows, asset lists, or one record per occurrence;
- emit no `polars-bt` diagnostics when Polars verbose mode is disabled.

If a hard error occurs after `start`, return the error normally. The error
itself is authoritative; a separate duplicate stderr error record is not
required.

## Performance

The production path is the verbose-disabled path. It must:

- avoid the optional input diagnostic pass;
- avoid per-row string formatting and logging;
- preserve the accepted 2,500-day by 5,000-asset benchmark target of at most
  five seconds;
- keep the same asymptotic memory behavior as the current implementation.

Verbose mode may perform one additional O(N) scan and allocate one bounded
aggregate report. It has no five-second requirement, but it must remain linear
and bounded independently of the number of anomalies.

## Testing

### Rust unit tests

- Count and locate every diagnostic category with small fixed slices.
- Confirm zero-valued categories are omitted from rendered summaries.
- Confirm only the first location per category is retained.
- Confirm negative-cash halt metadata is returned without changing snapshots.
- Confirm non-finite `cash`, `nav`, or `turnover` becomes a contextual
  `PolarsError`.
- Test deterministic log-line rendering without relying on global stderr.

### Python integration tests

Run logging checks in subprocesses so global Polars configuration and stderr
capture cannot leak between tests:

- verbose disabled: no `[polars-bt][mosaic]` stderr output;
- `pl.Config.set_verbose(True)`: start, summary, first-location, and finish
  records are visible;
- `with pl.Config(verbose=True)`: the context-manager path also works;
- a non-finite state raises an exception containing `day_index`, `stage`, and
  reason code;
- the returned schema remains exactly the existing five fields.

### Semantic regression tests

Existing reference parity remains mandatory. Add explicit fixtures proving
that diagnostics do not change:

- NaN-to-zero normalization;
- mixed `is_rebalance` `any()` behavior;
- invalid-price trade skipping;
- negative-weight target behavior;
- negative-cash snapshot inclusion and subsequent truncation.

### Performance regression

Run the existing 12.5-million-row benchmark with verbose disabled and retain
the five-second hard limit. The benchmark must not enable diagnostic logging.

## Documentation

README gains a short diagnostics section showing both Python configuration and
`POLARS_VERBOSE=1`, the stderr record format, the fact that diagnostics do not
alter the result schema, and the limitation that asset-order mistakes cannot
be detected without asset identifiers.

## Acceptance Criteria

The design is complete when:

- normal `mosaic` output is byte-for-byte schema compatible and numerically
  identical to the existing reference;
- verbose-disabled calls emit no package diagnostic logs;
- verbose-enabled calls expose bounded Rust-side summaries through Polars;
- negative-cash truncation is visible in verbose logs;
- non-finite internal state fails with day and stage context;
- the accepted 12.5-million-row benchmark still passes;
- `pulse` and `alpha-lab` are unchanged.
