pub mod matcher;

use std::collections::VecDeque;

use matcher::{
    DefaultMatcher,
    EasyMatcher,
    MarketData,
    Matcher,
};
use polars::prelude::*;
use pyo3_polars::derive::polars_expr;
use serde::Deserialize;

use crate::order::{
    Order,
    OrderDirection,
};
use crate::position::{
    Position,
    PositionDirection,
};

/// Backtest configuration parameters
#[derive(Deserialize, Debug)]
struct BacktestKwargs {
    step_1_limit: u64,
    step_2_limit: u64,
    verbose: bool,
}

/// Output field type for backtest result
pub fn output_type_backtest(_input_fields: &[Field]) -> PolarsResult<Field> {
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

/// Main backtest function that processes trading signals and returns performance metrics
#[polars_expr(output_type_func=output_type_backtest)]
pub fn backtest(inputs: &[Series], kwargs: BacktestKwargs) -> PolarsResult<Series> {
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
        let ask_price: f64 = ask_price_lst.get(i).unwrap();
        let bid_price: f64 = bid_price_lst.get(i).unwrap();

        // Use unwrap_or_default() for cleaner code
        let lg_signal: u8 = lg_signal_lst.get(i).unwrap_or_default();
        let st_signal: u8 = st_signal_lst.get(i).unwrap_or_default();
        let cl_signal: u8 = cl_signal_lst.get(i).unwrap_or_default();
        let cs_signal: u8 = cs_signal_lst.get(i).unwrap_or_default();

        let now: u64 = time_idx_lst.get(i).unwrap();

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

    // Use array literal instead of vec! for better performance
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

/// Low-frequency exchange simulator for backtesting
pub struct LofiEx {
    orders: VecDeque<(u64, Order)>,

    pub ask_price: f64,
    pub bid_price: f64,
    pub limit_up: f64,
    pub limit_dn: f64,

    pub lg_qty: u64,
    pub st_qty: u64,

    pub position: Position,

    matcher: Box<dyn Matcher>,
    verbose: bool,
    now: u64,

    touch_limit_up: bool,
    touch_limit_dn: bool,
}

impl LofiEx {
    pub fn new(prev_pos: u64, limit_dn: f64, limit_up: f64, verbose: bool) -> Self {
        let matcher: Box<dyn Matcher> = {
            match std::env::var("LOFIEX_MATCHER").as_deref() {
                Ok("default") | Err(_) => Box::new(DefaultMatcher),
                Ok("easy") => Box::new(EasyMatcher),
                _ => Box::new(DefaultMatcher),
            }
        };

        if verbose {
            println!(
                "LOFIEX_MATCHER: {}",
                std::env::var("LOFIEX_MATCHER").as_deref().unwrap()
            );
        }

        Self {
            orders: VecDeque::new(),
            ask_price: 0.0,
            bid_price: 0.0,
            lg_qty: 0,
            st_qty: 0,
            now: 0,

            position: Position::new(prev_pos),

            matcher,
            verbose,
            limit_up,
            limit_dn,

            touch_limit_up: false,
            touch_limit_dn: false,
        }
    }

    pub fn on_quote(&mut self, ask_price: f64, bid_price: f64, now: u64) {
        self.now = now;

        if ask_price >= self.limit_dn && ask_price < self.limit_up {
            self.ask_price = ask_price
        }
        if bid_price > self.limit_dn && bid_price <= self.limit_up {
            self.bid_price = bid_price
        }

        self.touch_limit_up = bid_price == self.limit_up && ask_price <= 0.1;
        self.touch_limit_dn = ask_price == self.limit_dn && bid_price <= 0.1;

        self.match_order()
    }

    fn match_order(&mut self) {
        let market_data = MarketData::new(
            self.ask_price,
            self.bid_price,
            self.touch_limit_up,
            self.touch_limit_dn,
        );

        while let Some((time, order)) = self.orders.front() {
            if *time > self.now {
                break;
            }

            let deal_resp = self
                .matcher
                .match_order(order, &market_data, &self.position);

            match order.direction {
                OrderDirection::Long => {
                    self.lg_qty -= order.qty;
                },
                OrderDirection::Short => {
                    self.st_qty -= order.qty;
                },
            }

            if deal_resp.matched {
                self.position.update_deal(
                    deal_resp.deal_price,
                    deal_resp.deal_qty,
                    deal_resp.direction,
                );

                if self.verbose {
                    println!("MATCH ORDER: {}", order);
                }

                self.orders.pop_front();
            } else {
                if self.verbose {
                    println!("UNMATCH ORDER: {}", order);
                }
                self.orders.pop_front();
            }
        }

        self.position.update_price(self.ask_price, self.bid_price);
    }

    fn on_order(&mut self, order: Order) {
        match order.direction {
            OrderDirection::Long => {
                self.lg_qty += order.qty;
            },
            OrderDirection::Short => {
                self.st_qty += order.qty;
            },
        }
        self.orders.push_back((self.now, order));
    }

    pub fn buy(&mut self, price: f64, qty: u64) {
        if price >= self.limit_up || price <= self.limit_dn {
            return;
        }
        self.on_order(Order::new(qty, price, OrderDirection::Long, self.now));
    }

    pub fn sell(&mut self, price: f64, qty: u64) {
        if price >= self.limit_up || price <= self.limit_dn {
            return;
        }
        self.on_order(Order::new(qty, price, OrderDirection::Short, self.now));
    }
}

/// Trading strategy implementation
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
            match d {
                PositionDirection::Long => {
                    if q > 0 {
                        api.sell(bid_price, q);
                        close_lg = true;
                    }
                },
                PositionDirection::Short => {},
                PositionDirection::Flat => {},
            }
        }

        if cs {
            let (d, q) = api.position.get_exposure();
            match d {
                PositionDirection::Long => {},
                PositionDirection::Short => {
                    if q > 0 {
                        api.buy(ask_price, q);
                        close_st = true;
                    }
                },
                PositionDirection::Flat => {},
            }
        }

        if lg && !close_lg {
            let (d, q) = api.position.get_exposure();
            match d {
                PositionDirection::Long => {
                    if q < self.exposure_limit {
                        api.buy(ask_price, 1)
                    }
                },
                PositionDirection::Short => {
                    if q > 0 {
                        api.buy(ask_price, 1)
                    }
                },
                PositionDirection::Flat => {
                    if api.position.all_signal < self.round_limit {
                        api.buy(ask_price, 1)
                    }
                },
            }
        }

        if st && !close_st {
            let (d, q) = api.position.get_exposure();
            match d {
                PositionDirection::Long => {
                    if q > 0 {
                        api.sell(bid_price, 1)
                    }
                },
                PositionDirection::Short => {
                    if q < self.exposure_limit {
                        api.sell(bid_price, 1)
                    }
                },
                PositionDirection::Flat => {
                    if api.position.all_signal < self.round_limit {
                        api.sell(ask_price, 1)
                    }
                },
            }
        }
    }
}
