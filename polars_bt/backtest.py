from pathlib import Path
from typing import Union

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def backtest(
    ask_price: Union[str, pl.Expr],
    bid_price: Union[str, pl.Expr],
    lg_signal: Union[str, pl.Expr],
    st_signal: Union[str, pl.Expr],
    cl_signal: Union[str, pl.Expr],
    cs_signal: Union[str, pl.Expr],
    time_idx: Union[str, pl.Expr],
    limit_down: Union[str, pl.Expr],
    limit_up: Union[str, pl.Expr],
    step_1_limit: int = 10,
    step_2_limit: int = 5,
    verbose: bool = False,
) -> pl.Expr:
    return register_plugin_function(
        args=[
            pl.col(ask_price).cast(pl.Float64),
            pl.col(bid_price).cast(pl.Float64),
            pl.col(lg_signal).cast(pl.UInt8),
            pl.col(st_signal).cast(pl.UInt8),
            pl.col(cl_signal).cast(pl.UInt8),
            pl.col(cs_signal).cast(pl.UInt8),
            pl.col(time_idx).cast(pl.UInt64),
            pl.col(limit_down).cast(pl.Float64),
            pl.col(limit_up).cast(pl.Float64),
        ],
        plugin_path=LIB,
        function_name="backtest",
        kwargs={
            "step_1_limit": step_1_limit,
            "step_2_limit": step_2_limit,
            "verbose": verbose,
        },
        is_elementwise=False,
    )