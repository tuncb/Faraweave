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

COMMAND_EVIDENCE = {
    "command:contracts-review": "python tools/validation/contracts.py review",
    "command:host-package-contract": "python tools/validation/contracts.py package ${{ matrix.target }}",
    "command:main-ci-debug-tests": "cargo test --workspace --all-targets --all-features",
    "command:main-ci-full-contracts": "python tools/validation/contracts.py full",
    "command:main-ci-release-tests": "cargo test --workspace --all-targets --all-features --release",
    "command:strict-c11-journey": "python tools/validation/c11_journey.py",
}

CUTOVER_EVIDENCE = {
    "FWIR-CUTOVER-001": (
        "python:tools/validation/contracts.py::validate_product_cutover",
        "rust:src/lowering.rs::lowering_materializes_the_only_typed_selection_decisions",
    ),
    "FWIR-CUTOVER-002": (
        "rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential",
        "rust:tests/parity_contracts.rs::fan_stable_id_matrix",
    ),
    "FWIR-CUTOVER-003": (
        "rust:src/c_emitter.rs::public_generated_c_matches_direct_ir_for_success_and_failure_corpus",
        "command:strict-c11-journey",
    ),
    "FWIR-CUTOVER-004": (
        "python:tools/validation/contracts.py::validate_product_cutover",
        "rust:src/c_emitter.rs::every_selected_id_emits_a_direct_kernel_symbol_without_type_redispatch",
    ),
    "FWIR-CUTOVER-005": (
        "rust:tests/fwir_conformance.rs::canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral",
        "rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact",
    ),
    "FWIR-CUTOVER-006": (
        "rust:tests/fwir_conformance.rs::traceability_references_complete_executable_evidence_sets",
        "python:tools/validation/contracts.py::validate_product_cutover",
    ),
    "FWIR-CUTOVER-007": (
        "python:tools/validation/contracts.py::validate_product_cutover",
    ),
    "FWIR-CUTOVER-008": (
        "command:main-ci-debug-tests",
        "command:main-ci-release-tests",
        "command:main-ci-full-contracts",
        "command:host-package-contract",
    ),
}

