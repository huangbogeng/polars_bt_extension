//! polars_bt - A high-performance backtesting engine plugin for Polars
//!
//! This library provides a Polars plugin for financial backtesting with
//! efficient order matching and position management.

pub mod mosaic;
pub mod pulse;

use pyo3::prelude::*;
use pyo3_polars::PolarsAllocator;

/// Internal Python module initialization
#[pymodule]
fn _internal(_py: Python, m: &Bound<PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

/// Global allocator for Polars memory management
#[global_allocator]
static ALLOC: PolarsAllocator = PolarsAllocator::new();
