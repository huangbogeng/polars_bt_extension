use polars::prelude::*;

fn ensure_finite(
    value: f64,
    day_index: usize,
    asset_index: Option<usize>,
    stage: &'static str,
    reason: &'static str,
    field: &'static str,
) -> PolarsResult<f64> {
    if value.is_finite() {
        return Ok(value);
    }

    let asset = asset_index
        .map(|index| format!(" asset_index={index}"))
        .unwrap_or_default();
    polars_bail!(ComputeError:
        "polars-bt mosaic failed: day_index={day_index}{asset} stage={stage} reason={reason} {field}={value}"
    )
}

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

    pub fn apply_returns(
        &mut self,
        returns: &[f64],
        day_index: usize,
        stage: &'static str,
    ) -> PolarsResult<()> {
        for (asset_index, (holding, value)) in self.hold.iter_mut().zip(returns).enumerate() {
            let clean = if value.is_nan() { 0.0 } else { *value };
            *holding = ensure_finite(
                *holding * (1.0 + clean),
                day_index,
                Some(asset_index),
                stage,
                "NONFINITE_HOLDING",
                "holding",
            )?;
        }
        Ok(())
    }

    pub fn target_difference(
        &self,
        weights: &[f64],
        day_index: usize,
        stage: &'static str,
    ) -> PolarsResult<Vec<f64>> {
        let scale = if self.total_hold() > 0.0001 {
            self.nav()
        } else {
            1.0
        };
        weights
            .iter()
            .zip(&self.hold)
            .enumerate()
            .map(|(asset_index, (weight, holding))| {
                let clean = if weight.is_nan() { 0.0 } else { *weight };
                ensure_finite(
                    scale * clean - holding,
                    day_index,
                    Some(asset_index),
                    stage,
                    "NONFINITE_TARGET_DIFFERENCE",
                    "difference",
                )
            })
            .collect()
    }

    pub fn apply_deals(
        &mut self,
        amounts: &[f64],
        fee_rate: f64,
        day_index: usize,
        stage: &'static str,
    ) -> PolarsResult<()> {
        let traded_abs = ensure_finite(
            amounts.iter().map(|value| value.abs()).sum(),
            day_index,
            None,
            stage,
            "NONFINITE_TRADE_TOTAL",
            "traded_abs",
        )?;
        let traded_net = ensure_finite(
            amounts.iter().sum(),
            day_index,
            None,
            stage,
            "NONFINITE_TRADE_TOTAL",
            "traded_net",
        )?;
        self.daily_trade_abs = ensure_finite(
            self.daily_trade_abs + traded_abs,
            day_index,
            None,
            stage,
            "NONFINITE_TURNOVER_BASE",
            "daily_trade_abs",
        )?;
        for (asset_index, (holding, amount)) in self.hold.iter_mut().zip(amounts).enumerate() {
            *holding = ensure_finite(
                *holding + amount,
                day_index,
                Some(asset_index),
                stage,
                "NONFINITE_HOLDING",
                "holding",
            )?;
        }
        self.cash = ensure_finite(
            self.cash - traded_net - traded_abs * fee_rate,
            day_index,
            None,
            stage,
            "NONFINITE_CASH",
            "cash",
        )?;
        Ok(())
    }

    pub fn finalize_turnover(&mut self, day_index: usize) -> PolarsResult<()> {
        let nav = ensure_finite(
            self.nav(),
            day_index,
            None,
            "snapshot",
            "NONFINITE_NAV",
            "nav",
        )?;
        self.daily_turnover = if nav > 1e-12 {
            ensure_finite(
                self.daily_trade_abs / nav,
                day_index,
                None,
                "snapshot",
                "NONFINITE_TURNOVER",
                "turnover",
            )?
        } else {
            0.0
        };
        Ok(())
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

    #[cfg(test)]
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
        state.apply_deals(&[0.4, 0.6], 0.0001, 0, "test").unwrap();
        state.apply_returns(&[0.10, -0.10], 0, "test").unwrap();
        assert!((state.cash() + 0.0001).abs() < 1e-15);
        assert!((state.holdings()[0] - 0.44).abs() < 1e-15);
        assert!((state.holdings()[1] - 0.54).abs() < 1e-15);
        assert!((state.nav() - 0.9799).abs() < 1e-15);
    }

    #[test]
    fn target_difference_uses_nav_after_initial_build() {
        let mut state = PortfolioState::new(2);
        assert_eq!(
            state.target_difference(&[0.4, 0.6], 0, "test").unwrap(),
            vec![0.4, 0.6]
        );
        state.apply_deals(&[0.4, 0.6], 0.0, 0, "test").unwrap();
        state.apply_returns(&[0.10, -0.10], 0, "test").unwrap();
        let diff = state.target_difference(&[0.5, 0.5], 0, "test").unwrap();
        assert!((diff[0] - 0.05).abs() < 1e-15);
        assert!((diff[1] + 0.05).abs() < 1e-15);
    }
}
