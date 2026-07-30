use faraweave::{
    AllocationFailureInjection, Conversion, ErrorKind, EvaluationConfiguration, ExecutionProfile,
    Feature, LiftMode, NodeKind, ResourceErrorReason, ResourceLimits, Value,
    compile_source_to_verified_program, evaluate_expression,
    evaluate_expression_with_configuration,
};

#[path = "support/backend_native.rs"]
mod backend_native_support;

use backend_native_support::{CANONICAL_NAN_BITS, assert_finite_envelope, double};

#[test]
fn sqrt_uses_reserved_ids_double_selection_lifting_and_feature_seven() {
    let program = compile_source_to_verified_program(
        "sqrt[4.0]\nsqrt[9]\nsqrt[(1.0 4.0)]\n",
        "sqrt-ids.faraweave",
    )
    .expect("sqrt program");
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
            (29, 54, 54, LiftMode::Scalar, faraweave::ScalarType::Double),
            (29, 54, 54, LiftMode::Scalar, faraweave::ScalarType::Double),
            (29, 54, 54, LiftMode::Vector, faraweave::ScalarType::Double),
        ]
    );
    assert_eq!(raw.edges[0].conversion, Conversion::Identity);
    assert_eq!(raw.edges[1].conversion, Conversion::PromoteIntToDouble);
    assert_eq!(raw.edges[2].conversion, Conversion::Identity);

    let bool_error = evaluate_expression("sqrt[true]").expect_err("Bool must be rejected");
    assert_eq!(bool_error.kind, ErrorKind::TypeError);
    assert_eq!(bool_error.primitive.as_deref(), Some("sqrt"));
}

#[test]
fn sqrt_special_values_finite_envelope_and_vectors_are_public_semantics() {
    for (source, expected) in [
        ("sqrt[0.0]", 0x0000_0000_0000_0000),
        ("sqrt[-0.0]", 0x8000_0000_0000_0000),
        ("sqrt[inf]", 0x7ff0_0000_0000_0000),
        ("sqrt[-1.0]", CANONICAL_NAN_BITS),
        ("sqrt[-inf]", CANONICAL_NAN_BITS),
        ("sqrt[nan]", CANONICAL_NAN_BITS),
        ("sqrt[4.0]", 0x4000_0000_0000_0000),
        ("sqrt[9]", 0x4008_0000_0000_0000),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, reference_bits) in [
        ("sqrt[5e-324]", 0x1e60_0000_0000_0000),
        ("sqrt[1.7976931348623157e308]", 0x5fefffffffffffff),
        ("sqrt[1e-300]", 1.0e-150_f64.to_bits()),
        ("sqrt[1e300]", 1.0e150_f64.to_bits()),
    ] {
        assert_finite_envelope(source, reference_bits, 1, 0.0);
    }

    assert_eq!(
        evaluate_expression("sqrt[(1.0 4.0 9.0 16.0)]")
            .expect("vector sqrt")
            .value,
        Value::DoubleVector(vec![1.0, 2.0, 3.0, 4.0])
    );
}

#[test]
fn sqrt_resource_work_and_allocation_refusals_are_exact() {
    let scalar = evaluate_expression_with_configuration(
        "sqrt[4]",
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
    assert_eq!(scalar.value, Value::Double(2.0));
    assert_eq!(scalar.usage.work_units, 1);
    assert_eq!(scalar.usage.allocation_attempts, 0);

    let vector = evaluate_expression_with_configuration(
        "sqrt[(1.0 4.0 9.0)]",
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

    let work = evaluate_expression_with_configuration(
        "sqrt[(1.0 4.0 9.0)]",
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits: ResourceLimits {
                max_vector_bytes: Some(24),
                max_live_evaluation_bytes: Some(48),
                max_work_units: Some(2),
                ..ResourceLimits::default()
            },
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect_err("one-past work");
    assert_eq!(work.kind, ErrorKind::ResourceError);
    assert_eq!(work.primitive.as_deref(), Some("sqrt"));
    let work_context = work.resource.expect("work context");
    assert_eq!(work_context.reason, ResourceErrorReason::ProfileLimit);
    assert_eq!(work_context.limit_kind, Some("max_work_units"));
    assert_eq!(work_context.refused_charge, Some(3));
    assert_eq!(work_context.allocation_ordinal, None);

    let allocation = evaluate_expression_with_configuration(
        "sqrt[(1.0 4.0)]",
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
    assert_eq!(allocation.primitive.as_deref(), Some("sqrt"));
    assert_eq!(
        allocation.resource.expect("allocation context").reason,
        ResourceErrorReason::AllocationUnavailable
    );

    let empty = evaluate_expression("sqrt[Double()]").expect("empty vector");
    assert_eq!(empty.value, Value::DoubleVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);
}
