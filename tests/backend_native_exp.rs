use faraweave::{
    AllocationFailureInjection, Conversion, ErrorKind, EvaluationConfiguration, ExecutionProfile,
    Feature, FwirEncodeOptions, LiftMode, NodeKind, ResourceErrorReason, ResourceLimits,
    ScalarType, Value, compile_source_to_fwir, compile_source_to_verified_program,
    evaluate_expression, evaluate_expression_with_configuration,
};

#[path = "support/backend_native.rs"]
mod backend_native_support;

use backend_native_support::{
    CANONICAL_NAN_BITS, assert_backend_native_feature_required, assert_canonical_roundtrip, double,
    order_key, selected_node,
};

fn assert_finite_envelope(source: &str, reference_bits: u64, max_ulps: u64) {
    backend_native_support::assert_finite_envelope(source, reference_bits, max_ulps, 0.0);
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
    let decoded = assert_canonical_roundtrip(&bytes, "exp");
    let raw = decoded.as_raw();
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

    let node = selected_node(&bytes, 30, "exp");
    backend_native_support::assert_identity_mismatches(
        &bytes,
        node,
        &[(24, 29), (28, 35), (32, 35)],
        "exp",
    );
    assert_backend_native_feature_required(&bytes);
}
