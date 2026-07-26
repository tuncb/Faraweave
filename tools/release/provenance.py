#!/usr/bin/env python3

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import tarfile
import tempfile
from typing import NoReturn
import zipfile


SEMVER = re.compile(
    rb"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)"
    rb"(?:-((?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*)"
    rb"(?:\.(?:0|[1-9][0-9]*|[0-9A-Za-z-]*[A-Za-z-][0-9A-Za-z-]*))*))?"
    rb"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
)
TARGETS = {"linux-x64", "windows-x64", "macos-arm64"}
WORKFLOW_PATH = ".github/workflows/future-release.yml"


def fail(message) -> NoReturn:
    raise ValueError(message)


def sha256_bytes(contents):
    return hashlib.sha256(contents).hexdigest()


def sha256_file(path):
    digest = hashlib.sha256()
    with path.open("rb") as source:
        while True:
            block = source.read(1024 * 1024)
            if not block:
                return digest.hexdigest()
            digest.update(block)


def read_version(path):
    contents = path.read_bytes()
    if path.name == "Cargo.toml":
        matches = re.findall(rb'(?m)^version = "([^"]+)"\r?$', contents)
        if len(matches) != 1:
            fail("Cargo.toml must contain exactly one package version")
        candidate = matches[0]
    else:
        if not contents.endswith(b"\n") or contents.count(b"\n") != 1 or b"\r" in contents:
            fail("version input must be one LF-terminated SemVer line")
        candidate = contents[:-1]
    match = SEMVER.fullmatch(candidate)
    if match is None:
        fail("product version is not valid SemVer")
    if any(int(match.group(index)) > 65535 for index in (1, 2, 3)):
        fail("VERSION numeric core exceeds the Windows PE component limit")
    return candidate.decode("ascii")


def validate_commit(value):
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{40}", value) is None:
        fail("source commit must be one exact lowercase 40-hex Git object ID")


def validate_target(value):
    if not isinstance(value, str) or value not in TARGETS:
        fail(f"unsupported target: {value}")


def validate_repository(value):
    if not isinstance(value, str) or re.fullmatch(
        r"[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+", value
    ) is None:
        fail("repository must have owner/name form")


def validate_tag(version, tag):
    if tag is None:
        return
    if not isinstance(tag, str):
        fail("tag must be a string or null")
    if "-" in version or "+" in version:
        fail("production tags require a stable version without prerelease or build metadata")
    if tag != f"v{version}":
        fail(f"tag must be exactly v{version}")


def validate_member_name(name):
    if not isinstance(name, str) or not name or "\\" in name:
        fail(f"unsafe archive path: {name!r}")
    path = PurePosixPath(name)
    if path.is_absolute() or any(part in ("", ".", "..") for part in path.parts):
        fail(f"unsafe archive path: {name!r}")
    return path


def read_archive_files(path):
    files = {}
    if path.name.endswith(".tar.gz"):
        try:
            with tarfile.open(path, "r:gz") as archive:
                for member in archive.getmembers():
                    member_path = validate_member_name(member.name)
                    normalized = member_path.as_posix()
                    if normalized in files:
                        fail(f"duplicate archive path: {normalized}")
                    if member.isdir():
                        continue
                    if not member.isfile():
                        fail(f"archive contains a link or special entry: {normalized}")
                    source = archive.extractfile(member)
                    if source is None:
                        fail(f"unable to read archive entry: {normalized}")
                    files[normalized] = source.read()
        except (tarfile.TarError, OSError) as error:
            fail(f"unable to read tar.gz archive: {error}")
    elif path.name.endswith(".zip"):
        try:
            with zipfile.ZipFile(path, "r") as archive:
                for member in archive.infolist():
                    member_path = validate_member_name(member.filename)
                    normalized = member_path.as_posix()
                    if normalized in files:
                        fail(f"duplicate archive path: {normalized}")
                    unix_mode = member.external_attr >> 16
                    if stat.S_ISLNK(unix_mode):
                        fail(f"archive contains a symbolic link: {normalized}")
                    if member.is_dir():
                        continue
                    files[normalized] = archive.read(member)
        except (zipfile.BadZipFile, OSError) as error:
            fail(f"unable to read zip archive: {error}")
    else:
        fail("archive must use .tar.gz or .zip")
    return files


