import importlib.metadata

import polars as pl


def test_plugin_imports_with_supported_polars():
    import polars_bt
    from polars_bt import _internal

    assert tuple(map(int, pl.__version__.split("."))) >= (1, 44, 2)
    assert tuple(map(int, pl.__version__.split(".")[:2])) == (1, 44)
    assert polars_bt.__version__ == "0.2.2"
    assert _internal.__version__ == polars_bt.__version__


def test_supported_python_range_starts_at_3_10():
    assert importlib.metadata.metadata("polars_bt")["Requires-Python"] == ">=3.10"
