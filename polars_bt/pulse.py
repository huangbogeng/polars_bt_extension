from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def pulse(
    ask_price: str | pl.Expr,
    bid_price: str | pl.Expr,
    lg_signal: str | pl.Expr,
    st_signal: str | pl.Expr,
    cl_signal: str | pl.Expr,
    cs_signal: str | pl.Expr,
    time_idx: str | pl.Expr,
    limit_down: str | pl.Expr,
    limit_up: str | pl.Expr,
    step_1_limit: int = 10,
    step_2_limit: int = 5,
    verbose: bool = False,
) -> pl.Expr:
    def expr(value: str | pl.Expr) -> pl.Expr:
        return pl.col(value) if isinstance(value, str) else value

    return register_plugin_function(
        args=[
            expr(ask_price).cast(pl.Float64),
            expr(bid_price).cast(pl.Float64),
            expr(lg_signal).cast(pl.UInt8),
            expr(st_signal).cast(pl.UInt8),
            expr(cl_signal).cast(pl.UInt8),
            expr(cs_signal).cast(pl.UInt8),
            expr(time_idx).cast(pl.UInt64),
            expr(limit_down).cast(pl.Float64),
            expr(limit_up).cast(pl.Float64),
        ],
        plugin_path=LIB,
        function_name="pulse",
        kwargs={
            "step_1_limit": step_1_limit,
            "step_2_limit": step_2_limit,
            "verbose": verbose,
        },
        is_elementwise=False,
        returns_scalar=True,
    )
