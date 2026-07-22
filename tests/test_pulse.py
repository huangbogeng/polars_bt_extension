import polars as pl
from polars_bt import pulse


def run_pulse(data: pl.DataFrame, *, step_1_limit: int = 10, step_2_limit: int = 5):
    return data.select(
        pulse(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=step_1_limit,
            step_2_limit=step_2_limit,
        ).alias("pulse")
    )


def test_pulse_returns_the_existing_t0_summary():
    data = pl.DataFrame(
        {
            "ask_price": [100.0, 101.0, 102.0, 101.5, 100.5],
            "bid_price": [99.5, 100.5, 101.5, 101.0, 100.0],
            "lg_signal": [1, 0, 0, 1, 0],
            "st_signal": [0, 1, 0, 0, 1],
            "cl_signal": [0, 0, 0, 0, 0],
            "cs_signal": [0, 0, 0, 0, 0],
            "time_idx": [1000, 2000, 3000, 4000, 5000],
            "limit_down": [90.0] * 5,
            "limit_up": [110.0] * 5,
        }
    )

    result = run_pulse(data)

    assert result.shape == (1, 1)
    assert result.schema["pulse"] == pl.Struct(
        {
            "lg_pos": pl.UInt64,
            "st_pos": pl.UInt64,
            "lg_win": pl.UInt64,
            "st_win": pl.UInt64,
            "all_signal": pl.UInt64,
            "floating_pnl": pl.Float64,
            "realized_pnl": pl.Float64,
        }
    )


def test_pulse_empty_signals():
    data = pl.DataFrame(
        {
            "ask_price": [100.0, 101.0, 102.0],
            "bid_price": [99.5, 100.5, 101.5],
            "lg_signal": [0, 0, 0],
            "st_signal": [0, 0, 0],
            "cl_signal": [0, 0, 0],
            "cs_signal": [0, 0, 0],
            "time_idx": [1000, 2000, 3000],
            "limit_down": [90.0] * 3,
            "limit_up": [110.0] * 3,
        }
    )

    assert run_pulse(data).shape == (1, 1)


def test_pulse_custom_limits():
    data = pl.DataFrame(
        {
            "ask_price": [100.0, 101.0],
            "bid_price": [99.5, 100.5],
            "lg_signal": [1, 0],
            "st_signal": [0, 0],
            "cl_signal": [0, 0],
            "cs_signal": [0, 0],
            "time_idx": [1000, 2000],
            "limit_down": [95.0] * 2,
            "limit_up": [105.0] * 2,
        }
    )

    assert run_pulse(data, step_1_limit=5, step_2_limit=2).shape == (1, 1)
