use polars::prelude::*;

fn ensure_finite(
    value: f64,
    time_index: usize,
    asset_index: Option<usize>,
    stage: &'static str,
    field: &'static str,
) -> PolarsResult<f64> {
    if value.is_finite() {
        return Ok(value);
    }

    let asset = asset_index
        .map(|index| format!(" asset_index={index}"))
        .unwrap_or_default();
    polars_bail!(
        ComputeError:
        "polars-bt tempo failed: time_index={time_index}{asset} stage={stage} \
         reason=NONFINITE_STATE {field}={value}"
    )
}

#[derive(Debug, Clone)]
pub struct PortfolioState {
    hold: Vec<f64>,
    sellable_hold: Vec<f64>,
    cash: f64,
}

impl PortfolioState {
    pub fn new(asset_num: usize) -> Self {
        Self {
            hold: vec![0.0; asset_num],
            sellable_hold: vec![0.0; asset_num],
            cash: 1.0,
        }
    }

    pub fn release_t1(&mut self) {
        self.sellable_hold.clone_from(&self.hold);
    }

    pub fn apply_returns(&mut self, returns: &[f64], time_index: usize) -> PolarsResult<()> {
        for (asset_index, value) in returns.iter().enumerate() {
            let clean = if value.is_nan() { 0.0 } else { *value };
            polars_ensure!(
                clean.is_finite(),
                ComputeError:
                "polars-bt tempo failed: time_index={time_index} asset_index={asset_index} \
                 stage=mark reason=NONFINITE_RETURN period_ret={value}"
            );
            let factor = 1.0 + clean;
            self.hold[asset_index] = ensure_finite(
                self.hold[asset_index] * factor,
                time_index,
                Some(asset_index),
                "mark",
                "holding",
            )?;
            self.sellable_hold[asset_index] = ensure_finite(
                self.sellable_hold[asset_index] * factor,
                time_index,
                Some(asset_index),
                "mark",
                "sellable_holding",
            )?;
        }
        Ok(())
    }

    pub fn target_differences(&self, weights: &[f64], time_index: usize) -> PolarsResult<Vec<f64>> {
        let nav = ensure_finite(self.nav(), time_index, None, "target", "nav")?;
        weights
            .iter()
            .zip(&self.hold)
            .enumerate()
            .map(|(asset_index, (weight, holding))| {
                let clean = if weight.is_nan() { 0.0 } else { *weight };
                ensure_finite(
                    nav * clean - holding,
                    time_index,
                    Some(asset_index),
                    "target",
                    "difference",
                )
            })
            .collect()
    }

    pub fn sellable(&self, asset_index: usize) -> f64 {
        self.sellable_hold[asset_index].max(0.0)
    }

    pub fn apply_sells(
        &mut self,
        amounts: &[f64],
        fee_rate: f64,
        time_index: usize,
    ) -> PolarsResult<f64> {
        let traded_abs: f64 = amounts.iter().map(|amount| amount.abs()).sum();
        let traded_net: f64 = amounts.iter().sum();
        for (asset_index, amount) in amounts.iter().enumerate() {
            self.hold[asset_index] = ensure_finite(
                self.hold[asset_index] + amount,
                time_index,
                Some(asset_index),
                "sell",
                "holding",
            )?;
            self.sellable_hold[asset_index] = ensure_finite(
                self.sellable_hold[asset_index] + amount,
                time_index,
                Some(asset_index),
                "sell",
                "sellable_holding",
            )?;
        }
        self.cash = ensure_finite(
            self.cash - traded_net - traded_abs * fee_rate,
            time_index,
            None,
            "sell",
            "cash",
        )?;
        Ok(traded_abs)
    }

    pub fn apply_buys(
        &mut self,
        amounts: &[f64],
        fee_rate: f64,
        t1: bool,
        time_index: usize,
    ) -> PolarsResult<f64> {
        let traded_abs: f64 = amounts.iter().sum();
        for (asset_index, amount) in amounts.iter().enumerate() {
            self.hold[asset_index] = ensure_finite(
                self.hold[asset_index] + amount,
                time_index,
                Some(asset_index),
                "buy",
                "holding",
            )?;
            if !t1 {
                self.sellable_hold[asset_index] = ensure_finite(
                    self.sellable_hold[asset_index] + amount,
                    time_index,
                    Some(asset_index),
                    "buy",
                    "sellable_holding",
                )?;
            }
        }
        self.cash = ensure_finite(
            self.cash - traded_abs * (1.0 + fee_rate),
            time_index,
            None,
            "buy",
            "cash",
        )?;
        Ok(traded_abs)
    }

    pub fn cash(&self) -> f64 {
        self.cash
    }

    pub fn nav(&self) -> f64 {
        self.cash + self.hold.iter().sum::<f64>()
    }

    pub fn holding_count(&self) -> i64 {
        self.hold.iter().filter(|value| value.abs() > 1e-8).count() as i64
    }
}
