use faraweave::{
    AllocationFailureInjection, Cardinality, Conversion, ErrorKind, EvaluationConfiguration,
    ExecutionProfile, Feature, FwirDecodeErrorKind, FwirDecodeLimits, FwirEncodeOptions, Invariant,
    LiftMode, NodeKind, ResourceErrorReason, ResourceLimits, ScalarType, Value, VerifyError,
    compile_source_to_fwir, compile_source_to_verified_program, decode_fwir, emit_c_source,
    encode_fwir, evaluate_expression, evaluate_expression_with_configuration,
    evaluate_source_with_arguments,
};

const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;
const MAX_ULPS: u64 = 16;
const MAX_ABSOLUTE_ERROR: f64 = 1.421_085_471_520_200_4e-14;

fn double(source: &str) -> f64 {
    match evaluate_expression(source).expect(source).value {
        Value::Double(value) => value,
        value => panic!("{source} returned {value:?}"),
    }
}

fn order_key(bits: u64) -> u64 {
    if bits >> 63 == 0 {
        bits | (1_u64 << 63)
    } else {
        !bits
    }
}

fn finite_conforms(actual: f64, reference_bits: u64) -> bool {
    let actual_bits = actual.to_bits();
    let reference = f64::from_bits(reference_bits);
    actual.is_finite()
        && actual_bits >> 63 == reference_bits >> 63
        && (order_key(actual_bits).abs_diff(order_key(reference_bits)) <= MAX_ULPS
            || (actual - reference).abs() <= MAX_ABSOLUTE_ERROR)
}

