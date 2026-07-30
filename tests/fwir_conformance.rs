use faraweave::{
    AllocationFailureInjection, EvaluationConfiguration, Feature, FwirDecodeErrorKind,
    FwirDecodeLimits, FwirEncodeOptions, FwirProducerMetadata, Invariant, OperationReference,
    Origin, OriginPosition, OriginSpan, RawProgramBuilder, RecordKind, ResourceErrorReason,
    ResourceEventKind, SourceUnit, Value, VerifyError, compile_source_to_verified_program,
    decode_fwir, encode_fwir, evaluate_source_with_arguments_and_observer,
    evaluate_verified_program_with_arguments, evaluate_verified_program_with_observer,
    inspect_fwir,
};
use std::collections::BTreeSet;
use std::fs;
use std::ops::Range;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CORPUS: &str = include_str!("fixtures/fwir-v1-corpus.tsv");
const TRACEABILITY: &str = include_str!("fixtures/fwir-v1-conformance.tsv");

#[derive(Clone, Copy, Debug)]
enum ExpectedDecodeError {
    InvalidHeader(&'static str),
    UnsupportedFormatVersion,
    NonCanonicalDirectory(&'static str),
    InvalidSectionLength,
    InvalidUtf8,
    NonCanonicalRecord(&'static str),
    MalformedProgram {
        invariant: Invariant,
        record: RecordKind,
        index: Option<u32>,
        field: &'static str,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ChangedByteRange {
    before: Range<usize>,
    after: Range<usize>,
}

#[derive(Debug)]
struct MutationCase {
    target_requirement: &'static str,
    name: &'static str,
    bytes: Vec<u8>,
    base_bytes: Vec<u8>,
    changed_ranges: Vec<ChangedByteRange>,
    expected: ExpectedDecodeError,
    offset: u64,
    section_id: Option<u16>,
    record_index: Option<u32>,
}

fn mutation(
    target_requirement: &'static str,
    name: &'static str,
    bytes: Vec<u8>,
    base_bytes: Vec<u8>,
    expected_diagnostic: (ExpectedDecodeError, usize, Option<u16>, Option<u32>),
) -> MutationCase {
    let (expected, offset, section_id, record_index) = expected_diagnostic;
    let changed_ranges = changed_byte_ranges(&base_bytes, &bytes);
    MutationCase {
        target_requirement,
        name,
        bytes,
        base_bytes,
        changed_ranges,
        expected,
        offset: offset as u64,
        section_id,
        record_index,
    }
}

fn changed_byte_ranges(before: &[u8], after: &[u8]) -> Vec<ChangedByteRange> {
    if before.len() != after.len() {
        let prefix = before
            .iter()
            .zip(after)
            .take_while(|(left, right)| left == right)
            .count();
        let suffix = before[prefix..]
            .iter()
            .rev()
            .zip(after[prefix..].iter().rev())
            .take_while(|(left, right)| left == right)
            .count();
        return vec![ChangedByteRange {
            before: prefix..before.len() - suffix,
            after: prefix..after.len() - suffix,
        }];
    }
    let mut ranges = Vec::new();
    let mut start = None;
    for (index, (left, right)) in before.iter().zip(after).enumerate() {
        match (left == right, start) {
            (false, None) => start = Some(index),
            (true, Some(begin)) => {
                ranges.push(ChangedByteRange {
                    before: begin..index,
                    after: begin..index,
                });
                start = None;
            }
            _ => {}
        }
    }
    if let Some(begin) = start {
        ranges.push(ChangedByteRange {
            before: begin..before.len(),
            after: begin..after.len(),
        });
    }
    ranges
}

fn assert_expected_decode_error(case: &MutationCase, error: &faraweave::FwirDecodeError) {
    assert_eq!(error.offset, case.offset, "{} offset", case.name);
    assert_eq!(error.section_id, case.section_id, "{} section", case.name);
    assert_eq!(
        error.record_index, case.record_index,
        "{} record",
        case.name
    );
    let kind_matches = match (case.expected, &error.kind) {
        (
            ExpectedDecodeError::InvalidHeader(expected),
            FwirDecodeErrorKind::InvalidHeader { field },
        ) => expected == *field,
        (
            ExpectedDecodeError::UnsupportedFormatVersion,
            FwirDecodeErrorKind::UnsupportedFormatVersion { .. },
        ) => true,
        (
            ExpectedDecodeError::NonCanonicalDirectory(expected),
            FwirDecodeErrorKind::NonCanonicalDirectory { field },
        ) => expected == *field,
        (ExpectedDecodeError::InvalidSectionLength, FwirDecodeErrorKind::InvalidSectionLength) => {
            true
        }
        (ExpectedDecodeError::InvalidUtf8, FwirDecodeErrorKind::InvalidUtf8) => true,
        (
            ExpectedDecodeError::NonCanonicalRecord(expected),
            FwirDecodeErrorKind::NonCanonicalRecord { field },
        ) => expected == *field,
        (
            ExpectedDecodeError::MalformedProgram {
                invariant,
                record,
                index,
                field,
            },
            FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(actual)),
        ) => {
            actual.invariant == invariant
                && actual.record == record
                && actual.index == index
                && actual.field == field
        }
        _ => false,
    };
    assert!(
        kind_matches,
        "{} category: expected {:?}, got {:?}",
        case.name, case.expected, error.kind
    );
}

fn example_bytes(name: &str) -> Vec<u8> {
    let text = match name {
        "empty" => include_str!("../spec/examples/fwir-v1-empty.hex"),
        "scalar-true" => include_str!("../spec/examples/fwir-v1-scalar-true.hex"),
        "complete" => include_str!("../spec/examples/fwir-v1-complete.hex"),
        _ => panic!("unknown canonical artifact {name}"),
    };
    text.split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16).expect("canonical hex byte"))
        .collect()
}

fn read_u16(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn read_u32(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
    ])
}

fn read_u64(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes([
        bytes[offset],
        bytes[offset + 1],
        bytes[offset + 2],
        bytes[offset + 3],
        bytes[offset + 4],
        bytes[offset + 5],
        bytes[offset + 6],
        bytes[offset + 7],
    ])
}

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
}

fn put_u64(bytes: &mut [u8], offset: usize, value: u64) {
    bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
}

fn section(bytes: &[u8], wanted: u16) -> (usize, usize, usize) {
    for index in 0..read_u32(bytes, 20) as usize {
        let entry = 32 + index * 24;
        if read_u16(bytes, entry) == wanted {
            return (
                entry,
                read_u64(bytes, entry + 8) as usize,
                read_u64(bytes, entry + 16) as usize,
            );
        }
    }
    panic!("canonical artifact lacks section {wanted}");
}

fn record_with_tag(bytes: &[u8], section_id: u16, size: usize, tag: u8) -> usize {
    let (_, offset, length) = section(bytes, section_id);
    bytes[offset..offset + length]
        .chunks_exact(size)
        .position(|record| record[0] == tag)
        .map(|index| offset + index * size)
        .unwrap_or_else(|| panic!("section {section_id} lacks tag {tag}"))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    bytes.iter().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
        (hash ^ u64::from(*byte)).wrapping_mul(0x0000_0100_0000_01b3)
    })
}

fn empty_with_extension(minor: u16, flags: u16) -> Vec<u8> {
    let empty = example_bytes("empty");
    let mut bytes = empty[..32].to_vec();
    put_u16(&mut bytes, 10, minor);
    put_u32(&mut bytes, 20, 2);
    let mut module_entry = empty[32..56].to_vec();
    put_u64(&mut module_entry, 8, 80);
    bytes.extend_from_slice(&module_entry);
    bytes.extend_from_slice(&100_u16.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&0_u32.to_le_bytes());
    bytes.extend_from_slice(&88_u64.to_le_bytes());
    bytes.extend_from_slice(&1_u64.to_le_bytes());
    bytes.extend_from_slice(&empty[56..64]);
    bytes.push(0xa5);
    bytes
}

fn empty_with_feature(id: u16, class: u8) -> Vec<u8> {
    let empty = example_bytes("empty");
    let mut bytes = empty[..32].to_vec();
    put_u32(&mut bytes, 20, 2);
    let mut module_entry = empty[32..56].to_vec();
    put_u64(&mut module_entry, 8, 80);
    bytes.extend_from_slice(&module_entry);
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&3_u16.to_le_bytes());
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&88_u64.to_le_bytes());
    bytes.extend_from_slice(&4_u64.to_le_bytes());
    bytes.extend_from_slice(&empty[56..64]);
    bytes.extend_from_slice(&id.to_le_bytes());
    bytes.push(class);
    bytes.push(0);
    if id == Feature::BackendNativeMathV1.numeric() {
        put_u16(&mut bytes, 82, 1);
    }
    bytes
}

fn empty_with_duplicate_module_section() -> Vec<u8> {
    let empty = example_bytes("empty");
    let mut bytes = empty[..32].to_vec();
    put_u32(&mut bytes, 20, 2);
    let mut first = empty[32..56].to_vec();
    put_u64(&mut first, 8, 80);
    let mut second = first.clone();
    put_u64(&mut second, 8, 88);
    bytes.extend_from_slice(&first);
    bytes.extend_from_slice(&second);
    bytes.extend_from_slice(&empty[56..64]);
    bytes.extend_from_slice(&empty[56..64]);
    bytes
}

