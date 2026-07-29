#!/usr/bin/env python3
"""Compile and execute evaluator, emitted-C, and native-build parity journeys."""
from __future__ import annotations

import os
from pathlib import Path
import platform
import re
import shutil
import struct
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[2]
SQRT_OUTPUT = (
    b"2.0\n3.0\n(0.0 -0.0 nan nan inf nan)\n"
    b"(1.0 2.0 3.0 4.0)\n2.2227587494850775e-162\n"
)
# Numeric leaves in SQRT_OUTPUT, in root/element order. Signed zeros are exact
# special values; every other finite sqrt leaf uses FWIR-MATH-003's one-ULP
# checked-reference envelope.
SQRT_OUTPUT_LEAVES = (
    (0x4000_0000_0000_0000, 1),
    (0x4008_0000_0000_0000, 1),
    (0x0000_0000_0000_0000, 0),
    (0x8000_0000_0000_0000, 0),
    (0x3FF0_0000_0000_0000, 1),
    (0x4000_0000_0000_0000, 1),
    (0x4008_0000_0000_0000, 1),
    (0x4010_0000_0000_0000, 1),
    (0x1E60_0000_0000_0000, 1),
)
EXP_OUTPUT = (
    b"1.0\n1.0\n(0.0 inf nan)\n"
    b"(2.718281828459045 0.36787944117144233 7.38905609893065)\n"
    b"1e-323\n0.0\n1.7976931348622732e308\ninf\n"
)
# Exact exp special leaves use a zero envelope. Every other finite leaf uses
# FWIR-MATH-003's four-ULP checked-reference envelope.
EXP_OUTPUT_LEAVES = (
    (0x3FF0_0000_0000_0000, 0),
    (0x3FF0_0000_0000_0000, 0),
    (0x0000_0000_0000_0000, 0),
    (0x4005_BF0A_8B14_5769, 4),
    (0x3FD7_8B56_362C_EF38, 4),
    (0x401D_8E64_B8D4_DDAE, 4),
    (0x0000_0000_0000_0002, 4),
    (0x0000_0000_0000_0000, 0),
    (0x7FEF_FFFF_FFFF_FF2A, 4),
)
LOG_OUTPUT = (
    b"-inf\n-inf\n(nan nan inf nan)\n0.0\n"
    b"(0.6931471805599453 2.302585092994046 -744.4400719213812)\n"
    b"2.2204460492503128e-16\n-1.1102230246251565e-16\n"
    b"709.782712893384\n"
)
# log(1) is exact positive zero. Every other numeric leaf uses
# FWIR-MATH-003's four-ULP checked-reference envelope.
LOG_OUTPUT_LEAVES = (
    (0x0000_0000_0000_0000, 0),
    (0x3FE6_2E42_FEFA_39EF, 4),
    (0x4002_6BB1_BBB5_5516, 4),
    (0xC087_4385_446D_71C3, 4),
    (0x3CAF_FFFF_FFFF_FFFF, 4),
    (0xBCA0_0000_0000_0000, 4),
    (0x4086_2E42_FEFA_39EF, 4),
)
LOG10_OUTPUT = (
    b"-inf\n-inf\n(nan nan inf nan)\n0.0\n"
    b"(-1.0 0.3010299956639812 1.0 3.0 -323.3062153431158)\n"
    b"0.9999999999999999\n1.0000000000000002\n308.25471555991675\n"
)
# log10(1) is exact positive zero. Every other numeric leaf uses
# FWIR-MATH-003's four-ULP checked-reference envelope.
LOG10_OUTPUT_LEAVES = (
    (0x0000_0000_0000_0000, 0),
    (0xBFF0_0000_0000_0000, 4),
    (0x3FD3_4413_509F_79FF, 4),
    (0x3FF0_0000_0000_0000, 4),
    (0x4008_0000_0000_0000, 4),
    (0xC074_34E6_420F_4374, 4),
    (0x3FEF_FFFF_FFFF_FFFF, 4),
    (0x3FF0_0000_0000_0001, 4),
    (0x4073_4413_509F_79FF, 4),
)
NUMERIC_LEAF = re.compile(
    rb"(?<![A-Za-z0-9_.])[-+]?(?:[0-9]+\.[0-9]+|[0-9]+)"
    rb"(?:e[-+]?[0-9]+)?(?![A-Za-z0-9_.])"
)


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


def binary64_bits(value: float) -> int:
    return int.from_bytes(struct.pack(">d", value), "big")


