use crate::pulse::order::OrderDirection;

/// Position direction enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PositionDirection {
    Long,
    Short,
    Flat,
}

/// Backtest result summary structure
#[derive(Debug, Clone, Copy)]
pub struct Summary {
    pub lg_pos: u64,
    pub st_pos: u64,
    pub lg_win: u64,
    pub st_win: u64,
    pub all_signal: u64,
    pub floating_pnl: f64,
    pub realized_pnl: f64,
}

/// Position management structure for tracking trading positions and P&L
#[derive(Debug)]
pub struct Position {
    floating_pnl: f64,
    realized_pnl: f64,

    ask_price: f64,
    bid_price: f64,

    lg_pos: u64,
    st_pos: u64,

    prev_pos: u64,

    exposure_avg_price: f64,

    lg_win: u64,
    st_win: u64,

    pub all_signal: u64,
}

impl Default for Position {
    fn default() -> Self {
        Self {
            prev_pos: 0,
            lg_pos: 0,
            st_pos: 0,

            floating_pnl: 0.0,
            realized_pnl: 0.0,

            ask_price: 0.0,
            bid_price: 0.0,

            exposure_avg_price: 0.0,

            lg_win: 0,
            st_win: 0,

            all_signal: 0,
        }
    }
}

impl Position {
    /// Create a new position with the specified previous position limit
    pub fn new(prev_pos: u64) -> Self {
        Self {
            prev_pos,
            ..Default::default()
        }
    }

    /// Get available long position quantity
    pub fn get_long_avl_qty(&self) -> u64 {
        self.prev_pos - self.lg_pos
    }

    /// Get available short position quantity
    pub fn get_short_avl_qty(&self) -> u64 {
        self.prev_pos - self.st_pos
    }

    /// Get current exposure (position direction and quantity)
    pub fn get_exposure(&self) -> (PositionDirection, u64) {
        match self.lg_pos.cmp(&self.st_pos) {
            std::cmp::Ordering::Greater => (PositionDirection::Long, self.lg_pos - self.st_pos),
            std::cmp::Ordering::Less => (PositionDirection::Short, self.st_pos - self.lg_pos),
            std::cmp::Ordering::Equal => (PositionDirection::Flat, 0),
        }
    }

    /// Get position summary with all metrics
    pub fn get_summary(&self) -> Summary {
        Summary {
            lg_pos: self.lg_pos,
            st_pos: self.st_pos,
            lg_win: self.lg_win,
            st_win: self.st_win,
            all_signal: self.all_signal,
            floating_pnl: self.floating_pnl,
            realized_pnl: self.realized_pnl,
        }
    }

    /// Increase position and update average price
    fn raise_position(
        &mut self,
        deal_price: f64,
        deal_qty: u64,
        direction: OrderDirection,
        old_exposure_qty: u64,
    ) {
        self.exposure_avg_price = (old_exposure_qty as f64 * self.exposure_avg_price
            + deal_qty as f64 * deal_price)
            / (deal_qty as f64 + old_exposure_qty as f64);

        self.floating_pnl = match direction {
            OrderDirection::Long => {
                (self.bid_price - self.exposure_avg_price) * ((old_exposure_qty + deal_qty) as f64)
            },
            OrderDirection::Short => {
                (self.exposure_avg_price - self.ask_price) * ((old_exposure_qty + deal_qty) as f64)
            },
        }
    }

    /// Update position after a deal is executed
    pub fn update_deal(&mut self, deal_price: f64, deal_qty: u64, direction: OrderDirection) {
        let (old_exposure_direction, old_exposure_qty) = self.get_exposure();

        match direction {
            OrderDirection::Long => {
                self.lg_pos += deal_qty;
                match old_exposure_direction {
                    PositionDirection::Long => {
                        self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                    },
                    PositionDirection::Flat => {
                        self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                        self.all_signal += 1;
                    },
                    PositionDirection::Short => {
                        let close_qty = std::cmp::min(old_exposure_qty, deal_qty);
                        let current_profit: f64 =
                            (self.exposure_avg_price - deal_price) * close_qty as f64;
                        self.realized_pnl += current_profit;

                        if deal_qty > old_exposure_qty {
                            if current_profit >= 0.0 {
                                self.st_win += 1;
                            }
                            self.all_signal += 1;
                            self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                        } else if deal_qty == old_exposure_qty {
                            self.floating_pnl = 0.0;
                            if current_profit >= 0.0 {
                                self.st_win += 1;
                            }
                            self.exposure_avg_price = 0.0;
                        } else if deal_qty < old_exposure_qty {
                            self.floating_pnl =
                                self.exposure_avg_price * (old_exposure_qty - deal_qty) as f64;
                        }
                    },
                }
            },
            OrderDirection::Short => {
                self.st_pos += deal_qty;
                match old_exposure_direction {
                    PositionDirection::Short => {
                        self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                    },
                    PositionDirection::Flat => {
                        self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                        self.all_signal += 1;
                    },
                    PositionDirection::Long => {
                        let close_qty = std::cmp::min(old_exposure_qty, deal_qty);
                        let current_profit: f64 =
                            (deal_price - self.exposure_avg_price) * close_qty as f64;
                        self.realized_pnl += current_profit;

                        if deal_qty > old_exposure_qty {
                            if current_profit >= 0.0 {
                                self.lg_win += 1;
                            }
                            self.all_signal += 1;
                            self.raise_position(deal_price, deal_qty, direction, old_exposure_qty);
                        } else if deal_qty == old_exposure_qty {
                            self.floating_pnl = 0.0;
                            if current_profit >= 0.0 {
                                self.lg_win += 1;
                            }
                            self.exposure_avg_price = 0.0;
                        } else if deal_qty < old_exposure_qty {
                            self.floating_pnl =
                                self.exposure_avg_price * (old_exposure_qty - deal_qty) as f64;
                        }
                    },
                }
            },
        }
    }

    /// Update market prices and recalculate floating P&L
    pub fn update_price(&mut self, ask_price: f64, bid_price: f64) {
        if ask_price > 0.0 {
            self.ask_price = ask_price;
        }
        if bid_price > 0.0 {
            self.bid_price = bid_price;
        }
        let (d, exposure_qty) = self.get_exposure();
        self.floating_pnl = match d {
            PositionDirection::Long => {
                if self.bid_price > 0.0 {
                    (self.bid_price - self.exposure_avg_price) * exposure_qty as f64
                } else {
                    0.0
                }
            },
            PositionDirection::Short => {
                if self.ask_price > 0.0 {
                    (self.exposure_avg_price - self.ask_price) * exposure_qty as f64
                } else {
                    0.0
                }
            },
            PositionDirection::Flat => 0.0,
        }
    }
}
