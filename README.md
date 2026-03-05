# polars_bt

A high-performance backtesting engine plugin for Polars, designed for financial trading strategy evaluation with efficient order matching and position management.

## Features

- **High Performance**: Built with Rust and integrated with Polars for lightning-fast backtesting
- **Flexible Order Matching**: Supports multiple matching algorithms (DefaultMatcher, EasyMatcher)
- **Position Management**: Comprehensive tracking of long/short positions with P&L calculation
- **Limit Price Support**: Built-in support for trading limits (limit up/down)
- **Easy Integration**: Simple Python API that works seamlessly with Polars DataFrames
- **Production Ready**: Thoroughly tested with clippy and rustfmt for code quality

## Installation

### From Source

```bash
# Clone the repository
git clone https://github.com/yourusername/polars_bt.git
cd polars_bt

# Install in development mode
maturin develop

# Or install in release mode for better performance
maturin develop --release
```

### From PyPI (Coming Soon)

```bash
pip install polars_bt
```

## Quick Start

```python
import polars as pl
from polars_bt import backtest

# Create sample data
data = pl.DataFrame({
    "ask_price": [100.0, 101.0, 102.0, 101.5, 100.5],
    "bid_price": [99.5, 100.5, 101.5, 101.0, 100.0],
    "lg_signal": [1, 0, 0, 1, 0],  # Long signal
    "st_signal": [0, 1, 0, 0, 1],  # Short signal
    "cl_signal": [0, 0, 0, 0, 0],  # Close long signal
    "cs_signal": [0, 0, 0, 0, 0],  # Close short signal
    "time_idx": [1000, 2000, 3000, 4000, 5000],
    "limit_down": [90.0] * 5,
    "limit_up": [110.0] * 5,
})

# Run backtest
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

print(result)
```

## API Reference

### `backtest()`

Main backtesting function that processes trading signals and returns performance metrics.

**Parameters:**

- `ask_price` (str | pl.Expr): Ask price column
- `bid_price` (str | pl.Expr): Bid price column
- `lg_signal` (str | pl.Expr): Long signal column (0 or 1)
- `st_signal` (str | pl.Expr): Short signal column (0 or 1)
- `cl_signal` (str | pl.Expr): Close long signal column (0 or 1)
- `cs_signal` (str | pl.Expr): Close short signal column (0 or 1)
- `time_idx` (str | pl.Expr): Time index column
- `limit_down` (str | pl.Expr): Limit down price column
- `limit_up` (str | pl.Expr): Limit up price column
- `step_1_limit` (int, default=10): Maximum number of signals per round
- `step_2_limit` (int, default=5): Maximum exposure limit
- `verbose` (bool, default=False): Enable verbose logging

**Returns:**

A Polars struct containing the following fields:

- `lg_pos` (UInt64): Long position count
- `st_pos` (UInt64): Short position count
- `lg_win` (UInt64): Long position win count
- `st_win` (UInt64): Short position win count
- `all_signal` (UInt64): Total signal count
- `floating_pnl` (Float64): Floating profit and loss
- `realized_pnl` (Float64): Realized profit and loss

## Order Matching Algorithms

polars_bt_extension supports two order matching algorithms:

### DefaultMatcher (Default)

Implements strict cross matching rules:
- **Long orders**: price >= ask_price, available long position > 0, not touching limit up
- **Short orders**: price <= bid_price, available short position > 0, not touching limit down

### EasyMatcher

Implements more lenient matching rules:
- **Long orders**: price >= ask_price, available long position > 0
- **Short orders**: price <= bid_price, available short position > 0
- No limit price restrictions

**To switch matchers, set the environment variable:**

```bash
export LOFIEX_MATCHER=easy  # Use EasyMatcher
export LOFIEX_MATCHER=default  # Use DefaultMatcher (default)
```

## Architecture

The project is organized into several core modules:

- **`exchange`**: Low-frequency exchange simulator with order matching
- **`order`**: Order management and direction handling
- **`position`**: Position tracking and P&L calculation
- **`matcher`**: Pluggable order matching algorithms

## Development

### Prerequisites

- Rust 1.70 or later
- Python 3.9 or later
- maturin

### Building

```bash
# Development build
maturin develop

# Release build (optimized)
maturin develop --release

# Build wheel
maturin build --release
```

### Testing

```bash
# Run Rust tests
cargo test

# Run Python tests
python test_backtest.py

# Run clippy for code quality checks
cargo clippy --all-targets --all-features -- -D warnings

# Format code
cargo fmt
```

### Code Quality

This project maintains high code quality standards:

- **Clippy**: All code passes strict clippy checks with `-D warnings`
- **Rustfmt**: Code is formatted using standard rustfmt rules
- **Documentation**: All public APIs have comprehensive documentation
- **Testing**: Comprehensive test coverage for core functionality

## Performance

polars_bt_extension is optimized for performance:

- **Zero-copy data transfer**: Efficient integration with Polars memory model
- **Native Rust implementation**: Compiled to optimized machine code
- **Minimal overhead**: Direct integration with Polars expression engine
- **Scalable**: Handles large datasets efficiently

## Contributing

Contributions are welcome! Please feel free to submit a Pull Request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## License

This project is licensed under the MIT License - see the LICENSE file for details.

## Acknowledgments

- Built with [Polars](https://pola.rs/) - High-performance DataFrame library
- Powered by [PyO3](https://pyo3.rs/) - Rust bindings for Python
- Inspired by [polars-plugins-tutorial](https://marcogorelli.github.io/polars-plugins-tutorial/)

## Contact

For questions, suggestions, or issues, please open an issue on GitHub.