fn string_section_payload(strings: &[&str]) -> Vec<u8> {
    let mut payload = Vec::new();
    payload.extend_from_slice(&(strings.len() as u32).to_le_bytes());
    let mut offset = 0_u32;
    for value in strings {
        payload.extend_from_slice(&offset.to_le_bytes());
        payload.extend_from_slice(&(value.len() as u32).to_le_bytes());
        offset += value.len() as u32;
    }
    for value in strings {
        payload.extend_from_slice(value.as_bytes());
    }
    payload
}

fn replace_section_payload(base: &[u8], section_id: u16, payload: &[u8]) -> Vec<u8> {
    let (entry, offset, length) = section(base, section_id);
    let mut bytes = base.to_vec();
    bytes.splice(offset..offset + length, payload.iter().copied());
    put_u64(&mut bytes, entry + 16, payload.len() as u64);
    let delta = payload.len() as i64 - length as i64;
    for index in 0..read_u32(&bytes, 20) as usize {
        let directory_entry = 32 + index * 24;
        let old_offset = read_u64(&bytes, directory_entry + 8);
        if old_offset > offset as u64 {
            put_u64(
                &mut bytes,
                directory_entry + 8,
                (old_offset as i64 + delta) as u64,
            );
        }
    }
    bytes
}

fn producer_artifact() -> Vec<u8> {
    let empty = decode_fwir(&example_bytes("empty"), &FwirDecodeLimits::default())
        .expect("canonical empty producer input");
    encode_fwir(
        &empty,
        &FwirEncodeOptions {
            producer_metadata: Some(FwirProducerMetadata::Sha256([0xa5; 32])),
        },
    )
    .expect("canonical producer artifact")
}

fn operation_reference_artifact() -> Vec<u8> {
    let mut builder = RawProgramBuilder::new();
    builder
        .push_feature(Feature::OperationReferences.numeric())
        .expect("operation-reference feature");
    let source = builder
        .push_source_unit(SourceUnit {
            diagnostic_name: "reference.fw".to_owned(),
            byte_length: 4,
        })
        .expect("operation-reference source");
    let origin = builder
        .push_origin(Origin {
            source_unit: source,
            span: OriginSpan {
                begin: OriginPosition {
                    offset: 1,
                    line: 1,
                    column: 1,
                },
                end: OriginPosition {
                    offset: 5,
                    line: 1,
                    column: 5,
                },
            },
        })
        .expect("operation-reference origin");
    builder
        .push_operation_reference(OperationReference {
            primitive_id: 5,
            signature_id: 9,
            implementation_id: 9,
            origin,
        })
        .expect("operation-reference record");
    let program = builder
        .finish()
        .expect("operation-reference program")
        .verify()
        .expect("valid operation-reference program");
    encode_fwir(&program, &FwirEncodeOptions::default())
        .expect("canonical operation-reference artifact")
}

fn producer_mutations() -> Vec<(&'static str, Vec<u8>)> {
    let producer = producer_artifact();
    let (entry, offset, length) = section(&producer, 32769);
    let name_length = read_u32(&producer, offset) as usize;
    let version_length_offset = offset + 4 + name_length;
    let version_length = read_u32(&producer, version_length_offset) as usize;
    let digest_header = version_length_offset + 4 + version_length;
    let mutate_byte = |mutation_offset: usize, value: u8| {
        let mut bytes = producer.clone();
        bytes[mutation_offset] = value;
        bytes
    };
    let mut truncated_digest = producer.clone();
    put_u64(&mut truncated_digest, entry + 16, (length - 1) as u64);
    truncated_digest.pop();
    vec![
        ("producer-name", mutate_byte(offset + 4, b'X')),
        (
            "producer-version",
            mutate_byte(version_length_offset + 4, b'v'),
        ),
        ("producer-digest-algorithm", mutate_byte(digest_header, 2)),
        ("producer-digest-length", mutate_byte(digest_header + 2, 31)),
        ("producer-digest-truncated", truncated_digest),
    ]
}

#[test]
fn canonical_corpus_manifest_is_exact_roundtrippable_and_host_neutral() {
    let mut names = BTreeSet::new();
    for (line_index, line) in CORPUS.lines().enumerate().skip(1) {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 5, "corpus row {}", line_index + 1);
        let name = columns[0];
        assert!(names.insert(name), "duplicate corpus artifact {name}");
        assert_eq!(
            columns[1],
            format!("spec/examples/fwir-v1-{name}.hex"),
            "{name}"
        );
        let bytes = example_bytes(name);
        assert_eq!(bytes.len().to_string(), columns[2], "{name}");
        assert_eq!(format!("{:016x}", fnv1a64(&bytes)), columns[3], "{name}");
        assert!(bytes.starts_with(b"FWIR\r\n\x1a\n"), "{name}");
        let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("canonical decode");
        assert_eq!(
            encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("canonical reencode"),
            bytes,
            "{name}"
        );
        let inspection = inspect_fwir(&decoded).expect("canonical inspection");
        assert!(inspection.ends_with('\n'));
        for source in &decoded.as_raw().source_units {
            assert!(!source.diagnostic_name.contains('\\'), "{name}");
            assert!(!source.diagnostic_name.contains(':'), "{name}");
            assert!(!source.diagnostic_name.starts_with('/'), "{name}");
        }
        assert!(
            !bytes
                .windows(19)
                .any(|window| window[4] == b'-' && window[7] == b'-' && window[10] == b'T'),
            "{name} contains a timestamp-like payload"
        );
    }
    assert_eq!(names, BTreeSet::from(["complete", "empty", "scalar-true"]));
}

#[test]
fn immutable_binding_artifact_versions_tags_and_provenance_are_conformant() {
    let program = compile_source_to_verified_program(
        "let value = iota[3]\nsum[value]\n",
        "binding-conformance.faraweave",
    )
    .expect("compile immutable binding");
    let bytes =
        encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode immutable binding");
    assert_eq!(read_u16(&bytes, 8), 1);
    assert_eq!(read_u16(&bytes, 10), 3);
    let decoded =
        decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode immutable binding");
    assert_eq!(decoded.as_raw().module.semantic_minor, 3);
    assert!(
        decoded
            .as_raw()
            .features
            .contains(&Feature::ImmutableBindings.numeric())
    );
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("canonical reencode"),
        bytes
    );
    let inspection = inspect_fwir(&decoded).expect("inspect immutable binding");
    assert!(inspection.contains("id=9"));
    assert!(inspection.contains("Binding {"));
    assert!(inspection.contains("BindingBorrowWhole"));

    let (_, features, feature_length) = section(&bytes, 2);
    let feature_record = (0..feature_length / 4)
        .find(|index| {
            read_u16(&bytes, features + index * 4) == Feature::ImmutableBindings.numeric()
        })
        .map(|index| features + index * 4)
        .expect("feature 9 record");
    let binding_node = record_with_tag(&bytes, 14, 56, 8);
    let (_, edges, edge_length) = section(&bytes, 11);
    let binding_edge = (0..edge_length / 24)
        .find(|index| matches!(bytes[edges + index * 24 + 8], 6..=8))
        .map(|index| edges + index * 24)
        .expect("binding access");

    let mut missing_feature = bytes.clone();
    put_u16(
        &mut missing_feature,
        feature_record,
        Feature::ConnectedApplicationBindings.numeric(),
    );
    let missing_feature_error =
        decode_fwir(&missing_feature, &FwirDecodeLimits::default()).expect_err("missing feature");
    assert!(
        matches!(
            &missing_feature_error,
            faraweave::FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord { field: "access" },
                ..
            }
        ),
        "{missing_feature_error:?}"
    );

    let mut old_physical = bytes.clone();
    put_u16(&mut old_physical, 10, 2);
    assert!(matches!(
        decode_fwir(&old_physical, &FwirDecodeLimits::default()),
        Err(faraweave::FwirDecodeError {
            kind: FwirDecodeErrorKind::NonCanonicalRecord {
                field: "feature_format_minor"
            },
            ..
        })
    ));

    let mut old_semantics = bytes.clone();
    let (_, module, _) = section(&old_semantics, 1);
    put_u16(&mut old_semantics, module + 2, 2);
    assert!(matches!(
        decode_fwir(&old_semantics, &FwirDecodeLimits::default()),
        Err(faraweave::FwirDecodeError {
            kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                faraweave::MalformedProgram {
                    invariant: Invariant::UnsupportedVersion,
                    ..
                }
            )),
            ..
        })
    ));

    let mut invalid_node = bytes.clone();
    invalid_node[binding_node] = 11;
    assert!(matches!(
        decode_fwir(&invalid_node, &FwirDecodeLimits::default()),
        Err(faraweave::FwirDecodeError {
            kind: FwirDecodeErrorKind::NonCanonicalRecord { field: "kind" },
            ..
        })
    ));

    let mut invalid_access = bytes.clone();
    invalid_access[binding_edge + 8] = 9;
    assert!(matches!(
        decode_fwir(&invalid_access, &FwirDecodeLimits::default()),
        Err(faraweave::FwirDecodeError {
            kind: FwirDecodeErrorKind::NonCanonicalRecord { field: "access" },
            ..
        })
    ));

    let mut invalid_provenance = bytes;
    put_u32(&mut invalid_provenance, binding_node + 28, u32::MAX);
    assert!(matches!(
        decode_fwir(&invalid_provenance, &FwirDecodeLimits::default()),
        Err(faraweave::FwirDecodeError {
            kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                faraweave::MalformedProgram {
                    invariant: Invariant::IndexOutOfBounds,
                    record: RecordKind::Node,
                    field: "name_origin",
                    ..
                }
            )),
            ..
        })
    ));
}