def binary64_order_key(bits: int) -> int:
    return (~bits & 0xFFFF_FFFF_FFFF_FFFF) if bits >> 63 else bits | (1 << 63)


def canonical_binary64_text(bits: int) -> bytes:
    value = binary64(bits)
    magnitude = abs(value)
    if bits == 0:
        return b"0.0"
    if bits == 0x8000_0000_0000_0000:
        return b"-0.0"
    if magnitude >= 1.0e6 or magnitude < 1.0e-4:
        for precision in range(17):
            candidate = f"{value:.{precision}e}"
            if binary64_bits(float(candidate)) == bits:
                mantissa, exponent = candidate.split("e")
                return f"{mantissa}e{int(exponent)}".encode("ascii")
    else:
        for precision in range(21):
            candidate = f"{value:.{precision}f}"
            if binary64_bits(float(candidate)) == bits:
                if "." not in candidate:
                    candidate += ".0"
                return candidate.encode("ascii")
    raise ValueError(f"cannot canonically format binary64 {bits:016x}")


def numeric_leaf_view(output: bytes) -> tuple[bytes, list[bytes]]:
    skeleton = bytearray()
    leaves = []
    cursor = 0
    for match in NUMERIC_LEAF.finditer(output):
        skeleton.extend(output[cursor : match.start()])
        skeleton.extend(b"<backend-math-leaf>")
        leaves.append(match.group())
        cursor = match.end()
    skeleton.extend(output[cursor:])
    return bytes(skeleton), leaves


def backend_math_output_mismatch(
    operation: str,
    actual: bytes,
    reference_output: bytes,
    leaf_specs: tuple[tuple[int, int], ...],
) -> str | None:
    reference_skeleton, reference_leaves = numeric_leaf_view(reference_output)
    actual_skeleton, actual_leaves = numeric_leaf_view(actual)
    if actual_skeleton != reference_skeleton:
        return (
            f"{operation} structural/special output changed: "
            f"actual={actual!r} reference={reference_output!r}"
        )
    if len(reference_leaves) != len(leaf_specs):
        return f"{operation} checked-reference leaf map is stale"
    if len(actual_leaves) != len(reference_leaves):
        return f"{operation} numeric leaf count changed"

    for index, (actual_text, reference_text, leaf) in enumerate(
        zip(actual_leaves, reference_leaves, leaf_specs)
    ):
        reference_bits, max_ulps = leaf
        try:
            actual_bits = binary64_bits(float(actual_text.decode("ascii")))
            formatted_reference_bits = binary64_bits(
                float(reference_text.decode("ascii"))
            )
        except (UnicodeDecodeError, ValueError, OverflowError):
            return (
                f"{operation} numeric leaf {index} "
                "is not canonical ASCII binary64"
            )
        if formatted_reference_bits != reference_bits:
            return (
                f"{operation} checked-reference leaf {index} "
                "no longer matches its bits"
            )

        actual_value = binary64(actual_bits)
        if actual_value != actual_value or abs(actual_value) == float("inf"):
            return f"{operation} numeric leaf {index} is not finite"
        if (actual_bits >> 63) != (reference_bits >> 63):
            return f"{operation} numeric leaf {index} changed sign"
        ulps = abs(
            binary64_order_key(actual_bits) - binary64_order_key(reference_bits)
        )
        if ulps > max_ulps:
            return (
                f"{operation} numeric leaf {index} exceeds {max_ulps} ULP: "
                f"actual={actual_bits:016x} reference={reference_bits:016x}"
            )
        if actual_text != canonical_binary64_text(actual_bits):
            return (
                f"{operation} numeric leaf {index} "
                "changed canonical formatting"
            )
    return None


def sqrt_output_mismatch(
    actual: bytes,
    reference_output: bytes = SQRT_OUTPUT,
    leaf_specs: tuple[tuple[int, int], ...] = SQRT_OUTPUT_LEAVES,
) -> str | None:
    return backend_math_output_mismatch(
        "sqrt",
        actual,
        reference_output,
        leaf_specs,
    )


def exp_output_mismatch(
    actual: bytes,
    reference_output: bytes = EXP_OUTPUT,
    leaf_specs: tuple[tuple[int, int], ...] = EXP_OUTPUT_LEAVES,
) -> str | None:
    return backend_math_output_mismatch(
        "exp",
        actual,
        reference_output,
        leaf_specs,
    )


