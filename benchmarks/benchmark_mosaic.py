from __future__ import annotations

import argparse
import time

import polars as pl

from polars_bt import mosaic


def make_panel(days: int = 2_500, assets: int = 5_000) -> pl.DataFrame:
    rows = days * assets
    return (
        pl.DataFrame({"row": pl.arange(0, rows, eager=True)})
        .select(
            (pl.col("row") // assets).cast(pl.Int32).alias("date"),
            pl.lit(1.0 / assets).alias("weight"),
            pl.lit(0.0001).alias("ovn_ret"),
            pl.lit(0.0).alias("ind_ret"),
            pl.lit(True).alias("buyable"),
            pl.lit(True).alias("sellable"),
            pl.lit(10.0).alias("prev_close"),
            pl.lit(10.0).alias("vwap"),
            pl.lit(True).alias("is_rebalance"),
        )
        .rechunk()
    )


def run_mosaic(panel: pl.DataFrame, assets: int) -> pl.DataFrame:
    return panel.select(
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
            asset_num=assets,
        ).alias("daily")
    ).unnest("daily")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--days", type=int, default=2_500)
    parser.add_argument("--assets", type=int, default=5_000)
    parser.add_argument("--target-seconds", type=float, default=5.0)
    args = parser.parse_args()

    panel = make_panel(args.days, args.assets)
    warmup_days = min(args.days, 20)
    run_mosaic(panel.head(warmup_days * args.assets), args.assets)

    started = time.perf_counter()
    result = run_mosaic(panel, args.assets)
    elapsed = time.perf_counter() - started

    rows = panel.height
    passed = elapsed <= args.target_seconds
    print(f"rows={rows:,}")
    print(f"elapsed_seconds={elapsed:.6f}")
    print(f"throughput_rows_per_second={rows / elapsed:,.0f}")
    print(f"output_days={result.height}")
    print(f"target_seconds={args.target_seconds:.3f}")
    print(f"target_passed={str(passed).lower()}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