#[test]
fn operation_reference_artifact_is_versioned_canonical_and_roundtrippable() {
    let bytes = operation_reference_artifact();
    assert_eq!(read_u16(&bytes, 8), 1);
    assert_eq!(read_u16(&bytes, 10), 1);
    let (_, module, module_length) = section(&bytes, 1);
    assert_eq!(module_length, 8);
    assert_eq!(read_u16(&bytes, module), 1);
    assert_eq!(read_u16(&bytes, module + 2), 1);
    let (_, features, feature_length) = section(&bytes, 2);
    assert_eq!(feature_length, 4);
    assert_eq!(&bytes[features..features + feature_length], &[6, 0, 0, 0]);

    let (entry, references, reference_length) = section(&bytes, 18);
    assert_eq!(read_u16(&bytes, entry), 18);
    assert_eq!(read_u16(&bytes, entry + 2), 3);
    assert_eq!(read_u32(&bytes, entry + 4), 16);
    assert_eq!(reference_length, 16);
    assert_eq!(
        &bytes[references..references + reference_length],
        &[5, 0, 9, 0, 9, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]
    );

    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("canonical OPRF decode");
    assert_eq!(
        decoded.as_raw().features,
        [Feature::OperationReferences.numeric()]
    );
    assert_eq!(
        decoded.as_raw().operation_references,
        [OperationReference {
            primitive_id: 5,
            signature_id: 9,
            implementation_id: 9,
            origin: faraweave::OriginIndex(0),
        }]
    );
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("canonical OPRF reencode"),
        bytes
    );
}

