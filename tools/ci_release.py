"""Verify and promote CI distributions; requires Python 3.11+ and no packages."""

from __future__ import annotations

import argparse
import email.parser
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tomllib
import zipfile

MANIFEST = "release-manifest.json"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def version_from_cargo(data: bytes) -> str:
    version = tomllib.loads(data.decode())["package"]["version"]
    require(re.fullmatch(r"\d+\.\d+\.\d+", version) is not None, "invalid version")
    return version


def filenames(version: str) -> tuple[str, str]:
    return (
        f"polars_bt-{version}-cp310-abi3-manylinux_2_17_x86_64.manylinux2014_x86_64.whl",
        f"polars_bt-{version}.tar.gz",
    )


def check_distributions(directory: Path, version: str) -> None:
    wheel_name, sdist_name = filenames(version)
    for name in (wheel_name, sdist_name):
        path = directory / name
        require(
            path.is_file() and not path.is_symlink(), f"missing regular file: {name}"
        )
    with zipfile.ZipFile(directory / wheel_name) as wheel:
        metadata_names = [
            n for n in wheel.namelist() if n.endswith(".dist-info/METADATA")
        ]
        require(len(metadata_names) == 1, "wheel must contain one METADATA file")
        metadata = email.parser.Parser().parsestr(
            wheel.read(metadata_names[0]).decode()
        )
        require(
            metadata["Name"].replace("-", "_") == "polars_bt", "wrong wheel project"
        )
        require(metadata["Version"] == version, "wheel version mismatch")
    with tarfile.open(directory / sdist_name) as sdist:
        root = f"polars_bt-{version}"
        for name in (
            "Cargo.toml",
            "Cargo.lock",
            "pyproject.toml",
            "src/lib.rs",
            "polars_bt/__init__.py",
        ):
            member = sdist.getmember(f"{root}/{name}")
            require(member.isfile(), f"sdist missing regular file: {name}")
        cargo = sdist.extractfile(f"{root}/Cargo.toml")
        require(cargo is not None, "sdist missing Cargo.toml")
        require(version_from_cargo(cargo.read()) == version, "sdist version mismatch")


def provenance(repository: str, commit: str, run_id: str, attempt: str) -> dict:
    require(
        re.fullmatch(r"[\w.-]+/[\w.-]+", repository) is not None, "invalid repository"
    )
    require(re.fullmatch(r"[a-f0-9]{40}", commit) is not None, "invalid commit")
    require(run_id.isdecimal() and int(run_id) > 0, "invalid run ID")
    require(attempt.isdecimal() and int(attempt) > 0, "invalid run attempt")
    return {
        "repository": repository,
        "commit": commit,
        "run_id": run_id,
        "run_attempt": attempt,
    }


def create_manifest(directory: Path, version: str, source: dict) -> None:
    check_distributions(directory, version)
    require(
        {p.name for p in directory.iterdir()} == set(filenames(version)),
        "unexpected distribution files",
    )
    manifest = {"schema": 1, "version": version, **source, "files": {}}
    for name in filenames(version):
        data = (directory / name).read_bytes()
        manifest["files"][name] = {
            "sha256": hashlib.sha256(data).hexdigest(),
            "size": len(data),
        }
    (directory / MANIFEST).write_text(json.dumps(manifest, indent=2) + "\n")


def verify_manifest(directory: Path, version: str, source: dict) -> None:
    path = directory / MANIFEST
    require(path.is_file() and not path.is_symlink(), "missing regular manifest")
    manifest = json.loads(path.read_text())
    expected = {"schema": 1, "version": version, **source}
    for key, value in expected.items():
        require(manifest.get(key) == value, f"manifest {key} mismatch")
    names = set(filenames(version))
    require(set(manifest["files"]) == names, "manifest file list mismatch")
    require(
        {p.name for p in directory.iterdir()} == names | {MANIFEST},
        "unexpected artifact files",
    )
    check_distributions(directory, version)
    for name in names:
        data = (directory / name).read_bytes()
        require(
            manifest["files"][name]
            == {"sha256": hashlib.sha256(data).hexdigest(), "size": len(data)},
            f"file hash/size mismatch: {name}",
        )


