use crate::pulse::order::{
    Order,
    OrderDirection,
};
use crate::pulse::position::Position;

/// Market data structure containing current price information
#[derive(Debug, Clone, Copy)]
pub struct MarketData {
    pub ask_price: f64,
    pub bid_price: f64,
    pub touch_limit_up: bool,
    pub touch_limit_dn: bool,
}

impl MarketData {
    /// Create new market data with specified prices and limit status
    pub fn new(ask_price: f64, bid_price: f64, touch_limit_up: bool, touch_limit_dn: bool) -> Self {
        Self {
            ask_price,
            bid_price,
            touch_limit_up,
            touch_limit_dn,
        }
    }
}

/// Deal response structure containing matching results
#[derive(Debug, Clone, Copy)]
pub struct DealResp {
    pub matched: bool,
    pub deal_price: f64,
    pub deal_qty: u64,
    pub direction: OrderDirection,
}

/// Order matcher trait for implementing different matching algorithms
pub trait Matcher {
    /// Match an order against current market data and position
    fn match_order(&self, order: &Order, market: &MarketData, position: &Position) -> DealResp;
}

/// Default matcher implementing strict cross matching
///
/// This matcher implements strict cross matching rules:
/// - Long orders: price >= ask_price, available long position > 0, not touching limit up
/// - Short orders: price <= bid_price, available short position > 0, not touching limit down
pub struct DefaultMatcher;

impl Matcher for DefaultMatcher {
    fn match_order(&self, order: &Order, market: &MarketData, position: &Position) -> DealResp {
        let can_match = match order.direction {
            OrderDirection::Long => {
                (order.price >= market.ask_price)
                    && (position.get_long_avl_qty() > 0)
                    && (!market.touch_limit_up)
            },
            OrderDirection::Short => {
                (order.price <= market.bid_price)
                    && (position.get_short_avl_qty() > 0)
                    && (!market.touch_limit_dn)
            },
        };

        DealResp {
            matched: can_match,
            deal_price: match order.direction {
                OrderDirection::Long => market.ask_price,
                OrderDirection::Short => market.bid_price,
            },
            deal_qty: order.qty,
            direction: order.direction,
        }
    }
}

/// Easy matcher implementing relaxed matching rules
///
/// This matcher implements more lenient matching rules:
/// - Long orders: price >= ask_price, available long position > 0
/// - Short orders: price <= bid_price, available short position > 0 No limit price restrictions
pub struct EasyMatcher;

impl Matcher for EasyMatcher {
    fn match_order(&self, order: &Order, market: &MarketData, position: &Position) -> DealResp {
        let can_match = match order.direction {
            OrderDirection::Long => {
                (order.price >= market.ask_price) && (position.get_long_avl_qty() > 0)
            },
            OrderDirection::Short => {
                (order.price <= market.bid_price) && (position.get_short_avl_qty() > 0)
            },
        };

        DealResp {
            matched: can_match,
            deal_price: match order.direction {
                OrderDirection::Long => market.ask_price,
                OrderDirection::Short => market.bid_price,
            },
            deal_qty: order.qty,
            direction: order.direction,
        }
    }
}
