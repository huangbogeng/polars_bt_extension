mod exchange;
mod matcher;
mod order;
mod position;

use exchange::LofiEx;
use polars::prelude::*;
use position::PositionDirection;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;

#[derive(Deserialize, Debug)]
struct PulseKwargs {
    step_1_limit: u64,
    step_2_limit: u64,
    verbose: bool,
}

pub fn output_type_pulse(_input_fields: &[Field]) -> PolarsResult<Field> {
    Ok(Field::new(
        "output".into(),
        DataType::Struct(vec![
            Field::new("lg_pos".into(), DataType::UInt64),
            Field::new("st_pos".into(), DataType::UInt64),
            Field::new("lg_win".into(), DataType::UInt64),
            Field::new("st_win".into(), DataType::UInt64),
            Field::new("all_signal".into(), DataType::UInt64),
            Field::new("floating_pnl".into(), DataType::Float64),
            Field::new("realized_pnl".into(), DataType::Float64),
        ]),
    ))
}

#[polars_expr(output_type_func=output_type_pulse)]
pub fn pulse(inputs: &[Series], kwargs: PulseKwargs) -> PolarsResult<Series> {
    let ask_price_lst = inputs[0].f64()?;
    let bid_price_lst = inputs[1].f64()?;
    let lg_signal_lst = inputs[2].u8()?;
    let st_signal_lst = inputs[3].u8()?;
    let cl_signal_lst = inputs[4].u8()?;
    let cs_signal_lst = inputs[5].u8()?;
    let time_idx_lst = inputs[6].u64()?;

    let limit_dn = inputs[7].f64()?.get(0).unwrap();
    let limit_up = inputs[8].f64()?.get(0).unwrap();

    let mut ex = LofiEx::new(
        kwargs.step_1_limit * kwargs.step_2_limit,
        limit_dn,
        limit_up,
        kwargs.verbose,
    );
    let mut st = Strategy::new(kwargs.step_1_limit, kwargs.step_2_limit);

    for i in 0..ask_price_lst.len() {
        let ask_price = ask_price_lst.get(i).unwrap();
        let bid_price = bid_price_lst.get(i).unwrap();
        let lg_signal = lg_signal_lst.get(i).unwrap_or_default();
        let st_signal = st_signal_lst.get(i).unwrap_or_default();
        let cl_signal = cl_signal_lst.get(i).unwrap_or_default();
        let cs_signal = cs_signal_lst.get(i).unwrap_or_default();
        let now = time_idx_lst.get(i).unwrap();

        ex.on_quote(ask_price, bid_price, now);
        st.on_quote(
            lg_signal != 0,
            st_signal != 0,
            cl_signal != 0,
            cs_signal != 0,
            &mut ex,
        );
    }

    let result = ex.position.get_summary();
    let out = [
        Series::new("lg_pos".into(), vec![result.lg_pos]),
        Series::new("st_pos".into(), vec![result.st_pos]),
        Series::new("lg_win".into(), vec![result.lg_win]),
        Series::new("st_win".into(), vec![result.st_win]),
        Series::new("all_signal".into(), vec![result.all_signal]),
        Series::new("floating_pnl".into(), vec![result.floating_pnl]),
        Series::new("realized_pnl".into(), vec![result.realized_pnl]),
    ];

    StructChunked::from_series("output".into(), 1, out.iter()).map(|ca| ca.into_series())
}

struct Strategy {
    round_limit: u64,
    exposure_limit: u64,
}

impl Strategy {
    fn new(round_limit: u64, exposure_limit: u64) -> Self {
        Self {
            round_limit,
            exposure_limit,
        }
    }

    fn on_quote(&mut self, lg: bool, st: bool, cl: bool, cs: bool, api: &mut LofiEx) {
        let ask_price = api.ask_price;
        let bid_price = api.bid_price;

        let mut close_lg = false;
        let mut close_st = false;

        if cl {
            let (d, q) = api.position.get_exposure();
            if d == PositionDirection::Long && q > 0 {
                api.sell(bid_price, q);
                close_lg = true;
            }
        }

        if cs {
            let (d, q) = api.position.get_exposure();
            if d == PositionDirection::Short && q > 0 {
                api.buy(ask_price, q);
                close_st = true;
            }
        }

        if lg && !close_lg {
            let (d, q) = api.position.get_exposure();
            match d {
                PositionDirection::Long if q < self.exposure_limit => api.buy(ask_price, 1),
                PositionDirection::Short if q > 0 => api.buy(ask_price, 1),
                PositionDirection::Flat if api.position.all_signal < self.round_limit => {
                    api.buy(ask_price, 1)
                },
                _ => {},
            }
        }

        if st && !close_st {
            let (d, q) = api.position.get_exposure();
            match d {
                PositionDirection::Long if q > 0 => api.sell(bid_price, 1),
                PositionDirection::Short if q < self.exposure_limit => api.sell(bid_price, 1),
                PositionDirection::Flat if api.position.all_signal < self.round_limit => {
                    api.sell(ask_price, 1)
                },
                _ => {},
            }
        }
    }
}
