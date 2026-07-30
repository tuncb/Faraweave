#![allow(dead_code)]

use faraweave::{
    AllocationFailureInjection, ErrorKind, EvaluationConfiguration, ExecutionProfile, Feature,
    FwirDecodeErrorKind, FwirDecodeLimits, FwirEncodeOptions, Invariant, ResourceErrorReason,
    ResourceLimits, Value, VerifiedProgram, VerifyError, decode_fwir, encode_fwir,
    evaluate_expression, evaluate_expression_with_configuration,
};

pub const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

pub fn double(source: &str) -> f64 {
    match evaluate_expression(source).expect(source).value {
        Value::Double(value) => value,
        value => panic!("{source} returned {value:?}"),
    }
}

pub fn assert_exact(source: &str, expected_bits: u64) {
    assert_eq!(double(source).to_bits(), expected_bits, "{source}");
}

pub fn order_key(bits: u64) -> u64 {
    if bits >> 63 == 0 {
        bits | (1_u64 << 63)
    } else {
        !bits
    }
}

pub fn finite_conforms(
    actual: f64,
    reference_bits: u64,
    max_ulps: u64,
    max_absolute_error: f64,
) -> bool {
    let actual_bits = actual.to_bits();
    let reference = f64::from_bits(reference_bits);
    actual.is_finite()
        && actual_bits >> 63 == reference_bits >> 63
        && (order_key(actual_bits).abs_diff(order_key(reference_bits)) <= max_ulps
            || (actual - reference).abs() <= max_absolute_error)
}

pub fn assert_finite_envelope(
    source: &str,
    reference_bits: u64,
    max_ulps: u64,
    max_absolute_error: f64,
) {
    let actual = double(source);
    assert!(
        finite_conforms(actual, reference_bits, max_ulps, max_absolute_error),
        "{source}: actual={:016x} reference={reference_bits:016x}",
        actual.to_bits()
    );
}

pub fn read_u16(bytes: &[u8], offset: usize) -> u16 {
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

pub fn section(bytes: &[u8], wanted: u16) -> (usize, usize) {
    for index in 0..read_u32(bytes, 20) as usize {
        let entry = 32 + index * 24;
        if read_u16(bytes, entry) == wanted {
            return (
                read_u64(bytes, entry + 8) as usize,
                read_u64(bytes, entry + 16) as usize,
            );
        }
    }
    panic!("canonical artifact lacks section {wanted}");
}

pub fn selected_node(bytes: &[u8], primitive_id: u32, primitive: &str) -> usize {
    let (offset, length) = section(bytes, 14);
    bytes[offset..offset + length]
        .chunks_exact(56)
        .position(|record| record[0] == 4 && read_u32(record, 24) == primitive_id)
        .map(|index| offset + index * 56)
        .unwrap_or_else(|| panic!("canonical artifact lacks {primitive} node"))
}

pub fn assert_canonical_roundtrip(bytes: &[u8], primitive: &str) -> VerifiedProgram {
    let decoded = decode_fwir(bytes, &FwirDecodeLimits::default())
        .unwrap_or_else(|error| panic!("decode {primitive} artifact: {error:?}"));
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default())
            .unwrap_or_else(|error| panic!("re-encode {primitive} artifact: {error:?}")),
        bytes
    );
    assert_eq!(
        decoded.as_raw().features,
        vec![
            Feature::StableSemanticIds.numeric(),
            Feature::BackendNativeMathV1.numeric(),
        ]
    );
    decoded
}

pub fn assert_identity_mismatches(
    bytes: &[u8],
    node: usize,
    replacements: &[(usize, u32)],
    primitive: &str,
) {
    for &(relative, replacement) in replacements {
        let mut malformed = bytes.to_vec();
        malformed[node + relative..node + relative + 4].copy_from_slice(&replacement.to_le_bytes());
        let failure = format!("mismatched {primitive} identity");
        let error = decode_fwir(&malformed, &FwirDecodeLimits::default()).expect_err(&failure);
        assert_eq!(
            error.kind,
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "semantic_id"
            }
        );
        assert_eq!(usize::try_from(error.offset).ok(), Some(node + 24));
        assert_eq!(error.section_id, Some(14));
    }
}

