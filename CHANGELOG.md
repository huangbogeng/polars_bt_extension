# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Split fast CI checks from parallel Rust checks and distribution builds, then
  collect wheel tests and artifact verification in a single CI gate
- Publish the exact main CI distributions after commit, version and SHA256
  verification, replacing release recompilation and repeated matrix tests
- Pin runners, reduce native debug artifacts, and separate matrix cache keys

### Added

- Release artifact manifests, provenance validation and failure-path tests
- Main-branch release rehearsals that verify artifacts without uploading to PyPI

## [0.2.4] - 2026-10-30

### Changed

- Support Polars 2.1.x on Python 3.11–3.12 while retaining Polars 2.0.x support
- Keep Python 3.10 on Polars 2.0.x through dependency markers because Polars 2.1
  requires Python >=3.11
- Extend CI and release wheel-test matrices to cover both Polars minor versions
- Retain the verified Rust plugin stack and all three backtest APIs

### Added

- Regression coverage for numeric-string normalization before native execution

## [0.2.3] - 2026-10-09

### Changed

- Adapt to Python Polars >=2.0.0,<2.1 and rebuild with the existing Rust Polars
  0.55.2 / pyo3-polars 0.28 stack; preserve the three backtest APIs
- Update installation requirements and both wheel-test workflow matrices
- Document explicit panel ordering and eager execution under Polars 2.0

### Added

- Regression coverage for lazy preprocessing followed by eager whole-table
  backtests, including streaming engine affinity

## [0.2.2] - 2026-09-12

### Changed

- Upgrade to Python Polars >=1.44.2,<1.45 with Rust Polars 0.55.2,
  pyo3-polars 0.28, and PyO3 0.29; rebuild native extensions for this combination
- Align development dependencies and wheel CI with the supported Polars range
- Document all three engines and their plugin compatibility boundaries

### Added

- `tempo`, a timestamp-based cross-sectional engine with T+1 holdings,
  complete-panel validation, and Polars verbose diagnostics
- Plugin regression coverage for chunked and sliced inputs, date/datetime types,
  categorical assets, and output schema inference
- Rust/Python formatting command and Python formatting checks in CI and releases

### Fixed

- Example run targets now use the maintained example script

## [0.2.1] - 2026-07-22

### Fixed

- `mosaic` now accepts nullable numeric expressions without exposing Rust contiguous-array errors
- Numeric nulls remain observable as NaN diagnostics instead of being silently replaced with zero
- Residual Rust-side numeric nulls now report the affected public input name

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

[Unreleased]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.2.2...HEAD
[0.2.2]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/huangbogeng/polars_bt_extension/compare/v0.1.2...v0.2.0
[0.1.2]: https://github.com/huangbogeng/polars_bt_extension/releases/tag/v0.1.2