fn named_mutations() -> Vec<(&'static str, Vec<u8>)> {
    let complete = example_bytes("complete");
    let empty = example_bytes("empty");
    let (_, module, _) = section(&complete, 1);
    let (_, features, _) = section(&complete, 2);
    let (_, strings, _) = section(&complete, 3);
    let string_count = read_u32(&complete, strings) as usize;
    let string_data = strings + 4 + string_count * 8;
    let (_, sources, _) = section(&complete, 4);
    let (_, parameters, _) = section(&complete, 5);
    let scalar_type = record_with_tag(&complete, 6, 12, 1);
    let (_, type_elements, _) = section(&complete, 7);
    let scalar_constant = record_with_tag(&complete, 8, 20, 1);
    let bool_constant = complete[section(&complete, 8).1..]
        .chunks_exact(20)
        .position(|record| record[0] == 1 && record[1] == 1)
        .map(|index| section(&complete, 8).1 + index * 20)
        .expect("scalar Bool constant");
    let (_, constant_elements, _) = section(&complete, 9);
    let (_, origins, _) = section(&complete, 10);
    let (_, edges, edge_length) = section(&complete, 11);
    let (_, shape_checks, _) = section(&complete, 12);
    let (_, branches, _) = section(&complete, 13);
    let selected_apply = record_with_tag(&complete, 14, 56, 4);
    let constant_node = record_with_tag(&complete, 14, 56, 1);
    let (_, ownership, _) = section(&complete, 15);
    let (_, roots, _) = section(&complete, 16);

    let mutate = |base: &[u8], offset: usize, replacement: &[u8]| {
        let mut bytes = base.to_vec();
        bytes[offset..offset + replacement.len()].copy_from_slice(replacement);
        bytes
    };
    let mut cases = vec![
        ("header-magic", mutate(&empty, 0, &[0])),
        ("format-major", mutate(&empty, 8, &2_u16.to_le_bytes())),
        ("format-minor-current-extension", empty_with_extension(0, 0)),
        ("header-size", mutate(&empty, 12, &31_u32.to_le_bytes())),
        (
            "directory-entry-size",
            mutate(&empty, 16, &23_u16.to_le_bytes()),
        ),
        ("header-reserved", mutate(&empty, 18, &1_u16.to_le_bytes())),
        (
            "directory-offset",
            mutate(&empty, 24, &33_u64.to_le_bytes()),
        ),
        ("directory-id", mutate(&empty, 32, &0_u16.to_le_bytes())),
        ("directory-flags", mutate(&empty, 34, &7_u16.to_le_bytes())),
        (
            "directory-record-size",
            mutate(&empty, 36, &9_u32.to_le_bytes()),
        ),
        (
            "directory-payload-offset",
            mutate(&empty, 40, &57_u64.to_le_bytes()),
        ),
        (
            "directory-payload-length",
            mutate(&empty, 48, &7_u64.to_le_bytes()),
        ),
        (
            "directory-known-section-duplicate",
            empty_with_duplicate_module_section(),
        ),
        (
            "module-semantic-major",
            mutate(&complete, section(&complete, 1).1, &2_u16.to_le_bytes()),
        ),
        (
            "module-semantic-minor",
            mutate(
                &complete,
                section(&complete, 1).1 + 2,
                &u16::MAX.to_le_bytes(),
            ),
        ),
        (
            "module-parameter-header-origin",
            mutate(&complete, module + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "feature-zero",
            mutate(&complete, features, &0_u16.to_le_bytes()),
        ),
        ("feature-reserved", mutate(&complete, features + 3, &[1])),
        (
            "string-offset",
            mutate(&complete, strings + 4, &1_u32.to_le_bytes()),
        ),
        (
            "string-length",
            mutate(&complete, strings + 8, &u32::MAX.to_le_bytes()),
        ),
        ("string-utf8", mutate(&complete, string_data, &[0xff])),
        (
            "string-duplicate",
            replace_section_payload(&complete, 3, &string_section_payload(&["x", "x"])),
        ),
        ("string-out-of-order", {
            let mut bytes = complete.clone();
            bytes[string_data..string_data + 8].copy_from_slice(b"zzzzzzzz");
            bytes[string_data + 8] = b'a';
            bytes
        }),
        (
            "string-unused",
            mutate(&complete, parameters + 4, &0_u32.to_le_bytes()),
        ),
        (
            "source-name-index",
            mutate(&complete, sources, &u32::MAX.to_le_bytes()),
        ),
        (
            "source-byte-length",
            mutate(&complete, sources + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "parameter-slot",
            mutate(&complete, parameters, &u32::MAX.to_le_bytes()),
        ),
        (
            "parameter-name-index",
            mutate(&complete, parameters + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "parameter-scalar-type",
            mutate(&complete, parameters + 8, &[0]),
        ),
        (
            "parameter-reserved",
            mutate(&complete, parameters + 9, &[1]),
        ),
        (
            "parameter-declaration-origin",
            mutate(&complete, parameters + 12, &u32::MAX.to_le_bytes()),
        ),
        (
            "parameter-name-origin",
            mutate(&complete, parameters + 16, &u32::MAX.to_le_bytes()),
        ),
        ("type-kind", mutate(&complete, scalar_type, &[0])),
        ("type-scalar-type", mutate(&complete, scalar_type + 1, &[0])),
        (
            "type-reserved",
            mutate(&complete, scalar_type + 2, &1_u16.to_le_bytes()),
        ),
        (
            "type-element-start",
            mutate(&complete, scalar_type + 4, &1_u32.to_le_bytes()),
        ),
        (
            "type-element-count",
            mutate(&complete, scalar_type + 8, &1_u32.to_le_bytes()),
        ),
        (
            "type-element-index",
            mutate(&complete, type_elements, &u32::MAX.to_le_bytes()),
        ),
        ("constant-kind", mutate(&complete, scalar_constant, &[0])),
        (
            "constant-scalar-type",
            mutate(&complete, scalar_constant + 1, &[0]),
        ),
        (
            "constant-reserved",
            mutate(&complete, scalar_constant + 2, &1_u16.to_le_bytes()),
        ),
        (
            "constant-payload",
            mutate(&complete, bool_constant + 12, &2_u64.to_le_bytes()),
        ),
        (
            "constant-element-start",
            mutate(&complete, scalar_constant + 4, &1_u32.to_le_bytes()),
        ),
        (
            "constant-element-count",
            mutate(&complete, scalar_constant + 8, &1_u32.to_le_bytes()),
        ),
        (
            "constant-element-scalar-type",
            mutate(&complete, constant_elements, &[0]),
        ),
        (
            "constant-element-reserved",
            mutate(&complete, constant_elements + 1, &[1]),
        ),
        ("constant-element-payload", {
            let mut bytes = mutate(&complete, constant_elements, &[1]);
            put_u64(&mut bytes, constant_elements + 4, 2);
            bytes
        }),
        (
            "provenance-source",
            mutate(&complete, origins, &u32::MAX.to_le_bytes()),
        ),
        (
            "provenance-origin",
            mutate(&complete, origins + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "origin-begin-line",
            mutate(&complete, origins + 8, &0_u32.to_le_bytes()),
        ),
        (
            "origin-begin-column",
            mutate(&complete, origins + 12, &0_u32.to_le_bytes()),
        ),
        (
            "origin-end-offset",
            mutate(&complete, origins + 16, &0_u32.to_le_bytes()),
        ),
        (
            "origin-end-line",
            mutate(&complete, origins + 20, &0_u32.to_le_bytes()),
        ),
        (
            "origin-end-column",
            mutate(&complete, origins + 24, &0_u32.to_le_bytes()),
        ),
        (
            "graph-edge-producer",
            mutate(&complete, edges, &u32::MAX.to_le_bytes()),
        ),
        (
            "graph-edge-position",
            mutate(&complete, edges + 4, &u32::MAX.to_le_bytes()),
        ),
        ("edge-access", mutate(&complete, edges + 8, &[0])),
        ("edge-cardinality", mutate(&complete, edges + 9, &[0xff])),
        ("edge-conversion", mutate(&complete, edges + 10, &[0])),
        ("edge-ownership", mutate(&complete, edges + 11, &[0])),
        (
            "edge-origin",
            mutate(&complete, edges + 20, &u32::MAX.to_le_bytes()),
        ),
        (
            "shape-check-position",
            mutate(&complete, shape_checks, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-node-start",
            mutate(&complete, branches, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-node-count",
            mutate(&complete, branches + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-root",
            mutate(&complete, branches + 8, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-placeholder-origin",
            mutate(&complete, branches + 12, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-origin",
            mutate(&complete, branches + 16, &u32::MAX.to_le_bytes()),
        ),
        ("node-kind", mutate(&complete, selected_apply, &[0])),
        (
            "node-cardinality",
            mutate(&complete, selected_apply + 1, &[0xff]),
        ),
        ("node-lift", mutate(&complete, selected_apply + 2, &[0])),
        (
            "node-result-scalar",
            mutate(&complete, selected_apply + 3, &[0]),
        ),
        (
            "node-result-type",
            mutate(&complete, selected_apply + 4, &u32::MAX.to_le_bytes()),
        ),
        (
            "node-cardinality-length",
            mutate(&complete, selected_apply + 8, &1_u32.to_le_bytes()),
        ),
        (
            "node-edge-start",
            mutate(&complete, selected_apply + 12, &u32::MAX.to_le_bytes()),
        ),
        (
            "node-edge-count",
            mutate(&complete, selected_apply + 16, &u32::MAX.to_le_bytes()),
        ),
        (
            "node-origin",
            mutate(&complete, selected_apply + 20, &u32::MAX.to_le_bytes()),
        ),
        (
            "node-unused-variant",
            mutate(&complete, constant_node + 52, &1_u32.to_le_bytes()),
        ),
        (
            "primitive-id",
            mutate(&complete, selected_apply + 24, &u16::MAX.to_le_bytes()),
        ),
        (
            "signature-id",
            mutate(&complete, selected_apply + 28, &u16::MAX.to_le_bytes()),
        ),
        (
            "implementation-id",
            mutate(&complete, selected_apply + 32, &u16::MAX.to_le_bytes()),
        ),
        (
            "ownership-owner",
            mutate(&complete, ownership, &u32::MAX.to_le_bytes()),
        ),
        (
            "ownership-release-kind",
            mutate(&complete, ownership + 4, &[0]),
        ),
        ("ownership-reserved", mutate(&complete, ownership + 5, &[1])),
        (
            "ownership-release-index",
            mutate(&complete, ownership + 8, &u32::MAX.to_le_bytes()),
        ),
        (
            "root-node",
            mutate(&complete, roots, &u32::MAX.to_le_bytes()),
        ),
        (
            "root-origin",
            mutate(&complete, roots + 4, &u32::MAX.to_le_bytes()),
        ),
    ];

    let mut combined = complete.clone();
    put_u16(&mut combined, 34, 7);
    combined[selected_apply] = 0;
    cases.push(("combined-directory-record-error", combined));

    let edge_records = &complete[edges..edges + edge_length];
    let whole = edge_records
        .chunks_exact(24)
        .position(|record| record[8] != 2)
        .map(|index| edges + index * 24)
        .expect("whole-value edge");
    cases.push((
        "graph-edge-access-index",
        mutate(&complete, whole + 12, &1_u32.to_le_bytes()),
    ));
    let non_static_vector = edge_records
        .chunks_exact(24)
        .position(|record| record[9] != 2)
        .map(|index| edges + index * 24)
        .expect("non-static-vector edge");
    cases.push((
        "graph-edge-cardinality-length",
        mutate(&complete, non_static_vector + 16, &1_u32.to_le_bytes()),
    ));
    let mut trailing = empty;
    trailing.push(0);
    cases.push(("trailing-byte", trailing));
    cases
}

fn mutation_target(name: &str) -> &'static str {
    match name {
        "header-magic" => "header.magic",
        "format-major" => "header.format_major",
        "format-minor-current-extension" => "header.format_minor",
        "header-size" => "header.header_size",
        "directory-entry-size" => "header.directory_entry_size",
        "header-reserved" => "header.reserved",
        "directory-offset" => "header.directory_offset",
        "directory-id" => "directory.section_id_order",
        "directory-flags" => "directory.flags",
        "directory-record-size" => "directory.record_size",
        "directory-payload-offset" => "directory.payload_offset",
        "directory-payload-length" => "directory.payload_length",
        "directory-known-section-duplicate" => "directory.known_section_uniqueness",
        "module-semantic-major" => "modl.semantic_major",
        "module-semantic-minor" => "modl.semantic_minor",
        "module-parameter-header-origin" => "modl.parameter_header_origin",
        "feature-zero" => "feat.id",
        "feature-reserved" => "feat.reserved",
        "string-offset" => "strs.descriptor_offset",
        "string-length" => "strs.descriptor_length",
        "string-utf8" => "strs.utf8",
        "string-duplicate" => "strs.unique",
        "string-out-of-order" => "strs.ordered",
        "string-unused" => "strs.used",
        "source-name-index" => "srcu.diagnostic_name",
        "source-byte-length" => "srcu.byte_length",
        "parameter-slot" => "parm.slot",
        "parameter-name-index" => "parm.name",
        "parameter-scalar-type" => "parm.scalar_type",
        "parameter-reserved" => "parm.reserved",
        "parameter-declaration-origin" => "parm.declaration_origin",
        "parameter-name-origin" => "parm.name_origin",
        "type-kind" => "type.kind",
        "type-scalar-type" => "type.scalar_type",
        "type-reserved" => "type.reserved",
        "type-element-start" => "type.element_start",
        "type-element-count" => "type.element_count",
        "type-element-index" => "tyel.type_index",
        "constant-kind" => "cons.kind",
        "constant-scalar-type" => "cons.scalar_type",
        "constant-reserved" => "cons.reserved",
        "constant-element-start" => "cons.element_start",
        "constant-element-count" => "cons.element_count",
        "constant-payload" => "cons.payload",
        "constant-element-scalar-type" => "coel.scalar_type",
        "constant-element-reserved" => "coel.reserved",
        "constant-element-payload" => "coel.payload",
        "provenance-source" => "orig.source_unit",
        "provenance-origin" => "orig.begin_offset",
        "origin-begin-line" => "orig.begin_line",
        "origin-begin-column" => "orig.begin_column",
        "origin-end-offset" => "orig.end_offset",
        "origin-end-line" => "orig.end_line",
        "origin-end-column" => "orig.end_column",
        "graph-edge-producer" => "edge.producer",
        "graph-edge-position" => "edge.argument_position",
        "edge-access" => "edge.access",
        "edge-cardinality" => "edge.cardinality_kind",
        "edge-conversion" => "edge.conversion",
        "edge-ownership" => "edge.ownership",
        "graph-edge-access-index" => "edge.access_index",
        "graph-edge-cardinality-length" => "edge.cardinality_length",
        "edge-origin" => "edge.origin",
        "shape-check-position" => "shck.argument_position",
        "branch-node-start" => "bran.node_start",
        "branch-node-count" => "bran.node_count",
        "branch-root" => "bran.root",
        "branch-placeholder-origin" => "bran.placeholder_origin",
        "branch-origin" => "bran.origin",
        "node-kind" => "node.kind",
        "node-cardinality" => "node.cardinality_kind",
        "node-lift" => "node.lift_mode",
        "node-result-scalar" => "node.result_element_scalar_type",
        "node-result-type" => "node.result_type",
        "node-cardinality-length" => "node.cardinality_length",
        "node-edge-start" => "node.edge_start",
        "node-edge-count" => "node.edge_count",
        "node-origin" => "node.origin",
        "node-unused-variant" => "node.variant_words",
        "primitive-id" => "node.primitive_id",
        "signature-id" => "node.signature_id",
        "implementation-id" => "node.implementation_id",
        "ownership-owner" => "ownr.owner",
        "ownership-release-kind" => "ownr.release_kind",
        "ownership-reserved" => "ownr.reserved",
        "ownership-release-index" => "ownr.release_index",
        "root-node" => "root.node",
        "root-origin" => "root.origin",
        "trailing-byte" => "directory.contiguous_extents",
        "combined-directory-record-error" => "decoder.deterministic_first_error",
        "producer-name" => "prod.name",
        "producer-version" => "prod.version",
        "producer-digest-algorithm" => "prod.digest_algorithm",
        "producer-digest-length" => "prod.digest_length",
        "producer-digest-truncated" => "prod.digest",
        "operation-reference-feature-missing" => "oprf.feature_required",
        "operation-reference-format-minor" => "oprf.format_minor",
        "operation-reference-semantic-minor" => "oprf.semantic_minor",
        "operation-reference-record-size" => "oprf.record_size",
        "operation-reference-record-count" => "oprf.record_count",
        "operation-reference-reserved16" => "oprf.reserved16",
        "operation-reference-reserved32" => "oprf.reserved32",
        "operation-reference-primitive-mismatch" => "oprf.primitive_id",
        "operation-reference-signature-mismatch" => "oprf.signature_id",
        "operation-reference-implementation-mismatch" => "oprf.implementation_id",
        "operation-reference-structural" => "oprf.structural_behavior",
        "operation-reference-origin" => "oprf.origin",
        _ => panic!("mutation {name} has no explicit target requirement"),
    }
}

fn operation_reference_mutations() -> Vec<MutationCase> {
    let canonical = operation_reference_artifact();
    let (_, features, _) = section(&canonical, 2);
    let (_, module, _) = section(&canonical, 1);
    let (reference_entry, references, _) = section(&canonical, 18);
    let directory_index = u32::try_from((reference_entry - 32) / 24).ok();
    let mutate_u16 = |offset: usize, value: u16| {
        let mut bytes = canonical.clone();
        put_u16(&mut bytes, offset, value);
        bytes
    };
    let mutate_u32 = |offset: usize, value: u32| {
        let mut bytes = canonical.clone();
        put_u32(&mut bytes, offset, value);
        bytes
    };
    let mutate_u64 = |offset: usize, value: u64| {
        let mut bytes = canonical.clone();
        put_u64(&mut bytes, offset, value);
        bytes
    };
    let semantic_error = |invariant, field| ExpectedDecodeError::MalformedProgram {
        invariant,
        record: RecordKind::Module,
        index: None,
        field,
    };
    let mut structural = canonical.clone();
    put_u16(&mut structural, references, 19);
    put_u16(&mut structural, references + 2, 34);
    put_u16(&mut structural, references + 4, 34);

    [
        (
            "operation-reference-feature-missing",
            mutate_u16(features, Feature::BackendNativeMathV1.numeric()),
            (
                semantic_error(Invariant::MissingFeature, "operation_references"),
                0,
                None,
                None,
            ),
        ),
        (
            "operation-reference-format-minor",
            mutate_u16(10, 0),
            (
                ExpectedDecodeError::NonCanonicalRecord("feature_format_minor"),
                features,
                Some(2),
                Some(0),
            ),
        ),
        (
            "operation-reference-semantic-minor",
            mutate_u16(module + 2, 0),
            (
                semantic_error(
                    Invariant::UnsupportedVersion,
                    "operation_references_version",
                ),
                0,
                None,
                None,
            ),
        ),
        (
            "operation-reference-record-size",
            mutate_u32(reference_entry + 4, 15),
            (
                ExpectedDecodeError::NonCanonicalDirectory("record_size"),
                reference_entry + 4,
                Some(18),
                directory_index,
            ),
        ),
        (
            "operation-reference-record-count",
            mutate_u64(reference_entry + 16, 8),
            (
                ExpectedDecodeError::InvalidSectionLength,
                reference_entry + 16,
                Some(18),
                directory_index,
            ),
        ),
        (
            "operation-reference-reserved16",
            mutate_u16(references + 6, 1),
            (
                ExpectedDecodeError::NonCanonicalRecord("reserved"),
                references + 6,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-reserved32",
            mutate_u32(references + 12, 1),
            (
                ExpectedDecodeError::NonCanonicalRecord("reserved"),
                references + 12,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-primitive-mismatch",
            mutate_u16(references, 6),
            (
                ExpectedDecodeError::NonCanonicalRecord("semantic_id"),
                references,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-signature-mismatch",
            mutate_u16(references + 2, 10),
            (
                ExpectedDecodeError::NonCanonicalRecord("semantic_id"),
                references,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-implementation-mismatch",
            mutate_u16(references + 4, 35),
            (
                ExpectedDecodeError::NonCanonicalRecord("semantic_id"),
                references,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-structural",
            structural,
            (
                ExpectedDecodeError::NonCanonicalRecord("semantic_id"),
                references,
                Some(18),
                Some(0),
            ),
        ),
        (
            "operation-reference-origin",
            mutate_u32(references + 8, u32::MAX),
            (
                ExpectedDecodeError::MalformedProgram {
                    invariant: Invariant::IndexOutOfBounds,
                    record: RecordKind::OperationReference,
                    index: Some(0),
                    field: "origin",
                },
                0,
                None,
                None,
            ),
        ),
    ]
    .into_iter()
    .map(|(name, bytes, expected)| {
        mutation(
            mutation_target(name),
            name,
            bytes,
            canonical.clone(),
            expected,
        )
    })
    .collect()
}

#[test]
fn filter_fwir_rejects_non_predicate_reference_identity_after_physical_decode() {
    let program =
        compile_source_to_verified_program("filter[@odd (1 2 3)]\n", "hostile-filter.faraweave")
            .expect("compile filter artifact");
    let mut bytes =
        encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode filter artifact");
    let (_, references, _) = section(&bytes, 18);
    put_u16(&mut bytes, references, 1);
    put_u16(&mut bytes, references + 2, 1);
    put_u16(&mut bytes, references + 4, 1);
    let error = decode_fwir(&bytes, &FwirDecodeLimits::default())
        .expect_err("inc is not a declared total Int predicate");
    assert!(matches!(
        error.kind,
        FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(ref malformed))
            if malformed.invariant == Invariant::InvalidSemanticIdentity
                && malformed.record == RecordKind::Node
                && malformed.field == "operation_reference"
    ));
}

fn targeted_mutations() -> Vec<MutationCase> {
    let complete = example_bytes("complete");
    let (_, features, _) = section(&complete, 2);
    let (_, strings, _) = section(&complete, 3);
    let string_count = read_u32(&complete, strings) as usize;
    let string_data = strings + 4 + string_count * 8;
    let (_, sources, _) = section(&complete, 4);
    let (_, parameters, _) = section(&complete, 5);
    let scalar_type = record_with_tag(&complete, 6, 12, 1);
    let scalar_constant = record_with_tag(&complete, 8, 20, 1);
    let bool_constant = complete[section(&complete, 8).1..]
        .chunks_exact(20)
        .position(|record| record[0] == 1 && record[1] == 1)
        .map(|index| section(&complete, 8).1 + index * 20)
        .expect("scalar Bool constant");
    let (_, constant_elements, _) = section(&complete, 9);
    let (_, edges, edge_length) = section(&complete, 11);
    let selected_apply = record_with_tag(&complete, 14, 56, 4);
    let constant_node = record_with_tag(&complete, 14, 56, 1);
    let (_, ownership, _) = section(&complete, 15);
    let edge_records = &complete[edges..edges + edge_length];
    let whole_edge = edge_records
        .chunks_exact(24)
        .position(|record| record[8] != 2)
        .map(|index| edges + index * 24)
        .expect("whole-value edge");
    let non_static_vector_edge = edge_records
        .chunks_exact(24)
        .position(|record| record[9] != 2)
        .map(|index| edges + index * 24)
        .expect("non-static-vector edge");
    let producer = producer_artifact();
    let (_, producer_offset, _) = section(&producer, 32769);
    let name_length = read_u32(&producer, producer_offset) as usize;
    let version_length_offset = producer_offset + 4 + name_length;
    let version_length = read_u32(&producer, version_length_offset) as usize;
    let digest_header = version_length_offset + 4 + version_length;

    let mut raw = named_mutations();
    raw.extend(producer_mutations());
    let mut cases: Vec<_> = raw
        .into_iter()
        .map(|(name, bytes)| {
            let target = mutation_target(name);
            let (expected, offset, section_id, record_index) = match name {
                "header-magic" => (ExpectedDecodeError::InvalidHeader("magic"), 0, None, None),
                "format-major" => (ExpectedDecodeError::UnsupportedFormatVersion, 8, None, None),
                "format-minor-current-extension" => (
                    ExpectedDecodeError::NonCanonicalDirectory("unknown_extension"),
                    56,
                    Some(100),
                    Some(1),
                ),
                "header-size" => (
                    ExpectedDecodeError::InvalidHeader("header_size"),
                    12,
                    None,
                    None,
                ),
                "directory-entry-size" => (
                    ExpectedDecodeError::InvalidHeader("directory_entry_size"),
                    16,
                    None,
                    None,
                ),
                "header-reserved" => (
                    ExpectedDecodeError::InvalidHeader("reserved"),
                    18,
                    None,
                    None,
                ),
                "directory-offset" => (
                    ExpectedDecodeError::InvalidHeader("directory_offset"),
                    24,
                    None,
                    None,
                ),
                "directory-id" => (
                    ExpectedDecodeError::NonCanonicalDirectory("order"),
                    32,
                    Some(0),
                    Some(0),
                ),
                "directory-flags" | "combined-directory-record-error" => (
                    ExpectedDecodeError::NonCanonicalDirectory("flags"),
                    34,
                    Some(1),
                    Some(0),
                ),
                "directory-record-size" => (
                    ExpectedDecodeError::NonCanonicalDirectory("record_size"),
                    36,
                    Some(1),
                    Some(0),
                ),
                "directory-payload-offset" => (
                    ExpectedDecodeError::NonCanonicalDirectory("contiguous_payload"),
                    40,
                    Some(1),
                    Some(0),
                ),
                "directory-payload-length" => (
                    ExpectedDecodeError::InvalidSectionLength,
                    48,
                    Some(1),
                    Some(0),
                ),
                "directory-known-section-duplicate" => (
                    ExpectedDecodeError::NonCanonicalDirectory("order"),
                    56,
                    Some(1),
                    Some(1),
                ),
                "feature-zero" => (
                    ExpectedDecodeError::NonCanonicalRecord("feature_order"),
                    features,
                    Some(2),
                    Some(0),
                ),
                "feature-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    features + 3,
                    Some(2),
                    Some(0),
                ),
                "string-offset" => (
                    ExpectedDecodeError::NonCanonicalRecord("string_extent"),
                    strings + 4,
                    Some(3),
                    Some(0),
                ),
                "string-duplicate" | "string-out-of-order" => (
                    ExpectedDecodeError::NonCanonicalRecord("string_order"),
                    strings + 12,
                    Some(3),
                    Some(1),
                ),
                "string-unused" => (
                    ExpectedDecodeError::NonCanonicalRecord("unused_string"),
                    strings,
                    Some(3),
                    Some(1),
                ),
                "string-length" => (
                    ExpectedDecodeError::InvalidSectionLength,
                    strings + 4,
                    Some(3),
                    Some(0),
                ),
                "string-utf8" => (
                    ExpectedDecodeError::InvalidUtf8,
                    string_data,
                    Some(3),
                    Some(0),
                ),
                "source-name-index" => (
                    ExpectedDecodeError::NonCanonicalRecord("diagnostic_name"),
                    sources,
                    Some(4),
                    Some(0),
                ),
                "parameter-name-index" => (
                    ExpectedDecodeError::NonCanonicalRecord("name"),
                    parameters + 4,
                    Some(5),
                    Some(0),
                ),
                "parameter-scalar-type" => (
                    ExpectedDecodeError::NonCanonicalRecord("scalar_type"),
                    parameters + 8,
                    Some(5),
                    Some(0),
                ),
                "parameter-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    parameters + 9,
                    Some(5),
                    Some(0),
                ),
                "type-kind" => (
                    ExpectedDecodeError::NonCanonicalRecord("kind"),
                    scalar_type,
                    Some(6),
                    Some(((scalar_type - section(&complete, 6).1) / 12) as u32),
                ),
                "type-scalar-type" | "type-element-start" | "type-element-count" => (
                    ExpectedDecodeError::NonCanonicalRecord("unused_type_range"),
                    scalar_type + 1,
                    Some(6),
                    Some(((scalar_type - section(&complete, 6).1) / 12) as u32),
                ),
                "type-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    scalar_type + 2,
                    Some(6),
                    Some(((scalar_type - section(&complete, 6).1) / 12) as u32),
                ),
                "constant-kind" => (
                    ExpectedDecodeError::NonCanonicalRecord("kind"),
                    scalar_constant,
                    Some(8),
                    Some(((scalar_constant - section(&complete, 8).1) / 20) as u32),
                ),
                "constant-scalar-type" | "constant-element-start" | "constant-element-count" => (
                    ExpectedDecodeError::NonCanonicalRecord("scalar_payload"),
                    scalar_constant + 1,
                    Some(8),
                    Some(((scalar_constant - section(&complete, 8).1) / 20) as u32),
                ),
                "constant-payload" => (
                    ExpectedDecodeError::NonCanonicalRecord("scalar_payload"),
                    bool_constant + 1,
                    Some(8),
                    Some(((bool_constant - section(&complete, 8).1) / 20) as u32),
                ),
                "constant-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    scalar_constant + 2,
                    Some(8),
                    Some(((scalar_constant - section(&complete, 8).1) / 20) as u32),
                ),
                "constant-element-scalar-type" => (
                    ExpectedDecodeError::NonCanonicalRecord("scalar_payload"),
                    constant_elements,
                    Some(9),
                    Some(0),
                ),
                "constant-element-payload" => (
                    ExpectedDecodeError::NonCanonicalRecord("scalar_payload"),
                    constant_elements,
                    Some(9),
                    Some(0),
                ),
                "constant-element-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    constant_elements + 1,
                    Some(9),
                    Some(0),
                ),
                "edge-access" => (
                    ExpectedDecodeError::NonCanonicalRecord("access"),
                    edges + 8,
                    Some(11),
                    Some(0),
                ),
                "edge-cardinality" => (
                    ExpectedDecodeError::NonCanonicalRecord("cardinality"),
                    edges + 9,
                    Some(11),
                    Some(0),
                ),
                "edge-conversion" => (
                    ExpectedDecodeError::NonCanonicalRecord("conversion"),
                    edges + 10,
                    Some(11),
                    Some(0),
                ),
                "edge-ownership" => (
                    ExpectedDecodeError::NonCanonicalRecord("ownership"),
                    edges + 11,
                    Some(11),
                    Some(0),
                ),
                "graph-edge-access-index" => (
                    ExpectedDecodeError::NonCanonicalRecord("access"),
                    whole_edge + 8,
                    Some(11),
                    Some(((whole_edge - edges) / 24) as u32),
                ),
                "graph-edge-cardinality-length" => (
                    ExpectedDecodeError::NonCanonicalRecord("cardinality"),
                    non_static_vector_edge + 9,
                    Some(11),
                    Some(((non_static_vector_edge - edges) / 24) as u32),
                ),
                "node-kind" => (
                    ExpectedDecodeError::NonCanonicalRecord("kind"),
                    selected_apply,
                    Some(14),
                    Some(((selected_apply - section(&complete, 14).1) / 56) as u32),
                ),
                "node-cardinality" => (
                    ExpectedDecodeError::NonCanonicalRecord("cardinality"),
                    selected_apply + 1,
                    Some(14),
                    Some(((selected_apply - section(&complete, 14).1) / 56) as u32),
                ),
                "node-lift" | "node-result-scalar" => (
                    ExpectedDecodeError::NonCanonicalRecord("selected_apply"),
                    selected_apply + 2,
                    Some(14),
                    Some(((selected_apply - section(&complete, 14).1) / 56) as u32),
                ),
                "node-unused-variant" => (
                    ExpectedDecodeError::NonCanonicalRecord("unused_variant"),
                    constant_node + 2,
                    Some(14),
                    Some(((constant_node - section(&complete, 14).1) / 56) as u32),
                ),
                "primitive-id" | "signature-id" | "implementation-id" => (
                    ExpectedDecodeError::NonCanonicalRecord("semantic_id"),
                    selected_apply + 24,
                    Some(14),
                    Some(((selected_apply - section(&complete, 14).1) / 56) as u32),
                ),
                "ownership-release-kind" => (
                    ExpectedDecodeError::NonCanonicalRecord("release_kind"),
                    ownership + 4,
                    Some(15),
                    Some(0),
                ),
                "ownership-reserved" => (
                    ExpectedDecodeError::NonCanonicalRecord("reserved"),
                    ownership + 5,
                    Some(15),
                    Some(0),
                ),
                "trailing-byte" => (
                    ExpectedDecodeError::NonCanonicalDirectory("trailing_bytes"),
                    64,
                    None,
                    None,
                ),
                "producer-name" => (
                    ExpectedDecodeError::NonCanonicalRecord("producer_name"),
                    producer_offset + 4,
                    Some(32769),
                    None,
                ),
                "producer-version" => (
                    ExpectedDecodeError::NonCanonicalRecord("producer_version"),
                    version_length_offset + 4,
                    Some(32769),
                    None,
                ),
                "producer-digest-algorithm" => (
                    ExpectedDecodeError::NonCanonicalRecord("digest_algorithm"),
                    digest_header,
                    Some(32769),
                    None,
                ),
                "producer-digest-length" | "producer-digest-truncated" => (
                    ExpectedDecodeError::NonCanonicalRecord("digest_length"),
                    digest_header + 2,
                    Some(32769),
                    None,
                ),
                "module-semantic-major" | "module-semantic-minor" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::UnsupportedVersion,
                        record: RecordKind::Module,
                        index: None,
                        field: "semantic_version",
                    },
                    0,
                    None,
                    None,
                ),
                "module-parameter-header-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InvalidRecord,
                        record: RecordKind::Module,
                        index: None,
                        field: "parameter_header_origin",
                    },
                    0,
                    None,
                    None,
                ),
                "source-byte-length" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::RangeOverflow,
                        record: RecordKind::SourceUnit,
                        index: Some(0),
                        field: "byte_length",
                    },
                    0,
                    None,
                    None,
                ),
                "parameter-slot" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InvalidRecord,
                        record: RecordKind::Parameter,
                        index: Some(0),
                        field: "slot",
                    },
                    0,
                    None,
                    None,
                ),
                "parameter-declaration-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Parameter,
                        index: Some(0),
                        field: "declaration_origin",
                    },
                    0,
                    None,
                    None,
                ),
                "parameter-name-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Parameter,
                        index: Some(0),
                        field: "name_origin",
                    },
                    0,
                    None,
                    None,
                ),
                "type-element-index" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::NonPostorderReference,
                        record: RecordKind::Type,
                        index: Some(5),
                        field: "elements",
                    },
                    0,
                    None,
                    None,
                ),
                "provenance-source" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Origin,
                        index: Some(0),
                        field: "source_unit",
                    },
                    0,
                    None,
                    None,
                ),
                "provenance-origin"
                | "origin-begin-line"
                | "origin-begin-column"
                | "origin-end-offset"
                | "origin-end-line"
                | "origin-end-column" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InvalidRecord,
                        record: RecordKind::Origin,
                        index: Some(0),
                        field: "span",
                    },
                    0,
                    None,
                    None,
                ),
                "graph-edge-producer" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::NonPostorderReference,
                        record: RecordKind::Edge,
                        index: Some(0),
                        field: "producer",
                    },
                    0,
                    None,
                    None,
                ),
                "graph-edge-position" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InvalidRecord,
                        record: RecordKind::Edge,
                        index: Some(0),
                        field: "argument_position",
                    },
                    0,
                    None,
                    None,
                ),
                "edge-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Edge,
                        index: Some(0),
                        field: "origin",
                    },
                    0,
                    None,
                    None,
                ),
                "shape-check-position" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InconsistentResultMetadata,
                        record: RecordKind::Node,
                        index: Some(20),
                        field: "shape",
                    },
                    0,
                    None,
                    None,
                ),
                "branch-node-start" | "branch-node-count" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::RangeOverflow,
                        record: RecordKind::Branch,
                        index: Some(0),
                        field: "nodes",
                    },
                    0,
                    None,
                    None,
                ),
                "branch-root" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::NonPostorderReference,
                        record: RecordKind::Branch,
                        index: Some(0),
                        field: "nodes",
                    },
                    0,
                    None,
                    None,
                ),
                "branch-placeholder-origin" | "branch-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Branch,
                        index: Some(0),
                        field: "origin",
                    },
                    0,
                    None,
                    None,
                ),
                "node-result-type" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Node,
                        index: Some(10),
                        field: "result_type",
                    },
                    0,
                    None,
                    None,
                ),
                "node-cardinality-length" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::InconsistentResultMetadata,
                        record: RecordKind::Node,
                        index: Some(10),
                        field: "result",
                    },
                    0,
                    None,
                    None,
                ),
                "node-edge-start" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::RangeMismatch,
                        record: RecordKind::Node,
                        index: Some(10),
                        field: "edges",
                    },
                    0,
                    None,
                    None,
                ),
                "node-edge-count" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::RangeOverflow,
                        record: RecordKind::Node,
                        index: Some(10),
                        field: "edges",
                    },
                    0,
                    None,
                    None,
                ),
                "node-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Node,
                        index: Some(10),
                        field: "origin",
                    },
                    0,
                    None,
                    None,
                ),
                "ownership-owner" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::AmbiguousOwnership,
                        record: RecordKind::Ownership,
                        index: Some(0),
                        field: "owner",
                    },
                    0,
                    None,
                    None,
                ),
                "ownership-release-index" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::AmbiguousOwnership,
                        record: RecordKind::Ownership,
                        index: Some(0),
                        field: "release_after",
                    },
                    0,
                    None,
                    None,
                ),
                "root-node" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Root,
                        index: Some(0),
                        field: "node",
                    },
                    0,
                    None,
                    None,
                ),
                "root-origin" => (
                    ExpectedDecodeError::MalformedProgram {
                        invariant: Invariant::IndexOutOfBounds,
                        record: RecordKind::Root,
                        index: Some(0),
                        field: "origin",
                    },
                    0,
                    None,
                    None,
                ),
                _ => panic!("mutation {name} has no exact expected diagnostic"),
            };
            let base_bytes = match name {
                "format-minor-current-extension" => empty_with_extension(1, 0),
                "header-magic"
                | "format-major"
                | "header-size"
                | "directory-entry-size"
                | "header-reserved"
                | "directory-offset"
                | "directory-id"
                | "directory-flags"
                | "directory-record-size"
                | "directory-payload-offset"
                | "directory-payload-length"
                | "directory-known-section-duplicate"
                | "trailing-byte" => example_bytes("empty"),
                "producer-name"
                | "producer-version"
                | "producer-digest-algorithm"
                | "producer-digest-length"
                | "producer-digest-truncated" => producer.clone(),
                _ => complete.clone(),
            };
            mutation(
                target,
                name,
                bytes,
                base_bytes,
                (expected, offset, section_id, record_index),
            )
        })
        .collect();
    cases.extend(operation_reference_mutations());
    cases
}

