use std::collections::HashSet;

use polars::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub asset_num: usize,
    pub starts: Vec<usize>,
}

pub struct ValidationInputs<'a> {
    pub datetimes: &'a [i64],
    pub assets: &'a [&'a str],
    pub weight: &'a [f64],
    pub period_ret: &'a [f64],
    pub is_rebalance: &'a [bool],
}

fn validate_numeric(inputs: &ValidationInputs<'_>) -> PolarsResult<()> {
    for row in 0..inputs.datetimes.len() {
        let datetime = inputs.datetimes[row];
        let asset = inputs.assets[row];
        let weight = inputs.weight[row];
        let period_ret = inputs.period_ret[row];
        polars_ensure!(
            !weight.is_infinite(),
            ComputeError:
            "tempo weight cannot be infinite: row={row} datetime={datetime} asset={asset}"
        );
        polars_ensure!(
            weight.is_nan() || weight >= 0.0,
            ComputeError:
            "tempo weight must be non-negative: row={row} datetime={datetime} asset={asset} \
             weight={weight}"
        );
        polars_ensure!(
            !period_ret.is_infinite(),
            ComputeError:
            "tempo period_ret cannot be infinite: row={row} datetime={datetime} asset={asset}"
        );
    }
    Ok(())
}

pub fn validate_layout(inputs: ValidationInputs<'_>) -> PolarsResult<Layout> {
    let row_count = inputs.datetimes.len();
    polars_ensure!(
        inputs.assets.len() == row_count
            && inputs.weight.len() == row_count
            && inputs.period_ret.len() == row_count
            && inputs.is_rebalance.len() == row_count,
        ShapeMismatch: "all tempo validation inputs must have equal length"
    );
    if row_count == 0 {
        return Ok(Layout {
            asset_num: 0,
            starts: Vec::new(),
        });
    }

    validate_numeric(&inputs)?;

    let first_datetime = inputs.datetimes[0];
    let asset_num = inputs
        .datetimes
        .iter()
        .take_while(|value| **value == first_datetime)
        .count();
    let mut seen = HashSet::with_capacity(asset_num);
    for (asset_index, asset) in inputs.assets[..asset_num].iter().enumerate() {
        polars_ensure!(
            seen.insert(*asset),
            ComputeError:
            "tempo duplicate asset in first block: row={asset_index} \
             datetime={first_datetime} asset={asset}"
        );
    }

    let canonical = &inputs.assets[..asset_num];
    let mut starts = Vec::with_capacity(row_count / asset_num + 1);
    let mut start = 0;
    let mut previous_datetime = None;
    while start < row_count {
        let datetime = inputs.datetimes[start];
        if let Some(previous) = previous_datetime {
            polars_ensure!(
                datetime > previous,
                ComputeError:
                "tempo datetime must be strictly increasing between blocks: \
                 row={start} datetime={datetime} previous={previous}"
            );
        }
        let mut end = start + 1;
        while end < row_count && inputs.datetimes[end] == datetime {
            end += 1;
        }
        let actual = end - start;
        polars_ensure!(
            actual == asset_num,
            ShapeMismatch:
            "tempo incomplete asset block: row={start} datetime={datetime} \
             expected={asset_num} actual={actual}"
        );

        let first_rebalance = inputs.is_rebalance[start];
        for row in start + 1..end {
            polars_ensure!(
                inputs.is_rebalance[row] == first_rebalance,
                ComputeError:
                "tempo mixed is_rebalance inside block: row={row} datetime={datetime}"
            );
        }
        for asset_index in 0..asset_num {
            let row = start + asset_index;
            let asset = inputs.assets[row];
            let expected = canonical[asset_index];
            polars_ensure!(
                asset == expected,
                ShapeMismatch:
                "tempo asset order mismatch: row={row} datetime={datetime} \
                 asset={asset} expected={expected}"
            );
        }

        starts.push(start);
        previous_datetime = Some(datetime);
        start = end;
    }

    Ok(Layout { asset_num, starts })
}

#[cfg(test)]
mod tests {
    use super::{validate_layout, ValidationInputs};

    fn inputs<'a>(
        datetimes: &'a [i64],
        assets: &'a [&'a str],
        weight: &'a [f64],
        period_ret: &'a [f64],
        is_rebalance: &'a [bool],
    ) -> ValidationInputs<'a> {
        ValidationInputs {
            datetimes,
            assets,
            weight,
            period_ret,
            is_rebalance,
        }
    }

    #[test]
    fn infers_layout_from_complete_ordered_blocks() {
        let data = ValidationInputs {
            datetimes: &[1, 1, 2, 2],
            assets: &["A", "B", "A", "B"],
            weight: &[0.5, 0.4, 0.0, 0.8],
            period_ret: &[0.0; 4],
            is_rebalance: &[true, true, false, false],
        };

        let layout = validate_layout(data).unwrap();

        assert_eq!(layout.asset_num, 2);
        assert_eq!(layout.starts, vec![0, 2]);
    }

    #[test]
    fn rejects_duplicate_asset_in_first_block() {
        let data = inputs(&[1, 1], &["A", "A"], &[0.5, 0.4], &[0.0; 2], &[true; 2]);

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("duplicate asset"));
        assert!(error.contains("row=1"));
        assert!(error.contains("asset=A"));
    }

    #[test]
    fn rejects_changed_asset_order() {
        let data = inputs(
            &[1, 1, 2, 2],
            &["A", "B", "B", "A"],
            &[0.5; 4],
            &[0.0; 4],
            &[true; 4],
        );

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("asset order mismatch"));
        assert!(error.contains("row=2"));
        assert!(error.contains("datetime=2"));
        assert!(error.contains("asset=B"));
        assert!(error.contains("expected=A"));
    }

    #[test]
    fn rejects_incomplete_later_block() {
        let data = inputs(
            &[1, 1, 2],
            &["A", "B", "A"],
            &[0.5; 3],
            &[0.0; 3],
            &[true; 3],
        );

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("incomplete asset block"));
        assert!(error.contains("datetime=2"));
        assert!(error.contains("expected=2"));
        assert!(error.contains("actual=1"));
    }

    #[test]
    fn rejects_mixed_rebalance_flags() {
        let data = inputs(&[1, 1], &["A", "B"], &[0.5; 2], &[0.0; 2], &[true, false]);

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("mixed is_rebalance"));
        assert!(error.contains("row=1"));
    }

    #[test]
    fn rejects_unsorted_datetime_blocks() {
        let data = inputs(
            &[2, 2, 1, 1],
            &["A", "B", "A", "B"],
            &[0.5; 4],
            &[0.0; 4],
            &[true; 4],
        );

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("datetime must be strictly increasing"));
        assert!(error.contains("row=2"));
    }

    #[test]
    fn rejects_negative_weight() {
        let data = inputs(&[1, 1], &["A", "B"], &[-0.1, 0.5], &[0.0; 2], &[true; 2]);

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("weight must be non-negative"));
        assert!(error.contains("row=0"));
        assert!(error.contains("asset=A"));
    }

    #[test]
    fn rejects_infinite_numeric_input() {
        let data = inputs(
            &[1, 1],
            &["A", "B"],
            &[0.5; 2],
            &[0.0, f64::INFINITY],
            &[true; 2],
        );

        let error = validate_layout(data).unwrap_err().to_string();

        assert!(error.contains("period_ret cannot be infinite"));
        assert!(error.contains("row=1"));
        assert!(error.contains("asset=B"));
    }
}
