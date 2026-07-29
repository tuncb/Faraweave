#!/usr/bin/env python3
"""Exercise the interpreter-only public source and FWIR CLI journeys."""

from __future__ import annotations

import os
from pathlib import Path
import platform
import re
import subprocess
import tempfile


ROOT = Path(__file__).resolve().parents[2]
EXECUTABLE = ROOT / "target" / "release" / (
    "faraweave.exe" if os.name == "nt" else "faraweave"
)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def run(*arguments: str) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(
            [str(EXECUTABLE), *arguments],
            cwd=ROOT,
            capture_output=True,
            check=False,
            timeout=60,
        )
    except (OSError, subprocess.SubprocessError) as error:
        raise SystemExit(f"unable to run Faraweave: {error}") from error


def require_success(result: subprocess.CompletedProcess[bytes], subject: str) -> None:
    require(
        result.returncode == 0 and result.stderr == b"",
        f"{subject} failed: status={result.returncode}, stderr={result.stderr!r}",
    )


def main() -> None:
    require(EXECUTABLE.is_file(), f"missing Release executable: {EXECUTABLE}")
    help_result = run("--help")
    require_success(help_result, "help")
    help_text = help_result.stdout.decode("utf-8", errors="strict")
    removed_command = re.search(
        r"(?m)^\s*(?:emit-c|emit-c-ir|build|build-ir)(?:\s|$)", help_text
    )
    require(removed_command is None, "help advertises a removed execution backend")

    with tempfile.TemporaryDirectory(prefix="faraweave-interpreter-") as temporary:
        work = Path(temporary)
        success_source = work / "success.faraweave"
        success_artifact = work / "success.fwir"
        success_source.write_text(
            "parameters[count Int scale Double enabled Bool]\n"
            "add[count 1]\n"
            "mul[scale 2.0]\n"
            "not[enabled]\n"
            "scanl[@add 0 (1 2 3)]\n",
            encoding="utf-8",
            newline="\n",
        )
        arguments = ("4", "2.5", "true")
        source_result = run("run", str(success_source), "--", *arguments)
        require_success(source_result, "source interpreter journey")
        require(
            source_result.stdout == b"5\n5.0\nfalse\n(0 1 3 6)\n",
            f"source interpreter output mismatch: {source_result.stdout!r}",
        )

        compile_result = run(
            "compile-ir", str(success_source), "-o", str(success_artifact)
        )
        require_success(compile_result, "FWIR compilation")
        require(compile_result.stdout == b"", "FWIR compilation wrote stdout")
        require(success_artifact.is_file(), "FWIR compilation published no artifact")

        artifact_result = run("run-ir", str(success_artifact), "--", *arguments)
        require_success(artifact_result, "decoded FWIR interpreter journey")
        require(
            artifact_result.stdout == source_result.stdout,
            "source and decoded FWIR interpreter outputs differ",
        )

        first_inspection = run("inspect-ir", str(success_artifact))
        second_inspection = run("inspect-ir", str(success_artifact))
        require_success(first_inspection, "first FWIR inspection")
        require_success(second_inspection, "second FWIR inspection")
        require(
            first_inspection.stdout == second_inspection.stdout
            and first_inspection.stdout.startswith(b"FWIR inspection v1\n")
            and b"\ncanonical-hex " in first_inspection.stdout,
            "FWIR inspection is empty or nondeterministic",
        )

        source_count_failure = run("run", str(success_source), "--", "4")
        artifact_count_failure = run("run-ir", str(success_artifact), "--", "4")
        require(
            source_count_failure.returncode == artifact_count_failure.returncode == 1
            and source_count_failure.stdout == artifact_count_failure.stdout == b""
            and source_count_failure.stderr == artifact_count_failure.stderr
            and b"faraweave_argument_error reason=missing" in source_count_failure.stderr,
            "source and decoded FWIR argument failures differ: "
            f"source={source_count_failure!r}, artifact={artifact_count_failure!r}",
        )

        failure_source = work / "failure.faraweave"
        failure_artifact = work / "failure.fwir"
        failure_source.write_text("div[1 0]\n", encoding="utf-8", newline="\n")
        compile_failure = run(
            "compile-ir", str(failure_source), "-o", str(failure_artifact)
        )
        require_success(compile_failure, "failing-program FWIR compilation")
        source_failure = run("run", str(failure_source))
        artifact_failure = run("run-ir", str(failure_artifact))
        require(
            source_failure.returncode == artifact_failure.returncode == 1
            and source_failure.stdout == artifact_failure.stdout == b""
            and source_failure.stderr == artifact_failure.stderr
            and b"DomainError: div failed: division_by_zero" in source_failure.stderr,
            "source and decoded FWIR runtime failures differ: "
            f"source={source_failure!r}, artifact={artifact_failure!r}",
        )

        malformed = work / "malformed.fwir"
        malformed_bytes = bytearray(success_artifact.read_bytes())
        require(bool(malformed_bytes), "compiled FWIR artifact is empty")
        malformed_bytes[0] ^= 0xFF
        malformed.write_bytes(malformed_bytes)
        malformed_result = run("run-ir", str(malformed))
        require(
            malformed_result.returncode == 1
            and malformed_result.stdout == b""
            and b"artifact error" in malformed_result.stderr,
            "malformed FWIR reached interpreter execution",
        )

    print(
        "Interpreter journey: PASS "
        f"({platform.system()} {platform.machine()}, {EXECUTABLE.name})"
    )


if __name__ == "__main__":
    main()
