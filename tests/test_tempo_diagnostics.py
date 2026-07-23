from __future__ import annotations

import os
import subprocess
import sys
import textwrap


def run_diagnostic_script(tmp_path, *, verbose: bool) -> subprocess.CompletedProcess[str]:
    script = textwrap.dedent(
        f"""
        from datetime import datetime

        import polars as pl
        from polars_bt import tempo

        panel = pl.DataFrame({{
            "datetime": [
                datetime(2024, 1, 2, 9, 31),
                datetime(2024, 1, 2, 9, 31),
                datetime(2024, 1, 2, 14, 30),
                datetime(2024, 1, 2, 14, 30),
            ],
            "asset": ["A", "B", "A", "B"],
            "weight": [float("nan"), 0.4, 0.7, 0.7],
            "period_ret": [0.0, float("nan"), 0.0, 0.0],
            "buyable": [True] * 4,
            "sellable": [True] * 4,
            "is_rebalance": [True] * 4,
        }})

        pl.Config.set_verbose({verbose!r})
        result = panel.select(
            tempo(
                "datetime",
                "asset",
                "weight",
                "period_ret",
                "buyable",
                "sellable",
                "is_rebalance",
                st_fee=0.0,
                lg_fee=0.0,
            ).alias("result")
        ).unnest("result")
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


def test_tempo_is_quiet_when_polars_verbose_is_disabled(tmp_path):
    completed = run_diagnostic_script(tmp_path, verbose=False)

    assert completed.returncode == 0, completed.stderr
    assert "[polars-bt][tempo]" not in completed.stderr


def test_tempo_emits_bounded_verbose_diagnostics(tmp_path):
    completed = run_diagnostic_script(tmp_path, verbose=True)

    assert completed.returncode == 0, completed.stderr
    assert "[polars-bt][tempo][INFO] event=start" in completed.stderr
    assert "[polars-bt][tempo][WARN] event=input_summary" in completed.stderr
    assert "nan_weight=1" in completed.stderr
    assert "nan_period_ret=1" in completed.stderr
    assert "overweight_timestamps=1" in completed.stderr
    assert "code=NAN_WEIGHT time_index=0 asset_index=0" in completed.stderr
    assert "code=NAN_PERIOD_RET time_index=0 asset_index=1" in completed.stderr
    assert "[polars-bt][tempo][INFO] event=finish" in completed.stderr