#[test]
fn deterministic_mutation_corpus_is_rejected_without_panic_or_partial_program() {
    let cases = targeted_mutations();
    let mut names = BTreeSet::new();
    for case in cases {
        assert!(names.insert(case.name), "duplicate mutation {}", case.name);
        assert!(
            !case.changed_ranges.is_empty(),
            "{} changed no bytes",
            case.name
        );
        assert_eq!(
            case.changed_ranges,
            changed_byte_ranges(&case.base_bytes, &case.bytes),
            "{} changed-byte ranges",
            case.name
        );
        let first = decode_fwir(&case.bytes, &FwirDecodeLimits::default());
        let second = decode_fwir(&case.bytes, &FwirDecodeLimits::default());
        assert!(first.is_err(), "mutation {} was accepted", case.name);
        assert_eq!(first, second, "mutation {} was nondeterministic", case.name);
        assert_expected_decode_error(&case, &first.expect_err("checked error"));
    }
}

#[test]
fn every_truncation_and_oversized_claim_is_bounded_and_deterministic() {
    for name in ["empty", "scalar-true", "complete"] {
        let bytes = example_bytes(name);
        for length in 0..bytes.len() {
            let first = decode_fwir(&bytes[..length], &FwirDecodeLimits::default());
            let second = decode_fwir(&bytes[..length], &FwirDecodeLimits::default());
            assert!(first.is_err(), "{name} truncation {length}");
            assert_eq!(first, second, "{name} truncation {length}");
        }
    }

    let complete = example_bytes("complete");
    let limit_cases = [
        FwirDecodeLimits {
            max_artifact_bytes: complete.len() - 1,
            ..FwirDecodeLimits::default()
        },
        FwirDecodeLimits {
            max_sections: 1,
            ..FwirDecodeLimits::default()
        },
        FwirDecodeLimits {
            max_records_per_section: 1,
            ..FwirDecodeLimits::default()
        },
        FwirDecodeLimits {
            max_total_records: 1,
            ..FwirDecodeLimits::default()
        },
        FwirDecodeLimits {
            max_string_bytes: 1,
            ..FwirDecodeLimits::default()
        },
    ];
    for limits in limit_cases {
        assert!(decode_fwir(&complete, &limits).is_err());
    }

    let mut claimed = example_bytes("empty");
    put_u32(&mut claimed, 20, u32::MAX);
    assert!(matches!(
        decode_fwir(&claimed, &FwirDecodeLimits::default()),
        Err(error)
            if matches!(
                error.kind,
                FwirDecodeErrorKind::ResourceLimit { .. } | FwirDecodeErrorKind::Truncated { .. }
            )
    ));
}

