from __future__ import annotations

import math

import polars as pl


def _clean(value: float | None) -> float:
    return 0.0 if value is None or math.isnan(value) else value


def reference_tempo(
    df: pl.DataFrame,
    *,
    t1: bool,
    st_fee: float,
    lg_fee: float,
) -> pl.DataFrame:
    datetimes = df["datetime"].to_list()
    first_datetime = datetimes[0]
    asset_num = next(
        (
            index
            for index, value in enumerate(datetimes)
            if value != first_datetime
        ),
        df.height,
    )
    columns = {name: df[name].to_list() for name in df.columns}
    hold = [0.0] * asset_num
    sellable_hold = [0.0] * asset_num
    cash = 1.0
    current_date = None
    rows: list[dict[str, object]] = []

    for start in range(0, df.height, asset_num):
        end = start + asset_num
        timestamp = columns["datetime"][start]
        trade_date = timestamp.date()
        if trade_date != current_date:
            sellable_hold = hold.copy()
            current_date = trade_date

        for asset_index, value in enumerate(columns["period_ret"][start:end]):
            factor = 1.0 + _clean(value)
            hold[asset_index] *= factor
            sellable_hold[asset_index] *= factor

        traded_abs = 0.0
        if columns["is_rebalance"][start]:
            weights = [_clean(value) for value in columns["weight"][start:end]]
            nav = cash + sum(hold)
            differences = [
                nav * weight - holding
                for weight, holding in zip(weights, hold)
            ]
            sells = [0.0] * asset_num
            for asset_index, amount in enumerate(differences):
                if (
                    amount < 0.0
                    and columns["sellable"][start + asset_index]
                ):
                    sells[asset_index] = max(
                        amount, -max(sellable_hold[asset_index], 0.0)
                    )
            sold_abs = sum(abs(amount) for amount in sells)
            traded_abs += sold_abs
            for asset_index, amount in enumerate(sells):
                hold[asset_index] += amount
                sellable_hold[asset_index] += amount
            cash -= sum(sells)
            cash -= sold_abs * st_fee

            nav = cash + sum(hold)
            differences = [
                nav * weight - holding
                for weight, holding in zip(weights, hold)
            ]
            available_cash = cash
            cumulative = 0.0
            buys = [0.0] * asset_num
            for asset_index, amount in enumerate(differences):
                eligible = (
                    amount > 0.0
                    and columns["buyable"][start + asset_index]
                )
                candidate = amount if eligible else 0.0
                cumulative += candidate * (1.0 + lg_fee)
                if eligible and cumulative < available_cash:
                    buys[asset_index] = amount
            bought_abs = sum(buys)
            traded_abs += bought_abs
            for asset_index, amount in enumerate(buys):
                hold[asset_index] += amount
                if not t1:
                    sellable_hold[asset_index] += amount
            cash -= bought_abs * (1.0 + lg_fee)

        nav = cash + sum(hold)
        rows.append(
            {
                "datetime": timestamp,
                "cash": cash,
                "nav": nav,
                "turnover": traded_abs / nav if nav > 1e-12 else 0.0,
                "holding_count": sum(
                    abs(value) > 1e-8 for value in hold
                ),
            }
        )

    return pl.DataFrame(rows)
