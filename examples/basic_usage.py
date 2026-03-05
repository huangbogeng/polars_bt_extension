"""
Example usage of polars_bt_extension backtesting engine.

This example demonstrates how to use the backtest function
to evaluate a simple trading strategy.
"""

import polars as pl
import polars_bt as pbt


def main():
    """Run a simple backtest example."""
    
    # Create sample market data
    # In a real scenario, you would load this from a file or database
    data = pl.DataFrame({
        "ask_price": [
            100.0, 101.0, 102.0, 101.5, 100.5,
            99.0, 98.5, 99.5, 100.5, 101.0,
            102.5, 103.0, 102.0, 101.0, 100.0
        ],
        "bid_price": [
            99.5, 100.5, 101.5, 101.0, 100.0,
            98.5, 98.0, 99.0, 100.0, 100.5,
            102.0, 102.5, 101.5, 100.5, 99.5
        ],
        # Long signals (1 = buy signal, 0 = no signal)
        "lg_signal": [
            1, 0, 0, 0, 0,
            0, 0, 1, 0, 0,
            0, 0, 0, 0, 0
        ],
        # Short signals (1 = sell signal, 0 = no signal)
        "st_signal": [
            0, 1, 0, 0, 1,
            0, 0, 0, 0, 1,
            0, 0, 0, 0, 0
        ],
        # Close long signals (1 = close long position, 0 = no signal)
        "cl_signal": [0] * 15,
        # Close short signals (1 = close short position, 0 = no signal)
        "cs_signal": [0] * 15,
        # Time index (timestamp in milliseconds)
        "time_idx": [
            1000, 2000, 3000, 4000, 5000,
            6000, 7000, 8000, 9000, 10000,
            11000, 12000, 13000, 14000, 15000
        ],
        # Limit down price (minimum allowed trading price)
        "limit_down": [90.0] * 15,
        # Limit up price (maximum allowed trading price)
        "limit_up": [110.0] * 15,
    })

    print("Input Data:")
    print(data)
    print()

    # Run backtest
    result = data.select(
        pbt.backtest(
            ask_price="ask_price",
            bid_price="bid_price",
            lg_signal="lg_signal",
            st_signal="st_signal",
            cl_signal="cl_signal",
            cs_signal="cs_signal",
            time_idx="time_idx",
            limit_down="limit_down",
            limit_up="limit_up",
            step_1_limit=10,  # Maximum signals per round
            step_2_limit=5,   # Maximum exposure limit
            verbose=False,       # Disable verbose logging
        ).alias("backtest_result")
    )

    print("Backtest Results:")
    print(result)
    print()

    # Extract individual metrics
    metrics = result.select(
        pl.col("backtest_result").struct.field("*")
    )

    print("Detailed Metrics:")
    print(metrics)


def example_with_custom_matcher():
    """Example showing how to use different matchers."""
    
    print("\n" + "="*60)
    print("Example with Custom Matcher")
    print("="*60)
    
    # Set environment variable to use EasyMatcher
    # Note: This must be set before importing the module
    import os
    os.environ["LOFIEX_MATCHER"] = "easy"
    
    # You would need to reimport or restart the Python interpreter
    # for this to take effect
    print("To use EasyMatcher, set LOFIEX_MATCHER=easy")
    print("To use DefaultMatcher, set LOFIEX_MATCHER=default")
    print("Then restart your Python interpreter.")


def example_with_multiple_assets():
    """Example showing backtesting multiple assets."""
    
    print("\n" + "="*60)
    print("Example with Multiple Assets")
    print("="*60)
    
    # Create data for multiple assets
    data = pl.DataFrame({
        "asset": ["AAPL", "AAPL", "AAPL", "MSFT", "MSFT", "MSFT"],
        "ask_price": [150.0, 151.0, 152.0, 300.0, 301.0, 302.0],
        "bid_price": [149.5, 150.5, 151.5, 299.5, 300.5, 301.5],
        "lg_signal": [1, 0, 0, 1, 0, 0],
        "st_signal": [0, 1, 0, 0, 1, 0],
        "cl_signal": [0, 0, 0, 0, 0, 0],
        "cs_signal": [0, 0, 0, 0, 0, 0],
        "time_idx": [1000, 2000, 3000, 1000, 2000, 3000],
        "limit_down": [140.0, 140.0, 140.0, 290.0, 290.0, 290.0],
        "limit_up": [160.0, 160.0, 160.0, 310.0, 310.0, 310.0],
    })

    print("Multiple Assets Data:")
    print(data)
    print()

    # Group by asset and run backtest for each
    result = data.group_by("asset").agg(
        pbt.backtest(
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

    print("Backtest Results by Asset:")
    print(result)


if __name__ == "__main__":
    main()
    example_with_custom_matcher()
    example_with_multiple_assets()
    
    print("\n" + "="*60)
    print("For more information, see README.md")
    print("="*60)