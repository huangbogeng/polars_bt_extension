use polars::prelude::*;

use super::portfolio::PortfolioState;

pub struct DailySlice<'a> {
    pub weight: &'a [f64],
    pub ovn_ret: &'a [f64],
    pub ind_ret: &'a [f64],
    pub buyable: &'a [bool],
    pub sellable: &'a [bool],
    pub prev_close: &'a [f64],
    pub vwap: &'a [f64],
    pub is_rebalance: &'a [bool],
}

#[derive(Clone, Copy)]
pub struct Config {
    pub asset_num: usize,
    pub st_fee: f64,
    pub lg_fee: f64,
}

impl Config {
    pub fn new(asset_num: usize, st_fee: f64, lg_fee: f64) -> Self {
        Self {
            asset_num,
            st_fee,
            lg_fee,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DailySnapshot {
    pub cash: f64,
    pub nav: f64,
    pub turnover: f64,
    pub holding_count: i64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Halt {
    pub day_index: usize,
    pub cash: f64,
}

#[derive(Debug)]
pub struct RunOutcome {
    pub snapshots: Vec<DailySnapshot>,
    pub halt: Option<Halt>,
}

fn validate_day(day: &DailySlice<'_>, asset_num: usize) -> PolarsResult<()> {
    let lengths = [
        day.weight.len(),
        day.ovn_ret.len(),
        day.ind_ret.len(),
        day.buyable.len(),
        day.sellable.len(),
        day.prev_close.len(),
        day.vwap.len(),
        day.is_rebalance.len(),
    ];
    if lengths.iter().any(|length| *length != asset_num) {
        polars_bail!(ShapeMismatch: "every daily slice must contain exactly {asset_num} assets");
    }
    Ok(())
}

pub fn run_slices(days: &[DailySlice<'_>], config: Config) -> PolarsResult<RunOutcome> {
    let mut state = PortfolioState::new(config.asset_num);
    let mut snapshots = Vec::with_capacity(days.len());
    let mut halt = None;

    for (day_index, day) in days.iter().enumerate() {
        validate_day(day, config.asset_num)?;
        state.apply_returns(day.ovn_ret, day_index, "overnight_settlement")?;
        state.begin_day();

        if day.is_rebalance.iter().any(|value| *value) {
            let sell_diff = state.target_difference(day.weight, day_index, "sell_target")?;
            let mut executed = vec![0.0; config.asset_num];
            for index in 0..config.asset_num {
                let amount = sell_diff[index];
                if amount < 0.0
                    && day.sellable[index]
                    && day.prev_close[index] > 0.0
                    && day.vwap[index] > 0.0
                {
                    executed[index] = amount;
                }
            }
            state.apply_deals(&executed, config.st_fee, day_index, "sell_execution")?;

            let buy_diff = state.target_difference(day.weight, day_index, "buy_target")?;
            let available_cash = state.cash();
            let mut cumulative = 0.0;
            executed.fill(0.0);
            for index in 0..config.asset_num {
                let eligible = buy_diff[index] > 0.0
                    && day.buyable[index]
                    && day.prev_close[index] > 0.0
                    && day.vwap[index] > 0.0;
                let amount = if eligible { buy_diff[index] } else { 0.0 };
                cumulative += amount * (1.0 + config.lg_fee);
                if eligible && cumulative < available_cash {
                    executed[index] = amount;
                }
            }
            state.apply_deals(&executed, config.lg_fee, day_index, "buy_execution")?;
        }

        state.apply_returns(day.ind_ret, day_index, "intraday_settlement")?;
        state.finalize_turnover(day_index)?;
        snapshots.push(DailySnapshot {
            cash: state.cash(),
            nav: state.nav(),
            turnover: state.turnover(),
            holding_count: state.holding_count(),
        });

        if state.cash() < -0.00001 {
            halt = Some(Halt {
                day_index,
                cash: state.cash(),
            });
            break;
        }
    }

    Ok(RunOutcome { snapshots, halt })
}

#[cfg(test)]
mod tests {
    use super::{run_slices, Config, DailySlice};

    #[test]
    fn runs_sell_before_buy_and_preserves_row_order() {
        let days = vec![
            DailySlice {
                weight: &[0.7, 0.2],
                ovn_ret: &[0.0, 0.0],
                ind_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                prev_close: &[10.0, 10.0],
                vwap: &[10.0, 10.0],
                is_rebalance: &[true, true],
            },
            DailySlice {
                weight: &[0.0, 0.5],
                ovn_ret: &[0.0, 0.0],
                ind_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                prev_close: &[10.0, 10.0],
                vwap: &[10.0, 10.0],
                is_rebalance: &[true, true],
            },
        ];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0))
            .unwrap()
            .snapshots;

        assert_eq!(snapshots.len(), 2);
        assert!((snapshots[0].nav - 1.0).abs() < 1e-15);
        assert!((snapshots[1].turnover - 1.0).abs() < 1e-15);
        assert_eq!(snapshots[1].holding_count, 1);
    }

    #[test]
    fn skips_trading_when_rebalance_is_false() {
        let days = [DailySlice {
            weight: &[0.4, 0.4],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[true, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[false, false],
        }];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0))
            .unwrap()
            .snapshots;

        assert_eq!(snapshots[0].turnover, 0.0);
        assert_eq!(snapshots[0].holding_count, 0);
    }

    #[test]
    fn unsellable_asset_is_retained() {
        let days = vec![
            DailySlice {
                weight: &[0.4, 0.0],
                ovn_ret: &[0.0, 0.0],
                ind_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[true, true],
                prev_close: &[10.0, 10.0],
                vwap: &[10.0, 10.0],
                is_rebalance: &[true, true],
            },
            DailySlice {
                weight: &[0.0, 0.4],
                ovn_ret: &[0.0, 0.0],
                ind_ret: &[0.0, 0.0],
                buyable: &[true, true],
                sellable: &[false, true],
                prev_close: &[10.0, 10.0],
                vwap: &[10.0, 10.0],
                is_rebalance: &[true, true],
            },
        ];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0))
            .unwrap()
            .snapshots;

        assert!((snapshots[1].turnover - 0.4).abs() < 1e-15);
        assert_eq!(snapshots[1].holding_count, 2);
    }

