use faraweave::{
    AllocationFailureInjection, EvaluationConfiguration, FwirDecodeErrorKind, FwirDecodeLimits,
    FwirEncodeOptions, FwirProducerMetadata, ResourceErrorReason, ResourceEventKind, Value,
    compile_source_to_verified_program, decode_fwir, emit_c_from_verified_program, encode_fwir,
    evaluate_source_with_arguments_and_observer, evaluate_verified_program_with_arguments,
    evaluate_verified_program_with_observer, inspect_fwir,
};
use std::collections::BTreeSet;
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

const CORPUS: &str = include_str!("fixtures/fwir-v1-corpus.tsv");
const TRACEABILITY: &str = include_str!("fixtures/fwir-v1-conformance.tsv");

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
    bytes
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

fn named_mutations() -> Vec<(&'static str, Vec<u8>)> {
    let complete = example_bytes("complete");
    let empty = example_bytes("empty");
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
            "string-order",
            mutate(&complete, strings + 4, &1_u32.to_le_bytes()),
        ),
        (
            "source-name-index",
            mutate(&complete, sources, &u32::MAX.to_le_bytes()),
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
        ("type-kind", mutate(&complete, scalar_type, &[0])),
        ("type-scalar-type", mutate(&complete, scalar_type + 1, &[0])),
        (
            "type-reserved",
            mutate(&complete, scalar_type + 2, &1_u16.to_le_bytes()),
        ),
        (
            "type-range",
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
            "constant-range",
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
        (
            "constant-element-payload",
            mutate(&complete, constant_elements, &[0]),
        ),
        (
            "provenance-source",
            mutate(&complete, origins, &u32::MAX.to_le_bytes()),
        ),
        (
            "provenance-origin",
            mutate(&complete, origins + 4, &u32::MAX.to_le_bytes()),
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
            "shape-check-position",
            mutate(&complete, shape_checks, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-range",
            mutate(&complete, branches, &u32::MAX.to_le_bytes()),
        ),
        (
            "branch-root",
            mutate(&complete, branches + 8, &u32::MAX.to_le_bytes()),
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
            "node-edge-range",
            mutate(&complete, selected_apply + 12, &u32::MAX.to_le_bytes()),
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

#[test]
fn deterministic_mutation_corpus_is_rejected_without_panic_or_partial_program() {
    let cases = named_mutations();
    let mut names = BTreeSet::new();
    for (name, bytes) in cases {
        assert!(names.insert(name), "duplicate mutation {name}");
        let first = decode_fwir(&bytes, &FwirDecodeLimits::default());
        let second = decode_fwir(&bytes, &FwirDecodeLimits::default());
        assert!(first.is_err(), "mutation {name} was accepted");
        assert_eq!(first, second, "mutation {name} was nondeterministic");
        if name == "combined-directory-record-error" {
            assert!(matches!(
                first,
                Err(error)
                    if error.offset == 34
                        && matches!(
                            error.kind,
                            FwirDecodeErrorKind::NonCanonicalDirectory { field: "flags" }
                        )
            ));
        }
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
    assert!(decode_fwir(&empty_with_feature(1, 1), &FwirDecodeLimits::default()).is_err());
    assert!(decode_fwir(&empty_with_feature(100, 0), &FwirDecodeLimits::default()).is_err());
}

#[test]
fn producer_metadata_corruption_is_rejected_and_identity_stays_host_neutral() {
    let empty = decode_fwir(&example_bytes("empty"), &FwirDecodeLimits::default()).expect("empty");
    let producer = encode_fwir(
        &empty,
        &FwirEncodeOptions {
            producer_metadata: Some(FwirProducerMetadata::Sha256([0xa5; 32])),
        },
    )
    .expect("producer");
    let (_, offset, _) = section(&producer, 32769);
    let name_length = read_u32(&producer, offset) as usize;
    let version_length_offset = offset + 4 + name_length;
    let version_length = read_u32(&producer, version_length_offset) as usize;
    let digest_header = version_length_offset + 4 + version_length;
    let cases = [
        ("producer-name", offset + 4, b'X'),
        ("producer-version", version_length_offset + 4, b'v'),
        ("producer-digest-algorithm", digest_header, 2),
        ("producer-digest-length", digest_header + 2, 31),
    ];
    for (name, mutation_offset, value) in cases {
        let mut bytes = producer.clone();
        bytes[mutation_offset] = value;
        assert!(
            decode_fwir(&bytes, &FwirDecodeLimits::default()).is_err(),
            "{name}"
        );
    }
}

fn unique_directory(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    std::env::temp_dir().join(format!("faraweave-{name}-{nonce}"))
}

#[test]
fn every_public_artifact_consumer_verifies_before_arguments_or_backends() {
    let directory = unique_directory("fwir-backend-gate");
    fs::create_dir_all(&directory).expect("temporary directory");
    let malformed = directory.join("malformed.fwir");
    let c_destination = directory.join("preserved.c");
    let native_destination = directory.join(if cfg!(windows) {
        "preserved.exe"
    } else {
        "preserved"
    });
    let mut bytes = example_bytes("complete");
    let (_, roots, _) = section(&bytes, 16);
    put_u32(&mut bytes, roots, u32::MAX);
    fs::write(&malformed, bytes).expect("malformed artifact");
    fs::write(&c_destination, b"preserve-c").expect("C sentinel");
    fs::write(&native_destination, b"preserve-native").expect("native sentinel");

    let binary = PathBuf::from(env!("CARGO_BIN_EXE_faraweave"));
    let commands = [
        vec!["inspect-ir".into(), malformed.as_os_str().to_owned()],
        vec![
            "run-ir".into(),
            malformed.as_os_str().to_owned(),
            "--".into(),
            "not-an-argument".into(),
        ],
        vec![
            "emit-c-ir".into(),
            malformed.as_os_str().to_owned(),
            "-o".into(),
            c_destination.as_os_str().to_owned(),
        ],
        vec![
            "build-ir".into(),
            malformed.as_os_str().to_owned(),
            "-o".into(),
            native_destination.as_os_str().to_owned(),
            "--cc".into(),
            "compiler-must-not-run".into(),
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
        assert!(
            !String::from_utf8_lossy(&output.stderr).contains("compiler-must-not-run"),
            "{:?}",
            output.stderr
        );
    }
    assert_eq!(fs::read(&c_destination).expect("C sentinel"), b"preserve-c");
    assert_eq!(
        fs::read(&native_destination).expect("native sentinel"),
        b"preserve-native"
    );
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
fn source_memory_decoded_interpreter_c_resources_faults_and_cleanup_are_identical() {
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
    assert_eq!(
        emit_c_from_verified_program(&memory, EvaluationConfiguration::default())
            .expect("memory C"),
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default())
            .expect("decoded C")
    );

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
    let mutation_cases: BTreeSet<_> = named_mutations()
        .into_iter()
        .map(|(name, _)| name)
        .chain([
            "producer-name",
            "producer-version",
            "producer-digest-algorithm",
            "producer-digest-length",
        ])
        .collect();
    let positive_cases = BTreeSet::from([
        "canonical-empty",
        "canonical-scalar",
        "canonical-complete",
        "canonical-producer",
        "compat-forward-minor",
        "compat-advisory-feature",
    ]);
    let behavioral_evidence = BTreeSet::from([
        "all-truncations",
        "artifact-byte-limit",
        "corpus-hash",
        "corpus-host-neutral",
        "differential-resource-faults",
        "differential-runtime",
        "known-advisory-feature",
        "public-backend-gate",
        "record-count-limit",
        "section-count-limit",
        "strict-native-journey",
        "string-byte-limit",
        "string-count-limit",
        "total-record-limit",
        "unknown-mandatory",
        "unknown-mandatory-feature",
        "unknown-optional-current-minor",
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
            mutation_cases.contains(columns[2]) || behavioral_evidence.contains(columns[2]),
            "unknown negative evidence {}",
            columns[2]
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
