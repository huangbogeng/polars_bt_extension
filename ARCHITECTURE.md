# Project Structure

This document provides an overview of the polars_bt_extension project structure and organization.

## Directory Layout

```
polars_bt_extension/
├── .github/
│   └── workflows/
│       ├── ci.yml          # Continuous Integration workflow
│       └── release.yml    # Release and PyPI publishing workflow
├── polars_bt_extension/                # Python package
│   ├── __init__.py       # Package initialization
│   └── backtest.py       # Python API for backtest function
├── src/                  # Rust source code
│   ├── exchange/
│   │   ├── mod.rs        # Exchange simulator and main backtest logic
│   │   └── matcher.rs    # Order matching algorithms
│   ├── order.rs          # Order management structures
│   ├── position.rs       # Position tracking and P&L calculation
│   └── lib.rs           # Library entry point
├── .gitignore           # Git ignore patterns
├── .cargo/              # Cargo configuration
├── Cargo.toml           # Rust dependencies and metadata
├── CHANGELOG.md         # Version changelog
├── CONTRIBUTING.md       # Contribution guidelines
├── LICENSE              # MIT License
├── Makefile            # Development tasks
├── pyproject.toml       # Python package configuration
├── README.md            # Project documentation
├── requirements.txt      # Python development dependencies
├── rust-toolchain.toml  # Rust toolchain configuration
├── rustfmt.toml        # Rust formatting configuration
└── test_backtest.py     # Python tests
```

## Core Modules

### `src/exchange/`

**Purpose**: Low-frequency exchange simulator with order matching capabilities

**Key Components**:
- `LofiEx`: Main exchange simulator
- `Strategy`: Trading strategy implementation
- `backtest()`: Main Polars plugin function

**Dependencies**: `order`, `position`, `matcher`

### `src/order.rs`

**Purpose**: Order management and direction handling

**Key Components**:
- `Order`: Order structure with price, quantity, direction, and timestamp
- `OrderDirection`: Long/Short direction enumeration

**Dependencies**: `chrono` (for timestamp formatting)

### `src/position.rs`

**Purpose**: Position tracking and profit/loss calculation

**Key Components**:
- `Position`: Position management with P&L tracking
- `PositionDirection`: Long/Short/Flat position direction
- `Summary`: Backtest result summary

**Dependencies**: `order`

### `src/exchange/matcher.rs`

**Purpose**: Pluggable order matching algorithms

**Key Components**:
- `Matcher`: Trait for order matching algorithms
- `DefaultMatcher`: Strict cross matching with limit restrictions
- `EasyMatcher`: Lenient matching without limit restrictions
- `MarketData`: Current market price information
- `DealResp`: Order matching result

**Dependencies**: `order`, `position`

### `polars_bt_extension/` (Python Package)

**Purpose**: Python API interface

**Key Components**:
- `backtest()`: Python wrapper for Rust backtest function

**Dependencies**: `polars`, `pyo3-polars`

## Build System

### Rust (Cargo)

- **Cargo.toml**: Defines Rust dependencies and package metadata
- **rust-toolchain.toml**: Specifies Rust toolchain version
- **rustfmt.toml**: Rust code formatting rules

### Python (Maturin)

- **pyproject.toml**: Python package configuration
- **requirements.txt**: Development dependencies

### CI/CD

- **.github/workflows/ci.yml**: Runs tests on push/PR
- **.github/workflows/release.yml**: Builds and publishes on tag

## Development Workflow

1. **Code Changes**: Modify Rust or Python code
2. **Testing**: Run `cargo test` and `python test_backtest.py`
3. **Quality Checks**: Run `cargo clippy` and `cargo fmt`
4. **Build**: Run `maturin develop` or `maturin develop --release`
5. **Commit**: Follow conventional commit format
6. **Push**: CI will automatically run tests

## Key Design Decisions

### Flat Directory Structure

- Removed `bt_lite/` intermediate directory for simpler structure
- Direct module organization under `src/`
- Easier navigation and maintenance

### Separation of Concerns

- **Exchange**: Order matching and execution
- **Order**: Order data structures
- **Position**: Position tracking and P&L
- **Matcher**: Pluggable matching algorithms

### Performance Optimization

- Zero-copy data transfer with Polars
- Native Rust implementation
- Efficient memory management with PolarsAllocator
- Array literals instead of `vec!` where possible

### Code Quality

- All public APIs documented with English comments
- Clippy strict mode (`-D warnings`)
- Rustfmt for consistent formatting
- Comprehensive test coverage

## Dependencies

### Rust

- `polars`: DataFrame library
- `pyo3`: Python bindings
- `pyo3-polars`: Polars integration
- `serde`: Serialization
- `chrono`: Date/time handling

### Python

- `polars`: DataFrame library
- `pytest`: Testing framework
- `maturin`: Build tool

## Testing

### Unit Tests

- Rust tests in `src/` modules
- Python tests in `test_backtest.py`

### Integration Tests

- CI runs tests on multiple Python versions (3.9-3.12)
- CI runs tests on multiple OS (Linux, macOS, Windows)

### Quality Checks

- `cargo clippy`: Linting
- `cargo fmt`: Formatting
- `cargo doc`: Documentation

## Release Process

1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Create git tag: `git tag -a v1.0.0 -m "Release v1.0.0"`
4. Push tag: `git push origin v1.0.0`
5. GitHub Actions builds wheels and publishes to PyPI

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md) for detailed guidelines.

## License

MIT License - see [LICENSE](LICENSE) file for details.