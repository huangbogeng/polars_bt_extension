"""Release promotion contracts, independent of the native extension."""

import io
import json
from pathlib import Path
import sys
import tarfile
import zipfile

import pytest

if sys.version_info < (3, 11):
    pytest.skip("CI tooling runs on Python 3.12", allow_module_level=True)

from tools import ci_release as release

VERSION = "0.2.4"
COMMIT = "a" * 40
REPOSITORY = "huangbogeng/polars_bt_extension"
SOURCE = release.provenance(REPOSITORY, COMMIT, "123", "2")


def write_distributions(
    directory: Path, *, wheel_version=VERSION, sdist_version=VERSION
):
    wheel_name, sdist_name = release.filenames(VERSION)
    with zipfile.ZipFile(directory / wheel_name, "w") as wheel:
        wheel.writestr(
            f"polars_bt-{VERSION}.dist-info/METADATA",
            f"Name: polars_bt\nVersion: {wheel_version}\n",
        )
    with tarfile.open(directory / sdist_name, "w:gz") as sdist:
        for name, data in {
            "Cargo.toml": f'[package]\nname = "polars_bt"\nversion = "{sdist_version}"\n',
            "Cargo.lock": "",
            "pyproject.toml": "",
            "src/lib.rs": "",
            "polars_bt/__init__.py": "",
        }.items():
            content = data.encode()
            member = tarfile.TarInfo(f"polars_bt-{VERSION}/{name}")
            member.size = len(content)
            sdist.addfile(member, io.BytesIO(content))


@pytest.fixture
def distributions(tmp_path):
    write_distributions(tmp_path)
    return tmp_path


def test_manifest_roundtrip_preserves_artifact_hashes(distributions):
    release.create_manifest(distributions, VERSION, SOURCE)
    release.verify_manifest(distributions, VERSION, SOURCE)
    manifest = json.loads((distributions / release.MANIFEST).read_text())
    assert set(manifest["files"]) == set(release.filenames(VERSION))
    assert all(len(file["sha256"]) == 64 for file in manifest["files"].values())


@pytest.mark.parametrize("kind", ["wheel", "sdist"])
def test_rejects_wrong_distribution_version(tmp_path, kind):
    write_distributions(
        tmp_path,
        wheel_version="0.2.3" if kind == "wheel" else VERSION,
        sdist_version="0.2.3" if kind == "sdist" else VERSION,
    )
    with pytest.raises(ValueError, match=f"{kind} version mismatch"):
        release.create_manifest(tmp_path, VERSION, SOURCE)


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("repository", "other/repo"),
        ("commit", "b" * 40),
        ("run_id", "124"),
        ("run_attempt", "1"),
    ],
)
def test_rejects_different_provenance(distributions, field, value):
    release.create_manifest(distributions, VERSION, SOURCE)
    with pytest.raises(ValueError, match=f"manifest {field} mismatch"):
        release.verify_manifest(distributions, VERSION, {**SOURCE, field: value})


def test_rejects_tampered_wheel(distributions):
    release.create_manifest(distributions, VERSION, SOURCE)
    wheel = distributions / release.filenames(VERSION)[0]
    # A valid ZIP can contain trailing data, so metadata checks alone miss this.
    with wheel.open("ab") as file:
        file.write(b"changed after tests")
    with pytest.raises(ValueError, match="file hash/size mismatch"):
        release.verify_manifest(distributions, VERSION, SOURCE)


def test_rejects_extra_upload_file(distributions):
    release.create_manifest(distributions, VERSION, SOURCE)
    (distributions / "other.whl").write_bytes(b"unverified")
    with pytest.raises(ValueError, match="unexpected artifact files"):
        release.verify_manifest(distributions, VERSION, SOURCE)


def test_rejects_missing_source_distribution(distributions):
    (distributions / release.filenames(VERSION)[1]).unlink()
    with pytest.raises(ValueError, match="missing regular file"):
        release.create_manifest(distributions, VERSION, SOURCE)


def test_rejects_distribution_symlink(distributions, tmp_path):
    wheel = distributions / release.filenames(VERSION)[0]
    actual = tmp_path.parent / f"{tmp_path.name}-outside.whl"
    wheel.rename(actual)
    wheel.symlink_to(actual)
    with pytest.raises(ValueError, match="missing regular file"):
        release.create_manifest(distributions, VERSION, SOURCE)


