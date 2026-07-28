#!/usr/bin/env python3
"""Compile and execute evaluator, emitted-C, and native-build parity journeys."""
from __future__ import annotations

import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]


def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)


def run(
    arguments: list[str],
    *,
    environment: dict[str, str] | None = None,
    expected: int | None = 0,
) -> subprocess.CompletedProcess[bytes]:
    result = subprocess.run(
        arguments,
        cwd=ROOT,
        env=environment,
        capture_output=True,
        check=False,
    )
    if expected is not None:
        require(
            result.returncode == expected,
            f"{arguments!r} exited {result.returncode}\n"
            f"stdout={result.stdout!r}\nstderr={result.stderr!r}",
        )
    return result


def windows_compiler_environment() -> tuple[str, dict[str, str]]:
    candidates = []
    vswhere = Path(os.environ.get("ProgramFiles(x86)", r"C:\Program Files (x86)")) / (
        "Microsoft Visual Studio/Installer/vswhere.exe"
    )
    if vswhere.is_file():
        query = subprocess.run(
            [
                str(vswhere),
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        if query.returncode == 0 and query.stdout.strip():
            candidates.append(
                Path(query.stdout.strip()) / "Common7/Tools/VsDevCmd.bat"
            )
    candidates.extend(
        Path(rf"C:\Program Files\Microsoft Visual Studio\{year}\{edition}\Common7\Tools\VsDevCmd.bat")
        for year in ("2022", "18")
        for edition in ("Enterprise", "Community", "Professional", "BuildTools")
    )
    script = next((candidate for candidate in candidates if candidate.is_file()), None)
    require(script is not None, "unable to locate an x64 Visual Studio C compiler")
    with tempfile.NamedTemporaryFile(
        mode="w", suffix=".cmd", encoding="utf-8", delete=False
    ) as command_file:
        command_file.write(
            f'@call "{script}" -no_logo -arch=amd64 >nul\r\n'
            "@if errorlevel 1 exit /b %errorlevel%\r\n"
            "@set\r\n"
        )
        command_path = command_file.name
    try:
        configured = subprocess.run(
            ["cmd.exe", "/d", "/c", command_path],
            capture_output=True,
            text=True,
            check=False,
        )
    finally:
        Path(command_path).unlink(missing_ok=True)
    require(configured.returncode == 0, configured.stderr)
    environment = os.environ.copy()
    for line in configured.stdout.splitlines():
        if "=" in line:
            name, value = line.split("=", 1)
            environment[name] = value
    search_path = environment.get("Path") or environment.get("PATH")
    compiler = shutil.which("cl.exe", path=search_path)
    require(bool(compiler), "Visual Studio environment did not expose cl.exe")
    return str(compiler), environment


def compiler_environment() -> tuple[str, dict[str, str]]:
    if os.name == "nt":
        return windows_compiler_environment()
    compiler = os.environ.get("CC") or shutil.which("cc")
    require(bool(compiler), "unable to locate a C11 compiler")
    return str(compiler), os.environ.copy()


def compile_c(
    compiler: str,
    environment: dict[str, str],
    source: Path,
    output: Path,
) -> None:
    if os.name == "nt":
        arguments = [
            compiler,
            "/nologo",
            "/std:c11",
            "/W4",
            "/WX",
            "/fp:strict",
            str(source),
            f"/Fe:{output}",
            f"/Fo:{output}.obj",
        ]
    else:
        arguments = [
            compiler,
            "-std=c11",
            "-frounding-math",
            "-ffp-contract=off",
            "-fno-fast-math",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pedantic-errors",
            str(source),
            "-o",
            str(output),
            "-lm",
        ]
    run(arguments, environment=environment)


def compile_c_sanitized(
    compiler: str,
    environment: dict[str, str],
    source: Path,
    output: Path,
) -> None:
    require(os.name != "nt", "sanitizer compiler is Unix-only")
    run(
        [
            compiler,
            "-std=c11",
            "-frounding-math",
            "-ffp-contract=off",
            "-fno-fast-math",
            "-Wall",
            "-Wextra",
            "-Werror",
            "-pedantic-errors",
            "-fsanitize=address,undefined",
            "-fno-omit-frame-pointer",
            str(source),
            "-o",
            str(output),
            "-lm",
        ],
        environment=environment,
    )


def normalize_newlines(value: bytes) -> bytes:
    return value.replace(b"\r\n", b"\n")


def main() -> None:
    executable = Path(
        os.environ.get(
            "FARAWEAVE_EXE",
            ROOT
            / "target"
            / "release"
            / ("faraweave.exe" if os.name == "nt" else "faraweave"),
        )
    )
    require(executable.is_file(), f"missing Faraweave executable: {executable}")
    compiler, environment = compiler_environment()
    suffix = ".exe" if os.name == "nt" else ""

    with tempfile.TemporaryDirectory(prefix="faraweave-c11-") as temporary:
        work = Path(temporary)
        fixtures = [
            (
                ROOT / "tests/fixtures/public-path-success.bennu",
                [],
                (ROOT / "tests/fixtures/public-path-success.out").read_bytes(),
            ),
            (
                ROOT / "tests/fixtures/parameterized-artifact-success.bennu",
                ["3", "2.5", "true"],
                b"3\n2.5\ntrue\n(1 2 3)\n5.5\nfalse\n",
            ),
            (
                ROOT / "tests/fixtures/parameterized-artifact-double.bennu",
                ["0.1"],
                b"0.1\n",
            ),
        ]
        for index, (fixture, arguments, expected) in enumerate(fixtures):
            artifact = work / f"fixture-{index}.fwir"
            emitted = work / f"fixture-{index}.c"
            emitted_ir = work / f"fixture-{index}-ir.c"
            native = work / f"fixture-{index}{suffix}"
            run(
                [
                    str(executable),
                    "compile-ir",
                    str(fixture),
                    "-o",
                    str(artifact),
                ],
                environment=environment,
            )
            run(
                [
                    str(executable),
                    "emit-c",
                    str(fixture),
                    "-o",
                    str(emitted),
                ],
                environment=environment,
            )
            run(
                [
                    str(executable),
                    "emit-c-ir",
                    str(artifact),
                    "-o",
                    str(emitted_ir),
                ],
                environment=environment,
            )
            require(
                emitted.read_bytes() == emitted_ir.read_bytes(),
                f"source/artifact C mismatch for {fixture.name}",
            )
            source = emitted.read_text(encoding="utf-8")
            require("strcmp(name" not in source, "runtime primitive-name lookup returned")
            compile_c(compiler, environment, emitted, native)
            generated = run(
                [str(native), *arguments],
                environment=environment,
            )
            evaluator = run(
                [str(executable), "run", str(fixture), "--", *arguments],
                environment=environment,
            )
            artifact_runner = run(
                [str(executable), "run-ir", str(artifact), "--", *arguments],
                environment=environment,
            )
            require(
                normalize_newlines(generated.stdout) == normalize_newlines(expected),
                f"generated output mismatch for {fixture.name}",
            )
            require(
                normalize_newlines(evaluator.stdout)
                == normalize_newlines(generated.stdout),
                f"evaluator/generated mismatch for {fixture.name}",
            )
            require(
                normalize_newlines(artifact_runner.stdout)
                == normalize_newlines(evaluator.stdout),
                f"source/artifact evaluator mismatch for {fixture.name}",
            )
            require(
                not evaluator.stderr
                and not artifact_runner.stderr
                and not generated.stderr,
                fixture.name,
            )
            if index == 0 and platform.system() == "Linux":
                sanitized = work / "fixture-sanitized"
                compile_c_sanitized(compiler, environment, emitted, sanitized)
                sanitized_result = run([str(sanitized)], environment=environment)
                require(
                    sanitized_result.stdout == expected and not sanitized_result.stderr,
                    "ASan/UBSan generated-C journey",
                )
            if index == 0 and Path("/dev/full").exists():
                expected_failure = (
                    b"faraweave_output_error reason=write_failed "
                    + f"pending_byte_count={len(expected)} ".encode()
                    + b"accepted_byte_count=0 output_position=0\n"
                )
                with Path("/dev/full").open("wb") as full:
                    evaluator_failure = subprocess.run(
                        [str(executable), "run", str(fixture)],
                        cwd=ROOT,
                        env=environment,
                        stdout=full,
                        stderr=subprocess.PIPE,
                        check=False,
                    )
                with Path("/dev/full").open("wb") as full:
                    generated_failure = subprocess.run(
                        [str(native)],
                        cwd=ROOT,
                        env=environment,
                        stdout=full,
                        stderr=subprocess.PIPE,
                        check=False,
                    )
                require(
                    evaluator_failure.returncode == generated_failure.returncode == 1,
                    "output-device failure exit",
                )
                require(
                    evaluator_failure.stderr
                    == generated_failure.stderr
                    == expected_failure,
                    "output-device failure diagnostic",
                )

        argument_fixture = ROOT / "tests/fixtures/parameterized-artifact-success.bennu"
        emitted = work / "arguments.c"
        generated_executable = work / f"arguments{suffix}"
        run(
            [str(executable), "emit-c", str(argument_fixture), "-o", str(emitted)],
            environment=environment,
        )
        compile_c(compiler, environment, emitted, generated_executable)
        for arguments in ([], ["3"], ["3", "2.5", "truth"], ["3", "2.5", "true", "x"]):
            evaluator = run(
                [str(executable), "run", str(argument_fixture), "--", *arguments],
                environment=environment,
                expected=None,
            )
            generated = run(
                [str(generated_executable), *arguments],
                environment=environment,
                expected=None,
            )
            require(evaluator.returncode == generated.returncode == 1, "argument exit")
            require(not evaluator.stdout and not generated.stdout, "argument stdout")
            require(evaluator.stderr == generated.stderr, "argument diagnostic mismatch")

        deep_source = work / "deep.faraweave"
        depth = 512
        deep_bytes = ("[" * depth + "1" + "]" * depth + "\n").encode()
        deep_source.write_bytes(deep_bytes)
        deep_c = work / "deep.c"
        deep_native = work / f"deep{suffix}"
        run(
            [str(executable), "emit-c", str(deep_source), "-o", str(deep_c)],
            environment=environment,
        )
        compile_c(compiler, environment, deep_c, deep_native)
        require(
            normalize_newlines(run([str(deep_native)], environment=environment).stdout)
            == deep_bytes,
            "deep generated output",
        )

        built = work / f"native-build{suffix}"
        run(
            [
                str(executable),
                "build",
                str(argument_fixture),
                "-o",
                str(built),
                "--cc",
                compiler,
            ],
            environment=environment,
        )
        native_result = run(
            [str(built), "3", "2.5", "true"],
            environment=environment,
        )
        require(
            normalize_newlines(native_result.stdout)
            == b"3\n2.5\ntrue\n(1 2 3)\n5.5\nfalse\n",
            "native build output",
        )
        artifact = work / "native-build.fwir"
        built_ir = work / f"native-build-ir{suffix}"
        run(
            [
                str(executable),
                "compile-ir",
                str(argument_fixture),
                "-o",
                str(artifact),
            ],
            environment=environment,
        )
        run(
            [
                str(executable),
                "build-ir",
                str(artifact),
                "-o",
                str(built_ir),
                "--cc",
                compiler,
            ],
            environment=environment,
        )
        native_ir_result = run(
            [str(built_ir), "3", "2.5", "true"],
            environment=environment,
        )
        require(
            normalize_newlines(native_ir_result.stdout)
            == normalize_newlines(native_result.stdout),
            "source/artifact native build output",
        )

    print(
        f"C11/native journeys: PASS ({platform.system()} {platform.machine()}, {compiler})"
    )


if __name__ == "__main__":
    main()
