#!/usr/bin/env python3
"""Compile and execute evaluator, emitted-C, and native-build parity journeys."""
from __future__ import annotations

import os
from pathlib import Path
import platform
import shutil
import struct
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


def canonical_artifact(name: str) -> bytes:
    path = ROOT / "spec/examples" / f"fwir-v1-{name}.hex"
    return bytes.fromhex(path.read_text(encoding="ascii"))


def binary64(bits: int) -> float:
    return struct.unpack(">d", bits.to_bytes(8, "big"))[0]


def binary64_order_key(bits: int) -> int:
    return (~bits & 0xFFFF_FFFF_FFFF_FFFF) if bits >> 63 else bits | (1 << 63)


def validate_backend_native_math_policy(
    compiler: str,
    environment: dict[str, str],
    work: Path,
    suffix: str,
) -> None:
    cases = [
        # operation, input bits, reference bits, maximum ULPs, absolute floor
        (0, 0x4000000000000000, 0x3FF6A09E667F3BCD, 1, 0.0),
        (0, 0x0000000000000001, 0x1E60000000000000, 1, 0.0),
        (0, 0x7FEFFFFFFFFFFFFF, 0x5FEFFFFFFFFFFFFF, 1, 0.0),
        (1, 0x3FF0000000000000, 0x4005BF0A8B145769, 4, 0.0),
        (1, 0xC087400000000000, 0x0000000000000002, 4, 0.0),
        (1, 0x40862E42FEFA39EF, 0x7FEFFFFFFFFFFF2A, 4, 0.0),
        (2, 0x0000000000000001, 0xC0874385446D71C3, 4, 0.0),
        (2, 0x3FF0000000000001, 0x3CAFFFFFFFFFFFFF, 4, 0.0),
        (3, 0x0000000000000001, 0xC07434E6420F4374, 4, 0.0),
        (3, 0x3FF0000000000001, 0x3C9BCB7B1526E50D, 4, 0.0),
        (4, 0x7E37E43C8800759C, 0xBFEA2C16B010E385, 8, 2.0**-48),
        (4, 0x0000000000000001, 0x0000000000000001, 8, 2.0**-48),
        (5, 0x7E37E43C8800759C, 0xBFE2699022ADC4C1, 8, 2.0**-48),
        (5, 0x0000000000000001, 0x3FF0000000000000, 8, 2.0**-48),
        (6, 0x3FF921FB54442D17, 0x4329153D9443ED0B, 16, 2.0**-46),
        (6, 0x3FF921FB54442D19, 0xC33617A15494767A, 16, 2.0**-46),
        (6, 0x7E37E43C8800759C, 0x3FF6BE411F37AC77, 16, 2.0**-46),
        (6, 0x0000000000000001, 0x0000000000000001, 16, 2.0**-46),
        (7, 0x0000000000000001, 0x0000000000000000, 0, 0.0),
        (7, 0x8000000000000001, 0xBFF0000000000000, 0, 0.0),
        (8, 0x0000000000000001, 0x3FF0000000000000, 0, 0.0),
        (8, 0x8000000000000001, 0x8000000000000000, 0, 0.0),
        (9, 0x0000000000000001, 0x0000000000000000, 0, 0.0),
        (9, 0x8000000000000001, 0x8000000000000000, 0, 0.0),
        (7, 0x432FFFFFFFFFFFFF, 0x432FFFFFFFFFFFFE, 0, 0.0),
        (8, 0x432FFFFFFFFFFFFF, 0x4330000000000000, 0, 0.0),
        (9, 0x432FFFFFFFFFFFFF, 0x432FFFFFFFFFFFFE, 0, 0.0),
        # Exact special values and signs; -1 means exact/classification mode.
        (0, 0x8000000000000000, 0x8000000000000000, -1, 0.0),
        (0, 0xBFF0000000000000, 0x7FF8000000000000, -1, 0.0),
        (0, 0x7FF0000000000000, 0x7FF0000000000000, -1, 0.0),
        (1, 0x8000000000000000, 0x3FF0000000000000, -1, 0.0),
        (1, 0xFFF0000000000000, 0x0000000000000000, -1, 0.0),
        (1, 0x7FF0000000000000, 0x7FF0000000000000, -1, 0.0),
        (2, 0x8000000000000000, 0xFFF0000000000000, -1, 0.0),
        (2, 0xBFF0000000000000, 0x7FF8000000000000, -1, 0.0),
        (3, 0x0000000000000000, 0xFFF0000000000000, -1, 0.0),
        (3, 0x3FF0000000000000, 0x0000000000000000, -1, 0.0),
        (4, 0x8000000000000000, 0x8000000000000000, -1, 0.0),
        (4, 0x7FF0000000000000, 0x7FF8000000000000, -1, 0.0),
        (5, 0x8000000000000000, 0x3FF0000000000000, -1, 0.0),
        (5, 0xFFF0000000000000, 0x7FF8000000000000, -1, 0.0),
        (6, 0x8000000000000000, 0x8000000000000000, -1, 0.0),
        (6, 0x7FF0000000000000, 0x7FF8000000000000, -1, 0.0),
        (7, 0x8000000000000000, 0x8000000000000000, -1, 0.0),
        (8, 0xFFF0000000000000, 0xFFF0000000000000, -1, 0.0),
        (9, 0x7FF0000000000000, 0x7FF0000000000000, -1, 0.0),
        (9, 0x7FF8000000000000, 0x7FF8000000000000, -1, 0.0),
    ]
    source = work / "backend-native-math-policy.c"
    executable = work / f"backend-native-math-policy{suffix}"
    initializers = ",\n".join(
        f"    {{{operation}U, UINT64_C(0x{input_bits:016x})}}"
        for operation, input_bits, _, _, _ in cases
    )
    source.write_text(
        """
#include <inttypes.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

typedef struct { unsigned operation; uint64_t input; } Case;

static double from_bits(uint64_t bits) {
    double value;
    (void)memcpy(&value, &bits, sizeof(value));
    return value;
}

static uint64_t to_bits(double value) {
    uint64_t bits;
    (void)memcpy(&bits, &value, sizeof(bits));
    return bits;
}

static double invoke(unsigned operation, double value) {
    switch (operation) {
        case 0U: return sqrt(value);
        case 1U: return exp(value);
        case 2U: return log(value);
        case 3U: return log10(value);
        case 4U: return sin(value);
        case 5U: return cos(value);
        case 6U: return tan(value);
        case 7U: return floor(value);
        case 8U: return ceil(value);
        case 9U: return trunc(value);
        default: return value;
    }
}

int main(void) {
    static const Case cases[] = {
"""
        + initializers
        + """
    };
    size_t index;
    for (index = 0U; index < sizeof(cases) / sizeof(cases[0]); ++index) {
        const uint64_t result = to_bits(invoke(cases[index].operation,
                                               from_bits(cases[index].input)));
        if (printf("%016" PRIx64 "\\n", result) < 0) return 1;
    }
    return 0;
}
""",
        encoding="ascii",
    )
    compile_c(compiler, environment, source, executable)
    result = run([str(executable)], environment=environment)
    outputs = normalize_newlines(result.stdout).decode("ascii").splitlines()
    require(len(outputs) == len(cases), "backend-native math result count")
    for output, case in zip(outputs, cases):
        operation, input_bits, reference_bits, max_ulps, max_absolute = case
        actual_bits = int(output, 16)
        if (
            actual_bits & 0x7FF0_0000_0000_0000
            == 0x7FF0_0000_0000_0000
            and actual_bits & 0x000F_FFFF_FFFF_FFFF
        ):
            actual_bits = 0x7FF8_0000_0000_0000
        if max_ulps < 0:
            require(
                actual_bits == reference_bits,
                "backend-native math exact special "
                f"operation={operation} input={input_bits:016x} "
                f"actual={actual_bits:016x} reference={reference_bits:016x}",
            )
            continue
        actual = binary64(actual_bits)
        reference = binary64(reference_bits)
        same_sign = (actual_bits >> 63) == (reference_bits >> 63)
        ulps = abs(
            binary64_order_key(actual_bits) - binary64_order_key(reference_bits)
        )
        require(
            same_sign
            and actual == actual
            and abs(actual) != float("inf")
            and (
                ulps <= max_ulps
                or abs(actual - reference) <= max_absolute
            ),
            "backend-native math envelope "
            f"operation={operation} input={input_bits:016x} "
            f"actual={actual_bits:016x} reference={reference_bits:016x}",
        )


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
        validate_backend_native_math_policy(compiler, environment, work, suffix)
        fixtures = [
            (
                ROOT / "tests/fixtures/public-path-success.bennu",
                [],
                (ROOT / "tests/fixtures/public-path-success.out").read_bytes(),
            ),
            (
                ROOT / "tests/fixtures/parameterized-artifact-success.bennu",
                ["3", "2.5", "true"],
                b"3\n2.5\ntrue\n(1 2 3)\n3\n(1 2 3)\n6\ntrue\ntrue\ntrue\n5.5\nfalse\n",
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
            if index == 0:
                hostile_source = work / "fixture-hostile-fp.c"
                hostile_native = work / f"fixture-hostile-fp{suffix}"
                generated_source = emitted.read_text(encoding="utf-8")
                require(
                    "int main(int argc, char **argv)" in generated_source,
                    "generated main signature for hostile FP journey",
                )
                hostile_source.write_text(
                    generated_source.replace(
                        "int main(int argc, char **argv)",
                        "int fw_generated_main(int argc, char **argv)",
                        1,
                    )
                    + r"""
int main(int argc, char **argv) {
#if defined(__x86_64__) || defined(_M_X64)
  unsigned int original=_mm_getcsr();
  unsigned int hostile=(original|0x0040U|0x4000U|0x8000U|0x1f80U)&~0x003fU;
  unsigned int restored;int result;
  _mm_setcsr(hostile);result=fw_generated_main(argc,argv);restored=_mm_getcsr();
  _mm_setcsr(original);
  if(result!=0)return result;
  return restored==hostile?0:2;
#elif defined(__aarch64__)
  uint64_t original=UINT64_C(0),hostile,restored=UINT64_C(0);int result;
  __asm__ volatile("mrs %0, fpcr":"=r"(original));
  hostile=original|UINT64_C(0x01c00000);
  __asm__ volatile("msr fpcr, %0\n\tisb"::"r"(hostile):"memory");
  result=fw_generated_main(argc,argv);
  __asm__ volatile("mrs %0, fpcr":"=r"(restored));
  __asm__ volatile("msr fpcr, %0\n\tisb"::"r"(original):"memory");
  if(result!=0)return result;
  return restored==hostile?0:2;
#else
#error "Faraweave requires an x86-64 or AArch64 floating-point environment"
#endif
}
""",
                    encoding="ascii",
                )
                compile_c(compiler, environment, hostile_source, hostile_native)
                hostile_result = run(
                    [str(hostile_native)],
                    environment=environment,
                )
                require(
                    normalize_newlines(hostile_result.stdout)
                    == normalize_newlines(expected)
                    and not hostile_result.stderr,
                    "hostile FP generated-C journey: "
                    f"stdout={hostile_result.stdout!r} stderr={hostile_result.stderr!r}",
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

        for name, source_text, expected_reason in [
            (
                "div-zero",
                "div[(8 9 10) (2 0 5)]\n",
                b"DomainError: div failed: division_by_zero at result index 1\n",
            ),
            (
                "div-overflow",
                "div[-9223372036854775808 -1]\n",
                b"DomainError: div failed: integer_overflow\n",
            ),
            (
                "sum-overflow",
                "sum[(9223372036854775807 1 -1)]\n",
                b"DomainError: sum failed: integer_overflow at result index 1\n",
            ),
        ]:
            fixture = work / f"{name}.bennu"
            artifact = work / f"{name}.fwir"
            emitted = work / f"{name}.c"
            emitted_ir = work / f"{name}-ir.c"
            native = work / f"{name}{suffix}"
            fixture.write_text(source_text, encoding="ascii")
            run(
                [str(executable), "compile-ir", str(fixture), "-o", str(artifact)],
                environment=environment,
            )
            run(
                [str(executable), "emit-c", str(fixture), "-o", str(emitted)],
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
                f"{name} source/artifact C mismatch",
            )
            compile_c(compiler, environment, emitted, native)
            evaluator = run(
                [str(executable), "run", str(fixture)],
                environment=environment,
                expected=None,
            )
            artifact_runner = run(
                [str(executable), "run-ir", str(artifact)],
                environment=environment,
                expected=None,
            )
            generated = run([str(native)], environment=environment, expected=None)
            require(
                evaluator.returncode
                == artifact_runner.returncode
                == generated.returncode
                == 1,
                f"{name} failure exit",
            )
            require(
                not evaluator.stdout
                and not artifact_runner.stdout
                and not generated.stdout,
                f"{name} failure stdout",
            )
            require(
                evaluator.stderr == artifact_runner.stderr,
                f"{name} source/artifact diagnostic mismatch: "
                f"evaluator={evaluator.stderr!r} artifact={artifact_runner.stderr!r}",
            )
            require(
                evaluator.stderr.endswith(b":1:1: " + expected_reason)
                and generated.stderr == b"<generated>:1:1: " + expected_reason,
                f"{name} exact diagnostic reason",
            )

        canonical_fixtures = [
            ("empty", [], b""),
            ("scalar-true", [], b"true\n"),
            (
                "complete",
                ["3"],
                b"[true 1 2.0 (1 2) 3]\n"
                b"(3 4)\n"
                b"3.0\n"
                b"(2 3)\n"
                b"(1 2 3)\n"
                b"[(2 3 4) (11 12 13)]\n",
            ),
        ]
        for name, arguments, expected in canonical_fixtures:
            canonical = canonical_artifact(name)
            artifact = work / f"canonical-{name}.fwir"
            emitted = work / f"canonical-{name}.c"
            generated = work / f"canonical-{name}-generated{suffix}"
            built = work / f"canonical-{name}-built{suffix}"
            artifact.write_bytes(canonical)

            first_inspection = run(
                [str(executable), "inspect-ir", str(artifact)],
                environment=environment,
            )
            second_inspection = run(
                [str(executable), "inspect-ir", str(artifact)],
                environment=environment,
            )
            require(
                first_inspection.stdout == second_inspection.stdout
                and not first_inspection.stderr
                and first_inspection.stdout.endswith(
                    b"canonical-hex " + canonical.hex().encode("ascii") + b"\n"
                ),
                f"canonical inspection mismatch for {name}",
            )
            interpreted = run(
                [str(executable), "run-ir", str(artifact), "--", *arguments],
                environment=environment,
            )
            require(
                normalize_newlines(interpreted.stdout) == expected
                and not interpreted.stderr,
                f"canonical interpreter mismatch for {name}",
            )
            run(
                [
                    str(executable),
                    "emit-c-ir",
                    str(artifact),
                    "-o",
                    str(emitted),
                ],
                environment=environment,
            )
            compile_c(compiler, environment, emitted, generated)
            generated_result = run(
                [str(generated), *arguments],
                environment=environment,
            )
            require(
                normalize_newlines(generated_result.stdout) == expected
                and not generated_result.stderr,
                f"canonical generated-C mismatch for {name}",
            )
            run(
                [
                    str(executable),
                    "build-ir",
                    str(artifact),
                    "-o",
                    str(built),
                    "--cc",
                    compiler,
                ],
                environment=environment,
            )
            built_result = run(
                [str(built), *arguments],
                environment=environment,
            )
            require(
                normalize_newlines(built_result.stdout) == expected
                and not built_result.stderr,
                f"canonical native mismatch for {name}",
            )
            require(
                artifact.read_bytes() == canonical,
                f"canonical artifact changed while consumed: {name}",
            )
            if name == "scalar-true" and platform.system() == "Linux":
                sanitized = work / "canonical-scalar-true-sanitized"
                compile_c_sanitized(compiler, environment, emitted, sanitized)
                sanitized_result = run([str(sanitized)], environment=environment)
                require(
                    sanitized_result.stdout == expected
                    and not sanitized_result.stderr,
                    "canonical ASan/UBSan generated-C journey",
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
            == b"3\n2.5\ntrue\n(1 2 3)\n3\n(1 2 3)\n6\ntrue\ntrue\ntrue\n5.5\nfalse\n",
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
