from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def _expr(value: str | pl.Expr) -> pl.Expr:
    return pl.col(value) if isinstance(value, str) else value


def _numeric_expr(value: str | pl.Expr) -> pl.Expr:
    return _expr(value).cast(pl.Float64).fill_null(float("nan"))


def tempo(
    datetime: str | pl.Expr,
    asset: str | pl.Expr,
    weight: str | pl.Expr,
    period_ret: str | pl.Expr,
    buyable: str | pl.Expr,
    sellable: str | pl.Expr,
    is_rebalance: str | pl.Expr,
    *,
    t1: bool = True,
    st_fee: float = 6e-4,
    lg_fee: float = 1e-4,
) -> pl.Expr:
    datetime_expr = _expr(datetime).cast(pl.Datetime("us"))
    return register_plugin_function(
        args=[
            datetime_expr,
            datetime_expr.dt.date(),
            _expr(asset).cast(pl.String),
            _numeric_expr(weight),
            _numeric_expr(period_ret),
            _expr(buyable).cast(pl.Boolean),
            _expr(sellable).cast(pl.Boolean),
            _expr(is_rebalance).cast(pl.Boolean),
        ],
        plugin_path=LIB,
        function_name="tempo",
        kwargs={"t1": t1, "st_fee": st_fee, "lg_fee": lg_fee},
        is_elementwise=False,
        changes_length=True,
    )
