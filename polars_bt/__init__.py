from __future__ import annotations
import importlib.metadata

__version__ = importlib.metadata.version("polars_bt_extension")

from .backtest import backtest

__all__ = ["backtest"]