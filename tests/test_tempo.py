from __future__ import annotations

from datetime import datetime

import polars as pl
import pytest
from polars.testing import assert_frame_equal

from polars_bt import tempo
from tests.reference_tempo import reference_tempo


def dense_panel() -> pl.DataFrame:
    return pl.DataFrame(
        {
            "datetime": [
                datetime(2024, 1, 2, 9, 31),
                datetime(2024, 1, 2, 9, 31),
                datetime(2024, 1, 2, 14, 30),
                datetime(2024, 1, 2, 14, 30),
            ],
            "asset": ["A", "B", "A", "B"],
            "weight": [0.4, 0.4, 0.0, 0.8],
            "period_ret": [0.0, 0.0, 0.01, -0.01],
            "buyable": [True] * 4,
            "sellable": [True] * 4,
            "is_rebalance": [True] * 4,
        }
    )


def run_tempo(
    df: pl.DataFrame,
    *,
    t1: bool = True,
    st_fee: float = 0.0,
    lg_fee: float = 0.0,
) -> pl.DataFrame:
    return df.select(
        tempo(
            "datetime",
            "asset",
            "weight",
            "period_ret",
            "buyable",
            "sellable",
            "is_rebalance",
            t1=t1,
            st_fee=st_fee,
            lg_fee=lg_fee,
        ).alias("result")
    ).unnest("result")


def test_tempo_returns_one_row_per_datetime():
    result = run_tempo(dense_panel())

    assert result.columns == [
        "datetime",
        "cash",
        "nav",
        "turnover",
        "holding_count",
    ]
    assert result.height == 2
    assert result.schema["datetime"] == pl.Datetime("us")


def multiday_panel() -> pl.DataFrame:
    timestamps = [
        datetime(2024, 1, 2, 9, 31),
        datetime(2024, 1, 2, 11, 0),
        datetime(2024, 1, 2, 14, 30),
        datetime(2024, 1, 3, 9, 31),
    ]
    return pl.DataFrame(
        {
            "datetime": [
                timestamp
                for timestamp in timestamps
                for _ in range(2)
            ],
            "asset": ["A", "B"] * len(timestamps),
            "weight": [0.8, 0.0, 0.8, 0.0, 0.0, 0.8, 0.0, 0.8],
            "period_ret": [0.0, 0.0, 0.1, 0.0, 0.0, 0.0, -0.02, 0.01],
            "buyable": [True, True, True, True, True, False, True, True],
            "sellable": [True, True, True, True, True, True, True, True],
            "is_rebalance": [
                True,
                True,
                False,
                False,
                True,
                True,
                True,
                True,
            ],
        }
    )


@pytest.mark.parametrize("t1", [True, False])
@pytest.mark.parametrize(
    ("st_fee", "lg_fee"),
    [(0.0, 0.0), (6e-4, 1e-4)],
)
def test_tempo_matches_reference(
    t1: bool,
    st_fee: float,
    lg_fee: float,
):
    panel = multiday_panel()

    actual = run_tempo(panel, t1=t1, st_fee=st_fee, lg_fee=lg_fee)
    expected = reference_tempo(
        panel,
        t1=t1,
        st_fee=st_fee,
        lg_fee=lg_fee,
    )

    assert_frame_equal(actual, expected, rel_tol=1e-12, abs_tol=1e-12)


def test_t1_blocks_same_day_sale_and_releases_next_day():
    result = run_tempo(multiday_panel(), t1=True)

    assert result["turnover"][2] == 0.0
    assert result["turnover"][3] > 1.5


@pytest.mark.parametrize("column", ["weight", "period_ret"])
def test_nullable_numeric_input_matches_nan_semantics(column: str):
    panel = dense_panel()
    nullable = panel.with_columns(
        pl.when(pl.int_range(pl.len()) == 0)
        .then(None)
        .otherwise(pl.col(column))
        .alias(column)
    )
    nan = panel.with_columns(
        pl.when(pl.int_range(pl.len()) == 0)
        .then(float("nan"))
        .otherwise(pl.col(column))
        .alias(column)
    )

    assert_frame_equal(run_tempo(nullable), run_tempo(nan))


def test_rejects_changed_asset_order():
    panel = dense_panel().with_columns(
        pl.when(pl.int_range(pl.len()) >= 2)
        .then(pl.col("asset").reverse())
        .otherwise(pl.col("asset"))
        .alias("asset")
    )

    with pytest.raises(Exception, match="asset order mismatch"):
        run_tempo(panel)


def test_rejects_incomplete_asset_block():
    with pytest.raises(Exception, match="incomplete asset block"):
        run_tempo(dense_panel().head(3))


def test_rejects_mixed_rebalance_flags():
    panel = dense_panel().with_columns(
        pl.Series("is_rebalance", [True, False, True, True])
    )

    with pytest.raises(Exception, match="mixed is_rebalance"):
        run_tempo(panel)


def test_rejects_negative_weight():
    panel = dense_panel().with_columns(
        pl.Series("weight", [-0.1, 0.4, 0.0, 0.8])
    )

    with pytest.raises(Exception, match="weight must be non-negative"):
        run_tempo(panel)


@pytest.mark.parametrize("column", ["weight", "period_ret"])
def test_rejects_infinite_numeric_input(column: str):
    panel = dense_panel().with_columns(
        pl.when(pl.int_range(pl.len()) == 1)
        .then(float("inf"))
        .otherwise(pl.col(column))
        .alias(column)
    )

    with pytest.raises(Exception, match=rf"{column} cannot be infinite"):
        run_tempo(panel)


@pytest.mark.parametrize("column", ["buyable", "sellable", "is_rebalance"])
def test_rejects_nullable_boolean_input(column: str):
    panel = dense_panel().with_columns(
        pl.when(pl.int_range(pl.len()) == 0)
        .then(None)
        .otherwise(pl.col(column))
        .alias(column)
    )

    with pytest.raises(Exception, match=rf"Boolean input {column} cannot contain nulls"):
        run_tempo(panel)
