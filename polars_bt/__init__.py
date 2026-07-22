from __future__ import annotations
import importlib.metadata

__version__ = importlib.metadata.version("polars_bt")

from .pulse import pulse

__all__ = ["pulse"]
