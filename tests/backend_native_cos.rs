use faraweave::{
    AllocationFailureInjection, Cardinality, Conversion, ErrorKind, EvaluationConfiguration,
    ExecutionProfile, Feature, FwirEncodeOptions, LiftMode, NodeKind, ResourceErrorReason,
    ResourceLimits, ScalarType, Value, compile_source_to_fwir, compile_source_to_verified_program,
    evaluate_expression, evaluate_expression_with_configuration, evaluate_source_with_arguments,
};

#[path = "support/backend_native.rs"]
mod backend_native_support;

use backend_native_support::{
    CANONICAL_NAN_BITS, assert_backend_native_feature_required, assert_canonical_roundtrip, double,
    order_key, selected_node,
};

const MAX_ULPS: u64 = 8;
const MAX_ABSOLUTE_ERROR: f64 = 3.552_713_678_800_501e-15;

fn assert_finite_envelope(source: &str, reference_bits: u64) {
    backend_native_support::assert_finite_envelope(
        source,
        reference_bits,
        MAX_ULPS,
        MAX_ABSOLUTE_ERROR,
    );
}

#[test]
fn cos_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\ncos[1.0]\ncos[2]\ncos[(0 1)]\ncos[iota[count]]\n",
        "cos-ids.faraweave",
    )
    .expect("cos program");
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
                primitive_id: 34,
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
                59,
                59,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                59,
                59,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                59,
                59,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                59,
                59,
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
                        primitive_id: 34,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn cos_special_quadrants_boundaries_and_finite_envelope_are_public_semantics() {
    for (source, expected) in [
        ("cos[0.0]", 0x3ff0_0000_0000_0000),
        ("cos[-0.0]", 0x3ff0_0000_0000_0000),
        ("cos[inf]", CANONICAL_NAN_BITS),
        ("cos[-inf]", CANONICAL_NAN_BITS),
        ("cos[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("cos[1.0]", 0x3fe1_4a28_0fb5_068c),
        ("cos[1.0471975511965976]", 0x3fe0_0000_0000_0001),
        ("cos[1.5707963267948966]", 0x3c91_a626_3314_5c07),
        ("cos[1.5707963267948963]", 0x3cb4_6989_8cc5_1702),
        ("cos[1.5707963267948968]", 0xbca7_2cec_e675_d1fd),
        ("cos[3.141592653589793]", 0xbff0_0000_0000_0000),
        ("cos[4.71238898038469]", 0xbcaa_7939_4c9e_8a0a),
        ("cos[6.283185307179586]", 0x3ff0_0000_0000_0000),
        ("cos[5e-324]", 0x3ff0_0000_0000_0000),
        ("cos[1e300]", 0xbfe2_6990_22ad_c4c1),
        ("cos[1.7976931348623157e308]", 0xbfef_ffe6_2ecf_ab75),
    ] {
        assert_finite_envelope(source, reference_bits);
    }

    let static_vector = evaluate_expression("cos[(0.0 1.0 -1.0)]")
        .expect("static cos vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("cos vector returned {static_vector:?}");
    };
    assert_eq!(values[0].to_bits(), 0x3ff0_0000_0000_0000);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3fe1_4a28_0fb5_068c, 0x3fe1_4a28_0fb5_068c])
    {
        let reference_value = f64::from_bits(reference);
        let ulps = order_key(value.to_bits()).abs_diff(order_key(reference));
        assert!(ulps <= 8 || (*value - reference_value).abs() <= MAX_ABSOLUTE_ERROR);
    }

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\ncos[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic cos vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic cos root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic cos returned {dynamic:?}");
    };
    assert_eq!(values.len(), 3);
    for (value, reference) in values.iter().zip([
        0x3fe1_4a28_0fb5_068c,
        0xbfda_a226_5753_7205,
        0xbfef_ae04_be85_e5d2,
    ]) {
        let reference_value = f64::from_bits(reference);
        let ulps = order_key(value.to_bits()).abs_diff(order_key(reference));
        assert!(ulps <= 8 || (*value - reference_value).abs() <= MAX_ABSOLUTE_ERROR);
    }
}

#[test]
fn cos_resources_failures_cleanup_and_diagnostics_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "cos[0]",
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
        "cos[(0.0 1.0 2.0)]",
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
            "cos",
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
            "cos",
        ),
    ] {
        let error = evaluate_expression_with_configuration(
            "cos[(0.0 1.0 2.0)]",
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
        "cos[(0.0 1.0)]",
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
    assert_eq!(allocation.primitive.as_deref(), Some("cos"));
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

    let empty = evaluate_expression("cos[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    for (source, kind) in [
        ("cos[true]", ErrorKind::TypeError),
        ("cos[1.0 2.0]", ErrorKind::ArityError),
        ("cos[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("cos"), "{source}");
    }
}

#[test]
fn cos_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("cos[(0.0 1.0)]\n", &FwirEncodeOptions::default())
        .expect("cos artifact");
    assert_canonical_roundtrip(&bytes, "cos");

    let node = selected_node(&bytes, 34, "cos");
    backend_native_support::assert_identity_mismatches(
        &bytes,
        node,
        &[(24, 33), (28, 58), (32, 58)],
        "cos",
    );
    assert_backend_native_feature_required(&bytes);
    backend_native_support::assert_semantic_minor_zero_rejected(&bytes, "cos");
}