def select_executable(files, executable_path):
    validate_member_name(executable_path)
    executable_names = [
        name for name in files if PurePosixPath(name).name in ("faraweave", "faraweave.exe")
    ]
    if executable_path not in files:
        fail(f"archive is missing executable path: {executable_path}")
    if executable_names != [executable_path]:
        fail("archive has missing or extra Faraweave executable entries")
    return files[executable_path]


def run_version(contents, executable_path, expected_version):
    suffix = ".exe" if executable_path.endswith(".exe") else ""
    with tempfile.TemporaryDirectory(prefix="faraweave-version-check-") as directory:
        candidate = Path(directory) / f"faraweave{suffix}"
        candidate.write_bytes(contents)
        candidate.chmod(0o755)
        try:
            result = subprocess.run(
                [str(candidate), "--version"],
                capture_output=True,
                check=False,
                timeout=30,
            )
        except (OSError, subprocess.SubprocessError) as error:
            fail(f"unable to execute packaged Faraweave: {error}")
    expected = f"faraweave {expected_version}".encode("utf-8") + b"\n"
    if result.returncode != 0 or result.stdout != expected or result.stderr != b"":
        fail(
            "packaged Faraweave --version does not match Cargo.toml exactly: "
            f"stdout={result.stdout!r}, stderr={result.stderr!r}, "
            f"status={result.returncode}"
        )


def canonical_bytes(document):
    return (json.dumps(document, sort_keys=True, separators=(",", ":")) + "\n").encode(
        "utf-8"
    )


def reject_duplicate_pairs(pairs):
    document = {}
    for key, value in pairs:
        if key in document:
            fail(f"duplicate JSON key: {key}")
        document[key] = value
    return document


def load_canonical(path, subject):
    contents = path.read_bytes()
    try:
        document = json.loads(contents, object_pairs_hook=reject_duplicate_pairs)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        fail(f"malformed {subject} JSON: {error}")
    if not isinstance(document, dict):
        fail(f"{subject} must be a JSON object")
    if canonical_bytes(document) != contents:
        fail(f"{subject} is not canonical JSON")
    return document


def require_keys(document, expected, subject):
    if not isinstance(document, dict) or set(document) != set(expected):
        fail(f"{subject} fields do not match schema")


def validate_digest(value, subject):
    if not isinstance(value, str) or re.fullmatch(r"[0-9a-f]{64}", value) is None:
        fail(f"{subject} is not a lowercase SHA-256 digest")


def validate_semver(value, subject):
    if not isinstance(value, str):
        fail(f"{subject} is not SemVer")
    try:
        encoded = value.encode("ascii")
    except UnicodeEncodeError:
        fail(f"{subject} is not SemVer")
    match = SEMVER.fullmatch(encoded)
    if match is None:
        fail(f"{subject} is not SemVer")
    if any(int(match.group(index)) > 65535 for index in (1, 2, 3)):
        fail(f"{subject} numeric core exceeds the Windows PE component limit")


def validate_fragment(document):
    require_keys(
        document,
        {
            "archive",
            "executable",
            "schema_version",
            "source_commit",
            "tag",
            "target",
            "trust_policy",
            "version",
        },
        "fragment",
    )
    if type(document["schema_version"]) is not int or document["schema_version"] != 1:
        fail("unsupported fragment schema version")
    validate_semver(document["version"], "fragment version")
    validate_commit(document["source_commit"])
    validate_target(document["target"])
    validate_tag(document["version"], document["tag"])
    if not isinstance(document["trust_policy"], dict):
        fail("fragment trust policy must be an object")
    expected_trust = trust_policy(
        document["trust_policy"].get("repository", ""),
        document["trust_policy"].get("workflow", ""),
        document["source_commit"],
    )
    if document["trust_policy"] != expected_trust:
        fail("fragment trust policy does not match the enforced policy")
    require_keys(document["archive"], {"name", "sha256"}, "fragment archive")
    require_keys(
        document["executable"], {"path", "sha256", "version"}, "fragment executable"
    )
    archive_name = document["archive"]["name"]
    if not isinstance(archive_name, str) or Path(archive_name).name != archive_name:
        fail("fragment archive name must be a basename")
    expected_suffix = ".zip" if document["target"].startswith("windows-") else ".tar.gz"
    if archive_name != f"faraweave-v{document['version']}-{document['target']}{expected_suffix}":
        fail("fragment archive name does not match version and target")
    validate_digest(document["archive"]["sha256"], "fragment archive digest")
    executable_path = document["executable"]["path"]
    if not isinstance(executable_path, str):
        fail("fragment executable path must be a string")
    validate_member_name(executable_path)
    validate_digest(document["executable"]["sha256"], "fragment executable digest")
    if not isinstance(document["executable"]["version"], str):
        fail("fragment executable version must be a string")
    if document["executable"]["version"] != document["version"]:
        fail("fragment executable version does not match product version")


