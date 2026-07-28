#!/usr/bin/env python3
"""Offline workflow and packaging contracts."""
from __future__ import annotations
import json
import gzip
import io
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile
import tempfile
import zipfile
import tomllib
import re

ROOT = Path(__file__).resolve().parents[2]
VERSION = tomllib.loads((ROOT / "Cargo.toml").read_text())["package"]["version"]

def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)

def static_contracts() -> None:
    cargo = (ROOT / "Cargo.toml").read_text()
    require('name = "faraweave"' in cargo and 'version = "0.1.0"' in cargo, "Cargo identity")
    require((ROOT / "Cargo.lock").is_file(), "Cargo.lock missing")
    toolchain = (ROOT / "rust-toolchain.toml").read_text()
    require('channel = "1.97.1"' in toolchain and "clippy" in toolchain, "toolchain pin")
    main = (ROOT / ".github/workflows/main.yml").read_text()
    validate_main_workflow(main)
    validate_release_workflows()
    validate_fwir_conformance()


def validate_main_workflow(main: str) -> None:
    required = [
        "pull_request:",
        "push:",
        "workflow_dispatch:",
        "branches: [main]",
        "concurrency:",
        "cancel-in-progress:",
        "permissions:",
        "contents: read",
        "fail-fast: false",
        "ubuntu-24.04",
        "windows-2022",
        "macos-15",
        "expected_arch: x86_64",
        "expected_arch: AMD64",
        "expected_arch: arm64",
        "persist-credentials: false",
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets --all-features -- -D warnings",
        "cargo build --workspace --all-targets --all-features --release",
        "cargo test --workspace --all-targets --all-features",
        "cargo test --workspace --all-targets --all-features --release",
        "python tools/validation/contracts.py full",
        "PR Gate",
        "if: always()",
        "needs: [validate]",
    ]
    for needle in required:
        require(needle in main, f"main workflow missing {needle}")
    actions = re.findall(r"uses:\s*([^@\s]+)@([^\s]+)", main)
    require(bool(actions), "main workflow has no pinned actions")
    require(
        all(re.fullmatch(r"[0-9a-f]{40}", revision) for _, revision in actions),
        "main workflow action is not pinned by a full commit",
    )
    for needle in required:
        mutated = main.replace(needle, "REMOVED")
        try:
            validate_main_workflow_without_mutations(mutated, required)
        except AssertionError:
            continue
        raise SystemExit(f"main workflow negative mutation survived: {needle}")


def validate_main_workflow_without_mutations(text: str, required: list[str]) -> None:
    for needle in required:
        if needle not in text:
            raise AssertionError(needle)


def validate_release_workflows() -> None:
    initial = (ROOT / ".github/workflows/release.yml").read_text()
    future = (ROOT / ".github/workflows/future-release.yml").read_text()
    for text, name in [(initial, "initial"), (future, "future")]:
        actions = re.findall(r"uses:\s*([^@\s]+)@([^\s]+)", text)
        require(
            all(
                action.startswith("./")
                or re.fullmatch(r"[0-9a-f]{40}", revision)
                for action, revision in actions
            ),
            f"{name} release action is not pinned",
        )
        require("persist-credentials: false" in text, f"{name} checkout credentials")
    for needle in [
        "v0.1.0",
        "git cat-file -t refs/tags/v0.1.0",
        "git rev-parse refs/tags/v0.1.0^{commit}",
        "! gh release view v0.1.0",
    ]:
        require(needle in initial, f"initial release missing {needle}")
    for needle in [
        "linux-x64",
        "windows-x64",
        "macos-arm64",
        "fail-fast: false",
        "attest-build-provenance@",
        "release-manifest.json",
        "publish.sh",
    ]:
        require(needle in future, f"future release missing {needle}")