@pytest.fixture
def api(monkeypatch):
    run = {
        "id": 123,
        "run_attempt": 2,
        "head_sha": COMMIT,
        "event": "push",
        "head_branch": "main",
        "status": "completed",
        "conclusion": "success",
        "workflow_id": 7,
        "repository": {"full_name": REPOSITORY},
        "head_repository": {"full_name": REPOSITORY},
    }
    gate = {"name": "CI gate", "status": "completed", "conclusion": "success"}
    artifact = {
        "id": 456,
        "name": f"verified-distributions-{COMMIT}-2",
        "expired": False,
    }
    data = {"run": run, "jobs": [gate], "artifacts": [artifact]}

    def request(path):
        if path.endswith("/workflows/ci.yml"):
            return {"id": 7}
        if "/workflows/ci.yml/runs?" in path:
            return {
                "workflow_runs": [
                    {"id": 123, "head_sha": COMMIT, "conclusion": "success"}
                ]
            }
        if path.endswith("/runs/123"):
            return data["run"]
        if "/attempts/2/jobs?" in path:
            return {"jobs": data["jobs"]}
        if "/runs/123/artifacts?" in path:
            return {"artifacts": data["artifacts"]}
        raise AssertionError(f"unexpected API request: {path}")

    monkeypatch.setattr(release, "github_api", request)
    return data


def test_selects_successful_main_run_and_current_attempt(api):
    assert release.select_source(REPOSITORY, COMMIT) == {
        **SOURCE,
        "artifact_name": f"verified-distributions-{COMMIT}-2",
        "artifact_id": "456",
    }


@pytest.mark.parametrize(
    ("field", "value"),
    [
        ("event", "pull_request"),
        ("event", "workflow_dispatch"),
        ("head_branch", "feature"),
        ("head_sha", "b" * 40),
        ("conclusion", "failure"),
        ("status", "in_progress"),
        ("workflow_id", 8),
        ("repository", {"full_name": "other/repo"}),
        ("head_repository", {"full_name": "fork/repo"}),
    ],
)
def test_rejects_untrusted_or_unsuccessful_run(api, field, value):
    api["run"][field] = value
    with pytest.raises(ValueError, match="no successful main push CI"):
        release.select_source(REPOSITORY, COMMIT)


@pytest.mark.parametrize("result", ["failure", "skipped", "cancelled", None])
def test_rejects_gate_that_did_not_pass(api, result):
    api["jobs"][0]["conclusion"] = result
    with pytest.raises(ValueError, match="no successful main push CI"):
        release.select_source(REPOSITORY, COMMIT)


@pytest.mark.parametrize(
    "kind", ["expired", "old_attempt", "missing", "duplicate", "missing_gate"]
)
def test_rejects_unavailable_or_ambiguous_verified_artifact(api, kind):
    if kind == "expired":
        api["artifacts"][0]["expired"] = True
    elif kind == "old_attempt":
        api["artifacts"][0]["name"] = f"verified-distributions-{COMMIT}-1"
    elif kind == "missing":
        api["artifacts"] = []
    elif kind == "duplicate":
        api["artifacts"] *= 2
    else:
        api["jobs"] = []
    with pytest.raises(ValueError, match="no successful main push CI"):
        release.select_source(REPOSITORY, COMMIT)


def test_reads_later_api_pages(monkeypatch):
    calls = []

    def request(path):
        calls.append(path)
        return {"artifacts": list(range(100)) if path.endswith("page=1") else [100]}

    monkeypatch.setattr(release, "github_api", request)
    assert list(release.pages("repos/test/repo/artifacts", "artifacts")) == list(
        range(101)
    )
    assert len(calls) == 2


def test_cli_copies_only_verified_distributions(distributions, monkeypatch):
    release.create_manifest(distributions, VERSION, SOURCE)
    cargo = distributions.parent / f"{distributions.name}-Cargo.toml"
    cargo.write_text(f'[package]\nversion = "{VERSION}"\n')
    output = distributions.parent / f"{distributions.name}-upload"
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "ci_release.py",
            "verify",
            "--directory",
            str(distributions),
            "--cargo",
            str(cargo),
            "--repository",
            REPOSITORY,
            "--commit",
            COMMIT,
            "--run-id",
            "123",
            "--run-attempt",
            "2",
            "--tag",
            f"v{VERSION}",
            "--output",
            str(output),
        ],
    )
    release.main()
    assert {path.name for path in output.iterdir()} == set(release.filenames(VERSION))
    for path in output.iterdir():
        assert path.read_bytes() == (distributions / path.name).read_bytes()


def test_cli_rejects_wrong_release_tag(distributions, monkeypatch):
    cargo = distributions.parent / f"{distributions.name}-Cargo.toml"
    cargo.write_text(f'[package]\nversion = "{VERSION}"\n')
    monkeypatch.setattr(
        sys,
        "argv",
        [
            "ci_release.py",
            "verify",
            "--cargo",
            str(cargo),
            "--repository",
            REPOSITORY,
            "--commit",
            COMMIT,
            "--tag",
            "v9.9.9",
        ],
    )
    with pytest.raises(ValueError, match="tag does not match Cargo version"):
        release.main()