#[test]
fn same_major_optional_compatibility_and_mandatory_rejection_are_exact() {
    let canonical = example_bytes("empty");
    let optional = empty_with_extension(1, 0);
    let decoded = decode_fwir(&optional, &FwirDecodeLimits::default()).expect("forward optional");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("strip advisory"),
        canonical
    );
    assert!(matches!(
        decode_fwir(&empty_with_extension(1, 1), &FwirDecodeLimits::default()),
        Err(error)
            if matches!(
                error.kind,
                FwirDecodeErrorKind::UnknownMandatoryExtension { id: 100 }
            )
    ));
    assert!(decode_fwir(&empty_with_extension(0, 0), &FwirDecodeLimits::default()).is_err());

    let advisory = decode_fwir(&empty_with_feature(100, 1), &FwirDecodeLimits::default())
        .expect("advisory feature");
    assert_eq!(
        encode_fwir(&advisory, &FwirEncodeOptions::default()).expect("strip advisory feature"),
        canonical
    );
    let math = empty_with_feature(Feature::BackendNativeMathV1.numeric(), 0);
    let decoded = decode_fwir(&math, &FwirDecodeLimits::default())
        .expect("known backend-native math feature");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default())
            .expect("retain mandatory backend-native math feature"),
        math
    );
    assert!(
        decode_fwir(
            &empty_with_feature(Feature::BackendNativeMathV1.numeric(), 1),
            &FwirDecodeLimits::default()
        )
        .is_err()
    );
    assert!(decode_fwir(&empty_with_feature(1, 1), &FwirDecodeLimits::default()).is_err());
    assert!(decode_fwir(&empty_with_feature(100, 0), &FwirDecodeLimits::default()).is_err());
}

