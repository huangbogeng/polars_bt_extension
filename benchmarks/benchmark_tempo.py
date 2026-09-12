from __future__ import annotations

import argparse
import time

import polars as pl

from polars_bt import tempo

MICROSECONDS_PER_HOUR = 3_600_000_000
MICROSECONDS_PER_DAY = 86_400_000_000
BASE_DATETIME_US = 1_704_067_200_000_000


def make_panel(
    days: int = 500,
    timestamps_per_day: int = 5,
    assets: int = 5_000,
) -> pl.DataFrame:
    timestamps = days * timestamps_per_day
    rows = timestamps * assets
    return (
        pl.DataFrame({"row": pl.arange(0, rows, eager=True)})
        .with_columns(
            (pl.col("row") // assets).alias("time_index"),
            (pl.col("row") % assets).cast(pl.UInt32).alias("asset"),
        )
        .select(
            (
                pl.lit(BASE_DATETIME_US)
                + (pl.col("time_index") // timestamps_per_day) * MICROSECONDS_PER_DAY
                + (pl.col("time_index") % timestamps_per_day) * MICROSECONDS_PER_HOUR
            )
            .cast(pl.Datetime("us"))
            .alias("datetime"),
            pl.col("asset"),
            pl.lit(1.0 / assets).alias("weight"),
            pl.lit(0.00001).alias("period_ret"),
            pl.lit(True).alias("buyable"),
            pl.lit(True).alias("sellable"),
            ((pl.col("time_index") % timestamps_per_day) % 2 == 0).alias(
                "is_rebalance"
            ),
        )
        .rechunk()
    )


def run_tempo(panel: pl.DataFrame) -> pl.DataFrame:
    return panel.select(
        tempo(
            "datetime",
            "asset",
            "weight",
            "period_ret",
            "buyable",
            "sellable",
            "is_rebalance",
        ).alias("result")
    ).unnest("result")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--days", type=int, default=500)
    parser.add_argument("--timestamps-per-day", type=int, default=5)
    parser.add_argument("--assets", type=int, default=5_000)
    parser.add_argument("--target-seconds", type=float, default=5.0)
    args = parser.parse_args()

    panel = make_panel(args.days, args.timestamps_per_day, args.assets)
    warmup_timestamps = min(args.days * args.timestamps_per_day, 20)
    run_tempo(panel.head(warmup_timestamps * args.assets))

    started = time.perf_counter()
    result = run_tempo(panel)
    elapsed = time.perf_counter() - started

    rows = panel.height
    expected_timestamps = args.days * args.timestamps_per_day
    passed = elapsed <= args.target_seconds and result.height == expected_timestamps
    print(f"rows={rows:,}")
    print(f"timestamps={expected_timestamps:,}")
    print(f"elapsed_seconds={elapsed:.6f}")
    print(f"throughput_rows_per_second={rows / elapsed:,.0f}")
    print(f"output_timestamps={result.height:,}")
    print(f"target_seconds={args.target_seconds:.3f}")
    print(f"target_passed={str(passed).lower()}")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
