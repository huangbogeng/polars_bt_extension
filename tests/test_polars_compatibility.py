"""Plugin boundary regressions for eager, whole-table backtests."""

from functools import partial

import polars as pl
import polars.selectors as cs
import pytest
from polars.testing import assert_frame_equal, assert_series_equal

from polars_bt import mosaic, tempo
from tests.test_mosaic import dense_panel as mosaic_panel
from tests.test_mosaic import make_seeded_panel, run_mosaic
from tests.test_pulse import run_pulse
from tests.test_tempo import multiday_panel, run_tempo


def pulse_panel() -> pl.DataFrame:
    return pl.DataFrame(
        {
            "ask_price": [100.0, 101.0, 102.0, 101.5, 100.5],
            "bid_price": [99.5, 100.5, 101.5, 101.0, 100.0],
            "lg_signal": [1, 0, 0, 1, 0],
            "st_signal": [0, 1, 0, 0, 1],
            "cl_signal": [0, 0, 1, 0, 0],
            "cs_signal": [0, 0, 0, 0, 1],
            "time_idx": [1000, 2000, 3000, 4000, 5000],
            "limit_down": [90.0] * 5,
            "limit_up": [110.0] * 5,
        },
        schema_overrides={
            "lg_signal": pl.UInt8,
            "st_signal": pl.UInt8,
            "cl_signal": pl.UInt8,
            "cs_signal": pl.UInt8,
            "time_idx": pl.UInt64,
        },
    )


@pytest.mark.parametrize("layout", ["multi_chunk", "offset_slice"])
@pytest.mark.parametrize(
    ("make_panel", "run"),
    [
        pytest.param(pulse_panel, run_pulse, id="pulse"),
        pytest.param(
            partial(
                make_seeded_panel,
                days=4,
                assets=3,
                seed=7,
                block_buys=True,
                block_sells=True,
            ),
            partial(run_mosaic, asset_num=3, st_fee=6e-4, lg_fee=1e-4),
            id="mosaic",
        ),
        pytest.param(multiday_panel, run_tempo, id="tempo"),
    ],
)
def test_eager_backtests_preserve_results_across_input_layouts(
    make_panel, run, layout: str
):
    contiguous = make_panel().rechunk()
    assert all(series.n_chunks() == 1 for series in contiguous)

    if layout == "multi_chunk":
        # Deliberately split inside asset blocks, so chunk boundaries cannot
        # accidentally become portfolio or timestamp boundaries.
        data = pl.concat(
            [contiguous.head(1), contiguous.slice(1, 2), contiguous.slice(3)],
            rechunk=False,
        )
        assert all(series.n_chunks() == 3 for series in data)
    else:
        # Slice a single allocation at an offset of one, including Boolean
        # bitmaps. Native input dtypes avoid casts hiding the sliced buffers.
        padded = pl.concat(
            [contiguous.head(1), contiguous, contiguous.tail(1)], rechunk=True
        )
        data = padded.slice(1, contiguous.height)
        assert all(series.n_chunks() == 1 for series in data)

    assert_frame_equal(data, contiguous, check_exact=True)
    assert_frame_equal(run(data), run(contiguous), check_exact=True)


@pytest.mark.parametrize(
    "date_dtype",
    [pl.String, pl.Date, pl.Datetime("ms"), pl.Datetime("us"), pl.Datetime("ns")],
    ids=["string", "date", "datetime_ms", "datetime_us", "datetime_ns"],
)
def test_mosaic_date_dtype_matches_inferred_schema(date_dtype):
    data = mosaic_panel()
    if date_dtype != pl.String:
        data = data.with_columns(pl.col("date").str.to_date().cast(date_dtype))
    expression = mosaic(
        "date",
        "weight",
        "ovn_ret",
        "ind_ret",
        "buyable",
        "sellable",
        "prev_close",
        "vwap",
        "is_rebalance",
        asset_num=2,
        st_fee=0.0,
        lg_fee=0.0,
    ).alias("result")

    result = data.select(expression).unnest("result")
    # Only infer the lazy schema; execution remains an eager whole-table call.
    inferred = data.lazy().select(expression).unnest("result").collect_schema()

    assert result.schema == inferred
    assert result.schema["date"] == date_dtype
    assert_series_equal(result["date"], data["date"].gather([0, 2]))
    assert_frame_equal(result.drop("date"), run_mosaic(mosaic_panel()).drop("date"))


@pytest.mark.parametrize("time_unit", ["ms", "ns"])
@pytest.mark.parametrize("asset_dtype", [pl.String, pl.Categorical])
def test_tempo_normalizes_datetime_and_asset_dtypes(time_unit, asset_dtype):
    baseline = multiday_panel()
    data = baseline.with_columns(
        pl.col("datetime").cast(pl.Datetime(time_unit)),
        pl.col("asset").cast(asset_dtype),
    )
    expression = tempo(
        "datetime",
        "asset",
        "weight",
        "period_ret",
        "buyable",
        "sellable",
        "is_rebalance",
        st_fee=0.0,
        lg_fee=0.0,
    ).alias("result")

    result = data.select(expression).unnest("result")
    inferred = data.lazy().select(expression).unnest("result").collect_schema()

    assert result.schema == inferred
    assert result.schema["datetime"] == pl.Datetime("us")
    assert_frame_equal(result, run_tempo(baseline), check_exact=True)


@pytest.mark.parametrize("engine_affinity", ["auto", "streaming", "in-memory"])
@pytest.mark.parametrize(
    ("make_panel", "run"),
    [
        pytest.param(pulse_panel, run_pulse, id="pulse"),
        pytest.param(mosaic_panel, run_mosaic, id="mosaic"),
        pytest.param(multiday_panel, run_tempo, id="tempo"),
    ],
)
def test_lazy_preprocessing_materialized_before_eager_backtest(
    make_panel, run, engine_affinity: str
):
    baseline = make_panel()
    # Restore a known row order after preprocessing, including when collect()
    # uses Polars 2.0's default streaming engine. The plugin sees the complete
    # materialized panel, never an individual preprocessing batch.
    with pl.Config(engine_affinity=engine_affinity):
        prepared = (
            baseline.with_row_index("_input_order")
            .reverse()
            .lazy()
            .sort("_input_order")
            .drop("_input_order")
            .collect()
        )
        actual = run(prepared)

    assert_frame_equal(prepared, baseline, check_exact=True)
    assert_frame_equal(actual, run(baseline), check_exact=True)


@pytest.mark.parametrize(
    ("make_panel", "run"),
    [
        pytest.param(pulse_panel, run_pulse, id="pulse"),
        pytest.param(mosaic_panel, run_mosaic, id="mosaic"),
        pytest.param(multiday_panel, run_tempo, id="tempo"),
    ],
)
def test_backtests_cast_numeric_strings_before_native_execution(make_panel, run):
    baseline = make_panel()
    # Polars 2.1 rejects mixed string/numeric arithmetic. The wrappers must
    # still normalize numeric input columns before passing them to Rust.
    prepared = baseline.lazy().with_columns(cs.numeric().cast(pl.String)).collect()

    assert_frame_equal(run(prepared), run(baseline), check_exact=True)