def github_api(path: str) -> dict:
    return json.loads(subprocess.check_output(["gh", "api", path], text=True))


def pages(path: str, key: str):
    separator = "&" if "?" in path else "?"
    page = 1
    while True:
        items = github_api(f"{path}{separator}per_page=100&page={page}")[key]
        yield from items
        if len(items) < 100:
            return
        page += 1


def select_source(repository: str, commit: str) -> dict:
    # Only a successful push to this repository's main branch can be promoted.
    # PR and workflow_dispatch artifacts cannot become release inputs.
    provenance(repository, commit, "1", "1")
    workflow = github_api(f"repos/{repository}/actions/workflows/ci.yml")
    runs = pages(
        f"repos/{repository}/actions/workflows/ci.yml/runs?branch=main&event=push&head_sha={commit}",
        "workflow_runs",
    )
    for candidate in runs:
        if candidate["head_sha"] != commit or candidate["conclusion"] != "success":
            continue
        run = github_api(f"repos/{repository}/actions/runs/{candidate['id']}")
        if not (
            run["head_sha"] == commit
            and run["event"] == "push"
            and run["head_branch"] == "main"
            and run["status"] == "completed"
            and run["conclusion"] == "success"
            and run["workflow_id"] == workflow["id"]
            and run["repository"]["full_name"] == repository
            and run["head_repository"]["full_name"] == repository
        ):
            continue
        source = provenance(repository, commit, str(run["id"]), str(run["run_attempt"]))
        jobs = list(
            pages(
                f"repos/{repository}/actions/runs/{run['id']}/attempts/{run['run_attempt']}/jobs",
                "jobs",
            )
        )
        gates = [job for job in jobs if job["name"] == "CI gate"]
        if (
            len(gates) != 1
            or gates[0]["conclusion"] != "success"
            or gates[0]["status"] != "completed"
        ):
            continue
        name = f"verified-distributions-{commit}-{run['run_attempt']}"
        artifacts = [
            a
            for a in pages(
                f"repos/{repository}/actions/runs/{run['id']}/artifacts", "artifacts"
            )
            if a["name"] == name and not a["expired"]
        ]
        if len(artifacts) == 1:
            return {
                **source,
                "artifact_name": name,
                "artifact_id": str(artifacts[0]["id"]),
            }
    raise ValueError(
        "no successful main push CI with an unexpired verified artifact for this commit; rerun the complete main CI for this commit and wait for success"
    )


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["create", "verify", "select"])
    parser.add_argument("--directory", type=Path)
    parser.add_argument("--cargo", type=Path, default=Path("Cargo.toml"))
    parser.add_argument("--repository", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id")
    parser.add_argument("--run-attempt")
    parser.add_argument("--tag")
    parser.add_argument(
        "--output",
        type=Path,
        help="Copy verified distributions here without the manifest",
    )
    args = parser.parse_args()
    version = version_from_cargo(args.cargo.read_bytes())
    if args.tag is not None:
        require(args.tag == f"v{version}", "tag does not match Cargo version")
    if args.command == "select":
        selected = select_source(args.repository, args.commit)
        print(json.dumps(selected, indent=2))
        with Path(os.environ["GITHUB_OUTPUT"]).open("a") as out:
            for key, value in selected.items():
                out.write(f"{key}={value}\n")
        return
    require(args.directory is not None, "--directory is required")
    require(
        args.run_id is not None and args.run_attempt is not None,
        "run ID and attempt are required",
    )
    source = provenance(args.repository, args.commit, args.run_id, args.run_attempt)
    if args.command == "create":
        create_manifest(args.directory, version, source)
    verify_manifest(args.directory, version, source)
    if args.output is not None:
        require(not args.output.exists(), "output directory must not already exist")
        args.output.mkdir(parents=True)
        for name in filenames(version):
            shutil.copyfile(args.directory / name, args.output / name)
    print(
        f"Verified {version} distributions for {args.commit}, run {args.run_id}/{args.run_attempt}"
    )


if __name__ == "__main__":
    main()