SEMANTIC_EVIDENCE = {
    "FWIR-SEM-001": (
        "python:tools/validation/contracts.py::validate_product_cutover",
        "rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential",
    ),
    "FWIR-SEM-002": (
        "rust:src/typed_program.rs::valid_fixtures_cover_every_node_and_edge_family",
        "rust:src/typed_program.rs::verifier_category_winners_follow_the_normative_order",
    ),
    "FWIR-SEM-003": (
        "rust:tests/parity_contracts.rs::typed_public_api_parameter_contract",
        "rust:tests/cli_contracts.rs::cli_parameters_and_diagnostics_contract",
    ),
    "FWIR-SEM-004": (
        "rust:tests/parity_contracts.rs::s16_empty_singleton_promotion_and_shape_contracts",
        "rust:tests/parity_contracts.rs::deep_structural_values_and_types_format_and_drop_iteratively",
    ),
    "FWIR-SEM-005": (
        "rust:tests/parity_contracts.rs::canonical_binary64_format_boundaries",
        "rust:tests/resource_contracts.rs::typed_api_rejects_noncanonical_nan_without_normalizing_it",
        "rust:tests/resource_contracts.rs::resource_observer_reports_commit_refusal_and_cleanup_order",
    ),
    "FWIR-SEM-006": (
        "rust:src/parser.rs::parses_literals_calls_tuples_parameters_and_fanout",
        "rust:tests/parity_contracts.rs::deep_unary_programs_use_iterative_parse_analysis_and_evaluation",
    ),
    "FWIR-SEM-007": (
        "rust:src/semantic_registry.rs::production_registry_is_complete_and_numeric_lookups_are_checked",
        "rust:src/c_emitter.rs::every_selected_id_emits_a_direct_kernel_symbol_without_type_redispatch",
    ),
    "FWIR-SEM-008": (
        "rust:tests/parity_contracts.rs::checked_arithmetic_has_no_partial_result",
        "rust:tests/resource_contracts.rs::vector_tuple_and_work_limits_cover_zero_exact_and_one_past",
        "rust:src/lowering.rs::exact_ir_golden_digests_cover_every_source_construct",
    ),
    "FWIR-SEM-009": (
        "rust:tests/parity_contracts.rs::tup_structural_format_spread_and_direct_preservation",
        "rust:src/evaluator.rs::lifting_and_tuples_are_canonical",
    ),
    "FWIR-SEM-010": (
        "rust:tests/resource_contracts.rs::tuple_allocation_ordinals_exclude_empty_tables_and_cleanup_failures",
        "rust:tests/resource_contracts.rs::live_limit_observes_children_before_outer_tuple_admission",
        "rust:tests/parity_contracts.rs::deep_structural_values_and_types_format_and_drop_iteratively",
    ),
    "FWIR-SEM-011": (
        "rust:tests/parity_contracts.rs::fan_stable_id_matrix",
        "rust:src/lowering.rs::fan_out_prefix_placeholder_borrows_prepare_and_preserves_elements",
        "rust:src/c_emitter.rs::public_generated_c_matches_direct_ir_for_success_and_failure_corpus",
    ),
    "FWIR-SEM-012": (
        "rust:tests/resource_contracts.rs::parameter_header_reason_and_span_contract_is_structured",
        "rust:tests/golden_corpus.rs::authored_section_15_and_16_failure_golden_corpus",
        "rust:tests/cli_contracts.rs::cli_parameters_and_diagnostics_contract",
    ),
    "FWIR-SEM-013": (
        "rust:tests/resource_contracts.rs::profile_configuration_precedes_source_and_backend_analysis",
        "rust:src/lowering.rs::whole_program_static_precedence_is_arity_then_type_then_shape",
    ),
    "FWIR-SEM-014": (
        "rust:tests/resource_contracts.rs::refusal_precedence_is_vector_then_live_then_work_then_allocation",
        "rust:tests/resource_contracts.rs::failure_usage_is_post_cleanup_and_work_remains_monotonic",
        "rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential",
    ),
    "FWIR-SEM-015": (
        "rust:tests/parity_contracts.rs::resource_profiles_limits_and_ordinals",
        "rust:tests/resource_contracts.rs::generated_runtime_embeds_profile_and_verified_primitive_selection",
    ),
    "FWIR-SEM-016": (
        "rust:src/typed_program.rs::identity_result_root_and_feature_invariants_are_rejected",
        "rust:tests/fwir_conformance.rs::deterministic_mutation_corpus_is_rejected_without_panic_or_partial_program",
    ),
    "FWIR-SEM-017": (
        "rust:src/lowering.rs::exact_ir_golden_digests_cover_every_source_construct",
        "rust:src/evaluator.rs::evaluates_complete_primitive_surface",
    ),
    "FWIR-SEM-018": (
        "rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact",
        "rust:tests/fwir_conformance.rs::canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral",
    ),
    "FWIR-SEM-019": (
        "python:tools/validation/contracts.py::validate_product_cutover",
        "rust:tests/fwir_conformance.rs::traceability_references_complete_executable_evidence_sets",
    ),
    "FWIR-SEM-020": (
        "python:tools/validation/contracts.py::validate_product_cutover",
        "command:contracts-review",
    ),
}

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
    validate_product_cutover()


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


def production_source(relative: str) -> str:
    return (ROOT / relative).read_text(encoding="utf-8").split("#[cfg(test)]", 1)[0]


def traceability_evidence(
    text: str, prefix: str, evidence_column: int
) -> dict[str, tuple[str, ...]]:
    rows: dict[str, tuple[str, ...]] = {}
    for line in text.splitlines():
        if not line.startswith("| `"):
            continue
        columns = [column.strip() for column in line.strip().strip("|").split("|")]
        match = re.fullmatch(rf"`({re.escape(prefix)}-\d{{3}})`(?:\s+—.*)?", columns[0])
        if match is None:
            continue
        key = match.group(1)
        require(key not in rows, f"duplicate traceability row {key}")
        rows[key] = tuple(re.findall(r"`([^`]+)`", columns[evidence_column]))
    return rows


