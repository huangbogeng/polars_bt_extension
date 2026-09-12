SHELL=/bin/bash
PYTHON=$(CURDIR)/.venv/bin/python
PYTHON_LIBDIR=$(shell $(PYTHON) -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))')
PYTHON_SOURCES=polars_bt tests benchmarks examples

.PHONY: venv install install-release fmt pre-commit test run run-release

venv:
	python3 -m venv .venv
	.venv/bin/pip install -r requirements.txt

install:
	unset CONDA_PREFIX && \
	source .venv/bin/activate && maturin develop --locked

install-release:
	unset CONDA_PREFIX && \
	source .venv/bin/activate && maturin develop --release --locked

fmt:
	cargo fmt --all
	$(PYTHON) -m ruff format $(PYTHON_SOURCES)

pre-commit:
	cargo fmt --all -- --check
	cargo clippy --locked --package polars_bt --all-targets --all-features -- -D warnings
	PYO3_PYTHON=$(PYTHON) LD_LIBRARY_PATH=$(PYTHON_LIBDIR) cargo test --locked --all-targets
	$(PYTHON) -m pytest tests -q
	$(PYTHON) -m ruff check $(PYTHON_SOURCES)
	$(PYTHON) -m ruff format --check $(PYTHON_SOURCES)

test:
	$(PYTHON) -m pytest tests -q

run: install
	$(PYTHON) examples/basic_usage.py

run-release: install-release
	$(PYTHON) examples/basic_usage.py