fn assert_finite_envelope(source: &str, reference_bits: u64) {
    let actual = double(source);
    let actual_bits = actual.to_bits();
    assert!(
        finite_conforms(actual, reference_bits),
        "{source}: actual={actual_bits:016x} reference={reference_bits:016x}"
    );
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

fn section(bytes: &[u8], wanted: u16) -> (usize, usize) {
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

fn tan_node(bytes: &[u8]) -> usize {
    let (offset, length) = section(bytes, 14);
    bytes[offset..offset + length]
        .chunks_exact(56)
        .position(|record| {
            record[0] == 4
                && u32::from_le_bytes([record[24], record[25], record[26], record[27]]) == 35
        })
        .map(|index| offset + index * 56)
        .unwrap_or_else(|| panic!("canonical artifact lacks tan node"))
}

#[test]
fn tan_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\ntan[1.0]\ntan[2]\ntan[(0 1)]\ntan[iota[count]]\n",
        "tan-ids.faraweave",
    )
    .expect("tan program");
    let raw = program.as_raw();
    assert_eq!(raw.module.semantic_minor, 1);
    assert_eq!(
        raw.features,
        vec![
            Feature::StableSemanticIds.numeric(),
            Feature::BackendNativeMathV1.numeric(),
        ]
    );

    let selections: Vec<_> = raw
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 35,
                signature_id,
                implementation_id,
                lift,
                result_element_type,
                ..
            } => {
                let edge = &raw.edges[node.edges.start as usize];
                Some((
                    signature_id,
                    implementation_id,
                    lift,
                    result_element_type,
                    edge.conversion,
                ))
            }
            _ => None,
        })
        .collect();
    assert_eq!(
        selections,
        vec![
            (
                60,
                60,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                60,
                60,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                60,
                60,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                60,
                60,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
        ]
    );
    assert_eq!(
        raw.nodes
            .iter()
            .rfind(|node| {
                matches!(
                    node.kind,
                    NodeKind::SelectedApply {
                        primitive_id: 35,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn tan_special_quadrants_boundaries_and_finite_envelope_are_public_semantics() {
    for (source, expected) in [
        ("tan[0.0]", 0x0000_0000_0000_0000),
        ("tan[-0.0]", 0x8000_0000_0000_0000),
        ("tan[inf]", CANONICAL_NAN_BITS),
        ("tan[-inf]", CANONICAL_NAN_BITS),
        ("tan[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("tan[1.0]", 0x3ff8_eb24_5cbe_e3a6),
        ("tan[1.0471975511965976]", 0x3ffb_b67a_e858_4ca8),
        ("tan[1.5707963267948966]", 0x434d_0296_7c31_cdb5),
        ("tan[1.5707963267948963]", 0x4329_153d_9443_ed0b),
        ("tan[1.5707963267948968]", 0xc336_17a1_5494_767a),
        ("tan[3.141592653589793]", 0xbca1_a626_3314_5c07),
        ("tan[4.71238898038469]", 0x4333_570e_fd76_8923),
        ("tan[6.283185307179586]", 0xbcb1_a626_3314_5c07),
        ("tan[5e-324]", 0x0000_0000_0000_0001),
        ("tan[1e300]", 0x3ff6_be41_1f37_ac77),
        ("tan[1.7976931348623157e308]", 0xbf74_530c_fe72_9484),
    ] {
        assert_finite_envelope(source, reference_bits);
    }

    let static_vector = evaluate_expression("tan[(0.0 1.0 -1.0)]")
        .expect("static tan vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("tan vector returned {static_vector:?}");
    };
    assert_eq!(values[0].to_bits(), 0x0000_0000_0000_0000);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3ff8_eb24_5cbe_e3a6, 0xbff8_eb24_5cbe_e3a6])
    {
        assert!(finite_conforms(*value, reference));
    }

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\ntan[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic tan vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic tan root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic tan returned {dynamic:?}");
    };
    assert_eq!(values.len(), 3);
    for (value, reference) in values.iter().zip([
        0x3ff8_eb24_5cbe_e3a6,
        0xc001_7af6_2e09_50f8,
        0xbfc2_3ef7_1254_b86f,
    ]) {
        assert!(finite_conforms(*value, reference));
    }
}

#[test]
fn tan_envelope_accepts_nine_through_sixteen_ulps() {
    let reference_bits = 0x4090_0000_0000_0000;
    for distance in 9..=MAX_ULPS {
        assert!(
            finite_conforms(f64::from_bits(reference_bits + distance), reference_bits),
            "{distance} ULPs"
        );
    }
    assert!(!finite_conforms(
        f64::from_bits(reference_bits + MAX_ULPS + 1),
        reference_bits
    ));
}

#[test]
fn tan_resources_failures_cleanup_and_diagnostics_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "tan[0]",
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits: ResourceLimits {
                max_work_units: Some(1),
                ..ResourceLimits::default()
            },
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect("scalar exact work");
    assert_eq!(scalar.value, Value::Double(0.0));
    assert_eq!(scalar.usage.work_units, 1);
    assert_eq!(scalar.usage.allocation_attempts, 0);

    let vector = evaluate_expression_with_configuration(
        "tan[(0.0 1.0 2.0)]",
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits: ResourceLimits {
                max_vector_bytes: Some(24),
                max_live_evaluation_bytes: Some(48),
                max_work_units: Some(3),
                ..ResourceLimits::default()
            },
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect("vector exact resources");
    assert_eq!(vector.usage.live_evaluation_bytes, 24);
    assert_eq!(vector.usage.peak_live_evaluation_bytes, 48);
    assert_eq!(vector.usage.work_units, 3);
    assert_eq!(vector.usage.allocation_attempts, 2);

    for (limits, expected_limit, expected_producer) in [
        (
            ResourceLimits {
                max_work_units: Some(2),
                max_vector_bytes: Some(24),
                max_live_evaluation_bytes: Some(48),
                ..ResourceLimits::default()
            },
            "max_work_units",
            "tan",
        ),
        (
            ResourceLimits {
                max_work_units: Some(3),
                max_vector_bytes: Some(16),
                max_live_evaluation_bytes: Some(48),
                ..ResourceLimits::default()
            },
            "max_vector_bytes",
            "vector_literal",
        ),
        (
            ResourceLimits {
                max_work_units: Some(3),
                max_vector_bytes: Some(24),
                max_live_evaluation_bytes: Some(47),
                ..ResourceLimits::default()
            },
            "max_live_evaluation_bytes",
            "tan",
        ),
    ] {
        let error = evaluate_expression_with_configuration(
            "tan[(0.0 1.0 2.0)]",
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
        assert_eq!(
            error
                .usage
                .expect("post-cleanup usage")
                .live_evaluation_bytes,
            0
        );
    }

    let allocation = evaluate_expression_with_configuration(
        "tan[(0.0 1.0)]",
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV2,
            limits: ResourceLimits::default(),
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
        },
    )
    .expect_err("vector allocation refusal");
    assert_eq!(allocation.kind, ErrorKind::ResourceError);
    assert_eq!(allocation.primitive.as_deref(), Some("tan"));
    assert_eq!(
        allocation.resource.expect("allocation context").reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(
        allocation
            .usage
            .expect("allocation post-cleanup usage")
            .live_evaluation_bytes,
        0
    );

    let empty = evaluate_expression("tan[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    for (source, kind) in [
        ("tan[true]", ErrorKind::TypeError),
        ("tan[1.0 2.0]", ErrorKind::ArityError),
        ("tan[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("tan"), "{source}");
    }
}

#[test]
fn tan_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("tan[(0.0 1.0)]\n", &FwirEncodeOptions::default())
        .expect("tan artifact");
    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode tan artifact");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("re-encode tan artifact"),
        bytes
    );
    assert_eq!(
        decoded.as_raw().features,
        vec![
            Feature::StableSemanticIds.numeric(),
            Feature::BackendNativeMathV1.numeric(),
        ]
    );

    let node = tan_node(&bytes);
    for (relative, replacement) in [(24, 34_u32), (28, 59), (32, 59)] {
        let mut malformed = bytes.clone();
        malformed[node + relative..node + relative + 4].copy_from_slice(&replacement.to_le_bytes());
        let error = decode_fwir(&malformed, &FwirDecodeLimits::default())
            .expect_err("mismatched tan identity");
        assert_eq!(
            error.kind,
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "semantic_id"
            }
        );
        assert_eq!(usize::try_from(error.offset).ok(), Some(node + 24));
        assert_eq!(error.section_id, Some(14));
    }

    let (feature_offset, feature_length) = section(&bytes, 2);
    let feature = bytes[feature_offset..feature_offset + feature_length]
        .chunks_exact(4)
        .position(|record| read_u16(record, 0) == Feature::BackendNativeMathV1.numeric())
        .map(|index| feature_offset + index * 4)
        .expect("backend-native feature record");
    let mut missing_feature = bytes.clone();
    missing_feature[feature..feature + 2].copy_from_slice(&8_u16.to_le_bytes());
    missing_feature[feature + 2] = 1;
    let error = decode_fwir(&missing_feature, &FwirDecodeLimits::default())
        .expect_err("missing backend-native feature");
    assert!(matches!(
        error.kind,
        FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(ref malformed))
            if malformed.invariant == Invariant::MissingFeature
                && malformed.field == "backend_native_math_v1"
    ));

    let (module, _) = section(&bytes, 1);
    let mut old_version = bytes.clone();
    old_version[module + 2..module + 4].copy_from_slice(&0_u16.to_le_bytes());
    let error = decode_fwir(&old_version, &FwirDecodeLimits::default())
        .expect_err("semantic 1.0 tan artifact");
    assert!(matches!(
        error.kind,
        FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(ref malformed))
            if malformed.invariant == Invariant::UnsupportedVersion
                && malformed.field == "semantic_version"
    ));
}

#[test]
fn tan_emitted_c_calls_math_h_tan_directly() {
    let source = emit_c_source("tan[(0.0 1.0)]\n")
        .expect("tan C emission")
        .source;
    assert!(source.contains("#include <math.h>"));
    assert!(source.contains("result=tan(input);"));
    assert!(source.contains("fw_set_double(out,fw_backend_native_tan(args[0].d))"));
    assert!(source.contains("static int fw_kernel_60("));
    assert!(source.contains("static int fw_impl_60("));
}
