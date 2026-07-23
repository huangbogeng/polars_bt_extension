use polars::prelude::*;

use super::portfolio::PortfolioState;

pub struct TimestampSlice<'a> {
    pub trade_date: i32,
    pub weight: &'a [f64],
    pub period_ret: &'a [f64],
    pub buyable: &'a [bool],
    pub sellable: &'a [bool],
    pub is_rebalance: bool,
}

#[derive(Clone, Copy)]
pub struct Config {
    asset_num: usize,
    st_fee: f64,
    lg_fee: f64,
    t1: bool,
}

impl Config {
    pub fn new(asset_num: usize, st_fee: f64, lg_fee: f64, t1: bool) -> Self {
        Self {
            asset_num,
            st_fee,
            lg_fee,
            t1,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Snapshot {
    pub cash: f64,
    pub nav: f64,
    pub turnover: f64,
    pub holding_count: i64,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub snapshots: Vec<Snapshot>,
}

fn validate_slice(slice: &TimestampSlice<'_>, asset_num: usize) -> PolarsResult<()> {
    let lengths = [
        slice.weight.len(),
        slice.period_ret.len(),
        slice.buyable.len(),
        slice.sellable.len(),
    ];
    polars_ensure!(
        lengths.iter().all(|length| *length == asset_num),
        ShapeMismatch: "every tempo timestamp must contain exactly {asset_num} assets"
    );
    Ok(())
}

pub fn run_slices(slices: &[TimestampSlice<'_>], config: Config) -> PolarsResult<RunOutcome> {
    let mut state = PortfolioState::new(config.asset_num);
    let mut snapshots = Vec::with_capacity(slices.len());
    let mut current_date = None;

    for (time_index, slice) in slices.iter().enumerate() {
        validate_slice(slice, config.asset_num)?;
        if current_date != Some(slice.trade_date) {
            state.release_t1();
            current_date = Some(slice.trade_date);
        }
        state.apply_returns(slice.period_ret, time_index)?;

        let mut traded_abs = 0.0;
        if slice.is_rebalance {
            let sell_diff = state.target_differences(slice.weight, time_index)?;
            let mut executed = vec![0.0; config.asset_num];
            for asset_index in 0..config.asset_num {
                let amount = sell_diff[asset_index];
                if amount < 0.0 && slice.sellable[asset_index] {
                    executed[asset_index] = amount.max(-state.sellable(asset_index));
                }
            }
            traded_abs += state.apply_sells(&executed, config.st_fee, time_index)?;

            let buy_diff = state.target_differences(slice.weight, time_index)?;
            let available_cash = state.cash();
            let mut cumulative = 0.0;
            executed.fill(0.0);
            for asset_index in 0..config.asset_num {
                let amount = buy_diff[asset_index];
                let eligible = amount > 0.0 && slice.buyable[asset_index];
                let candidate = if eligible { amount } else { 0.0 };
                cumulative += candidate * (1.0 + config.lg_fee);
                if eligible && cumulative < available_cash {
                    executed[asset_index] = amount;
                }
            }
            traded_abs += state.apply_buys(&executed, config.lg_fee, config.t1, time_index)?;
        }

        let nav = state.nav();
        polars_ensure!(
            nav.is_finite(),
            ComputeError:
            "polars-bt tempo failed: time_index={time_index} stage=snapshot \
             reason=NONFINITE_STATE nav={nav}"
        );
        snapshots.push(Snapshot {
            cash: state.cash(),
            nav,
            turnover: if nav > 1e-12 { traded_abs / nav } else { 0.0 },
            holding_count: state.holding_count(),
        });
    }

    Ok(RunOutcome { snapshots })
}

#[cfg(test)]
mod tests {
    use super::{run_slices, Config, TimestampSlice};

    #[test]
    fn marks_returns_before_rebalance_and_enforces_t1() {
        let slices = vec![
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.8, 0.0],
                period_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                is_rebalance: true,
            },
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.0, 0.8],
                period_ret: &[0.1, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                is_rebalance: true,
            },
            TimestampSlice {
                trade_date: 20_001,
                weight: &[0.0, 0.8],
                period_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                is_rebalance: true,
            },
        ];

        let snapshots = run_slices(&slices, Config::new(2, 0.0, 0.0, true))
            .unwrap()
            .snapshots;

        assert!((snapshots[1].nav - 1.08).abs() < 1e-12);
        assert!((snapshots[1].turnover - 0.0).abs() < 1e-12);
        assert!((snapshots[2].cash - 0.216).abs() < 1e-12);
        assert!(snapshots[2].turnover > 1.5);
        assert_eq!(snapshots[2].holding_count, 1);
    }

    #[test]
    fn t0_can_rotate_positions_inside_one_day() {
        let slices = vec![
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.4, 0.0],
                period_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                is_rebalance: true,
            },
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.0, 0.4],
                period_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                is_rebalance: true,
            },
        ];

        let snapshots = run_slices(&slices, Config::new(2, 0.0, 0.0, false))
            .unwrap()
            .snapshots;

        assert!((snapshots[1].turnover - 0.8).abs() < 1e-12);
        assert!((snapshots[1].cash - 0.6).abs() < 1e-12);
        assert_eq!(snapshots[1].holding_count, 1);
    }

    #[test]
    fn non_rebalance_timestamp_only_marks_existing_positions() {
        let slices = vec![
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.5],
                period_ret: &[0.0],
                buyable: &[true],
                sellable: &[true],
                is_rebalance: true,
            },
            TimestampSlice {
                trade_date: 20_000,
                weight: &[0.0],
                period_ret: &[0.2],
                buyable: &[true],
                sellable: &[true],
                is_rebalance: false,
            },
        ];

        let snapshots = run_slices(&slices, Config::new(1, 0.0, 0.0, true))
            .unwrap()
            .snapshots;

        assert!((snapshots[1].nav - 1.1).abs() < 1e-12);
        assert!((snapshots[1].turnover - 0.0).abs() < 1e-12);
        assert_eq!(snapshots[1].holding_count, 1);
    }
}
