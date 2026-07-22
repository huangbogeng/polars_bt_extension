#[derive(Debug, Clone)]
pub struct PortfolioState {
    hold: Vec<f64>,
    cash: f64,
    daily_trade_abs: f64,
    daily_turnover: f64,
}

impl PortfolioState {
    pub fn new(asset_num: usize) -> Self {
        Self {
            hold: vec![0.0; asset_num],
            cash: 1.0,
            daily_trade_abs: 0.0,
            daily_turnover: 0.0,
        }
    }

    pub fn begin_day(&mut self) {
        self.daily_trade_abs = 0.0;
        self.daily_turnover = 0.0;
    }

    pub fn apply_returns(&mut self, returns: &[f64]) {
        for (holding, value) in self.hold.iter_mut().zip(returns) {
            let clean = if value.is_nan() { 0.0 } else { *value };
            *holding *= 1.0 + clean;
        }
    }

    pub fn target_difference(&self, weights: &[f64]) -> Vec<f64> {
        let scale = if self.total_hold() > 0.0001 {
            self.nav()
        } else {
            1.0
        };
        weights
            .iter()
            .zip(&self.hold)
            .map(|(weight, holding)| {
                let clean = if weight.is_nan() { 0.0 } else { *weight };
                scale * clean - holding
            })
            .collect()
    }

    pub fn apply_deals(&mut self, amounts: &[f64], fee_rate: f64) {
        let traded_abs: f64 = amounts.iter().map(|value| value.abs()).sum();
        let traded_net: f64 = amounts.iter().sum();
        self.daily_trade_abs += traded_abs;
        for (holding, amount) in self.hold.iter_mut().zip(amounts) {
            *holding += amount;
        }
        self.cash -= traded_net;
        self.cash -= traded_abs * fee_rate;
    }

    pub fn finalize_turnover(&mut self) {
        let nav = self.nav();
        self.daily_turnover = if nav > 1e-12 {
            self.daily_trade_abs / nav
        } else {
            0.0
        };
    }

    pub fn total_hold(&self) -> f64 {
        self.hold.iter().sum()
    }

    pub fn nav(&self) -> f64 {
        self.cash + self.total_hold()
    }

    pub fn cash(&self) -> f64 {
        self.cash
    }

    pub fn turnover(&self) -> f64 {
        self.daily_turnover
    }

    pub fn holdings(&self) -> &[f64] {
        &self.hold
    }

    pub fn holding_count(&self) -> i64 {
        self.hold.iter().filter(|value| value.abs() > 1e-8).count() as i64
    }
}

#[cfg(test)]
mod tests {
    use super::PortfolioState;

    #[test]
    fn applies_returns_and_deals() {
        let mut state = PortfolioState::new(2);
        state.apply_deals(&[0.4, 0.6], 0.0001);
        state.apply_returns(&[0.10, -0.10]);
        assert!((state.cash() + 0.0001).abs() < 1e-15);
        assert!((state.holdings()[0] - 0.44).abs() < 1e-15);
        assert!((state.holdings()[1] - 0.54).abs() < 1e-15);
        assert!((state.nav() - 0.9799).abs() < 1e-15);
    }

    #[test]
    fn target_difference_uses_nav_after_initial_build() {
        let mut state = PortfolioState::new(2);
        assert_eq!(state.target_difference(&[0.4, 0.6]), vec![0.4, 0.6]);
        state.apply_deals(&[0.4, 0.6], 0.0);
        state.apply_returns(&[0.10, -0.10]);
        let diff = state.target_difference(&[0.5, 0.5]);
        assert!((diff[0] - 0.05).abs() < 1e-15);
        assert!((diff[1] + 0.05).abs() < 1e-15);
    }
}
