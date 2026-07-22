use std::fmt;

use chrono::offset::FixedOffset;
use chrono::DateTime;

/// Convert timestamp to Beijing timezone string representation
fn timestamp_to_beijing(timestamp_ms: i64) -> String {
    let timestamp_sec = timestamp_ms / 1000;
    let utc_time = DateTime::from_timestamp(timestamp_sec, 0).unwrap();
    let beijing_offset = FixedOffset::east_opt(8 * 3600).unwrap();
    let beijing_time = utc_time.with_timezone(&beijing_offset);
    beijing_time.format("%Y-%m-%d %H:%M:%S").to_string()
}

/// Order direction enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderDirection {
    Long = 1,
    Short = 2,
}

impl fmt::Display for OrderDirection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            OrderDirection::Long => write!(f, "Long"),
            OrderDirection::Short => write!(f, "Short"),
        }
    }
}

/// Order structure representing a trading order
#[derive(Debug)]
pub struct Order {
    pub qty: u64,
    pub price: f64,
    pub direction: OrderDirection,
    pub ts_event: u64,
}

impl Order {
    /// Create a new order with the specified parameters
    pub fn new(qty: u64, price: f64, direction: OrderDirection, ts_event: u64) -> Self {
        Self {
            qty,
            price,
            direction,
            ts_event,
        }
    }
}

impl fmt::Display for Order {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Order {{ qty: {}, price: {}, direction: {}, ts_event: {} , timestamp: {}}}",
            self.qty,
            self.price,
            self.direction,
            self.ts_event,
            timestamp_to_beijing(self.ts_event as i64),
        )
    }
}
