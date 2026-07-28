#!/usr/bin/env python3
"""Reproduce FWIR v1 encoding-choice sizes and decode the hand examples.

This is a specification measurement model, not a product encoder or decoder.
It uses only the Python standard library and intentionally has no crate API.
"""

from __future__ import annotations

import argparse
import ast
import inspect
import json
import struct
from pathlib import Path


MAGIC = b"FWIR\r\n\x1a\n"
HEADER_SIZE = 32
DIRECTORY_ENTRY_SIZE = 24
MANDATORY_IDENTITY = 3
NONE = 0xFFFF_FFFF

SECTIONS = {
    "MODL": (1, 8),
    "FEAT": (2, 4),
    "STRS": (3, 0),
    "SRCU": (4, 8),
    "PARM": (5, 20),
    "TYPE": (6, 12),
    "TYEL": (7, 4),
    "CONS": (8, 20),
    "COEL": (9, 12),
    "ORIG": (10, 28),
    "EDGE": (11, 24),
    "SHCK": (12, 4),
    "BRAN": (13, 20),
    "NODE": (14, 56),
    "OWNR": (15, 12),
    "ROOT": (16, 8),
}


def empty_program() -> dict:
    return {
        "module": {
            "semantic_major": 1,
            "semantic_minor": 0,
            "parameter_header_origin": NONE,
        },
        "features": [],
        "strings": [],
        "source_units": [],
        "parameters": [],
        "types": [],
        "type_elements": [],
        "constants": [],
        "constant_elements": [],
        "origins": [],
        "edges": [],
        "shape_checks": [],
        "branches": [],
        "nodes": [],
        "ownership": [],
        "roots": [],
    }


def scalar_true_program(root_count: int = 1) -> dict:
    program = empty_program()
    program["strings"] = ["example.fw"]
    program["source_units"] = [{"diagnostic_name": 0, "byte_length": 4}]
    program["types"] = [
        {
            "kind": 1,
            "scalar_type": 1,
            "element_start": 0,
            "element_count": 0,
        }
    ]
    program["constants"] = [
        {
            "kind": 1,
            "scalar_type": 1,
            "element_start": 0,
            "element_count": 0,
            "payload": 1,
        }
    ]
    program["origins"] = [
        {
            "source_unit": 0,
            "begin_offset": 1,
            "begin_line": 1,
            "begin_column": 1,
            "end_offset": 5,
            "end_line": 1,
            "end_column": 5,
        }
    ]
    for index in range(root_count):
        program["nodes"].append(
            {
                "kind": 1,
                "cardinality_kind": 1,
                "lift": 0,
                "result_element_type": 0,
                "result_type": 0,
                "cardinality_length": 0,
                "edge_start": 0,
                "edge_count": 0,
                "origin": 0,
                "args": [0] * 8,
            }
        )
        program["ownership"].append(
            {"owner": index, "release_kind": 2, "release_index": index}
        )
        program["roots"].append({"node": index, "origin": 0})
    return program


def _strings_payload(strings: list[str]) -> bytes:
    encoded = [value.encode("utf-8") for value in strings]
    descriptors = bytearray(struct.pack("<I", len(encoded)))
    data = bytearray()
    for value in encoded:
        descriptors.extend(struct.pack("<II", len(data), len(value)))
        data.extend(value)
    return bytes(descriptors + data)


