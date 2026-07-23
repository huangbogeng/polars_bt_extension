#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Location {
    pub time_index: usize,
    pub asset_index: Option<usize>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Counter {
    pub count: usize,
    pub first: Option<Location>,
}

impl Counter {
    fn record(&mut self, time_index: usize, asset_index: Option<usize>) {
        self.count += 1;
        self.first.get_or_insert(Location {
            time_index,
            asset_index,
        });
    }
}

#[derive(Clone, Debug, Default)]
pub struct TempoDiagnosticReport {
    pub nan_weight: Counter,
    pub nan_period_ret: Counter,
    pub return_below_minus_one: Counter,
    pub overweight_timestamps: Counter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FirstDiagnostic {
    pub code: &'static str,
    pub location: Location,
}

impl TempoDiagnosticReport {
    fn entries(&self) -> [(&'static str, &'static str, &Counter); 4] {
        [
            ("nan_weight", "NAN_WEIGHT", &self.nan_weight),
            ("nan_period_ret", "NAN_PERIOD_RET", &self.nan_period_ret),
            (
                "return_below_minus_one",
                "RETURN_BELOW_MINUS_ONE",
                &self.return_below_minus_one,
            ),
            (
                "overweight_timestamps",
                "OVERWEIGHT_TIMESTAMP",
                &self.overweight_timestamps,
            ),
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

pub fn inspect_inputs(
    weight: &[f64],
    period_ret: &[f64],
    is_rebalance: &[bool],
    asset_num: usize,
) -> TempoDiagnosticReport {
    let mut report = TempoDiagnosticReport::default();
    if asset_num == 0 {
        return report;
    }

    for row_index in 0..weight.len() {
        let time_index = row_index / asset_num;
        let asset_index = Some(row_index % asset_num);
        if weight[row_index].is_nan() {
            report.nan_weight.record(time_index, asset_index);
        }
        if period_ret[row_index].is_nan() {
            report.nan_period_ret.record(time_index, asset_index);
        } else if period_ret[row_index] < -1.0 {
            report
                .return_below_minus_one
                .record(time_index, asset_index);
        }
    }

    for (time_index, start) in (0..weight.len()).step_by(asset_num).enumerate() {
        let end = start + asset_num;
        if is_rebalance[start] {
            let weight_sum = weight[start..end]
                .iter()
                .map(|value| if value.is_nan() { 0.0 } else { *value })
                .sum::<f64>();
            if weight_sum > 1.0 + 1e-8 {
                report.overweight_timestamps.record(time_index, None);
            }
        }
    }
    report
}

#[cfg(test)]
mod tests {
    use super::inspect_inputs;

    #[test]
    fn keeps_bounded_counts_and_first_locations() {
        let report = inspect_inputs(
            &[f64::NAN, 0.4, 0.7, 0.7],
            &[0.0, f64::NAN, -1.1, 0.0],
            &[true; 4],
            2,
        );

        assert_eq!(report.nan_weight.count, 1);
        assert_eq!(report.nan_period_ret.count, 1);
        assert_eq!(report.return_below_minus_one.count, 1);
        assert_eq!(report.overweight_timestamps.count, 1);
        assert_eq!(
            report
                .first_diagnostics()
                .iter()
                .map(|diagnostic| diagnostic.code)
                .collect::<Vec<_>>(),
            [
                "NAN_WEIGHT",
                "NAN_PERIOD_RET",
                "RETURN_BELOW_MINUS_ONE",
                "OVERWEIGHT_TIMESTAMP",
            ]
        );
    }
}
