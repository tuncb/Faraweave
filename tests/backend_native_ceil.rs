use faraweave::{
    Cardinality, Conversion, ErrorKind, EvaluationConfiguration, Feature, FwirEncodeOptions,
    LiftMode, NodeKind, ResourceLimits, ScalarType, Value, compile_source_to_fwir,
    compile_source_to_verified_program, evaluate_expression, evaluate_source_with_arguments,
};

#[path = "support/backend_native.rs"]
mod backend_native_support;

use backend_native_support::{
    CANONICAL_NAN_BITS, assert_allocation_refusal, assert_backend_native_feature_required,
    assert_bounded_usage, assert_canonical_roundtrip, assert_empty_double_vector, assert_exact,
    assert_identity_mismatches, assert_profile_limit_refusal, assert_semantic_minor_zero_rejected,
    double, selected_node,
};

#[test]
fn ceil_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\nceil[1.0]\nceil[2]\nceil[(0 1)]\nceil[iota[count]]\n",
        "ceil-ids.faraweave",
    )
    .expect("ceil program");
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
                primitive_id: 37,
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
                62,
                62,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                62,
                62,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                62,
                62,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                62,
                62,
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
                        primitive_id: 37,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn ceil_fractional_signs_integral_boundaries_and_exact_results_are_public_semantics() {
    for (source, expected) in [
        ("ceil[0.0]", 0x0000_0000_0000_0000),
        ("ceil[-0.0]", 0x8000_0000_0000_0000),
        ("ceil[inf]", 0x7ff0_0000_0000_0000),
        ("ceil[-inf]", 0xfff0_0000_0000_0000),
        ("ceil[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, expected_bits) in [
        ("ceil[1.5]", 0x4000_0000_0000_0000),
        ("ceil[-1.5]", 0xbff0_0000_0000_0000),
        ("ceil[0.5]", 0x3ff0_0000_0000_0000),
        ("ceil[-0.5]", 0x8000_0000_0000_0000),
        ("ceil[5e-324]", 0x3ff0_0000_0000_0000),
        ("ceil[-5e-324]", 0x8000_0000_0000_0000),
        ("ceil[4503599627370495.5]", 0x4330_0000_0000_0000),
        ("ceil[-4503599627370495.5]", 0xc32f_ffff_ffff_fffe),
        ("ceil[4503599627370496.0]", 0x4330_0000_0000_0000),
        ("ceil[4503599627370497.0]", 0x4330_0000_0000_0001),
        ("ceil[-42.0]", 0xc045_0000_0000_0000),
        ("ceil[9223372036854775807]", 0x43e0_0000_0000_0000),
        ("ceil[1.7976931348623157e308]", 0x7fef_ffff_ffff_ffff),
    ] {
        assert_exact(source, expected_bits);
    }

    let static_vector = evaluate_expression("ceil[(0.5 -0.5 2.0)]")
        .expect("static ceil vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("ceil vector returned {static_vector:?}");
    };
    assert_eq!(
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        vec![
            0x3ff0_0000_0000_0000,
            0x8000_0000_0000_0000,
            0x4000_0000_0000_0000,
        ]
    );

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\nceil[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic ceil vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic ceil root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic ceil returned {dynamic:?}");
    };
    assert_eq!(values.len(), 3);
    assert_eq!(
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        vec![
            0x3ff0_0000_0000_0000,
            0x4000_0000_0000_0000,
            0x4008_0000_0000_0000,
        ]
    );
}

#[test]
fn ceil_resources_failures_cleanup_and_diagnostics_are_exact() {
    assert_bounded_usage(
        "ceil[0]",
        ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        },
        Some(Value::Double(0.0)),
        0,
        0,
        1,
        0,
    );

    assert_bounded_usage(
        "ceil[(0.0 1.0 2.0)]",
        ResourceLimits {
            max_vector_bytes: Some(24),
            max_live_evaluation_bytes: Some(48),
            max_work_units: Some(3),
            ..ResourceLimits::default()
        },
        None,
        24,
        48,
        3,
        2,
    );

    for (limits, expected_limit, expected_producer) in [
        (
            ResourceLimits {
                max_work_units: Some(2),
                max_vector_bytes: Some(24),
                max_live_evaluation_bytes: Some(48),
                ..ResourceLimits::default()
            },
            "max_work_units",
            "ceil",
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
            "ceil",
        ),
    ] {
        assert_profile_limit_refusal(
            "ceil[(0.0 1.0 2.0)]",
            limits,
            expected_limit,
            expected_producer,
            Some(0),
        );
    }

    assert_allocation_refusal("ceil[(0.0 1.0)]", "ceil", Some(0));
    assert_empty_double_vector("ceil[Double()]", 0, 0);

    for (source, kind) in [
        ("ceil[true]", ErrorKind::TypeError),
        ("ceil[1.0 2.0]", ErrorKind::ArityError),
        ("ceil[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("ceil"), "{source}");
    }
}

#[test]
fn ceil_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("ceil[(0.0 1.0)]\n", &FwirEncodeOptions::default())
        .expect("ceil artifact");
    assert_canonical_roundtrip(&bytes, "ceil");

    let node = selected_node(&bytes, 37, "ceil");
    assert_identity_mismatches(&bytes, node, &[(24, 36), (28, 61), (32, 61)], "ceil");
    assert_backend_native_feature_required(&bytes);
    assert_semantic_minor_zero_rejected(&bytes, "ceil");
}
