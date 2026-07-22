mod engine;
mod output;
mod portfolio;

use engine::{
    run_slices,
    Config,
    DailySlice,
};
use output::build_output;
use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;

pub fn output_type_mosaic(input_fields: &[Field]) -> PolarsResult<Field> {
    polars_ensure!(!input_fields.is_empty(), ComputeError: "mosaic requires date input");
    Ok(Field::new(
        "mosaic".into(),
        DataType::Struct(vec![
            Field::new("date".into(), input_fields[0].dtype().clone()),
            Field::new("cash".into(), DataType::Float64),
            Field::new("nav".into(), DataType::Float64),
            Field::new("turnover".into(), DataType::Float64),
            Field::new("holding_count".into(), DataType::Int64),
        ]),
    ))
}

#[derive(Deserialize)]
pub struct MosaicKwargs {
    asset_num: usize,
    st_fee: f64,
    lg_fee: f64,
}

fn boolean_values(series: &Series) -> PolarsResult<Vec<bool>> {
    let values = series.bool()?;
    polars_ensure!(
        values.null_count() == 0,
        ComputeError: "mosaic Boolean inputs cannot contain nulls"
    );
    Ok(values.iter().map(Option::unwrap).collect())
}

#[polars_expr(output_type_func=output_type_mosaic)]
pub fn mosaic(inputs: &[Series], kwargs: MosaicKwargs) -> PolarsResult<Series> {
    polars_ensure!(inputs.len() == 9, ComputeError: "mosaic requires exactly 9 inputs");
    polars_ensure!(kwargs.asset_num > 0, ComputeError: "asset_num must be positive");
    polars_ensure!(
        kwargs.st_fee.is_finite() && kwargs.st_fee >= 0.0,
        ComputeError: "st_fee must be finite and non-negative"
    );
    polars_ensure!(
        kwargs.lg_fee.is_finite() && kwargs.lg_fee >= 0.0,
        ComputeError: "lg_fee must be finite and non-negative"
    );

    let row_count = inputs[0].len();
    polars_ensure!(
        inputs.iter().all(|series| series.len() == row_count),
        ShapeMismatch: "all mosaic inputs must have equal length"
    );
    polars_ensure!(
        row_count % kwargs.asset_num == 0,
        ShapeMismatch: "row count must be divisible by asset_num"
    );

    let weight_series = inputs[1].rechunk();
    let ovn_ret_series = inputs[2].rechunk();
    let ind_ret_series = inputs[3].rechunk();
    let prev_close_series = inputs[6].rechunk();
    let vwap_series = inputs[7].rechunk();

    let weight = weight_series.f64()?.cont_slice()?;
    let ovn_ret = ovn_ret_series.f64()?.cont_slice()?;
    let ind_ret = ind_ret_series.f64()?.cont_slice()?;
    let prev_close = prev_close_series.f64()?.cont_slice()?;
    let vwap = vwap_series.f64()?.cont_slice()?;
    let buyable = boolean_values(&inputs[4])?;
    let sellable = boolean_values(&inputs[5])?;
    let is_rebalance = boolean_values(&inputs[8])?;

    let day_count = row_count / kwargs.asset_num;
    let mut days = Vec::with_capacity(day_count);
    for start in (0..row_count).step_by(kwargs.asset_num) {
        let end = start + kwargs.asset_num;
        days.push(DailySlice {
            weight: &weight[start..end],
            ovn_ret: &ovn_ret[start..end],
            ind_ret: &ind_ret[start..end],
            buyable: &buyable[start..end],
            sellable: &sellable[start..end],
            prev_close: &prev_close[start..end],
            vwap: &vwap[start..end],
            is_rebalance: &is_rebalance[start..end],
        });
    }

    let snapshots = run_slices(
        &days,
        Config::new(kwargs.asset_num, kwargs.st_fee, kwargs.lg_fee),
    )?;
    let date_indices = (0..snapshots.len())
        .map(|day| (day * kwargs.asset_num) as IdxSize)
        .collect::<Vec<_>>();
    let dates = inputs[0].take(&IdxCa::from_vec("date_idx".into(), date_indices))?;

    build_output(dates, &snapshots)
}
