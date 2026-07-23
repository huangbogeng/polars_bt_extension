mod engine;
mod output;
mod portfolio;
mod validation;

use engine::{run_slices, Config, TimestampSlice};
use output::build_output;
use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;
use validation::{validate_layout, ValidationInputs};

pub fn output_type_tempo(input_fields: &[Field]) -> PolarsResult<Field> {
    polars_ensure!(
        !input_fields.is_empty(),
        ComputeError: "tempo requires datetime input"
    );
    Ok(Field::new(
        "tempo".into(),
        DataType::Struct(vec![
            Field::new("datetime".into(), input_fields[0].dtype().clone()),
            Field::new("cash".into(), DataType::Float64),
            Field::new("nav".into(), DataType::Float64),
            Field::new("turnover".into(), DataType::Float64),
            Field::new("holding_count".into(), DataType::Int64),
        ]),
    ))
}

#[derive(Deserialize)]
pub struct TempoKwargs {
    t1: bool,
    st_fee: f64,
    lg_fee: f64,
}

fn numeric_values<'a>(series: &'a Series, name: &str) -> PolarsResult<&'a [f64]> {
    let values = series.f64()?;
    polars_ensure!(
        values.null_count() == 0,
        ComputeError: "tempo numeric input {name} cannot contain nulls"
    );
    values.cont_slice()
}

fn boolean_values(series: &Series, name: &str) -> PolarsResult<Vec<bool>> {
    let values = series.bool()?;
    polars_ensure!(
        values.null_count() == 0,
        ComputeError: "tempo Boolean input {name} cannot contain nulls"
    );
    Ok(values.iter().map(Option::unwrap).collect())
}

fn tempo_impl(inputs: &[Series], kwargs: TempoKwargs) -> PolarsResult<Series> {
    polars_ensure!(
        inputs.len() == 8,
        ComputeError: "tempo requires exactly 8 internal inputs"
    );
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
        ShapeMismatch: "all tempo inputs must have equal length"
    );

    let datetime_series = inputs[0].rechunk();
    let trade_date_series = inputs[1].rechunk();
    let asset_series = inputs[2].rechunk();
    let weight_series = inputs[3].rechunk();
    let period_ret_series = inputs[4].rechunk();

    let datetime_values = datetime_series.datetime()?;
    polars_ensure!(
        datetime_values.null_count() == 0,
        ComputeError: "tempo datetime cannot contain nulls"
    );
    let datetimes = datetime_values.physical().cont_slice()?;

    let trade_date_values = trade_date_series.date()?;
    polars_ensure!(
        trade_date_values.null_count() == 0,
        ComputeError: "tempo trade date cannot contain nulls"
    );
    let trade_dates = trade_date_values.physical().cont_slice()?;

    let asset_values = asset_series.str()?;
    polars_ensure!(
        asset_values.null_count() == 0,
        ComputeError: "tempo asset cannot contain nulls"
    );
    let assets = asset_values.iter().map(Option::unwrap).collect::<Vec<_>>();

    let weight = numeric_values(&weight_series, "weight")?;
    let period_ret = numeric_values(&period_ret_series, "period_ret")?;
    let buyable = boolean_values(&inputs[5], "buyable")?;
    let sellable = boolean_values(&inputs[6], "sellable")?;
    let is_rebalance = boolean_values(&inputs[7], "is_rebalance")?;

    let layout = validate_layout(ValidationInputs {
        datetimes,
        assets: &assets,
        weight,
        period_ret,
        is_rebalance: &is_rebalance,
    })?;

    let mut slices = Vec::with_capacity(layout.starts.len());
    for start in layout.starts.iter().copied() {
        let end = start + layout.asset_num;
        slices.push(TimestampSlice {
            trade_date: trade_dates[start],
            weight: &weight[start..end],
            period_ret: &period_ret[start..end],
            buyable: &buyable[start..end],
            sellable: &sellable[start..end],
            is_rebalance: is_rebalance[start],
        });
    }
    let snapshots = run_slices(
        &slices,
        Config::new(layout.asset_num, kwargs.st_fee, kwargs.lg_fee, kwargs.t1),
    )?
    .snapshots;

    let datetime_indices = layout
        .starts
        .iter()
        .take(snapshots.len())
        .map(|index| *index as IdxSize)
        .collect::<Vec<_>>();
    let output_datetimes =
        datetime_series.take(&IdxCa::from_vec("datetime_idx".into(), datetime_indices))?;
    build_output(output_datetimes, &snapshots)
}

#[polars_expr(output_type_func=output_type_tempo)]
pub fn tempo(inputs: &[Series], kwargs: TempoKwargs) -> PolarsResult<Series> {
    tempo_impl(inputs, kwargs)
}

#[cfg(test)]
mod tests {
    use polars::prelude::*;

    use super::{tempo_impl, TempoKwargs};

    #[test]
    fn plugin_returns_one_row_per_datetime_block() {
        let datetime = Series::new("datetime".into(), [1_000_i64, 1_000, 2_000, 2_000])
            .cast(&DataType::Datetime(TimeUnit::Microseconds, None))
            .unwrap();
        let trade_date = Series::new("trade_date".into(), [20_000_i32; 4])
            .cast(&DataType::Date)
            .unwrap();
        let inputs = vec![
            datetime,
            trade_date,
            Series::new("asset".into(), ["A", "B", "A", "B"]),
            Series::new("weight".into(), [0.4_f64, 0.4, 0.0, 0.8]),
            Series::new("period_ret".into(), [0.0_f64; 4]),
            Series::new("buyable".into(), [true; 4]),
            Series::new("sellable".into(), [true; 4]),
            Series::new("is_rebalance".into(), [true; 4]),
        ];

        let output = tempo_impl(
            &inputs,
            TempoKwargs {
                t1: true,
                st_fee: 0.0,
                lg_fee: 0.0,
            },
        )
        .unwrap();

        assert_eq!(output.len(), 2);
    }
}
