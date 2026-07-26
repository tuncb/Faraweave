#!/usr/bin/env python3
"""Offline positive and negative Faraweave provenance contracts."""
from __future__ import annotations
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import tarfile
import zipfile

COMMIT = "0123456789abcdef0123456789abcdef01234567"
REPOSITORY = "owner/faraweave"
WORKFLOW = ".github/workflows/future-release.yml"

def require(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)

def invoke(command: list[str], success: bool = True) -> subprocess.CompletedProcess[str]:
    result = subprocess.run(command, text=True, capture_output=True, check=False)
    require(result.returncode == (0 if success else result.returncode), result.stderr)
    if success:
        require(result.returncode == 0, result.stderr)
    else:
        require(result.returncode != 0, "negative mutation unexpectedly succeeded")
    return result

def main() -> None:
    require(len(sys.argv) == 6,
            "usage: release_provenance_test.py <python> <tool> <source> <faraweave> <target>")
    python, tool, source, executable, target = sys.argv[1:]
    require(
        target in {"windows-x64", "linux-x64", "macos-arm64"},
        "unknown release target",
    )
    source_path = Path(source)
    with tempfile.TemporaryDirectory(prefix="faraweave-provenance-") as temporary:
        root = Path(temporary)
        assets = root / "assets"
        fragments = root / "fragments"
        assets.mkdir()
        fragments.mkdir()
        executable_name = "faraweave.exe" if target == "windows-x64" else "faraweave"
        extension = "zip" if target == "windows-x64" else "tar.gz"
        archive = assets / f"faraweave-v0.1.0-{target}.{extension}"
        if target == "windows-x64":
            with zipfile.ZipFile(archive, "w", zipfile.ZIP_DEFLATED) as output:
                output.write(executable, executable_name)
                output.write(source_path / "LICENSE", "LICENSE")
        else:
            with tarfile.open(archive, "w:gz") as output:
                output.add(executable, executable_name)
                output.add(source_path / "LICENSE", "LICENSE")
        fragment = fragments / "windows.json"
        base = [
            python, tool, "fragment",
            "--version-file", str(source_path / "Cargo.toml"),
            "--target", target,
            "--source-commit", COMMIT,
            "--archive", str(archive),
            "--executable-path", executable_name,
            "--repository", REPOSITORY,
            "--workflow", WORKFLOW,
            "--output", str(fragment),
        ]
        invoke(base)
        document = json.loads(fragment.read_text())
        require(document["version"] == "0.1.0", "Cargo version was not canonical")
        require(document["source_commit"] == COMMIT, "commit mismatch")

        crlf_directory = root / "crlf"
        crlf_directory.mkdir()
        crlf_version = crlf_directory / "Cargo.toml"
        version_bytes = (source_path / "Cargo.toml").read_bytes().replace(b"\r\n", b"\n")
        crlf_version.write_bytes(version_bytes.replace(b"\n", b"\r\n"))
        crlf_fragment = fragments / "crlf.json"
        crlf_command = base.copy()
        crlf_command[crlf_command.index(str(source_path / "Cargo.toml"))] = str(
            crlf_version
        )
        crlf_command[crlf_command.index(str(fragment))] = str(crlf_fragment)
        invoke(crlf_command)
        require(
            json.loads(crlf_fragment.read_text())["version"] == "0.1.0",
            "CRLF Cargo version was not canonical",
        )

        bad_commit = base.copy()
        bad_commit[bad_commit.index(COMMIT)] = "not-a-commit"
        invoke(bad_commit, success=False)

        bad_target = base.copy()
        bad_target[bad_target.index(target)] = "unknown"
        invoke(bad_target, success=False)

        tampered = assets / archive.name
        original = tampered.read_bytes()
        tampered.write_bytes(original + b"tamper")
        invoke(base, success=False)
        tampered.write_bytes(original)

        unsafe = assets / f"unsafe.{extension}"
        if target == "windows-x64":
            with zipfile.ZipFile(unsafe, "w") as output:
                output.writestr(f"../{executable_name}", b"bad")
                output.write(source_path / "LICENSE", "LICENSE")
        else:
            with tarfile.open(unsafe, "w:gz") as output:
                member = tarfile.TarInfo(f"../{executable_name}")
                member.size = 3
                import io

                output.addfile(member, io.BytesIO(b"bad"))
                output.add(source_path / "LICENSE", "LICENSE")
        unsafe_command = base.copy()
        unsafe_command[unsafe_command.index(str(archive))] = str(unsafe)
        invoke(unsafe_command, success=False)

    print("release provenance contracts: PASS")

if __name__ == "__main__":
    main()