def _fixed_records(program: dict) -> dict[str, bytes]:
    module = program["module"]
    payloads: dict[str, bytes] = {
        "MODL": struct.pack(
            "<HHI",
            module["semantic_major"],
            module["semantic_minor"],
            module["parameter_header_origin"],
        )
    }

    def records(name: str, values: list[dict], pack_record) -> None:
        if values:
            payloads[name] = b"".join(pack_record(value) for value in values)

    if program["features"]:
        payloads["FEAT"] = b"".join(
            struct.pack("<HBB", feature["id"], feature["class"], 0)
            for feature in program["features"]
        )
    if program["strings"]:
        payloads["STRS"] = _strings_payload(program["strings"])
    records(
        "SRCU",
        program["source_units"],
        lambda row: struct.pack("<II", row["diagnostic_name"], row["byte_length"]),
    )
    records(
        "PARM",
        program["parameters"],
        lambda row: struct.pack(
            "<IIB3xII",
            row["slot"],
            row["name"],
            row["scalar_type"],
            row["declaration_origin"],
            row["name_origin"],
        ),
    )
    records(
        "TYPE",
        program["types"],
        lambda row: struct.pack(
            "<BBHII",
            row["kind"],
            row["scalar_type"],
            0,
            row["element_start"],
            row["element_count"],
        ),
    )
    if program["type_elements"]:
        payloads["TYEL"] = b"".join(
            struct.pack("<I", value) for value in program["type_elements"]
        )
    records(
        "CONS",
        program["constants"],
        lambda row: struct.pack(
            "<BBHIIQ",
            row["kind"],
            row["scalar_type"],
            0,
            row["element_start"],
            row["element_count"],
            row["payload"],
        ),
    )
    records(
        "COEL",
        program["constant_elements"],
        lambda row: struct.pack(
            "<B3xQ",
            row["scalar_type"],
            row["payload"],
        ),
    )
    records(
        "ORIG",
        program["origins"],
        lambda row: struct.pack(
            "<IIIIIII",
            row["source_unit"],
            row["begin_offset"],
            row["begin_line"],
            row["begin_column"],
            row["end_offset"],
            row["end_line"],
            row["end_column"],
        ),
    )
    records(
        "EDGE",
        program["edges"],
        lambda row: struct.pack(
            "<IIBBBBIII",
            row["producer"],
            row["argument_position"],
            row["access"],
            row["cardinality_kind"],
            row["conversion"],
            row["ownership"],
            row["access_index"],
            row["cardinality_length"],
            row["origin"],
        ),
    )
    if program["shape_checks"]:
        payloads["SHCK"] = b"".join(
            struct.pack("<I", value) for value in program["shape_checks"]
        )
    records(
        "BRAN",
        program["branches"],
        lambda row: struct.pack(
            "<IIIII",
            row["node_start"],
            row["node_count"],
            row["root"],
            row["placeholder_origin"],
            row["origin"],
        ),
    )
    records(
        "NODE",
        program["nodes"],
        lambda row: struct.pack(
            "<BBBBIIIIIIIIIIIII",
            row["kind"],
            row["cardinality_kind"],
            row["lift"],
            row["result_element_type"],
            row["result_type"],
            row["cardinality_length"],
            row["edge_start"],
            row["edge_count"],
            row["origin"],
            *row["args"],
        ),
    )
    records(
        "OWNR",
        program["ownership"],
        lambda row: struct.pack(
            "<IB3xI",
            row["owner"],
            row["release_kind"],
            row["release_index"],
        ),
    )
    records(
        "ROOT",
        program["roots"],
        lambda row: struct.pack("<II", row["node"], row["origin"]),
    )
    return payloads


def encode_sectioned(program: dict) -> bytes:
    payloads = _fixed_records(program)
    names = sorted(payloads, key=lambda name: SECTIONS[name][0])
    payload_offset = HEADER_SIZE + DIRECTORY_ENTRY_SIZE * len(names)
    header = struct.pack(
        "<8sHHIHHIQ",
        MAGIC,
        1,
        0,
        HEADER_SIZE,
        DIRECTORY_ENTRY_SIZE,
        0,
        len(names),
        HEADER_SIZE,
    )
    directory = bytearray()
    payload = bytearray()
    for name in names:
        section_id, record_size = SECTIONS[name]
        value = payloads[name]
        directory.extend(
            struct.pack(
                "<HHIQQ",
                section_id,
                MANDATORY_IDENTITY,
                record_size,
                payload_offset,
                len(value),
            )
        )
        payload.extend(value)
        payload_offset += len(value)
    return header + bytes(directory) + bytes(payload)


