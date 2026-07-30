#!/usr/bin/env python3
"""Offline workflow and packaging contracts."""
from __future__ import annotations
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
CHECKOUT_ACTION_REVISION = "3d3c42e5aac5ba805825da76410c181273ba90b1"
DEPRECATED_NODE20_CHECKOUT_REVISION = "11bd71901bbe5b1630ceea73d27597364c9af683"

def require(condition: bool, message: str) -> None:
    if not condition:
        raise SystemExit(message)

def static_contracts() -> None:
    cargo = (ROOT / "Cargo.toml").read_text()
    require('name = "faraweave"' in cargo and 'version = "0.2.0"' in cargo, "Cargo identity")
    locked_packages = tomllib.loads((ROOT / "Cargo.lock").read_text()).get("package", [])
    require(
        any(
            package.get("name") == "faraweave"
            and package.get("version") == VERSION
            for package in locked_packages
        ),
        "Cargo.lock identity",
    )
    toolchain = (ROOT / "rust-toolchain.toml").read_text()
    require('channel = "1.97.1"' in toolchain and "clippy" in toolchain, "toolchain pin")
    main = (ROOT / ".github/workflows/main.yml").read_text()
    validate_main_workflow(main)
    workflows = {
        str(path.relative_to(ROOT)): path.read_text()
        for path in sorted((ROOT / ".github/workflows").glob("*.y*ml"))
    }
    validate_action_pins(workflows)
    validate_release_workflows()
    validate_fwir_conformance()
    validate_product_cutover()
    validate_interpreter_only_documentation()


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
        "python tools/validation/contracts.py package ${{ matrix.target }}",
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


def validate_action_pins(workflows: dict[str, str]) -> None:
    try:
        validate_action_pins_without_mutations(workflows)
    except AssertionError as error:
        raise SystemExit(str(error)) from error

    for relative, text in workflows.items():
        checkout = f"actions/checkout@{CHECKOUT_ACTION_REVISION}"
        if checkout not in text:
            continue
        parts = text.split(checkout)
        for occurrence in range(len(parts) - 1):
            mutated = dict(workflows)
            mutated[relative] = (
                checkout.join(parts[: occurrence + 1])
                + f"actions/checkout@{DEPRECATED_NODE20_CHECKOUT_REVISION}"
                + checkout.join(parts[occurrence + 1 :])
            )
            try:
                validate_action_pins_without_mutations(mutated)
            except AssertionError:
                continue
            raise SystemExit(
                "workflow checkout revision negative mutation survived: "
                f"{relative} occurrence {occurrence + 1}"
            )


def validate_action_pins_without_mutations(workflows: dict[str, str]) -> None:
    checkout_revisions = []
    for relative, text in workflows.items():
        for action, revision in re.findall(r"uses:\s*([^@\s]+)@([^\s]+)", text):
            if not re.fullmatch(r"[0-9a-f]{40}", revision):
                raise AssertionError(
                    f"{relative} action {action} is not pinned by a full commit"
                )
            if action == "actions/checkout":
                checkout_revisions.append(revision)
    if not checkout_revisions:
        raise AssertionError("workflows have no actions/checkout usage")
    if any(revision != CHECKOUT_ACTION_REVISION for revision in checkout_revisions):
        raise AssertionError(
            "workflows do not pin actions/checkout to the approved Node 24 revision"
        )


