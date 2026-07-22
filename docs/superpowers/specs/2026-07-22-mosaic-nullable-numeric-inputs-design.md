# Mosaic Nullable Numeric Inputs Design

## Goal

Make `polars_bt.mosaic` accept nullable numeric Polars expressions without exposing Rust's contiguous-slice implementation detail, while preserving missing-market-data diagnostics.

## Public contract

The Python expression wrapper casts `weight`, `ovn_ret`, `ind_ret`, `prev_close`, and `vwap` to `Float64`, then converts Polars nulls to IEEE NaN. Existing NaNs remain NaN; the wrapper never blanket-fills missing numeric inputs with zero.

The existing engine semantics remain unchanged:

- missing weight behaves as target weight zero;
- missing overnight or intraday return behaves as zero return for NAV continuity;
- missing `prev_close` or `vwap` makes that asset ineligible to trade;
- verbose diagnostics preserve and report the corresponding NaN or invalid-price category.

## Rust boundary

Before calling `cont_slice()`, Rust checks every numeric series for residual Polars nulls. A violation raises `mosaic numeric input <name> cannot contain nulls`, rather than the misleading `chunked array is not contiguous` error. This is defensive: normal calls through the Python wrapper have already converted nulls to NaN.

## Compatibility and release

The `mosaic` signature, output schema, `pulse`, and all existing tolerated NaN behavior remain unchanged. Regression tests cover all five nullable numeric inputs and the Rust error helper. The package is released as `0.2.1` through the existing GitHub Release and PyPI Trusted Publishing workflow. No alpha-lab files are modified.