def verify_fragment_assets(document, asset_dir, execute):
    archive = asset_dir / document["archive"]["name"]
    if not archive.is_file():
        fail(f"archive is missing: {archive.name}")
    if sha256_file(archive) != document["archive"]["sha256"]:
        fail(f"archive digest mismatch: {archive.name}")
    files = read_archive_files(archive)
    executable_path = document["executable"]["path"]
    executable = select_executable(files, executable_path)
    if sha256_bytes(executable) != document["executable"]["sha256"]:
        fail(f"executable digest mismatch: {executable_path}")
    if execute:
        run_version(executable, executable_path, document["version"])


def write_new(path, contents):
    try:
        with path.open("xb") as output:
            output.write(contents)
    except FileExistsError:
        fail(f"refusing to overwrite existing output: {path}")


def trust_policy(repository, workflow, source_commit):
    validate_repository(repository)
    validate_commit(source_commit)
    if not isinstance(workflow, str):
        fail("workflow must be a string")
    if workflow != WORKFLOW_PATH:
        fail(f"workflow must be exactly {WORKFLOW_PATH}")
    return {
        "macos": "not-developer-id-or-notarized",
        "macos_issue_12": "CLOSED_NOT_PLANNED",
        "mechanism": "github-artifact-attestation",
        "repository": repository,
        "source_commit": source_commit,
        "windows": "not-authenticode",
        "workflow": workflow,
    }


def create_fragment(arguments):
    version = read_version(Path(arguments.version_file))
    validate_target(arguments.target)
    validate_commit(arguments.source_commit)
    validate_repository(arguments.repository)
    validate_tag(version, arguments.tag)
    archive = Path(arguments.archive)
    expected_suffix = ".zip" if arguments.target.startswith("windows-") else ".tar.gz"
    expected_name = f"faraweave-v{version}-{arguments.target}{expected_suffix}"
    if archive.name != expected_name:
        fail(f"archive name must be exactly {expected_name}")
    files = read_archive_files(archive)
    executable = select_executable(files, arguments.executable_path)
    run_version(executable, arguments.executable_path, version)
    document = {
        "archive": {"name": archive.name, "sha256": sha256_file(archive)},
        "executable": {
            "path": arguments.executable_path,
            "sha256": sha256_bytes(executable),
            "version": version,
        },
        "schema_version": 1,
        "source_commit": arguments.source_commit,
        "tag": arguments.tag,
        "target": arguments.target,
        "trust_policy": trust_policy(
            arguments.repository, arguments.workflow, arguments.source_commit
        ),
        "version": version,
    }
    write_new(Path(arguments.output), canonical_bytes(document))


def merge_fragments(arguments):
    asset_dir = Path(arguments.asset_dir)
    artifacts = []
    global_identity = None
    observed_targets = set()
    observed_archives = set()
    for fragment_value in arguments.fragment:
        fragment_path = Path(fragment_value)
        document = load_canonical(fragment_path, "fragment")
        validate_fragment(document)
        expected_fragment_name = f"{document['target']}.fragment.json"
        if fragment_path.name != expected_fragment_name:
            fail(f"fragment name must be exactly {expected_fragment_name}")
        identity = (
            document["version"],
            document["source_commit"],
            document["tag"],
            document["trust_policy"],
        )
        if global_identity is None:
            global_identity = identity
        elif identity != global_identity:
            fail("fragments have mismatched version, commit, tag, or trust policy")
        if document["target"] in observed_targets:
            fail(f"duplicate fragment target: {document['target']}")
        if document["archive"]["name"] in observed_archives:
            fail(f"duplicate archive name: {document['archive']['name']}")
        observed_targets.add(document["target"])
        observed_archives.add(document["archive"]["name"])
        verify_fragment_assets(document, asset_dir, False)
        artifacts.append(
            {
                "archive": document["archive"],
                "executable": document["executable"],
                "fragment": {
                    "name": fragment_path.name,
                    "sha256": sha256_file(fragment_path),
                },
                "target": document["target"],
            }
        )
    if global_identity is None:
        fail("at least one fragment is required")
    version, source_commit, tag, policy = global_identity
    if tag is not None and observed_targets != TARGETS:
        fail("production manifests require exactly all supported targets")
    manifest = {
        "artifacts": sorted(artifacts, key=lambda artifact: artifact["target"]),
        "schema_version": 1,
        "source_commit": source_commit,
        "tag": tag,
        "trust_policy": policy,
        "version": version,
    }
    write_new(Path(arguments.output), canonical_bytes(manifest))