    #[test]
    fn unbuyable_asset_leaves_cash_unchanged() {
        let days = [DailySlice {
            weight: &[0.4, 0.0],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[false, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[true, true],
        }];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0))
            .unwrap()
            .snapshots;

        assert_eq!(snapshots[0].cash, 1.0);
        assert_eq!(snapshots[0].turnover, 0.0);
    }

    #[test]
    fn fees_reduce_nav_by_traded_absolute_value() {
        let days = [DailySlice {
            weight: &[0.4, 0.0],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[true, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[true, true],
        }];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.01))
            .unwrap()
            .snapshots;

        assert!((snapshots[0].nav - 0.996).abs() < 1e-15);
    }

    #[test]
    fn rejects_buy_when_cumulative_cost_equals_cash() {
        let days = [DailySlice {
            weight: &[0.5, 0.5],
            ovn_ret: &[0.0, 0.0],
            ind_ret: &[0.0, 0.0],
            buyable: &[true, true],
            sellable: &[true, true],
            prev_close: &[10.0, 10.0],
            vwap: &[10.0, 10.0],
            is_rebalance: &[true, true],
        }];

        let snapshots = run_slices(&days, Config::new(2, 0.0, 0.0))
            .unwrap()
            .snapshots;

        assert_eq!(snapshots[0].cash, 0.5);
        assert!((snapshots[0].turnover - 0.5).abs() < 1e-15);
        assert_eq!(snapshots[0].holding_count, 1);
    }

    #[test]
    fn stops_after_first_negative_cash_snapshot() {
        let days = vec![
            DailySlice {
                weight: &[0.4],
                ovn_ret: &[0.0],
                ind_ret: &[0.0],
                buyable: &[true],
                sellable: &[true],
                prev_close: &[10.0],
                vwap: &[10.0],
                is_rebalance: &[true],
            },
            DailySlice {
                weight: &[0.0],
                ovn_ret: &[0.0],
                ind_ret: &[0.0],
                buyable: &[true],
                sellable: &[true],
                prev_close: &[10.0],
                vwap: &[10.0],
                is_rebalance: &[true],
            },
            DailySlice {
                weight: &[0.0],
                ovn_ret: &[0.0],
                ind_ret: &[0.0],
                buyable: &[true],
                sellable: &[true],
                prev_close: &[10.0],
                vwap: &[10.0],
                is_rebalance: &[true],
            },
        ];

        let outcome = run_slices(&days, Config::new(1, 3.0, 0.0)).unwrap();

        assert_eq!(outcome.snapshots.len(), 2);
        assert!(outcome.snapshots[1].cash < -0.00001);
        let halt = outcome.halt.unwrap();
        assert_eq!(halt.day_index, 1);
        assert_eq!(halt.cash, outcome.snapshots[1].cash);
    }

    #[test]
    fn rejects_non_finite_overnight_state_with_context() {
        let days = [DailySlice {
            weight: &[0.0],
            ovn_ret: &[f64::INFINITY],
            ind_ret: &[0.0],
            buyable: &[true],
            sellable: &[true],
            prev_close: &[10.0],
            vwap: &[10.0],
            is_rebalance: &[false],
        }];

        let error = run_slices(&days, Config::new(1, 0.0, 0.0)).unwrap_err();
        let message = error.to_string();
        assert!(message.contains("day_index=0"));
        assert!(message.contains("asset_index=0"));
        assert!(message.contains("stage=overnight_settlement"));
        assert!(message.contains("reason=NONFINITE_HOLDING"));
    }

    #[test]
    fn rejects_non_finite_weight_target_with_context() {
        let days = [DailySlice {
            weight: &[f64::INFINITY],
            ovn_ret: &[0.0],
            ind_ret: &[0.0],
            buyable: &[true],
            sellable: &[true],
            prev_close: &[10.0],
            vwap: &[10.0],
            is_rebalance: &[true],
        }];

        let message = run_slices(&days, Config::new(1, 0.0, 0.0))
            .unwrap_err()
            .to_string();
        assert!(message.contains("stage=sell_target"));
        assert!(message.contains("reason=NONFINITE_TARGET_DIFFERENCE"));
    }

    #[test]
    fn rejects_non_finite_intraday_state_with_context() {
        let days = [
            DailySlice {
                weight: &[0.4],
                ovn_ret: &[0.0],
                ind_ret: &[0.0],
                buyable: &[true],
                sellable: &[true],
                prev_close: &[10.0],
                vwap: &[10.0],
                is_rebalance: &[true],
            },
            DailySlice {
                weight: &[0.4],
                ovn_ret: &[0.0],
                ind_ret: &[f64::INFINITY],
                buyable: &[true],
                sellable: &[true],
                prev_close: &[10.0],
                vwap: &[10.0],
                is_rebalance: &[false],
            },
        ];

        let message = run_slices(&days, Config::new(1, 0.0, 0.0))
            .unwrap_err()
            .to_string();
        assert!(message.contains("day_index=1"));
        assert!(message.contains("stage=intraday_settlement"));
        assert!(message.contains("reason=NONFINITE_HOLDING"));
    }
}