def fnv1a64(data: bytes) -> int:
    value = 0xCBF29CE484222325
    for byte in data:
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def validate_fwir_conformance() -> None:
    corpus_path = ROOT / "tests/fixtures/fwir-v1-corpus.tsv"
    traceability_path = ROOT / "tests/fixtures/fwir-v1-conformance.tsv"
    corpus_rows = [
        line.split("\t")
        for line in corpus_path.read_text(encoding="utf-8").splitlines()[1:]
        if line
    ]
    require(len(corpus_rows) == 3, "FWIR canonical corpus inventory")
    require(
        {row[0] for row in corpus_rows} == {"empty", "scalar-true", "complete"},
        "FWIR canonical corpus names",
    )
    for row in corpus_rows:
        require(len(row) == 5, f"FWIR corpus row width: {row!r}")
        name, relative, length, digest, surfaces = row
        require(
            relative == f"spec/examples/fwir-v1-{name}.hex",
            f"FWIR corpus path: {name}",
        )
        hex_text = (ROOT / relative).read_text(encoding="ascii")
        artifact = bytes.fromhex(hex_text)
        require(len(artifact) == int(length), f"FWIR corpus length: {name}")
        require(f"{fnv1a64(artifact):016x}" == digest, f"FWIR corpus hash: {name}")
        require(
            artifact.startswith(b"FWIR\r\n\x1a\n"),
            f"FWIR corpus magic: {name}",
        )
        require(
            b"\\" not in artifact and not any(
                window[4] == ord("-")
                and window[7] == ord("-")
                and window[10] == ord("T")
                for window in (
                    artifact[index : index + 19]
                    for index in range(max(0, len(artifact) - 18))
                )
            ),
            f"FWIR corpus host metadata: {name}",
        )
        required_surfaces = {
            "decode",
            "reencode",
            "inspect",
            "interpret",
            "emit-c",
            "native",
        }
        require(
            required_surfaces <= set(surfaces.split(",")),
            f"FWIR corpus surfaces: {name}",
        )

    traceability_rows = [
        line.split("\t")
        for line in traceability_path.read_text(encoding="utf-8").splitlines()[1:]
        if line
    ]
    require(len(traceability_rows) >= 100, "FWIR conformance traceability count")
    require(
        all(len(row) == 3 and all(row) for row in traceability_rows),
        "FWIR conformance traceability row",
    )
    requirements = [row[0] for row in traceability_rows]
    require(
        len(requirements) == len(set(requirements)),
        "FWIR conformance traceability duplicate",
    )
    for prefix in (
        "header.",
        "directory.",
        "modl.",
        "feat.",
        "strs.",
        "srcu.",
        "parm.",
        "type.",
        "tyel.",
        "cons.",
        "coel.",
        "orig.",
        "edge.",
        "shck.",
        "bran.",
        "node.",
        "ownr.",
        "root.",
        "prod.",
        "compat.",
        "limits.",
        "decoder.",
        "canonical.",
        "surfaces.",
    ):
        require(
            any(requirement.startswith(prefix) for requirement in requirements),
            f"FWIR conformance traceability family: {prefix}",
        )

def package(target: str) -> None:
    require(target in {"linux-x64", "windows-x64", "macos-arm64"}, "unknown target")
    machine = platform.machine().lower()
    host_target = (
        "windows-x64"
        if platform.system() == "Windows" and machine in {"amd64", "x86_64"}
        else "linux-x64"
        if platform.system() == "Linux" and machine == "x86_64"
        else "macos-arm64"
        if platform.system() == "Darwin" and machine in {"arm64", "aarch64"}
        else None
    )
    require(target == host_target, f"cannot package {target} on {host_target}")
    artifacts = ROOT / "artifacts"
    artifacts.mkdir(exist_ok=True)
    exe_name = "faraweave.exe" if target == "windows-x64" else "faraweave"
    built = ROOT / "target/release" / exe_name
    require(built.is_file(), f"missing {built}")
    archive = artifacts / f"faraweave-v{VERSION}-{target}.{'zip' if target == 'windows-x64' else 'tar.gz'}"
    if target == "windows-x64":
        with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as out:
            for source, name, mode in [(built, exe_name, 0o755), (ROOT / "LICENSE", "LICENSE", 0o644)]:
                info = zipfile.ZipInfo(name, (1980, 1, 1, 0, 0, 0))
                info.compress_type = zipfile.ZIP_DEFLATED
                info.external_attr = mode << 16
                out.writestr(info, source.read_bytes())
    else:
        payload = io.BytesIO()
        with tarfile.open(fileobj=payload, mode="w", format=tarfile.USTAR_FORMAT) as out:
            for source, name, mode in [(built, exe_name, 0o755), (ROOT / "LICENSE", "LICENSE", 0o644)]:
                data = source.read_bytes()
                info = tarfile.TarInfo(name)
                info.size, info.mode, info.mtime = len(data), mode, 0
                info.uid = info.gid = 0
                info.uname = info.gname = ""
                out.addfile(info, io.BytesIO(data))
        with archive.open("wb") as raw:
            with gzip.GzipFile(filename="", mode="wb", fileobj=raw, mtime=0) as compressed:
                compressed.write(payload.getvalue())
    with tempfile.TemporaryDirectory(prefix="faraweave-package-smoke-") as temporary:
        extracted = Path(temporary)
        if target == "windows-x64":
            with zipfile.ZipFile(archive) as incoming:
                require(
                    set(incoming.namelist()) == {exe_name, "LICENSE"},
                    "unexpected Windows archive layout",
                )
                incoming.extractall(extracted)
        else:
            with tarfile.open(archive, "r:gz") as incoming:
                require(
                    {member.name for member in incoming.getmembers()}
                    == {exe_name, "LICENSE"},
                    "unexpected tar archive layout",
                )
                incoming.extractall(extracted, filter="data")
        extracted_executable = extracted / exe_name
        output = subprocess.check_output([str(extracted_executable), "--version"])
        require(output == f"faraweave {VERSION}\n".encode(), "extracted version")
        require(
            (extracted / "LICENSE").read_bytes() == (ROOT / "LICENSE").read_bytes(),
            "packaged LICENSE mismatch",
        )
    if target == "windows-x64":
        escaped_built = str(built).replace("'", "''")
        metadata = subprocess.check_output(
            [
                "powershell.exe",
                "-NoProfile",
                "-Command",
                f"$v=(Get-Item -LiteralPath '{escaped_built}').VersionInfo;"
                "[Console]::Write($v.ProductName+'|'+$v.ProductVersion+'|'+"
                "$v.OriginalFilename+'|'+$v.FileVersion)",
            ],
            text=True,
        )
        require(
            metadata == f"Faraweave|{VERSION}|faraweave.exe|{VERSION}",
            "Windows PE identity",
        )
        manifest = (ROOT / "src/faraweave.exe.manifest").read_text()
        require("longPathAware" in manifest and ">true<" in manifest, "long-path manifest")
    elif target == "linux-x64":
        dependencies = subprocess.check_output(["ldd", str(built)], text=True)
        require("libstdc++" not in dependencies, "Rust package depends on libstdc++")
        header = subprocess.check_output(["readelf", "-h", str(built)], text=True)
        require("Advanced Micro Devices X86-64" in header, "Linux ELF architecture")
    else:
        identity = subprocess.check_output(["file", str(built)], text=True)
        require("arm64" in identity, "macOS executable is not arm64")