def validate_manifest(document):
    require_keys(
        document,
        {
            "artifacts",
            "schema_version",
            "source_commit",
            "tag",
            "trust_policy",
            "version",
        },
        "manifest",
    )
    if type(document["schema_version"]) is not int or document["schema_version"] != 1:
        fail("unsupported manifest schema version")
    validate_commit(document["source_commit"])
    validate_semver(document["version"], "manifest version")
    validate_tag(document["version"], document["tag"])
    if not isinstance(document["trust_policy"], dict):
        fail("manifest trust policy must be an object")
    expected_trust = trust_policy(
        document["trust_policy"].get("repository", ""),
        document["trust_policy"].get("workflow", ""),
        document["source_commit"],
    )
    if document["trust_policy"] != expected_trust:
        fail("manifest trust policy does not match the enforced policy")
    if not isinstance(document["artifacts"], list) or not document["artifacts"]:
        fail("manifest artifacts must be a nonempty list")
    for artifact in document["artifacts"]:
        require_keys(
            artifact, {"archive", "executable", "fragment", "target"}, "manifest artifact"
        )
        validate_target(artifact["target"])
        require_keys(artifact["archive"], {"name", "sha256"}, "manifest archive")
        require_keys(
            artifact["executable"],
            {"path", "sha256", "version"},
            "manifest executable",
        )
        require_keys(artifact["fragment"], {"name", "sha256"}, "manifest fragment")
    if document["artifacts"] != sorted(
        document["artifacts"], key=lambda artifact: artifact["target"]
    ):
        fail("manifest artifacts are not sorted by target")


def verify_manifest(arguments):
    manifest = load_canonical(Path(arguments.manifest), "manifest")
    validate_manifest(manifest)
    validate_commit(arguments.expected_source_commit)
    validate_repository(arguments.expected_repository)
    if manifest["source_commit"] != arguments.expected_source_commit:
        fail("manifest source commit does not match the expected commit")
    expected_policy = trust_policy(
        arguments.expected_repository,
        arguments.expected_workflow,
        arguments.expected_source_commit,
    )
    if manifest["trust_policy"] != expected_policy:
        fail("manifest trust policy does not match expected repository/workflow/commit")
    execute_target = arguments.target
    if execute_target is not None:
        validate_target(execute_target)
    asset_dir = Path(arguments.asset_dir)
    fragment_dir = Path(arguments.fragment_dir)
    observed_targets = set()
    executed = False
    for artifact in manifest["artifacts"]:
        require_keys(
            artifact, {"archive", "executable", "fragment", "target"}, "manifest artifact"
        )
        target = artifact["target"]
        if target in observed_targets:
            fail(f"duplicate manifest target: {target}")
        observed_targets.add(target)
        fragment = {
            "archive": artifact["archive"],
            "executable": artifact["executable"],
            "schema_version": 1,
            "source_commit": manifest["source_commit"],
            "tag": manifest["tag"],
            "target": target,
            "trust_policy": manifest["trust_policy"],
            "version": manifest["version"],
        }
        validate_fragment(fragment)
        require_keys(artifact["fragment"], {"name", "sha256"}, "manifest fragment")
        fragment_name = f"{target}.fragment.json"
        if artifact["fragment"]["name"] != fragment_name:
            fail("manifest fragment name does not match its target")
        validate_digest(artifact["fragment"]["sha256"], "manifest fragment digest")
        fragment_path = fragment_dir / fragment_name
        if sha256_file(fragment_path) != artifact["fragment"]["sha256"]:
            fail(f"fragment digest mismatch: {fragment_name}")
        if load_canonical(fragment_path, "fragment") != fragment:
            fail(f"manifest fields do not match fragment: {fragment_name}")
        should_execute = target == execute_target
        verify_fragment_assets(fragment, asset_dir, should_execute)
        executed = executed or should_execute
    if execute_target is not None and not executed:
        fail(f"manifest has no artifact for target: {execute_target}")
    if manifest["tag"] is not None and observed_targets != TARGETS:
        fail("production manifest does not contain exactly all supported targets")


