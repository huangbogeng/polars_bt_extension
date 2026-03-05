"""Tests for polars_bt_extension backtesting engine."""

import polars as pl
from polars_bt import backtest


def test_basic_backtest():
    """Test basic backtest functionality."""
    data = pl.DataFrame({
        "ask_price": [100.0, 101.0, 102.0, 101.5, 100.5],
        "bid_price": [99.5, 100.5, 101.5, 101.0, 100.0],
        "lg_signal": [1, 0, 0, 1, 0],
        "st_signal": [0, 1, 0, 0, 1],
        "cl_signal": [0, 0, 0, 0, 0],
        "cs_signal": [0, 0, 0, 0, 0],
        "time_idx": [1000, 2000, 3000, 4000, 5000],
        "limit_down": [90.0] * 5,
        "limit_up": [110.0] * 5,
    })

    result = data.select(
        backtest(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=10,
            step_2_limit=5,
            verbose=False,
        ).alias("backtest_result")
    )

    assert result.shape == (1, 1)
    assert "backtest_result" in result.columns


def test_empty_signals():
    """Test backtest with no signals."""
    data = pl.DataFrame({
        "ask_price": [100.0, 101.0, 102.0],
        "bid_price": [99.5, 100.5, 101.5],
        "lg_signal": [0, 0, 0],
        "st_signal": [0, 0, 0],
        "cl_signal": [0, 0, 0],
        "cs_signal": [0, 0, 0],
        "time_idx": [1000, 2000, 3000],
        "limit_down": [90.0] * 3,
        "limit_up": [110.0] * 3,
    })

    result = data.select(
        backtest(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=10,
            step_2_limit=5,
            verbose=False,
        ).alias("backtest_result")
    )

    assert result.shape == (1, 1)


def test_custom_limits():
    """Test backtest with custom limits."""
    data = pl.DataFrame({
        "ask_price": [100.0, 101.0],
        "bid_price": [99.5, 100.5],
        "lg_signal": [1, 0],
        "st_signal": [0, 0],
        "cl_signal": [0, 0],
        "cs_signal": [0, 0],
        "time_idx": [1000, 2000],
        "limit_down": [95.0] * 2,
        "limit_up": [105.0] * 2,
    })

    result = data.select(
        backtest(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=5,
            step_2_limit=2,
            verbose=False,
        ).alias("backtest_result")
    )

    assert result.shape == (1, 1)


if __name__ == "__main__":
    test_basic_backtest()
    print("✅ test_basic_backtest passed")
    
    test_empty_signals()
    print("✅ test_empty_signals passed")
    
    test_custom_limits()
    print("✅ test_custom_limits passed")
    
    print("\n✅ All tests passed!")