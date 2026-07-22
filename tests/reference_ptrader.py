from __future__ import annotations

import math

import polars as pl


def _clean(value: float | None) -> float:
    return 0.0 if value is None or math.isnan(value) else value


def reference_ptrader(
    df: pl.DataFrame,
    asset_num: int,
    st_fee: float,
    lg_fee: float,
) -> pl.DataFrame:
    hold = [0.0] * asset_num
    cash = 1.0
    rows: list[dict[str, object]] = []
    columns = {name: df[name].to_list() for name in df.columns}

    for offset in range(0, df.height, asset_num):
        stop = offset + asset_num
        for index, value in enumerate(columns["ovn_ret"][offset:stop]):
            hold[index] *= 1.0 + _clean(value)

        daily_trade_abs = 0.0
        if any(columns["is_rebalance"][offset:stop]):
            total_hold = sum(hold)
            nav = cash + total_hold
            scale = nav if total_hold > 0.0001 else 1.0
            weights = [_clean(value) for value in columns["weight"][offset:stop]]
            diff = [scale * weight - value for weight, value in zip(weights, hold)]
            sells = [
                amount
                if amount < 0.0
                and columns["sellable"][offset + index]
                and columns["prev_close"][offset + index] > 0.0
                and columns["vwap"][offset + index] > 0.0
                else 0.0
                for index, amount in enumerate(diff)
            ]
            sold_abs = sum(abs(value) for value in sells)
            daily_trade_abs += sold_abs
            hold = [value + amount for value, amount in zip(hold, sells)]
            cash -= sum(sells)
            cash -= sold_abs * st_fee

            total_hold = sum(hold)
            nav = cash + total_hold
            scale = nav if total_hold > 0.0001 else 1.0
            diff = [scale * weight - value for weight, value in zip(weights, hold)]
            available_cash = cash
            cumulative = 0.0
            buys = [0.0] * asset_num
            for index, amount in enumerate(diff):
                eligible = (
                    amount > 0.0
                    and columns["buyable"][offset + index]
                    and columns["prev_close"][offset + index] > 0.0
                    and columns["vwap"][offset + index] > 0.0
                )
                candidate = amount if eligible else 0.0
                cumulative += candidate * (1.0 + lg_fee)
                if eligible and cumulative < available_cash:
                    buys[index] = amount
            bought_abs = sum(abs(value) for value in buys)
            daily_trade_abs += bought_abs
            hold = [value + amount for value, amount in zip(hold, buys)]
            cash -= sum(buys)
            cash -= bought_abs * lg_fee

        for index, value in enumerate(columns["ind_ret"][offset:stop]):
            hold[index] *= 1.0 + _clean(value)
        nav = cash + sum(hold)
        rows.append(
            {
                "date": columns["date"][offset],
                "cash": cash,
                "nav": nav,
                "turnover": daily_trade_abs / nav if nav > 1e-12 else 0.0,
                "holding_count": sum(abs(value) > 1e-8 for value in hold),
            }
        )
        if cash < -0.00001:
            break

    return pl.DataFrame(rows)
