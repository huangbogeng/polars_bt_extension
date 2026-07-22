use polars::prelude::*;

use super::engine::DailySnapshot;

pub fn build_output(mut dates: Series, snapshots: &[DailySnapshot]) -> PolarsResult<Series> {
    dates.rename("date".into());
    let out = [
        dates,
        Series::new(
            "cash".into(),
            snapshots
                .iter()
                .map(|snapshot| snapshot.cash)
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "nav".into(),
            snapshots
                .iter()
                .map(|snapshot| snapshot.nav)
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "turnover".into(),
            snapshots
                .iter()
                .map(|snapshot| snapshot.turnover)
                .collect::<Vec<_>>(),
        ),
        Series::new(
            "holding_count".into(),
            snapshots
                .iter()
                .map(|snapshot| snapshot.holding_count)
                .collect::<Vec<_>>(),
        ),
    ];

    StructChunked::from_series("mosaic".into(), snapshots.len(), out.iter())
        .map(|chunked| chunked.into_series())
}