def run_git_result(repository, git_arguments, text):
    try:
        result = subprocess.run(
            ["git", "-C", str(repository), *git_arguments],
            capture_output=True,
            check=False,
            text=text,
            timeout=30,
        )
    except (OSError, subprocess.SubprocessError) as error:
        fail(f"unable to inspect Git source identity: {error}")
    if result.returncode != 0:
        fail(f"Git source identity check failed: {' '.join(git_arguments)}")
    return result.stdout


def run_git(repository, git_arguments):
    return run_git_result(repository, git_arguments, True).strip()


def run_git_bytes(repository, git_arguments):
    return run_git_result(repository, git_arguments, False)


def production_gate(arguments):
    repository = Path(arguments.repository_dir).resolve()
    version_file = Path(arguments.version_file).resolve()
    if version_file != repository / "Cargo.toml":
        fail("production gate must read the repository-root Cargo.toml")
    version = read_version(version_file)
    if "-" in version or "+" in version:
        fail("production requires a stable version without prerelease or build metadata")
    validate_commit(arguments.source_commit)
    validate_tag(version, arguments.tag)
    if run_git(repository, ["rev-parse", "HEAD"]) != arguments.source_commit:
        fail("source commit does not match checked-out HEAD")
    committed_version = run_git_bytes(
        repository, ["show", f"{arguments.source_commit}:Cargo.toml"]
    )
    if committed_version != version_file.read_bytes():
        fail("Cargo.toml bytes are not supplied by the exact source commit")
    tag_ref = f"refs/tags/{arguments.tag}"
    if run_git(repository, ["cat-file", "-t", tag_ref]) != "tag":
        fail("production tag must be an annotated tag object")
    if run_git(repository, ["rev-parse", f"{tag_ref}^{{commit}}"]) != arguments.source_commit:
        fail("annotated tag does not resolve to the exact source commit")


def build_parser():
    parser = argparse.ArgumentParser(description="Create and verify Faraweave release provenance")
    subparsers = parser.add_subparsers(dest="command", required=True)
    fragment = subparsers.add_parser("fragment")
    fragment.add_argument("--version-file", required=True)
    fragment.add_argument("--target", required=True)
    fragment.add_argument("--source-commit", required=True)
    fragment.add_argument("--archive", required=True)
    fragment.add_argument("--executable-path", required=True)
    fragment.add_argument("--repository", required=True)
    fragment.add_argument("--workflow", required=True)
    fragment.add_argument("--tag")
    fragment.add_argument("--output", required=True)
    fragment.set_defaults(handler=create_fragment)
    merge = subparsers.add_parser("merge")
    merge.add_argument("--fragment", action="append", required=True)
    merge.add_argument("--asset-dir", required=True)
    merge.add_argument("--output", required=True)
    merge.set_defaults(handler=merge_fragments)
    verify = subparsers.add_parser("verify")
    verify.add_argument("--manifest", required=True)
    verify.add_argument("--asset-dir", required=True)
    verify.add_argument("--fragment-dir", required=True)
    verify.add_argument("--target")
    verify.add_argument("--expected-repository", required=True)
    verify.add_argument("--expected-workflow", required=True)
    verify.add_argument("--expected-source-commit", required=True)
    verify.set_defaults(handler=verify_manifest)
    gate = subparsers.add_parser("gate")
    gate.add_argument("--version-file", required=True)
    gate.add_argument("--repository-dir", required=True)
    gate.add_argument("--source-commit", required=True)
    gate.add_argument("--tag", required=True)
    gate.set_defaults(handler=production_gate)
    return parser


def main():
    parser = build_parser()
    arguments = parser.parse_args()
    try:
        arguments.handler(arguments)
    except (KeyError, OSError, ValueError) as error:
        print(f"error: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