def decode_sectioned_example(data: bytes) -> dict:
    """Independently decode framing and the fields used by the hand examples."""
    if len(data) < HEADER_SIZE:
        raise ValueError("truncated header")
    magic, major, minor, header_size, entry_size, reserved, count, offset = (
        struct.unpack_from("<8sHHIHHIQ", data)
    )
    if (
        magic != MAGIC
        or major != 1
        or minor != 0
        or header_size != HEADER_SIZE
        or entry_size != DIRECTORY_ENTRY_SIZE
        or reserved != 0
        or offset != HEADER_SIZE
    ):
        raise ValueError("noncanonical header")
    directory_end = offset + count * entry_size
    if directory_end > len(data):
        raise ValueError("truncated directory")
    previous_id = 0
    next_offset = directory_end
    decoded = {"format": "1.0", "sections": [], "program": {}}
    payloads = {}
    sizes = {section_id: size for section_id, size in SECTIONS.values()}
    names = {section_id: name for name, (section_id, _) in SECTIONS.items()}
    for index in range(count):
        entry_offset = offset + index * entry_size
        section_id, flags, record_size, payload_offset, payload_length = (
            struct.unpack_from("<HHIQQ", data, entry_offset)
        )
        if section_id <= previous_id or flags != MANDATORY_IDENTITY:
            raise ValueError("noncanonical directory ordering or flags")
        if payload_offset != next_offset or payload_offset + payload_length > len(data):
            raise ValueError("noncanonical payload extent")
        expected_size = sizes.get(section_id)
        if expected_size is None or record_size != expected_size:
            raise ValueError("unknown or mismatched section")
        if record_size and payload_length % record_size != 0:
            raise ValueError("partial fixed record")
        record_count = (
            payload_length // record_size
            if record_size
            else struct.unpack_from("<I", data, payload_offset)[0]
        )
        decoded["sections"].append(
            {
                "name": names[section_id],
                "offset": payload_offset,
                "length": payload_length,
                "records": record_count,
            }
        )
        payloads[names[section_id]] = data[
            payload_offset : payload_offset + payload_length
        ]
        previous_id = section_id
        next_offset = payload_offset + payload_length
    if next_offset != len(data):
        raise ValueError("trailing bytes")
    semantic = decoded["program"]
    semantic["module"] = dict(
        zip(
            ("semantic_major", "semantic_minor", "parameter_header_origin"),
            struct.unpack("<HHI", payloads["MODL"]),
        )
    )
    features = []
    for feature_id, feature_class, feature_reserved in struct.iter_unpack(
        "<HBB", payloads.get("FEAT", b"")
    ):
        if feature_reserved != 0 or feature_class not in (0, 1):
            raise ValueError("noncanonical feature record")
        if feature_id in (1, 2, 3, 4) and feature_class != 0:
            raise ValueError("known feature must use mandatory class")
        if feature_class == 0:
            features.append({"id": feature_id, "class": feature_class})
    semantic["features"] = features
    strings = []
    if "STRS" in payloads:
        value = payloads["STRS"]
        string_count = struct.unpack_from("<I", value)[0]
        data_start = 4 + string_count * 8
        for index in range(string_count):
            start, length = struct.unpack_from("<II", value, 4 + index * 8)
            strings.append(value[data_start + start : data_start + start + length].decode())
    semantic["strings"] = strings
    semantic["source_units"] = [
        dict(zip(("diagnostic_name", "byte_length"), row))
        for row in struct.iter_unpack("<II", payloads.get("SRCU", b""))
    ]
    semantic["types"] = [
        {
            "kind": row[0],
            "scalar_type": row[1],
            "element_start": row[3],
            "element_count": row[4],
        }
        for row in struct.iter_unpack("<BBHII", payloads.get("TYPE", b""))
    ]
    semantic["constants"] = [
        {
            "kind": row[0],
            "scalar_type": row[1],
            "element_start": row[3],
            "element_count": row[4],
            "payload": row[5],
        }
        for row in struct.iter_unpack("<BBHIIQ", payloads.get("CONS", b""))
    ]
    semantic["origins"] = [
        dict(
            zip(
                (
                    "source_unit",
                    "begin_offset",
                    "begin_line",
                    "begin_column",
                    "end_offset",
                    "end_line",
                    "end_column",
                ),
                row,
            )
        )
        for row in struct.iter_unpack("<IIIIIII", payloads.get("ORIG", b""))
    ]
    semantic["nodes"] = [
        {
            "kind": row[0],
            "cardinality_kind": row[1],
            "lift": row[2],
            "result_element_type": row[3],
            "result_type": row[4],
            "cardinality_length": row[5],
            "edge_start": row[6],
            "edge_count": row[7],
            "origin": row[8],
            "args": list(row[9:]),
        }
        for row in struct.iter_unpack(
            "<BBBBIIIIIIIIIIIII", payloads.get("NODE", b"")
        )
    ]
    semantic["ownership"] = [
        dict(zip(("owner", "release_kind", "release_index"), row))
        for row in struct.iter_unpack("<IB3xI", payloads.get("OWNR", b""))
    ]
    semantic["roots"] = [
        dict(zip(("node", "origin"), row))
        for row in struct.iter_unpack("<II", payloads.get("ROOT", b""))
    ]
    return decoded


