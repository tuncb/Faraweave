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
fn log_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\nlog[1.0]\nlog[2]\nlog[(1 2)]\nlog[iota[count]]\n",
        "log-ids.faraweave",
    )
    .expect("log program");
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
                primitive_id: 31,
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
                56,
                56,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                56,
                56,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                56,
                56,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                56,
                56,
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
                        primitive_id: 31,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn log_special_domain_and_finite_envelope_are_public_semantics() {
    for (source, expected) in [
        ("log[0.0]", 0xfff0_0000_0000_0000),
        ("log[-0.0]", 0xfff0_0000_0000_0000),
        ("log[1.0]", 0x0000_0000_0000_0000),
        ("log[-1.0]", CANONICAL_NAN_BITS),
        ("log[-inf]", CANONICAL_NAN_BITS),
        ("log[inf]", 0x7ff0_0000_0000_0000),
        ("log[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("log[2.0]", 0x3fe6_2e42_fefa_39ef),
        ("log[10.0]", 0x4002_6bb1_bbb5_5516),
        ("log[5e-324]", 0xc087_4385_446d_71c3),
        ("log[1.0000000000000002]", 0x3caf_ffff_ffff_ffff),
        ("log[0.9999999999999999]", 0xbca0_0000_0000_0000),
        ("log[1.7976931348623157e308]", 0x4086_2e42_fefa_39ef),
    ] {
        assert_finite_envelope(source, reference_bits);
    }

    let static_vector = evaluate_expression("log[(1.0 2.0 10.0)]")
        .expect("static log vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("log vector returned {static_vector:?}");
    };
    assert_eq!(values[0].to_bits(), 0);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3fe6_2e42_fefa_39ef, 0x4002_6bb1_bbb5_5516])
    {
        assert!(order_key(value.to_bits()).abs_diff(order_key(reference)) <= 4);
    }

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\nlog[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic log vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic log root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic log returned {dynamic:?}");
    };
    assert_eq!(values.len(), 3);
    assert_eq!(values[0].to_bits(), 0);
    for (value, reference) in values[1..]
        .iter()
        .zip([0x3fe6_2e42_fefa_39ef, 0x3ff1_93ea_7aad_030b])
    {
        assert!(order_key(value.to_bits()).abs_diff(order_key(reference)) <= 4);
    }
}

#[test]
fn log_resources_failures_cleanup_and_diagnostics_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "log[1]",
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
        "log[(1.0 2.0 10.0)]",
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
            "log",
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
            "log",
        ),
    ] {
        let error = evaluate_expression_with_configuration(
            "log[(1.0 2.0 10.0)]",
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
        let usage = error.usage.expect("post-cleanup usage");
        assert_eq!(usage.live_evaluation_bytes, 0);
    }

    let allocation = evaluate_expression_with_configuration(
        "log[(1.0 2.0)]",
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
    assert_eq!(allocation.primitive.as_deref(), Some("log"));
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

    let empty = evaluate_expression("log[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    for (source, kind) in [
        ("log[true]", ErrorKind::TypeError),
        ("log[1.0 2.0]", ErrorKind::ArityError),
        ("log[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("log"), "{source}");
    }
}

#[test]
fn log_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("log[(1.0 2.0)]\n", &FwirEncodeOptions::default())
        .expect("log artifact");
    assert_canonical_roundtrip(&bytes, "log");

    let node = selected_node(&bytes, 31, "log");
    backend_native_support::assert_identity_mismatches(
        &bytes,
        node,
        &[(24, 30), (28, 55), (32, 55)],
        "log",
    );
    assert_backend_native_feature_required(&bytes);
    backend_native_support::assert_semantic_minor_zero_rejected(&bytes, "log");
}
