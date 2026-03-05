# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- Initial release of polars_bt_extension backtesting engine plugin
- High-performance order matching with DefaultMatcher and EasyMatcher
- Comprehensive position management with P&L tracking
- Support for trading limits (limit up/down)
- Python API integration with Polars
- Complete documentation and examples

### Changed
- Refactored project structure for better maintainability
- Optimized code for performance with zero-copy data transfer
- Improved code quality with clippy and rustfmt

### Fixed
- All clippy warnings resolved
- Documentation formatting issues fixed

## [0.1.2] - 2025-03-04

### Added
- Initial version
- Core backtesting functionality
- Order matching algorithms
- Position management
- Python API

[Unreleased]: https://github.com/yourusername/polars_bt_extension/compare/v0.1.2...HEAD
[0.1.2]: https://github.com/yourusername/polars_bt_extension/releases/tag/v0.1.2