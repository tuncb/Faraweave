use faraweave::{
    AllocationFailureInjection, Conversion, ErrorKind, EvaluationConfiguration, ExecutionProfile,
    Feature, FwirDecodeErrorKind, FwirDecodeLimits, FwirEncodeOptions, Invariant, LiftMode,
    NodeKind, ResourceErrorReason, ResourceLimits, ScalarType, Value, VerifyError,
    compile_source_to_fwir, compile_source_to_verified_program, decode_fwir, encode_fwir,
    evaluate_expression, evaluate_expression_with_configuration,
};

const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

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

fn assert_finite_envelope(source: &str, reference_bits: u64, max_ulps: u64) {
    let actual = double(source);
    let actual_bits = actual.to_bits();
    assert!(actual.is_finite(), "{source}");
    assert_eq!(actual_bits >> 63, reference_bits >> 63, "{source}");
    assert!(
        order_key(actual_bits).abs_diff(order_key(reference_bits)) <= max_ulps,
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

fn exp_node(bytes: &[u8]) -> usize {
    let (offset, length) = section(bytes, 14);
    bytes[offset..offset + length]
        .chunks_exact(56)
        .position(|record| {
            record[0] == 4
                && u32::from_le_bytes([record[24], record[25], record[26], record[27]]) == 30
        })
        .map(|index| offset + index * 56)
        .unwrap_or_else(|| panic!("canonical artifact lacks exp node"))
}

#[test]
fn exp_uses_contiguous_ids_double_selection_lifting_and_feature_seven() {
    let program = compile_source_to_verified_program(
        "exp[1.0]\nexp[1]\nexp[(0.0 1.0)]\n",
        "exp-ids.faraweave",
    )
    .expect("exp program");
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
                primitive_id,
                signature_id,
                implementation_id,
                lift,
                result_element_type,
                ..
            } => Some((
                primitive_id,
                signature_id,
                implementation_id,
                lift,
                result_element_type,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        selections,
        vec![
            (30, 55, 55, LiftMode::Scalar, ScalarType::Double),
            (30, 55, 55, LiftMode::Scalar, ScalarType::Double),
            (30, 55, 55, LiftMode::Vector, ScalarType::Double),
        ]
    );
    assert_eq!(raw.edges[0].conversion, Conversion::Identity);
    assert_eq!(raw.edges[1].conversion, Conversion::PromoteIntToDouble);
    assert_eq!(raw.edges[2].conversion, Conversion::Identity);
}

#[test]
fn exp_special_values_thresholds_finite_envelope_and_vectors_are_public_semantics() {
    for (source, expected) in [
        ("exp[0.0]", 0x3ff0_0000_0000_0000),
        ("exp[-0.0]", 0x3ff0_0000_0000_0000),
        ("exp[-inf]", 0x0000_0000_0000_0000),
        ("exp[inf]", 0x7ff0_0000_0000_0000),
        ("exp[nan]", CANONICAL_NAN_BITS),
        ("exp[-746.0]", 0x0000_0000_0000_0000),
        ("exp[709.7827128933841]", 0x7ff0_0000_0000_0000),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("exp[1.0]", 0x4005_bf0a_8b14_5769),
        ("exp[-1.0]", 0x3fd7_8b56_362c_ef38),
        ("exp[2.0]", 0x401d_8e64_b8d4_ddae),
        ("exp[-744.0]", 0x0000_0000_0000_0002),
        ("exp[-745.0]", 0x0000_0000_0000_0001),
        ("exp[709.782712893384]", 0x7fef_ffff_ffff_ff2a),
    ] {
        assert_finite_envelope(source, reference_bits, 4);
    }

    assert_eq!(double("exp[5e-324]").to_bits(), 1.0_f64.to_bits());
    let vector = evaluate_expression("exp[(0.0 1.0 -1.0)]")
        .expect("vector exp")
        .value;
    let Value::DoubleVector(values) = vector else {
        panic!("exp vector returned {vector:?}");
    };
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].to_bits(), 1.0_f64.to_bits());
    for (value, reference) in values[1..]
        .iter()
        .zip([0x4005_bf0a_8b14_5769, 0x3fd7_8b56_362c_ef38])
    {
        assert!(value.is_finite());
        assert!(order_key(value.to_bits()).abs_diff(order_key(reference)) <= 4);
    }
}

#[test]
fn exp_resources_diagnostics_and_allocation_refusals_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "exp[0]",
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
    assert_eq!(scalar.value, Value::Double(1.0));
    assert_eq!(scalar.usage.work_units, 1);
    assert_eq!(scalar.usage.allocation_attempts, 0);

    let vector = evaluate_expression_with_configuration(
        "exp[(0.0 1.0 -1.0)]",
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
            "exp",
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
            "exp",
        ),
    ] {
        let error = evaluate_expression_with_configuration(
            "exp[(0.0 1.0 -1.0)]",
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
    }

    let allocation = evaluate_expression_with_configuration(
        "exp[(0.0 1.0)]",
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
    assert_eq!(allocation.primitive.as_deref(), Some("exp"));
    assert_eq!(
        allocation.resource.expect("allocation context").reason,
        ResourceErrorReason::AllocationUnavailable
    );

    let empty = evaluate_expression("exp[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    for (source, kind) in [
        ("exp[true]", ErrorKind::TypeError),
        ("exp[1.0 2.0]", ErrorKind::ArityError),
        ("exp[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("exp"), "{source}");
    }
}

#[test]
fn exp_fwir_roundtrip_and_malformed_identities_are_checked() {
    let bytes = compile_source_to_fwir("exp[(0.0 1.0)]\n", &FwirEncodeOptions::default())
        .expect("exp artifact");
    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode exp artifact");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("re-encode exp artifact"),
        bytes
    );
    let raw = decoded.as_raw();
    assert_eq!(
        raw.features,
        vec![
            Feature::StableSemanticIds.numeric(),
            Feature::BackendNativeMathV1.numeric(),
        ]
    );
    assert!(raw.nodes.iter().any(|node| {
        matches!(
            node.kind,
            NodeKind::SelectedApply {
                primitive_id: 30,
                signature_id: 55,
                implementation_id: 55,
                ..
            }
        )
    }));

    let node = exp_node(&bytes);
    for (relative, replacement) in [(24, 29_u32), (28, 35), (32, 35)] {
        let mut malformed = bytes.clone();
        malformed[node + relative..node + relative + 4].copy_from_slice(&replacement.to_le_bytes());
        let error = decode_fwir(&malformed, &FwirDecodeLimits::default())
            .expect_err("mismatched exp identity");
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
}
