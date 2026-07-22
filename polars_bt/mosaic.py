from pathlib import Path

import polars as pl
from polars.plugins import register_plugin_function

LIB = Path(__file__).parent


def _expr(value: str | pl.Expr) -> pl.Expr:
    return pl.col(value) if isinstance(value, str) else value


def mosaic(
    date: str | pl.Expr,
    weight: str | pl.Expr,
    ovn_ret: str | pl.Expr,
    ind_ret: str | pl.Expr,
    buyable: str | pl.Expr,
    sellable: str | pl.Expr,
    prev_close: str | pl.Expr,
    vwap: str | pl.Expr,
    is_rebalance: str | pl.Expr,
    *,
    asset_num: int,
    st_fee: float = 6e-4,
    lg_fee: float = 1e-4,
) -> pl.Expr:
    return register_plugin_function(
        args=[
            _expr(date),
            _expr(weight).cast(pl.Float64),
            _expr(ovn_ret).cast(pl.Float64),
            _expr(ind_ret).cast(pl.Float64),
            _expr(buyable).cast(pl.Boolean),
            _expr(sellable).cast(pl.Boolean),
            _expr(prev_close).cast(pl.Float64),
            _expr(vwap).cast(pl.Float64),
            _expr(is_rebalance).cast(pl.Boolean),
        ],
        plugin_path=LIB,
        function_name="mosaic",
        kwargs={"asset_num": asset_num, "st_fee": st_fee, "lg_fee": lg_fee},
        is_elementwise=False,
        changes_length=True,
    )
