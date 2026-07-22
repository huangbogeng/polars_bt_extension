#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub day_index: usize,
    pub asset_index: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counter {
    pub count: usize,
    pub first: Option<Location>,
}

impl Counter {
    fn record(&mut self, day_index: usize, asset_index: Option<usize>) {
        self.count += 1;
        self.first.get_or_insert(Location {
            day_index,
            asset_index,
        });
    }
}

#[derive(Clone, Debug, Default)]
pub struct MosaicDiagnosticReport {
    pub nan_weight: Counter,
    pub nan_ovn_ret: Counter,
    pub nan_ind_ret: Counter,
    pub inf_weight: Counter,
    pub inf_ovn_ret: Counter,
    pub inf_ind_ret: Counter,
    pub invalid_prev_close: Counter,
    pub invalid_vwap: Counter,
    pub negative_weight: Counter,
    pub return_below_minus_one: Counter,
    pub mixed_date_blocks: Counter,
    pub mixed_rebalance_blocks: Counter,
    pub overweight_days: Counter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirstDiagnostic {
    pub code: &'static str,
    pub location: Location,
}

impl MosaicDiagnosticReport {
    fn entries(&self) -> [(&'static str, &'static str, &Counter); 13] {
        [
            ("nan_weight", "NAN_WEIGHT", &self.nan_weight),
            ("nan_ovn_ret", "NAN_OVN_RET", &self.nan_ovn_ret),
            ("nan_ind_ret", "NAN_IND_RET", &self.nan_ind_ret),
            ("inf_weight", "INF_WEIGHT", &self.inf_weight),
            ("inf_ovn_ret", "INF_OVN_RET", &self.inf_ovn_ret),
            ("inf_ind_ret", "INF_IND_RET", &self.inf_ind_ret),
            (
                "invalid_prev_close",
                "INVALID_PREV_CLOSE",
                &self.invalid_prev_close,
            ),
            ("invalid_vwap", "INVALID_VWAP", &self.invalid_vwap),
            ("negative_weight", "NEGATIVE_WEIGHT", &self.negative_weight),
            (
                "return_below_minus_one",
                "RETURN_BELOW_MINUS_ONE",
                &self.return_below_minus_one,
            ),
            (
                "mixed_date_blocks",
                "MIXED_DATE_BLOCK",
                &self.mixed_date_blocks,
            ),
            (
                "mixed_rebalance_blocks",
                "MIXED_REBALANCE_BLOCK",
                &self.mixed_rebalance_blocks,
            ),
            ("overweight_days", "OVERWEIGHT_DAY", &self.overweight_days),
        ]
    }

    pub fn summary_fields(&self) -> Vec<(&'static str, String)> {
        self.entries()
            .into_iter()
            .filter(|(_, _, counter)| counter.count > 0)
            .map(|(field, _, counter)| (field, counter.count.to_string()))
            .collect()
    }

    pub fn first_diagnostics(&self) -> Vec<FirstDiagnostic> {
        self.entries()
            .into_iter()
            .filter_map(|(_, code, counter)| {
                counter
                    .first
                    .map(|location| FirstDiagnostic { code, location })
            })
            .collect()
    }
}

pub struct DiagnosticInputs<'a> {
    pub weight: &'a [f64],
    pub ovn_ret: &'a [f64],
    pub ind_ret: &'a [f64],
    pub prev_close: &'a [f64],
    pub vwap: &'a [f64],
    pub is_rebalance: &'a [bool],
}