#[test]
fn producer_metadata_corruption_is_rejected_and_identity_stays_host_neutral() {
    let canonical = example_bytes("empty");
    let producer = producer_artifact();
    let decoded =
        decode_fwir(&producer, &FwirDecodeLimits::default()).expect("canonical producer decode");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default())
            .expect("producer-free identity projection"),
        canonical
    );
    assert_eq!(
        encode_fwir(
            &decoded,
            &FwirEncodeOptions {
                producer_metadata: Some(FwirProducerMetadata::Sha256([0xa5; 32])),
            },
        )
        .expect("canonical producer reencode"),
        producer
    );
    let inspection = inspect_fwir(&decoded).expect("producer inspection");
    let canonical_hex = canonical
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    assert!(inspection.ends_with(&format!("canonical-hex {canonical_hex}\n")));
}

fn unique_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("faraweave-{name}-{nonce}"))
}

#[test]
fn every_public_artifact_consumer_verifies_before_arguments_or_execution() {
    let directory = unique_directory("fwir-interpreter-gate");
    fs::create_dir_all(&directory).expect("temporary directory");
    let malformed = directory.join("malformed.fwir");
    let mut bytes = example_bytes("complete");
    let (_, roots, _) = section(&bytes, 16);
    put_u32(&mut bytes, roots, u32::MAX);
    fs::write(&malformed, bytes).expect("malformed artifact");

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_faraweave"));
    let commands = [
        vec!["inspect-ir".into(), malformed.as_os_str().to_owned()],
        vec![
            "run-ir".into(),
            malformed.as_os_str().to_owned(),
            "--".into(),
            "not-an-argument".into(),
        ],
    ];
    for arguments in commands {
        let output = Command::new(&binary)
            .args(arguments)
            .output()
            .expect("artifact consumer");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains("artifact error"),
            "{:?}",
            output.stderr
        );
    }
    fs::remove_dir_all(directory).expect("temporary cleanup");
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Event {
    kind: ResourceEventKind,
    producer: String,
    bytes: Option<usize>,
    work: usize,
    ordinal: Option<usize>,
    refusal: Option<ResourceErrorReason>,
    usage: faraweave::ResourceUsage,
}

