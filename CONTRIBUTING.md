# Contributing to polars_bt_extension

Thank you for your interest in contributing to polars_bt_extension! This document provides guidelines and instructions for contributing to the project.

## Code of Conduct

Be respectful, inclusive, and constructive in all interactions. We aim to maintain a welcoming environment for all contributors.

## Getting Started

### Prerequisites

- Rust 1.70 or later
- Python 3.9 or later
- maturin
- Git

### Setting Up Development Environment

```bash
# Clone the repository
git clone https://github.com/yourusername/polars_bt_extension.git
cd polars_bt_extension

# Create a virtual environment
python -m venv .venv
source .venv/bin/activate  # On Windows: .venv\Scripts\activate

# Install development dependencies
pip install maturin polars pytest

# Install the package in development mode
maturin develop
```

## Development Workflow

### 1. Create a Branch

```bash
git checkout -b feature/your-feature-name
# or
git checkout -b fix/your-bug-fix
```

### 2. Make Changes

- Write clean, readable code
- Follow existing code style and conventions
- Add tests for new functionality
- Update documentation as needed

### 3. Code Quality Checks

Before submitting, ensure your code passes all quality checks:

```bash
# Format code
cargo fmt

# Run clippy (linting)
cargo clippy --all-targets --all-features -- -D warnings

# Run tests
cargo test
python test_backtest.py

# Check documentation
cargo doc --no-deps --all-features
```

### 4. Commit Changes

```bash
git add .
git commit -m "Add your commit message here"
```

**Commit Message Guidelines:**

- Use the present tense ("Add feature" not "Added feature")
- Use the imperative mood ("Move cursor to..." not "Moves cursor to...")
- Limit the first line to 72 characters or less
- Reference issues and pull requests liberally

Examples:
- `Add support for custom matchers`
- `Fix memory leak in position tracking`
- `Update documentation for API changes`

### 5. Push and Create Pull Request

```bash
git push origin feature/your-feature-name
```

Then create a Pull Request on GitHub with:
- Clear title and description
- Reference any related issues
- Explain the changes and their purpose
- Include screenshots if applicable

## Coding Standards

### Rust Code

- Follow [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- Use meaningful variable and function names
- Add doc comments for all public APIs
- Prefer `unwrap_or_default()` over manual `match` for Option handling
- Use array literals instead of `vec!` when possible

### Python Code

- Follow [PEP 8](https://pep8.org/)
- Use type hints where appropriate
- Add docstrings for all public functions
- Keep functions focused and small

### Documentation

- All public APIs must have documentation
- Use clear, concise language
- Include examples for complex functionality
- Keep documentation up to date with code changes

## Testing

### Writing Tests

- Write tests for all new functionality
- Test both success and failure cases
- Use descriptive test names
- Keep tests independent and fast

### Running Tests

```bash
# Rust tests
cargo test

# Python tests
pytest test_backtest.py -v

# Run specific test
cargo test test_name
pytest test_backtest.py::test_function -v
```

## Project Structure

```
polars_bt_extension/
├── src/
│   ├── exchange/       # Exchange simulator and matching logic
│   ├── order.rs        # Order management
│   ├── position.rs     # Position tracking
│   └── lib.rs         # Library entry point
├── polars_bt_extension/             # Python package
│   ├── __init__.py
│   └── backtest.py    # Python API
├── tests/             # Test files
├── .github/           # GitHub workflows
├── Cargo.toml         # Rust dependencies
├── pyproject.toml     # Python configuration
└── README.md          # Project documentation
```

## Release Process

Releases are handled by maintainers through GitHub Actions:

1. Update version in `Cargo.toml`
2. Create a git tag: `git tag -a v1.0.0 -m "Release version 1.0.0"`
3. Push the tag: `git push origin v1.0.0`
4. GitHub Actions will automatically build and publish to PyPI

## Questions?

Feel free to:
- Open an issue for bugs or feature requests
- Start a discussion for questions or ideas
- Contact maintainers directly for urgent matters

Thank you for contributing to polars_bt_extension! 🚀