pub fn assert_backend_native_feature_required(bytes: &[u8]) {
    let (feature_offset, feature_length) = section(bytes, 2);
    let feature = bytes[feature_offset..feature_offset + feature_length]
        .chunks_exact(4)
        .position(|record| read_u16(record, 0) == Feature::BackendNativeMathV1.numeric())
        .map(|index| feature_offset + index * 4)
        .expect("backend-native feature record");
    let mut missing_feature = bytes.to_vec();
    missing_feature[feature..feature + 2].copy_from_slice(&9_u16.to_le_bytes());
    missing_feature[feature + 2] = 1;
    let error = decode_fwir(&missing_feature, &FwirDecodeLimits::default())
        .expect_err("missing backend-native feature");
    assert!(matches!(
        error.kind,
        FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(ref malformed))
            if malformed.invariant == Invariant::MissingFeature
                && malformed.field == "backend_native_math_v1"
    ));
}

pub fn assert_semantic_minor_zero_rejected(bytes: &[u8], primitive: &str) {
    let (module, _) = section(bytes, 1);
    let mut old_version = bytes.to_vec();
    old_version[module + 2..module + 4].copy_from_slice(&0_u16.to_le_bytes());
    let failure = format!("semantic 1.0 {primitive} artifact");
    let error = decode_fwir(&old_version, &FwirDecodeLimits::default()).expect_err(&failure);
    assert!(matches!(
        error.kind,
        FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(ref malformed))
            if malformed.invariant == Invariant::UnsupportedVersion
                && malformed.field == "semantic_version"
    ));
}

pub fn assert_bounded_usage(
    source: &str,
    limits: ResourceLimits,
    expected_value: Option<Value>,
    expected_live_bytes: usize,
    expected_peak_live_bytes: usize,
    expected_work_units: usize,
    expected_allocation_attempts: usize,
) {
    let result = evaluate_expression_with_configuration(
        source,
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits,
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect(source);
    if let Some(expected) = expected_value {
        assert_eq!(result.value, expected, "{source}");
    }
    assert_eq!(
        result.usage.live_evaluation_bytes, expected_live_bytes,
        "{source}"
    );
    assert_eq!(
        result.usage.peak_live_evaluation_bytes, expected_peak_live_bytes,
        "{source}"
    );
    assert_eq!(result.usage.work_units, expected_work_units, "{source}");
    assert_eq!(
        result.usage.allocation_attempts, expected_allocation_attempts,
        "{source}"
    );
}

pub fn assert_profile_limit_refusal(
    source: &str,
    limits: ResourceLimits,
    expected_limit: &'static str,
    expected_producer: &str,
    expected_live_bytes_after_cleanup: Option<usize>,
) {
    let error = evaluate_expression_with_configuration(
        source,
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits,
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect_err(expected_limit);
    assert_eq!(error.kind, ErrorKind::ResourceError);
    assert_eq!(error.primitive.as_deref(), Some(expected_producer));
    let resource = error.resource.expect("resource context");
    assert_eq!(resource.reason, ResourceErrorReason::ProfileLimit);
    assert_eq!(resource.limit_kind, Some(expected_limit));
    if let Some(expected) = expected_live_bytes_after_cleanup {
        assert_eq!(
            error
                .usage
                .expect("post-cleanup usage")
                .live_evaluation_bytes,
            expected
        );
    }
}

pub fn assert_allocation_refusal(
    source: &str,
    primitive: &str,
    expected_live_bytes_after_cleanup: Option<usize>,
) {
    let error = evaluate_expression_with_configuration(
        source,
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV2,
            limits: ResourceLimits::default(),
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
        },
    )
    .expect_err("vector allocation refusal");
    assert_eq!(error.kind, ErrorKind::ResourceError);
    assert_eq!(error.primitive.as_deref(), Some(primitive));
    assert_eq!(
        error.resource.expect("allocation context").reason,
        ResourceErrorReason::AllocationUnavailable
    );
    if let Some(expected) = expected_live_bytes_after_cleanup {
        assert_eq!(
            error
                .usage
                .expect("allocation post-cleanup usage")
                .live_evaluation_bytes,
            expected
        );
    }
}

pub fn assert_empty_double_vector(
    source: &str,
    expected_work_units: usize,
    expected_allocation_attempts: usize,
) {
    let empty = evaluate_expression(source).expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, expected_work_units);
    assert_eq!(
        empty.usage.allocation_attempts,
        expected_allocation_attempts
    );
}