static EVENTS: Mutex<Vec<Event>> = Mutex::new(Vec::new());

fn observe(event: &faraweave::ResourceEvent<'_>) {
    let mut events = EVENTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    events.push(Event {
        kind: event.kind,
        producer: event.producer.to_owned(),
        bytes: event.requested_bytes,
        work: event.requested_work_units,
        ordinal: event.allocation_ordinal,
        refusal: event.refusal_reason,
        usage: event.usage,
    });
}

fn take_events() -> Vec<Event> {
    let mut events = EVENTS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    std::mem::take(&mut *events)
}

#[test]
fn source_memory_and_decoded_interpreter_resources_faults_and_cleanup_are_identical() {
    let source = "parameters[n Int]\nfanout[iota[n] {inc[_]} {add[_ 10]}]\n";
    let memory = compile_source_to_verified_program(source, "corpus.fw").expect("source lowering");
    let canonical = encode_fwir(&memory, &FwirEncodeOptions::default()).expect("encode");
    let decoded = decode_fwir(&canonical, &FwirDecodeLimits::default()).expect("decode");

    let memory_result = evaluate_verified_program_with_arguments(
        &memory,
        &["3"],
        EvaluationConfiguration::default(),
    )
    .expect("memory result");
    let decoded_result = evaluate_verified_program_with_arguments(
        &decoded,
        &["3"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded result");
    assert_eq!(memory_result, decoded_result);
    assert_eq!(memory_result.formatted, ["[(2 3 4) (11 12 13)]"]);
    let arguments = [Value::Int(3)];
    let _ = take_events();
    let source_observed = evaluate_source_with_arguments_and_observer(
        source,
        &arguments,
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("source observed");
    let source_events = take_events();
    let memory_observed = evaluate_verified_program_with_observer(
        &memory,
        &arguments,
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("memory observed");
    let memory_events = take_events();
    let decoded_observed = evaluate_verified_program_with_observer(
        &decoded,
        &arguments,
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("decoded observed");
    let decoded_events = take_events();
    assert_eq!(source_observed, memory_observed);
    assert_eq!(source_events, memory_events);
    assert_eq!(memory_observed, decoded_observed);
    assert_eq!(memory_events, decoded_events);

    let failure_argument = ["9223372036854775807"];
    let memory_error = evaluate_verified_program_with_arguments(
        &memory,
        &failure_argument,
        EvaluationConfiguration::default(),
    )
    .expect_err("memory overflow");
    let decoded_error = evaluate_verified_program_with_arguments(
        &decoded,
        &failure_argument,
        EvaluationConfiguration::default(),
    )
    .expect_err("decoded overflow");
    assert_eq!(memory_error, decoded_error);

    let mut saw_cleanup = false;
    for ordinal in 1..=3 {
        let configuration = EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(ordinal),
            },
            ..EvaluationConfiguration::default()
        };
        let _ = take_events();
        let source_result =
            evaluate_source_with_arguments_and_observer(source, &arguments, configuration, observe);
        let source_events = take_events();
        let memory_result =
            evaluate_verified_program_with_observer(&memory, &arguments, configuration, observe);
        let memory_events = take_events();
        let decoded_result =
            evaluate_verified_program_with_observer(&decoded, &arguments, configuration, observe);
        let decoded_events = take_events();
        assert_eq!(source_result, memory_result, "fault ordinal {ordinal}");
        assert_eq!(memory_result, decoded_result, "fault ordinal {ordinal}");
        assert_eq!(source_events, memory_events, "fault ordinal {ordinal}");
        assert_eq!(memory_events, decoded_events, "fault ordinal {ordinal}");
        assert!(
            memory_events
                .iter()
                .any(|event| event.kind == ResourceEventKind::Refusal),
            "fault ordinal {ordinal}"
        );
        saw_cleanup |= memory_events
            .iter()
            .any(|event| event.kind == ResourceEventKind::Release);
    }
    assert!(saw_cleanup, "fault matrix did not exercise prefix cleanup");
}

#[test]
fn traceability_references_complete_executable_evidence_sets() {
    let mutation_cases: BTreeSet<_> = targeted_mutations()
        .into_iter()
        .map(|case| (case.target_requirement, case.name))
        .collect();
    let positive_cases = BTreeSet::from([
        "canonical-empty",
        "canonical-scalar",
        "canonical-complete",
        "canonical-producer",
        "compat-forward-minor",
        "compat-advisory-feature",
        "canonical-application-plans",
        "canonical-operation-references",
        "canonical-connected-bindings",
        "canonical-immutable-bindings",
    ]);
    let behavioral_evidence = BTreeSet::from([
        ("header.section_count", "section-count-limit"),
        ("compat.format_major", "format-major"),
        (
            "directory.unknown_optional",
            "unknown-optional-current-minor",
        ),
        ("directory.unknown_mandatory", "unknown-mandatory"),
        ("feat.class", "known-advisory-feature"),
        ("strs.count", "string-count-limit"),
        (
            "compat.forward_minor_optional_section",
            "unknown-optional-current-minor",
        ),
        (
            "compat.forward_minor_mandatory_section",
            "unknown-mandatory",
        ),
        ("compat.optional_feature", "unknown-mandatory-feature"),
        ("limits.artifact_bytes", "artifact-byte-limit"),
        ("limits.sections", "section-count-limit"),
        ("limits.records_per_section", "record-count-limit"),
        ("limits.total_records", "total-record-limit"),
        ("limits.string_bytes", "string-byte-limit"),
        ("decoder.truncation", "all-truncations"),
        ("decoder.no_backend_before_verify", "public-backend-gate"),
        ("canonical.byte_identity", "corpus-hash"),
        ("canonical.host_neutral", "corpus-host-neutral"),
        (
            "appl.feature_required",
            "application-plan-section-without-feature",
        ),
        ("appl.section_required", "application-plan-section-missing"),
        ("appl.record_count", "application-plan-count"),
        ("appl.node_order", "application-plan-node-order"),
        ("appl.node_unique", "application-plan-node-duplicate"),
        ("appl.reserved", "application-plan-reserved"),
        ("appl.plan_id_known", "application-plan-id-unknown"),
        ("appl.implementation_match", "application-plan-id-mismatch"),
        ("appl.format_minor", "application-plan-format-minor"),
        ("appl.semantic_minor", "application-plan-semantic-minor"),
        (
            "oprf.section_presence",
            "operation-reference-section-presence",
        ),
        (
            "oprf.canonical_record",
            "operation-reference-canonical-record",
        ),
        (
            "oprf.canonical_roundtrip",
            "operation-reference-canonical-roundtrip",
        ),
        ("binding.feature_required", "connected-binding-feature"),
        ("binding.format_minor", "connected-binding-format-minor"),
        ("binding.semantic_minor", "connected-binding-semantic-minor"),
        ("binding.node_kind", "connected-binding-node-kind"),
        ("binding.access_kind", "connected-binding-access-kind"),
        ("user_binding.feature_required", "user-binding-feature"),
        ("user_binding.format_minor", "user-binding-format-minor"),
        ("user_binding.semantic_minor", "user-binding-semantic-minor"),
        ("user_binding.node_kind", "user-binding-node-kind"),
        ("user_binding.access_kind", "user-binding-access-kind"),
        ("user_binding.provenance", "user-binding-provenance"),
        ("surfaces.source_memory_decoded", "differential-runtime"),
        (
            "surfaces.resources_faults_cleanup",
            "differential-resource-faults",
        ),
    ]);
    let mut requirements = BTreeSet::new();
    for (line_index, line) in TRACEABILITY.lines().enumerate().skip(1) {
        let columns: Vec<_> = line.split('\t').collect();
        assert_eq!(columns.len(), 3, "traceability row {}", line_index + 1);
        assert!(
            requirements.insert(columns[0]),
            "duplicate requirement {}",
            columns[0]
        );
        assert!(
            positive_cases.contains(columns[1]),
            "unknown positive evidence {}",
            columns[1]
        );
        assert!(
            mutation_cases.contains(&(columns[0], columns[2]))
                || behavioral_evidence.contains(&(columns[0], columns[2])),
            "negative evidence {} does not target requirement {}",
            columns[2],
            columns[0]
        );
    }
    assert!(requirements.len() >= 100, "traceability is incomplete");
    for prefix in [
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
        "binding.",
        "user_binding.",
        "ownr.",
        "root.",
        "prod.",
        "compat.",
        "limits.",
        "decoder.",
        "canonical.",
        "surfaces.",
    ] {
        assert!(
            requirements
                .iter()
                .any(|requirement| requirement.starts_with(prefix)),
            "missing requirement family {prefix}"
        );
    }
}