def log_output_mismatch(
    actual: bytes,
    reference_output: bytes = LOG_OUTPUT,
    leaf_specs: tuple[tuple[int, int], ...] = LOG_OUTPUT_LEAVES,
) -> str | None:
    return backend_math_output_mismatch(
        "log",
        actual,
        reference_output,
        leaf_specs,
    )


def log10_output_mismatch(
    actual: bytes,
    reference_output: bytes = LOG10_OUTPUT,
    leaf_specs: tuple[tuple[int, int], ...] = LOG10_OUTPUT_LEAVES,
) -> str | None:
    return backend_math_output_mismatch(
        "log10",
        actual,
        reference_output,
        leaf_specs,
    )


def require_sqrt_output(actual: bytes, label: str) -> None:
    mismatch = sqrt_output_mismatch(actual)
    require(mismatch is None, f"{label}: {mismatch}")


def require_exp_output(actual: bytes, label: str) -> None:
    mismatch = exp_output_mismatch(actual)
    require(mismatch is None, f"{label}: {mismatch}")


def require_log_output(actual: bytes, label: str) -> None:
    mismatch = log_output_mismatch(actual)
    require(mismatch is None, f"{label}: {mismatch}")


def require_log10_output(actual: bytes, label: str) -> None:
    mismatch = log10_output_mismatch(actual)
    require(mismatch is None, f"{label}: {mismatch}")


def validate_sqrt_output_comparator() -> None:
    reference = b"1.0\n2.0\n(0.0 -0.0 nan inf)\n"
    leaves = (
        (0x3FF0_0000_0000_0000, 1),
        (0x4000_0000_0000_0000, 1),
        (0x0000_0000_0000_0000, 0),
        (0x8000_0000_0000_0000, 0),
    )
    require(
        sqrt_output_mismatch(reference, reference, leaves) is None,
        "sqrt comparator exact",
    )
    require(
        sqrt_output_mismatch(
            b"1.0000000000000002\n2.0\n(0.0 -0.0 nan inf)\n",
            reference,
            leaves,
        )
        is None,
        "sqrt comparator +1 ULP",
    )
    require(
        sqrt_output_mismatch(
            b"0.9999999999999999\n2.0\n(0.0 -0.0 nan inf)\n",
            reference,
            leaves,
        )
        is None,
        "sqrt comparator -1 ULP",
    )
    for invalid, label in [
        (b"1.0000000000000004\n2.0\n(0.0 -0.0 nan inf)\n", "two ULPs"),
        (b"1.0\n2.0\n(0.0 0.0 nan inf)\n", "signed zero"),
        (b"1.0\n2.0\n[0.0 -0.0 nan inf]\n", "structure"),
        (b"1.0\n2.0\n(0.0 -0.0 inf inf)\n", "special value"),
        (b"1\n2.0\n(0.0 -0.0 nan inf)\n", "canonical formatting"),
        (b"2.0\n1.0\n(0.0 -0.0 nan inf)\n", "root order"),
    ]:
        require(
            sqrt_output_mismatch(invalid, reference, leaves) is not None,
            f"sqrt comparator accepted invalid {label}",
        )


def validate_exp_output_comparator() -> None:
    reference = b"1.0\n0.0\n(2.0 inf nan)\n"
    leaves = (
        (0x3FF0_0000_0000_0000, 4),
        (0x0000_0000_0000_0000, 0),
        (0x4000_0000_0000_0000, 4),
    )
    require(
        exp_output_mismatch(reference, reference, leaves) is None,
        "exp comparator exact",
    )
    require(
        exp_output_mismatch(
            b"1.0000000000000009\n0.0\n(2.0 inf nan)\n",
            reference,
            leaves,
        )
        is None,
        "exp comparator +4 ULP",
    )
    require(
        exp_output_mismatch(
            b"0.9999999999999996\n0.0\n(2.0 inf nan)\n",
            reference,
            leaves,
        )
        is None,
        "exp comparator -4 ULP",
    )
    for invalid, label in [
        (b"1.000000000000001\n0.0\n(2.0 inf nan)\n", "five ULPs"),
        (b"1.00000000000000090\n0.0\n(2.0 inf nan)\n", "trailing zero"),
        (b"+1.0000000000000009\n0.0\n(2.0 inf nan)\n", "leading plus"),
        (b"1.0000000000000009e0\n0.0\n(2.0 inf nan)\n", "redundant exponent"),
        (b"0.99999999999999960\n0.0\n(2.0 inf nan)\n", "neighbor trailing zero"),
        (b"0.9999999999999996e0\n0.0\n(2.0 inf nan)\n", "neighbor exponent"),
        (b"1.0\n-0.0\n(2.0 inf nan)\n", "signed zero"),
        (b"1.0\n0.0\n[2.0 inf nan]\n", "structure"),
        (b"1.0\n0.0\n(2.0 nan nan)\n", "special value"),
        (b"1\n0.0\n(2.0 inf nan)\n", "canonical formatting"),
        (b"2.0\n0.0\n(1.0 inf nan)\n", "root order"),
    ]:
        require(
            exp_output_mismatch(invalid, reference, leaves) is not None,
            f"exp comparator accepted invalid {label}",
        )


