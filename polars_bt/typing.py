from typing import Union

import sys
import polars as pl

if sys.version_info >= (3, 10):
    from typing import TypeAlias
from polars.datatypes import DataType, DataTypeClass

IntoExprColumn: TypeAlias = Union[pl.Expr, str, pl.Series]
PolarsDataType: TypeAlias = Union[DataType, DataTypeClass]
