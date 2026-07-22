use std::collections::VecDeque;

use super::matcher::{
    DefaultMatcher,
    EasyMatcher,
    MarketData,
    Matcher,
};
use super::order::{
    Order,
    OrderDirection,
};
use super::position::Position;

/// Low-frequency exchange simulator for the Pulse T0 engine.
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
        let matcher: Box<dyn Matcher> = match std::env::var("LOFIEX_MATCHER").as_deref() {
            Ok("default") | Err(_) => Box::new(DefaultMatcher),
            Ok("easy") => Box::new(EasyMatcher),
            _ => Box::new(DefaultMatcher),
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
                OrderDirection::Long => self.lg_qty -= order.qty,
                OrderDirection::Short => self.st_qty -= order.qty,
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
            } else if self.verbose {
                println!("UNMATCH ORDER: {}", order);
            }
            self.orders.pop_front();
        }

        self.position.update_price(self.ask_price, self.bid_price);
    }

    fn on_order(&mut self, order: Order) {
        match order.direction {
            OrderDirection::Long => self.lg_qty += order.qty,
            OrderDirection::Short => self.st_qty += order.qty,
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
