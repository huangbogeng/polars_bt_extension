"""Runnable examples for the Pulse and Mosaic engines."""

import polars as pl

from polars_bt import mosaic, pulse


def pulse_example() -> pl.DataFrame:
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
    return quotes.select(
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


def mosaic_example() -> pl.DataFrame:
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
            asset_num=2,
        ).alias("daily")
    ).unnest("daily")


if __name__ == "__main__":
    print("Pulse")
    print(pulse_example())
    print("\nMosaic")
    print(mosaic_example())