def encode_canonical_json(program: dict) -> bytes:
    return (
        json.dumps(program, ensure_ascii=False, separators=(",", ":"), sort_keys=True)
        + "\n"
    ).encode("utf-8")


def _proto_varint(value: int) -> bytes:
    encoded = bytearray()
    while value >= 0x80:
        encoded.append((value & 0x7F) | 0x80)
        value >>= 7
    encoded.append(value)
    return bytes(encoded)


def _proto_uint(field: int, value: int) -> bytes:
    return _proto_varint(field << 3) + _proto_varint(value)


def _proto_bytes(field: int, value: bytes) -> bytes:
    return _proto_varint((field << 3) | 2) + _proto_varint(len(value)) + value


def _proto_message(values: list[int]) -> bytes:
    return b"".join(_proto_uint(index + 1, value) for index, value in enumerate(values))


def encode_protobuf_model(program: dict) -> bytes:
    """Encode the measured explicit-schema Protocol Buffers wire model."""
    module = program["module"]
    output = bytearray(
        _proto_bytes(
            1,
            _proto_message(
                [
                    module["semantic_major"],
                    module["semantic_minor"],
                    module["parameter_header_origin"],
                ]
            ),
        )
    )
    for feature in program["features"]:
        output.extend(_proto_bytes(2, _proto_message([feature["id"], feature["class"]])))
    for value in program["strings"]:
        output.extend(_proto_bytes(3, value.encode("utf-8")))
    table_fields = [
        ("source_units", 4, ["diagnostic_name", "byte_length"]),
        (
            "parameters",
            5,
            ["slot", "name", "scalar_type", "declaration_origin", "name_origin"],
        ),
        ("types", 6, ["kind", "scalar_type", "element_start", "element_count"]),
        (
            "constants",
            8,
            ["kind", "scalar_type", "element_start", "element_count", "payload"],
        ),
        (
            "constant_elements",
            9,
            ["scalar_type", "payload"],
        ),
        (
            "origins",
            10,
            [
                "source_unit",
                "begin_offset",
                "begin_line",
                "begin_column",
                "end_offset",
                "end_line",
                "end_column",
            ],
        ),
        (
            "edges",
            11,
            [
                "producer",
                "argument_position",
                "access",
                "cardinality_kind",
                "conversion",
                "ownership",
                "access_index",
                "cardinality_length",
                "origin",
            ],
        ),
        (
            "branches",
            13,
            ["node_start", "node_count", "root", "placeholder_origin", "origin"],
        ),
        (
            "ownership",
            15,
            ["owner", "release_kind", "release_index"],
        ),
        ("roots", 16, ["node", "origin"]),
    ]
    for table, field, keys in table_fields:
        for row in program[table]:
            output.extend(_proto_bytes(field, _proto_message([row[key] for key in keys])))
    for value in program["type_elements"]:
        output.extend(_proto_uint(7, value))
    for value in program["shape_checks"]:
        output.extend(_proto_uint(12, value))
    for row in program["nodes"]:
        values = [
            row["kind"],
            row["cardinality_kind"],
            row["lift"],
            row["result_element_type"],
            row["result_type"],
            row["cardinality_length"],
            row["edge_start"],
            row["edge_count"],
            row["origin"],
            *row["args"],
        ]
        output.extend(_proto_bytes(14, _proto_message(values)))
    return bytes(output)