def validate_release_workflows() -> None:
    release = (ROOT / ".github/workflows/release.yml").read_text()
    future = (ROOT / ".github/workflows/future-release.yml").read_text()
    publish = (ROOT / "tools/release/publish.sh").read_text()
    for text, name in [(release, "release"), (future, "future")]:
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
        "tags: [v0.2.0]",
        'test "${GITHUB_REF_NAME}" = v0.2.0',
        r'''test "$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)" = 0.2.0''',
        "git cat-file -t refs/tags/v0.2.0",
        "git rev-parse refs/tags/v0.2.0^{commit}",
        "! gh release view v0.2.0",
    ]:
        require(needle in release, f"release workflow missing {needle}")
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
    for needle in [
        'notes="doc/releases/${tag}.md"',
        'test -f "${notes}"',
        '--notes-file "${notes}"',
    ]:
        require(needle in publish, f"release publisher missing {needle}")
    notes = ROOT / f"doc/releases/v{VERSION}.md"
    require(notes.is_file(), f"release notes missing: {notes.relative_to(ROOT)}")


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
        interpreter_surfaces = {
            "decode",
            "reencode",
            "inspect",
            "interpret",
        }
        require(
            set(surfaces.split(",")) == interpreter_surfaces,
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
        "appl.",
        "oprf.",
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


def production_source(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]


def validate_product_cutover() -> None:
    lowering = production_source("src/lowering.rs")
    evaluator = production_source("src/evaluator.rs")
    interpreter = production_source("src/interpreter.rs")
    api = production_source("src/fwir_api.rs")
    library = production_source("src/lib.rs")
    cli = production_source("src/main.rs")
    cargo = (ROOT / "Cargo.toml").read_text(encoding="utf-8")
    production_tree = "\n".join(
        production_source(str(path.relative_to(ROOT)).replace("\\", "/"))
        for path in sorted((ROOT / "src").glob("*.rs"))
    )

    require("fn compile_source(" not in lowering, "temporary compile_source seam")
    require(
        "compile_source_with_name(" in lowering
        and "compile_parsed_source_with_name(" in lowering
        and "resolve_names(program)?" in lowering
        and "validate_program_arities(program)?" in lowering
        and "lower_program(" in lowering,
        "single source-to-verified-program lowerer",
    )
    for token in ("analyze_for_lowering", "TypeInfo", "fn analyze(", "fn select_call("):
        require(token not in production_tree, f"obsolete typed analyzer seam {token}")
    require(
        production_tree.count("fn select_descriptor(") == 1
        and "select_descriptor(name, &operands, location, &mut self.diagnostics)?" in lowering
        and "primitive_id: descriptor.primitive_id.numeric()" in lowering
        and "signature_id: descriptor.signature_id.numeric()" in lowering
        and "implementation_id: descriptor.implementation_id.numeric()" in lowering,
        "lowering is not the single typed selection authority",
    )
    require(
        "unsupported_signature_message(name, 1, diagnostics)?" in lowering,
        "iota type rejection bypasses fallible diagnostic construction",
    )
    require("[features]" not in cargo, "migration feature flags remain")

    require(
        "compile_parsed_source(" in evaluator
        and "evaluate_verified_program(" in evaluator,
        "source evaluation does not route through verified IR",
    )
    require(
        "program: &VerifiedProgram" in interpreter,
        "interpreter does not require VerifiedProgram",
    )
    for removed in ("src/c_emitter.rs", "src/native_builder.rs"):
        require(not (ROOT / removed).exists(), f"removed backend source remains: {removed}")
    require(
        not (ROOT / "tools/validation/c11_journey.py").exists()
        and (ROOT / "tools/validation/interpreter_journey.py").is_file(),
        "interpreter-only validation journey",
    )
    require(
        "evaluate_verified_program(" in api
        and "program: &VerifiedProgram" in api,
        "public artifact execution does not route through the verified interpreter",
    )
    for token in (
        "emit_c_source",
        "emit_c_from_verified_program",
        "build_native",
        "NativeBuild",
        "CEmitter",
    ):
        require(token not in production_tree, f"removed backend API remains: {token}")
    for token in ("mod c_emitter", "mod native_builder", "emit_c_", "build_native"):
        require(token not in library, f"removed library surface remains: {token}")
    require(
        re.search(r'"(?:emit-c|emit-c-ir|build|build-ir)"', cli) is None,
        "removed CLI command remains",
    )
    for relative, source, forbidden in (
        (
            "src/evaluator.rs",
            evaluator,
            ("evaluate_expr(", "select_call(", "ApplicationArgument", "TypeInfo"),
        ),
        (
            "src/interpreter.rs",
            interpreter,
            ("evaluate_expr(", "select_call(", "primitive_from_name(", "ExprKind"),
        ),
    ):
        for token in forbidden:
            require(token not in source, f"legacy backend token {token} in {relative}")


def validate_interpreter_only_documentation() -> None:
    active_documents = [
        "README.md",
        "doc/architecture.md",
        "examples/README.md",
        "spec/backend-native-math-v1.md",
        "spec/container-wide-application-plans.md",
        "spec/fwir-v1-encoding-measurements.md",
        "spec/fwir-v1-encoding.md",
        "spec/typed-fwir-semantic-contract.md",
    ]
    forbidden = (
        "emit-c",
        "emit-c-ir",
        "build-ir",
        "strict C11",
        "strict-C11",
        "generated C",
        "C compiler",
        "C emitter",
        "C/native",
        "native backend",
        "native builder",
        "native executable",
        "cross-backend",
        "src/c_emitter.rs",
        "src/native_builder.rs",
        "emit_c_",
        "build_native",
    )
    for relative in active_documents:
        text = (ROOT / relative).read_text(encoding="utf-8")
        for token in forbidden:
            require(
                token.casefold() not in text.casefold(),
                f"active documentation retains {token}: {relative}",
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
            [sys.executable, str(ROOT / "tools/validation/interpreter_journey.py")],
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
