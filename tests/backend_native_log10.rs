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

fn assert_finite_envelope(source: &str, reference_bits: u64) {
    backend_native_support::assert_finite_envelope(source, reference_bits, 4, 0.0);
}

#[test]
fn log10_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\nlog10[1.0]\nlog10[2]\nlog10[(1 10)]\nlog10[iota[count]]\n",
        "log10-ids.faraweave",
    )
    .expect("log10 program");
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
                primitive_id: 32,
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
                57,
                57,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                57,
                57,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                57,
                57,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                57,
                57,
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
                        primitive_id: 32,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn log10_special_domain_powers_and_finite_envelope_are_public_semantics() {
    for (source, expected) in [
        ("log10[0.0]", 0xfff0_0000_0000_0000),
        ("log10[-0.0]", 0xfff0_0000_0000_0000),
        ("log10[1.0]", 0x0000_0000_0000_0000),
        ("log10[-1.0]", CANONICAL_NAN_BITS),
        ("log10[-inf]", CANONICAL_NAN_BITS),
        ("log10[inf]", 0x7ff0_0000_0000_0000),
        ("log10[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("log10[0.1]", 0xbff0_0000_0000_0000),
        ("log10[2.0]", 0x3fd3_4413_509f_79ff),
        ("log10[10.0]", 0x3ff0_0000_0000_0000),
        ("log10[1000.0]", 0x4008_0000_0000_0000),
        ("log10[5e-324]", 0xc074_34e6_420f_4374),
        ("log10[9.999999999999996]", 0x3fef_ffff_ffff_ffff),
        ("log10[10.000000000000004]", 0x3ff0_0000_0000_0001),
        ("log10[1.7976931348623157e308]", 0x4073_4413_509f_79ff),
    ] {
        assert_finite_envelope(source, reference_bits);
    }

    let static_vector = evaluate_expression("log10[(1.0 10.0 1000.0)]")
        .expect("static log10 vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("log10 vector returned {static_vector:?}");
    };
    assert_eq!(values[0].to_bits(), 0);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3ff0_0000_0000_0000, 0x4008_0000_0000_0000])
    {
        assert!(order_key(value.to_bits()).abs_diff(order_key(reference)) <= 4);
    }

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\nlog10[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic log10 vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic log10 root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic log10 returned {dynamic:?}");
    };
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].to_bits(), 0);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3fd3_4413_509f_79ff, 0x3fde_8927_964f_d5fd])
    {
        assert!(order_key(value.to_bits()).abs_diff(order_key(reference)) <= 4);
    }
}

#[test]
fn log10_resources_failures_cleanup_and_diagnostics_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "log10[1]",
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
        "log10[(1.0 10.0 1000.0)]",
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
            "log10",
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
            "log10",
        ),
    ] {
        let error = evaluate_expression_with_configuration(
            "log10[(1.0 10.0 1000.0)]",
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
        "log10[(1.0 10.0)]",
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
    assert_eq!(allocation.primitive.as_deref(), Some("log10"));
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

    let empty = evaluate_expression("log10[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    for (source, kind) in [
        ("log10[true]", ErrorKind::TypeError),
        ("log10[1.0 2.0]", ErrorKind::ArityError),
        ("log10[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("log10"), "{source}");
    }
}

#[test]
fn log10_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("log10[(1.0 10.0)]\n", &FwirEncodeOptions::default())
        .expect("log10 artifact");
    assert_canonical_roundtrip(&bytes, "log10");

    let node = selected_node(&bytes, 32, "log10");
    backend_native_support::assert_identity_mismatches(
        &bytes,
        node,
        &[(24, 31), (28, 56), (32, 56)],
        "log10",
    );
    assert_backend_native_feature_required(&bytes);
    backend_native_support::assert_semantic_minor_zero_rejected(&bytes, "log10");
}