def validate_executable_evidence(identifier: str) -> None:
    if identifier.startswith(("rust:", "python:")):
        kind, target = identifier.split(":", 1)
        relative, function = target.rsplit("::", 1)
        path = ROOT / relative
        require(path.is_file(), f"{identifier}: missing source")
        source = path.read_text(encoding="utf-8")
        declaration = (
            rf"#\[test\](?:\s*#\[[^\]]+\])*\s*fn\s+{re.escape(function)}\s*\("
            if kind == "rust"
            else rf"\bdef\s+{re.escape(function)}\s*\("
        )
        require(
            re.search(declaration, source) is not None,
            f"{identifier}: missing executable function",
        )
        return
    require(identifier in COMMAND_EVIDENCE, f"unknown command evidence {identifier}")
    command_sources = "\n".join(
        (ROOT / relative).read_text(encoding="utf-8")
        for relative in (
            "doc/validation-ladder.md",
            "README.md",
            ".github/workflows/main.yml",
        )
    )
    command = COMMAND_EVIDENCE[identifier]
    if identifier == "command:host-package-contract":
        workflow = (ROOT / ".github/workflows/main.yml").read_text(encoding="utf-8")
        require(
            f"- run: {command}" in workflow,
            f"{identifier}: exact command is not executed by Main CI",
        )
    else:
        require(
            command in command_sources,
            f"{identifier}: exact command is not in the validation contract",
        )


def validate_traceability_evidence(
    actual: dict[str, tuple[str, ...]],
    expected: dict[str, tuple[str, ...]],
    family: str,
) -> None:
    require(actual == expected, f"{family} exact executable evidence mapping")
    for key, identifiers in actual.items():
        require(bool(identifiers), f"{key}: empty executable evidence")
        require(
            len(identifiers) == len(set(identifiers)),
            f"{key}: duplicate executable evidence",
        )
        for identifier in identifiers:
            validate_executable_evidence(identifier)


def validate_product_cutover() -> None:
    lowering = production_source("src/lowering.rs")
    evaluator = production_source("src/evaluator.rs")
    interpreter = production_source("src/interpreter.rs")
    emitter = production_source("src/c_emitter.rs")
    api = production_source("src/fwir_api.rs")
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
    require(
        "emit_verified_c_program(" in emitter
        and "program: &VerifiedProgram" in emitter
        and "emit_c_from_verified_program(" in api,
        "C/native generation does not route through verified IR",
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
        (
            "src/c_emitter.rs",
            emitter,
            (
                "struct CGenerator",
                "emit_parameterized_program(",
                "emit_constant_program(",
                "runtime_failure_program(",
                "static_expression_type(",
                "known_vector_length(",
                "primitive_tag(",
                "evaluate_source_with_configuration(",
                "static int fw_apply(",
                "fw_apply_scalar",
                "FW_INC",
            ),
        ),
    ):
        for token in forbidden:
            require(token not in source, f"legacy backend token {token} in {relative}")

    semantic = (ROOT / "spec/typed-fwir-semantic-contract.md").read_text(
        encoding="utf-8"
    )
    validate_traceability_evidence(
        traceability_evidence(semantic, "FWIR-SEM", 1),
        SEMANTIC_EVIDENCE,
        "FWIR semantic",
    )
    require(
        "tests/fixtures/fwir-v1-conformance.tsv" in semantic,
        "physical traceability is not linked from semantic traceability",
    )

    encoding = (ROOT / "spec/fwir-v1-encoding.md").read_text(encoding="utf-8")
    for policy in (
        "Accepted product, producer, and security policy",
        "Faraweave is the authoritative v1 producer",
        "FWIR is not confidential or encrypted",
        "input-safety boundary, not a sandbox",
    ):
        require(policy in encoding, f"FWIR v1 policy missing {policy}")
    examples = (ROOT / "spec/examples/README.md").read_text(encoding="utf-8")
    for name in ("empty", "scalar-true", "complete"):
        require(f"fwir-v1-{name}.hex" in examples, f"FWIR example inventory: {name}")
    architecture = (ROOT / "doc/architecture.md").read_text(encoding="utf-8")
    validate_traceability_evidence(
        traceability_evidence(architecture, "FWIR-CUTOVER", 2),
        CUTOVER_EVIDENCE,
        "FWIR product cutover",
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