def validate_log_output_comparator() -> None:
    reference = b"-inf\nnan\n0.0\n1.0\n"
    leaves = (
        (0x0000_0000_0000_0000, 0),
        (0x3FF0_0000_0000_0000, 4),
    )
    require(
        log_output_mismatch(reference, reference, leaves) is None,
        "log comparator exact",
    )
    require(
        log_output_mismatch(
            b"-inf\nnan\n0.0\n1.0000000000000009\n",
            reference,
            leaves,
        )
        is None,
        "log comparator +4 ULP",
    )
    require(
        log_output_mismatch(
            b"-inf\nnan\n0.0\n0.9999999999999996\n",
            reference,
            leaves,
        )
        is None,
        "log comparator -4 ULP",
    )
    for invalid, label in [
        (b"-inf\nnan\n0.0\n1.000000000000001\n", "five ULPs"),
        (b"-inf\nnan\n-0.0\n1.0\n", "signed zero"),
        (b"inf\nnan\n0.0\n1.0\n", "infinity sign"),
        (b"-inf\ninf\n0.0\n1.0\n", "domain special"),
        (b"(-inf)\nnan\n0.0\n1.0\n", "structure"),
        (b"-inf\nnan\n0\n1.0\n", "canonical formatting"),
        (b"-inf\nnan\n1.0\n0.0\n", "root order"),
    ]:
        require(
            log_output_mismatch(invalid, reference, leaves) is not None,
            f"log comparator accepted invalid {label}",
        )