pub fn inspect_inputs(
    dates: &Series,
    inputs: DiagnosticInputs<'_>,
    asset_num: usize,
) -> PolarsResult<MosaicDiagnosticReport> {
    let row_count = inputs.weight.len();
    polars_ensure!(asset_num > 0, ComputeError: "diagnostic asset_num must be positive");
    polars_ensure!(
        dates.len() == row_count
            && inputs.ovn_ret.len() == row_count
            && inputs.ind_ret.len() == row_count
            && inputs.prev_close.len() == row_count
            && inputs.vwap.len() == row_count
            && inputs.is_rebalance.len() == row_count,
        ShapeMismatch: "all diagnostic inputs must have equal length"
    );
    polars_ensure!(
        row_count.is_multiple_of(asset_num),
        ShapeMismatch: "diagnostic row count must be divisible by asset_num"
    );

    let mut report = MosaicDiagnosticReport::default();
    for row_index in 0..row_count {
        let day_index = row_index / asset_num;
        let asset_index = Some(row_index % asset_num);
        let weight = inputs.weight[row_index];
        let ovn_ret = inputs.ovn_ret[row_index];
        let ind_ret = inputs.ind_ret[row_index];
        let prev_close = inputs.prev_close[row_index];
        let vwap = inputs.vwap[row_index];

        if weight.is_nan() {
            report.nan_weight.record(day_index, asset_index);
        } else if weight.is_infinite() {
            report.inf_weight.record(day_index, asset_index);
        } else if weight < 0.0 {
            report.negative_weight.record(day_index, asset_index);
        }

        if ovn_ret.is_nan() {
            report.nan_ovn_ret.record(day_index, asset_index);
        } else if ovn_ret.is_infinite() {
            report.inf_ovn_ret.record(day_index, asset_index);
        } else if ovn_ret < -1.0 {
            report.return_below_minus_one.record(day_index, asset_index);
        }

        if ind_ret.is_nan() {
            report.nan_ind_ret.record(day_index, asset_index);
        } else if ind_ret.is_infinite() {
            report.inf_ind_ret.record(day_index, asset_index);
        } else if ind_ret < -1.0 {
            report.return_below_minus_one.record(day_index, asset_index);
        }

        if !prev_close.is_finite() || prev_close <= 0.0 {
            report.invalid_prev_close.record(day_index, asset_index);
        }
        if !vwap.is_finite() || vwap <= 0.0 {
            report.invalid_vwap.record(day_index, asset_index);
        }
    }

    for (day_index, start) in (0..row_count).step_by(asset_num).enumerate() {
        let end = start + asset_num;
        let first_date = dates.get(start)?;
        for asset_index in 1..asset_num {
            if dates.get(start + asset_index)? != first_date {
                report
                    .mixed_date_blocks
                    .record(day_index, Some(asset_index));
                break;
            }
        }

        let rebalance = &inputs.is_rebalance[start..end];
        let any_rebalance = rebalance.iter().any(|value| *value);
        if any_rebalance && rebalance.iter().any(|value| !*value) {
            let asset_index = rebalance
                .iter()
                .position(|value| *value != rebalance[0])
                .expect("mixed rebalance block must contain a differing value");
            report
                .mixed_rebalance_blocks
                .record(day_index, Some(asset_index));
        }

        if any_rebalance {
            let weight_sum: f64 = inputs.weight[start..end]
                .iter()
                .map(|value| if value.is_nan() { 0.0 } else { *value })
                .sum();
            if weight_sum.is_finite() && weight_sum > 1.0 + 1e-8 {
                report.overweight_days.record(day_index, None);
            }
        }
    }

    Ok(report)
}

#[cfg(test)]
mod tests {
    use polars::prelude::*;

    use super::{inspect_inputs, Counter, DiagnosticInputs, Location};

    #[test]
    fn counters_keep_only_the_first_location() {
        let mut counter = Counter::default();
        counter.record(3, Some(2));
        counter.record(8, Some(7));

        assert_eq!(counter.count, 2);
        assert_eq!(
            counter.first,
            Some(Location {
                day_index: 3,
                asset_index: Some(2),
            })
        );
    }

    #[test]
    fn inspects_numeric_and_block_anomalies() {
        let dates = Series::new("date".into(), [1_i32, 2, 3, 3, 4, 4]);
        let report = inspect_inputs(
            &dates,
            DiagnosticInputs {
                weight: &[f64::NAN, -0.1, 0.7, 0.7, f64::INFINITY, 0.0],
                ovn_ret: &[f64::INFINITY, 0.0, -1.1, 0.0, 0.0, 0.0],
                ind_ret: &[0.0, f64::NAN, 0.0, f64::NEG_INFINITY, 0.0, 0.0],
                prev_close: &[10.0, 0.0, 10.0, f64::NAN, 10.0, 10.0],
                vwap: &[10.0, 10.0, -1.0, f64::INFINITY, 10.0, 10.0],
                is_rebalance: &[false, true, true, true, false, false],
            },
            2,
        )
        .unwrap();

        assert_eq!(report.nan_weight.count, 1);
        assert_eq!(report.nan_ind_ret.count, 1);
        assert_eq!(report.inf_weight.count, 1);
        assert_eq!(report.inf_ovn_ret.count, 1);
        assert_eq!(report.inf_ind_ret.count, 1);
        assert_eq!(report.invalid_prev_close.count, 2);
        assert_eq!(report.invalid_vwap.count, 2);
        assert_eq!(report.negative_weight.count, 1);
        assert_eq!(report.return_below_minus_one.count, 1);
        assert_eq!(report.mixed_date_blocks.count, 1);
        assert_eq!(report.mixed_rebalance_blocks.count, 1);
        assert_eq!(report.overweight_days.count, 1);
        assert_eq!(
            report.mixed_date_blocks.first,
            Some(Location {
                day_index: 0,
                asset_index: Some(1),
            })
        );
    }

    #[test]
    fn renders_only_nonzero_categories_in_stable_order() {
        let mut report = super::MosaicDiagnosticReport::default();
        report.invalid_vwap.record(4, Some(1));
        report.nan_weight.record(2, Some(0));
        report.nan_weight.record(3, Some(1));

        assert_eq!(
            report.summary_fields(),
            vec![
                ("nan_weight", "2".to_string()),
                ("invalid_vwap", "1".to_string()),
            ]
        );

        let first = report.first_diagnostics();
        assert_eq!(first.len(), 2);
        assert_eq!(first[0].code, "NAN_WEIGHT");
        assert_eq!(first[0].location.day_index, 2);
        assert_eq!(first[1].code, "INVALID_VWAP");
        assert_eq!(first[1].location.asset_index, Some(1));
    }
}
use polars::prelude::*;
