import importlib.metadata

import polars as pl


def test_plugin_imports_with_supported_polars():
    import polars_bt

    assert tuple(map(int, pl.__version__.split(".")[:2])) == (1, 43)
    assert polars_bt.__version__ == "0.2.0"


def test_supported_python_range_starts_at_3_10():
    assert importlib.metadata.metadata("polars_bt")["Requires-Python"] == ">=3.10"
