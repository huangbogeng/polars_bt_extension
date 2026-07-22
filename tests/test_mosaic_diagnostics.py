from __future__ import annotations

import os
import subprocess
import sys
import textwrap


def run_diagnostic_script(tmp_path, mode: str) -> subprocess.CompletedProcess[str]:
    script = textwrap.dedent(
        f"""
        import polars as pl
        from polars_bt import mosaic

        mode = {mode!r}
        if mode == "halt":
            panel = pl.DataFrame({{
                "date": [1, 2, 3],
                "weight": [0.4, 0.0, 0.0],
                "ovn_ret": [0.0, 0.0, 0.0],
                "ind_ret": [0.0, 0.0, 0.0],
                "buyable": [True, True, True],
                "sellable": [True, True, True],
                "prev_close": [10.0, 10.0, 10.0],
                "vwap": [10.0, 10.0, 10.0],
                "is_rebalance": [True, True, True],
            }})
            asset_num = 1
            st_fee = 3.0
        elif mode == "nonfinite":
            panel = pl.DataFrame({{
                "date": [1, 2],
                "weight": [0.4, 0.4],
                "ovn_ret": [0.0, 0.0],
                "ind_ret": [0.0, float("inf")],
                "buyable": [True, True],
                "sellable": [True, True],
                "prev_close": [10.0, 10.0],
                "vwap": [10.0, 10.0],
                "is_rebalance": [True, False],
            }})
            asset_num = 1
            st_fee = 0.0
        elif mode == "nullable":
            panel = pl.DataFrame({{
                "date": [1, 1, 2, 2],
                "weight": [0.5, 0.5, 0.5, 0.5],
                "ovn_ret": [None, 0.0, 0.0, 0.0],
                "ind_ret": [0.0, 0.0, 0.0, 0.0],
                "buyable": [True, True, True, True],
                "sellable": [True, True, True, True],
                "prev_close": [10.0, 10.0, 10.0, 10.0],
                "vwap": [10.0, None, 10.0, 10.0],
                "is_rebalance": [True, True, True, True],
            }})
            asset_num = 2
            st_fee = 0.0
        else:
            panel = pl.DataFrame({{
                "date": [1, 2, 3, 3],
                "weight": [float("nan"), 0.4, 0.7, 0.7],
                "ovn_ret": [0.0, 0.0, 0.0, 0.0],
                "ind_ret": [0.0, 0.0, 0.0, 0.0],
                "buyable": [True, True, True, True],
                "sellable": [True, True, True, True],
                "prev_close": [10.0, 10.0, 10.0, 10.0],
                "vwap": [10.0, 10.0, 10.0, 10.0],
                "is_rebalance": [False, True, True, True],
            }})
            asset_num = 2
            st_fee = 0.0

        def execute():
            return panel.select(
                mosaic(
                    date="date",
                    weight="weight",
                    ovn_ret="ovn_ret",
                    ind_ret="ind_ret",
                    buyable="buyable",
                    sellable="sellable",
                    prev_close="prev_close",
                    vwap="vwap",
                    is_rebalance="is_rebalance",
                    asset_num=asset_num,
                    st_fee=st_fee,
                    lg_fee=0.0,
                ).alias("daily")
            ).unnest("daily")

        if mode == "quiet":
            pl.Config.set_verbose(False)
            result = execute()
        elif mode in {{"set", "halt", "nullable"}}:
            pl.Config.set_verbose(True)
            result = execute()
        elif mode == "toggle":
            pl.Config.set_verbose(False)
            execute()
            pl.Config.set_verbose(True)
            result = execute()
        elif mode == "nonfinite":
            pl.Config.set_verbose(False)
            result = execute()
        else:
            with pl.Config(verbose=True):
                result = execute()

        assert result.height == 2
        """
    )
    env = os.environ.copy()
    env.pop("POLARS_VERBOSE", None)
    return subprocess.run(
        [sys.executable, "-c", script],
        cwd=tmp_path,
        env=env,
        text=True,
        capture_output=True,
        check=False,
    )


def test_mosaic_is_quiet_when_polars_verbose_is_disabled(tmp_path):
    completed = run_diagnostic_script(tmp_path, "quiet")

    assert completed.returncode == 0, completed.stderr
    assert "[polars-bt][mosaic]" not in completed.stderr


def test_mosaic_emits_bounded_diagnostics_with_set_verbose(tmp_path):
    completed = run_diagnostic_script(tmp_path, "set")

    assert completed.returncode == 0, completed.stderr
    assert "[polars-bt][mosaic][INFO] event=start" in completed.stderr
    assert "[polars-bt][mosaic][WARN] event=input_summary" in completed.stderr
    assert "nan_weight=1" in completed.stderr
    assert "mixed_date_blocks=1" in completed.stderr
    assert "mixed_rebalance_blocks=1" in completed.stderr
    assert "code=NAN_WEIGHT day_index=0 asset_index=0" in completed.stderr
    assert "code=MIXED_DATE_BLOCK day_index=0 asset_index=1" in completed.stderr
    assert "[polars-bt][mosaic][INFO] event=finish" in completed.stderr


def test_mosaic_honors_polars_verbose_context_manager(tmp_path):
    completed = run_diagnostic_script(tmp_path, "context")

    assert completed.returncode == 0, completed.stderr
    assert "[polars-bt][mosaic][INFO] event=start" in completed.stderr
    assert "[polars-bt][mosaic][INFO] event=finish" in completed.stderr


def test_mosaic_reloads_verbose_after_a_quiet_call(tmp_path):
    completed = run_diagnostic_script(tmp_path, "toggle")

    assert completed.returncode == 0, completed.stderr
    assert completed.stderr.count("[polars-bt][mosaic][INFO] event=start") == 1
    assert completed.stderr.count("[polars-bt][mosaic][INFO] event=finish") == 1


def test_mosaic_preserves_nullable_market_data_diagnostics(tmp_path):
    completed = run_diagnostic_script(tmp_path, "nullable")

    assert completed.returncode == 0, completed.stderr
    assert "nan_ovn_ret=1" in completed.stderr
    assert "invalid_vwap=1" in completed.stderr
    assert "code=NAN_OVN_RET day_index=0 asset_index=0" in completed.stderr
    assert "code=INVALID_VWAP day_index=0 asset_index=1" in completed.stderr


def test_mosaic_logs_negative_cash_halt(tmp_path):
    completed = run_diagnostic_script(tmp_path, "halt")

    assert completed.returncode == 0, completed.stderr
    assert "event=halt reason=NEGATIVE_CASH day_index=1" in completed.stderr
    assert "event=finish completed_days=2 expected_days=3" in completed.stderr


def test_mosaic_rejects_non_finite_state_with_context(tmp_path):
    completed = run_diagnostic_script(tmp_path, "nonfinite")

    assert completed.returncode != 0
    assert "day_index=1" in completed.stderr
    assert "asset_index=0" in completed.stderr
    assert "stage=intraday_settlement" in completed.stderr
    assert "reason=NONFINITE_HOLDING" in completed.stderr
