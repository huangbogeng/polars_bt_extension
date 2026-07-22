SHELL=/bin/bash
PYTHON=$(CURDIR)/.venv/bin/python
PYTHON_LIBDIR=$(shell $(PYTHON) -c 'import sysconfig; print(sysconfig.get_config_var("LIBDIR"))')

venv:
	python3 -m venv .venv
	.venv/bin/pip install -r requirements.txt

install:
	unset CONDA_PREFIX && \
	source .venv/bin/activate && maturin develop

install-release:
	unset CONDA_PREFIX && \
	source .venv/bin/activate && maturin develop --release

pre-commit:
	cargo fmt --all -- --check
	cargo clippy --locked --package polars_bt --all-targets --all-features -- -D warnings
	PYO3_PYTHON=$(PYTHON) LD_LIBRARY_PATH=$(PYTHON_LIBDIR) cargo test --locked --all-targets
	$(PYTHON) -m pytest tests -q
	$(PYTHON) -m ruff check polars_bt tests benchmarks

test:
	$(PYTHON) -m pytest tests -q

run: install
	source .venv/bin/activate && python run.py

run-release: install-release
	source .venv/bin/activate && python run.py
