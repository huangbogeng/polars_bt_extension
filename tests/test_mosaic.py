import random

import polars as pl
import pytest
from polars_bt import mosaic
from polars.testing import assert_frame_equal

from tests.reference_ptrader import reference_ptrader


def dense_panel() -> pl.DataFrame:
    return pl.DataFrame(
        {
            "date": ["2024-01-02", "2024-01-02", "2024-01-03", "2024-01-03"],
            "weight": [0.5, 0.5, 0.0, 1.0],
            "ovn_ret": [0.0, 0.0, 0.0, 0.0],
            "ind_ret": [0.0, 0.0, 0.0, 0.0],
            "buyable": [True, True, True, True],
            "sellable": [True, True, True, True],
            "prev_close": [10.0, 10.0, 10.0, 10.0],
            "vwap": [10.0, 10.0, 10.0, 10.0],
            "is_rebalance": [True, True, True, True],
        }
    )


def run_mosaic(
    df: pl.DataFrame,
    asset_num: int = 2,
    st_fee: float = 0.0,
    lg_fee: float = 0.0,
) -> pl.DataFrame:
    return df.select(
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
            asset_num=asset_num,
            st_fee=st_fee,
            lg_fee=lg_fee,
        ).alias("daily")
    ).unnest("daily")


def make_seeded_panel(
    days: int,
    assets: int,
    seed: int,
    *,
    rebalance_every: int = 1,
    block_buys: bool = False,
    block_sells: bool = False,
) -> pl.DataFrame:
    rng = random.Random(seed)
    dates: list[int] = []
    weights: list[float] = []
    ovn_ret: list[float] = []
    ind_ret: list[float] = []
    buyable: list[bool] = []
    sellable: list[bool] = []
    is_rebalance: list[bool] = []

    for day in range(days):
        raw_weights = [rng.random() for _ in range(assets)]
        total = sum(raw_weights)
        dates.extend([day] * assets)
        weights.extend(0.9 * value / total for value in raw_weights)
        ovn_ret.extend(rng.uniform(-0.02, 0.02) for _ in range(assets))
        ind_ret.extend(rng.uniform(-0.02, 0.02) for _ in range(assets))
        buyable.extend(not block_buys or index % 4 != 0 for index in range(assets))
        sellable.extend(not block_sells or index % 5 != 0 for index in range(assets))
        is_rebalance.extend([day % rebalance_every == 0] * assets)

    rows = days * assets
    return pl.DataFrame(
        {
            "date": dates,
            "weight": weights,
            "ovn_ret": ovn_ret,
            "ind_ret": ind_ret,
            "buyable": buyable,
            "sellable": sellable,
            "prev_close": [10.0] * rows,
            "vwap": [10.0] * rows,
            "is_rebalance": is_rebalance,
        }
    )


def test_mosaic_returns_one_row_per_day():
    result = run_mosaic(dense_panel())

    assert result.columns == ["date", "cash", "nav", "turnover", "holding_count"]
    assert result.height == 2
    assert result["date"].to_list() == ["2024-01-02", "2024-01-03"]


def test_mosaic_rejects_non_rectangular_panel():
    with pytest.raises(Exception, match="divisible by asset_num"):
        run_mosaic(dense_panel().head(3))


@pytest.mark.parametrize(
    ("panel_kwargs", "st_fee", "lg_fee"),
    [
        ({}, 6e-4, 1e-4),
        ({"rebalance_every": 5}, 6e-4, 1e-4),
        ({"block_buys": True}, 6e-4, 1e-4),
        ({"block_sells": True}, 6e-4, 1e-4),
        ({}, 0.002, 0.001),
    ],
    ids=["complete", "sparse_rebalance", "blocked_buys", "blocked_sells", "fees"],
)
def test_mosaic_matches_reference(panel_kwargs, st_fee: float, lg_fee: float):
    assets = 30
    df = make_seeded_panel(days=20, assets=assets, seed=7, **panel_kwargs)
    expected = reference_ptrader(df, assets, st_fee=st_fee, lg_fee=lg_fee)
    actual = run_mosaic(df, asset_num=assets, st_fee=st_fee, lg_fee=lg_fee)

    assert_frame_equal(
        actual,
        expected,
        check_exact=False,
        rel_tol=0.0,
        abs_tol=1e-12,
    )