def validate_log10_output_comparator() -> None:
    reference = b"-inf\nnan\n0.0\n1.0\n"
    leaves = (
        (0x0000_0000_0000_0000, 0),
        (0x3FF0_0000_0000_0000, 4),
    )
    require(
        log10_output_mismatch(reference, reference, leaves) is None,
        "log10 comparator exact",
    )
    require(
        log10_output_mismatch(
            b"-inf\nnan\n0.0\n1.0000000000000009\n",
            reference,
            leaves,
        )
        is None,
        "log10 comparator +4 ULP",
    )
    require(
        log10_output_mismatch(
            b"-inf\nnan\n0.0\n0.9999999999999996\n",
            reference,
            leaves,
        )
        is None,
        "log10 comparator -4 ULP",
    )
    for invalid, label in [
        (b"-inf\nnan\n0.0\n1.000000000000001\n", "five ULPs"),
        (b"-inf\nnan\n-0.0\n1.0\n", "signed zero"),
        (b"inf\nnan\n0.0\n1.0\n", "infinity sign"),
        (b"-inf\ninf\n0.0\n1.0\n", "domain special"),
        (b"(-inf)\nnan\n0.0\n1.0\n", "structure"),
        (b"-inf\nnan\n0\n1.0\n", "canonical formatting"),
        (b"-inf\nnan\n1.0\n0.0\n", "root order"),
    ]:
        require(
            log10_output_mismatch(invalid, reference, leaves) is not None,
            f"log10 comparator accepted invalid {label}",
        )


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
    validate_sqrt_output_comparator()
    validate_exp_output_comparator()
    validate_log_output_comparator()
    validate_log10_output_comparator()
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
                b"3\n2.5\ntrue\n(1 2 3)\n3\n(1 2 3)\n6\ntrue\ntrue\ntrue\n6\n(0 1 3 6)\n5.5\nfalse\n",
            ),
            (
                ROOT / "tests/fixtures/parameterized-artifact-double.bennu",
                ["0.1"],
                b"0.1\n",
            ),
            (
                ROOT / "tests/fixtures/backend-native-sqrt.bennu",
                [],
                SQRT_OUTPUT,
            ),
            (
                ROOT / "tests/fixtures/backend-native-exp.bennu",
                [],
                EXP_OUTPUT,
            ),
            (
                ROOT / "tests/fixtures/backend-native-log.bennu",
                [],
                LOG_OUTPUT,
            ),
            (
                ROOT / "tests/fixtures/backend-native-log10.bennu",
                [],
                LOG10_OUTPUT,
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
            generated_output = normalize_newlines(generated.stdout)
            evaluator_output = normalize_newlines(evaluator.stdout)
            if fixture.name == "backend-native-sqrt.bennu":
                require_sqrt_output(generated_output, "generated sqrt output")
                require_sqrt_output(evaluator_output, "evaluator sqrt output")
            elif fixture.name == "backend-native-exp.bennu":
                require_exp_output(generated_output, "generated exp output")
                require_exp_output(evaluator_output, "evaluator exp output")
            elif fixture.name == "backend-native-log.bennu":
                require_log_output(generated_output, "generated log output")
                require_log_output(evaluator_output, "evaluator log output")
            elif fixture.name == "backend-native-log10.bennu":
                require_log10_output(generated_output, "generated log10 output")
                require_log10_output(evaluator_output, "evaluator log10 output")
            else:
                require(
                    generated_output == normalize_newlines(expected),
                    f"generated output mismatch for {fixture.name}",
                )
                require(
                    evaluator_output == generated_output,
                    f"evaluator/generated mismatch for {fixture.name}",
                )
            require(
                normalize_newlines(artifact_runner.stdout)
                == evaluator_output,
                f"source/artifact evaluator mismatch for {fixture.name}",
            )
            require(
                not evaluator.stderr
                and not artifact_runner.stderr
                and not generated.stderr,
                fixture.name,
            )
            if fixture.name in {
                "backend-native-sqrt.bennu",
                "backend-native-exp.bennu",
                "backend-native-log.bennu",
                "backend-native-log10.bennu",
            }:
                operation = fixture.stem.removeprefix("backend-native-")
                hostile_source = work / f"{fixture.stem}-hostile.c"
                hostile_native = work / f"{fixture.stem}-hostile{suffix}"
                generated_source = emitted.read_text(encoding="utf-8")
                generated_main = "int main(int argc, char **argv) {"
                require(
                    generated_main in generated_source,
                    f"{operation} generated main declaration",
                )
                hostile_generated_source = (
                    generated_source.replace(
                        generated_main,
                        "static int fw_generated_main(int argc, char **argv) {",
                        1,
                    )
                    + """
int main(int argc, char **argv) {
#if defined(__x86_64__) || defined(_M_X64)
  unsigned int original=_mm_getcsr();
  unsigned int hostile=(original|0x1f80U|0x0040U|0x4000U|0x8000U)&~0x003fU;
  unsigned int restored;
  int result;
  _mm_setcsr(hostile);
  result=fw_generated_main(argc,argv);
  restored=_mm_getcsr();
  _mm_setcsr(original);
  if(result!=0)return result;
  return restored==hostile?0:1;
#elif defined(__aarch64__)
  uint64_t original_control,original_status,requested_control,requested_status;
  uint64_t hostile_control,hostile_status,restored_control,restored_status;
  int result;
  __asm__ volatile("mrs %0, fpcr":"=r"(original_control));
  __asm__ volatile("mrs %0, fpsr":"=r"(original_status));
  requested_control=(original_control&~UINT64_C(0x00009f00))|
      UINT64_C(0x00c00000)|UINT64_C(0x03000000);
  requested_status=original_status|UINT64_C(0x0000009f);
  __asm__ volatile("msr fpcr, %0\\n\\tisb"::"r"(requested_control):"memory");
  __asm__ volatile("msr fpsr, %0"::"r"(requested_status):"memory");
  __asm__ volatile("mrs %0, fpcr":"=r"(hostile_control));
  __asm__ volatile("mrs %0, fpsr":"=r"(hostile_status));
  if((hostile_control&UINT64_C(0x00009f00))!=0U){
    __asm__ volatile("msr fpcr, %0\\n\\tisb"::"r"(original_control):"memory");
    __asm__ volatile("msr fpsr, %0"::"r"(original_status):"memory");
    return 1;
  }
  result=fw_generated_main(argc,argv);
  __asm__ volatile("mrs %0, fpcr":"=r"(restored_control));
  __asm__ volatile("mrs %0, fpsr":"=r"(restored_status));
  __asm__ volatile("msr fpcr, %0\\n\\tisb"::"r"(original_control):"memory");
  __asm__ volatile("msr fpsr, %0"::"r"(original_status):"memory");
  if(result!=0)return result;
  if((hostile_control&(UINT64_C(0x00c00000)|UINT64_C(0x03000000)))!=
     (requested_control&(UINT64_C(0x00c00000)|UINT64_C(0x03000000))))
    return 1;
  if((hostile_status&UINT64_C(0x0000009f))!=
     (requested_status&UINT64_C(0x0000009f)))return 1;
  return restored_control==hostile_control&&restored_status==hostile_status?0:1;
#else
  return fw_generated_main(argc,argv);
#endif
}
"""
                )
                require(
                    '"msr fpcr, %0\\n\\tisb"' in hostile_generated_source,
                    "sqrt hostile C wrapper escaped FPCR instruction separator",
                )
                require(
                    '"msr fpcr, %0\n' not in hostile_generated_source,
                    "sqrt hostile C wrapper contains a literal newline in an asm string",
                )
                require(
                    "requested_control=(original_control&~UINT64_C(0x00009f00))|"
                    in hostile_generated_source,
                    "sqrt hostile C wrapper must clear AArch64 exception enables",
                )
                require(
                    "if((hostile_control&UINT64_C(0x00009f00))!=0U){"
                    in hostile_generated_source,
                    "sqrt hostile C wrapper must reject a trapping FPCR",
                )
                require(
                    hostile_generated_source.find(
                        "if((hostile_control&UINT64_C(0x00009f00))!=0U){"
                    )
                    < hostile_generated_source.rfind(
                        "result=fw_generated_main(argc,argv);"
                    ),
                    "sqrt hostile C wrapper must reject traps before FP execution",
                )
                require(
                    "return restored_control==hostile_control&&"
                    "restored_status==hostile_status?0:1;"
                    in hostile_generated_source,
                    "sqrt hostile C wrapper must require exact state restoration",
                )
                hostile_source.write_text(
                    hostile_generated_source,
                    encoding="utf-8",
                )
                compile_c(
                    compiler,
                    environment,
                    hostile_source,
                    hostile_native,
                )
                hostile_result = run(
                    [str(hostile_native)],
                    environment=environment,
                )
                hostile_output = normalize_newlines(hostile_result.stdout)
                require(
                    not hostile_result.stderr,
                    f"{operation} hostile generated-C stderr",
                )
                require(
                    hostile_output == generated_output,
                    f"{operation} hostile FP state changed generated-C output",
                )
                if operation == "sqrt":
                    require_sqrt_output(
                        hostile_output,
                        "sqrt hostile generated-C output",
                    )
                elif operation == "exp":
                    require_exp_output(
                        hostile_output,
                        "exp hostile generated-C output",
                    )
                elif operation == "log":
                    require_log_output(
                        hostile_output,
                        "log hostile generated-C output",
                    )
                else:
                    require_log10_output(
                        hostile_output,
                        "log10 hostile generated-C output",
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
  hostile=(original&~UINT64_C(0x00009f00))|UINT64_C(0x01c00000);
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

        for name, source_text, expected_column, expected_reason in [
            (
                "div-zero",
                "div[(8 9 10) (2 0 5)]\n",
                1,
                b"DomainError: div failed: division_by_zero at result index 1\n",
            ),
            (
                "div-overflow",
                "div[-9223372036854775808 -1]\n",
                1,
                b"DomainError: div failed: integer_overflow\n",
            ),
            (
                "sum-overflow",
                "sum[(9223372036854775807 1 -1)]\n",
                1,
                b"DomainError: sum failed: integer_overflow at result index 1\n",
            ),
            (
                "foldl-overflow",
                "foldl[@add 9223372036854775807 (1)]\n",
                7,
                b"DomainError: add failed: integer_overflow at result index 0\n",
            ),
            (
                "foldl-div-zero",
                "foldl[@div 8 (2 0 4)]\n",
                7,
                b"DomainError: div failed: division_by_zero at result index 1\n",
            ),
            (
                "scanl-overflow",
                "scanl[@add 9223372036854775807 (1)]\n",
                7,
                b"DomainError: add failed: integer_overflow at result index 0\n",
            ),
            (
                "scanl-div-zero",
                "scanl[@div 8 (2 0 4)]\n",
                7,
                b"DomainError: div failed: division_by_zero at result index 1\n",
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
                evaluator.stderr.endswith(
                    f":1:{expected_column}: ".encode() + expected_reason
                )
                and generated.stderr
                == f"<generated>:1:{expected_column}: ".encode() + expected_reason,
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
            == b"3\n2.5\ntrue\n(1 2 3)\n3\n(1 2 3)\n6\ntrue\ntrue\ntrue\n6\n(0 1 3 6)\n5.5\nfalse\n",
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