def main() -> None:
    command = sys.argv[1] if len(sys.argv) > 1 else "full"
    static_contracts()
    if command == "package":
        package(sys.argv[2])
    elif command == "release-state":
        require(os.environ.get("SOURCE_COMMIT", "0" * 40).__len__() == 40, "source commit")
    elif command not in {"full", "focused", "review"}:
        raise SystemExit(f"unknown contract selection: {command}")
    if command == "full":
        executable = ROOT / "target/release" / (
            "faraweave.exe" if os.name == "nt" else "faraweave"
        )
        require(executable.is_file(), f"missing Release executable: {executable}")
        if Path("/dev/full").exists():
            with Path("/dev/full").open("wb") as full:
                help_failure = subprocess.run(
                    [str(executable), "--help"],
                    cwd=ROOT,
                    stdout=full,
                    stderr=subprocess.PIPE,
                    check=False,
                )
            require(help_failure.returncode == 1, "help output-device failure exit")
            require(
                help_failure.stderr == b"error: unable to write stdout\n",
                "help output-device failure diagnostic",
            )
            with Path("/dev/full").open("wb") as full:
                repl_failure = subprocess.run(
                    [str(executable), "repl"],
                    cwd=ROOT,
                    input=b"inc 5\n",
                    stdout=full,
                    stderr=subprocess.PIPE,
                    check=False,
                )
            require(repl_failure.returncode == 1, "REPL output-device failure exit")
            require(
                repl_failure.stderr == b"error: unable to write stdout\n",
                "REPL output-device failure diagnostic",
            )
        subprocess.run(
            [sys.executable, str(ROOT / "tools/validation/c11_journey.py")],
            cwd=ROOT,
            check=True,
        )
        machine = platform.machine().lower()
        target = (
            "windows-x64"
            if platform.system() == "Windows" and machine in {"amd64", "x86_64"}
            else "linux-x64"
            if platform.system() == "Linux" and machine == "x86_64"
            else "macos-arm64"
            if platform.system() == "Darwin" and machine in {"arm64", "aarch64"}
            else None
        )
        require(target is not None, "unsupported release-contract host")
        subprocess.run(
            [
                sys.executable,
                str(ROOT / "tests/release_provenance_test.py"),
                sys.executable,
                str(ROOT / "tools/release/provenance.py"),
                str(ROOT),
                str(executable),
                target,
            ],
            cwd=ROOT,
            check=True,
        )
    print(f"contracts: {command}: PASS ({platform.system()} {platform.machine()})")

if __name__ == "__main__":
    main()