def _complexity(function) -> tuple[int, int]:
    source = inspect.getsource(function)
    tree = ast.parse(source)
    branches = sum(
        isinstance(node, (ast.If, ast.For, ast.While, ast.Try, ast.Match))
        for node in ast.walk(tree)
    )
    lines = sum(
        bool(line.strip()) and not line.lstrip().startswith(('"""', "#"))
        for line in source.splitlines()
    )
    return lines, branches


def _hex_lines(data: bytes) -> str:
    return "\n".join(data[index : index + 16].hex(" ") for index in range(0, len(data), 16))


def measurement_markdown() -> str:
    fixtures = [
        ("empty", empty_program()),
        ("scalar-true", scalar_true_program()),
        ("1000-scalar-roots", scalar_true_program(1000)),
    ]
    rows = []
    for name, program in fixtures:
        rows.append(
            (
                name,
                len(encode_sectioned(program)),
                len(encode_canonical_json(program)),
                len(encode_protobuf_model(program)),
            )
        )
    decoder_lines, decoder_branches = _complexity(decode_sectioned_example)
    lines = [
        "# FWIR v1 encoding measurements",
        "",
        "Generated by `python tools/validation/fwir_v1_measurements.py report`",
        "with Python's standard library. Sizes are complete artifact bytes; the",
        "Protocol Buffers column is an explicit-message wire model using the",
        "official varint and length-delimited rules, without generated-code or",
        "runtime bytes.",
        "",
        "| Fixture | Sectioned binary | Canonical JSON | Protobuf wire model |",
        "| --- | ---: | ---: | ---: |",
    ]
    for name, binary_size, json_size, protobuf_size in rows:
        lines.append(f"| {name} | {binary_size} | {json_size} | {protobuf_size} |")
    lines.extend(
        [
            "",
            "The independent example decoder in the harness has",
            f"{decoder_lines} nonblank source lines and {decoder_branches} explicit",
            "branch/loop nodes (Python AST count). This deliberately excludes the",
            "semantic verifier shared by all choices and excludes parser/runtime",
            "internals for JSON and Protocol Buffers; those exclusions are why the",
            "comparison treats dependency and cross-language burden separately.",
            "",
        ]
    )
    return "\n".join(lines)


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("command", choices=("report", "examples"))
    parser.add_argument("--examples-dir", type=Path)
    arguments = parser.parse_args()
    if arguments.command == "report":
        print(measurement_markdown())
        return 0
    examples = {
        "empty": encode_sectioned(empty_program()),
        "scalar-true": encode_sectioned(scalar_true_program()),
    }
    for name, value in examples.items():
        decoded = decode_sectioned_example(value)
        expected = (
            empty_program()
            if name == "empty"
            else scalar_true_program()
        )
        comparable_keys = (
            "module",
            "features",
            "strings",
            "source_units",
            "types",
            "constants",
            "origins",
            "nodes",
            "ownership",
            "roots",
        )
        if any(decoded["program"][key] != expected[key] for key in comparable_keys):
            raise ValueError(f"{name} did not independently decode to its stated program")
        print(f"## {name} ({len(value)} bytes)")
        print(_hex_lines(value))
        print(json.dumps(decoded, separators=(",", ":"), sort_keys=True))
        if arguments.examples_dir:
            expected = arguments.examples_dir / f"fwir-v1-{name}.hex"
            file_bytes = bytes.fromhex(expected.read_text(encoding="utf-8"))
            if file_bytes != value:
                raise ValueError(f"{expected} differs from the specification model")
            decode_sectioned_example(file_bytes)
    hostile = empty_program()
    hostile["features"] = [{"id": 1, "class": 1}]
    try:
        decode_sectioned_example(encode_sectioned(hostile))
    except ValueError as error:
        if str(error) != "known feature must use mandatory class":
            raise
        print("hostile-known-feature-as-advisory: rejected")
    else:
        raise ValueError("known feature ID with advisory class was accepted")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
