import polars as pl
import pytest
from polars_bt import mosaic


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


def test_mosaic_returns_one_row_per_day():
    result = run_mosaic(dense_panel())

    assert result.columns == ["date", "cash", "nav", "turnover", "holding_count"]
    assert result.height == 2
    assert result["date"].to_list() == ["2024-01-02", "2024-01-03"]


def test_mosaic_rejects_non_rectangular_panel():
    with pytest.raises(Exception, match="divisible by asset_num"):
        run_mosaic(dense_panel().head(3))
