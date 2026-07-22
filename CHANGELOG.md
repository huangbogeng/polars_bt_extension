# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-07-22

### Added
- `pulse`, the isolated name for the existing T0 quote/signal engine
- `mosaic`, a zero-IPC cross-sectional engine for dense daily panels
- Python parity coverage and a 12.5-million-row performance benchmark

### Changed
- Package version is now 0.2.0 and supports CPython 3.10–3.12 with Polars 1.43.x
- The old public `backtest` name is replaced by `pulse`
- Rust modules are split by the Pulse and Mosaic state models

### Fixed
- CI now builds the current crate and runs the current Python test suite

## [0.1.2] - 2025-03-04

### Added
- Initial version
- Core backtesting functionality
- Order matching algorithms
- Position management
- Python API

[Unreleased]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/huangbogeng/polars_bt_extension/releases/tag/v0.1.2
