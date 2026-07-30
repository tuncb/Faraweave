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
fn trunc_uses_contiguous_ids_lifting_promotion_and_shared_feature() {
    let program = compile_source_to_verified_program(
        "parameters[count Int]\ntrunc[1.0]\ntrunc[2]\ntrunc[(0 1)]\ntrunc[iota[count]]\n",
        "trunc-ids.faraweave",
    )
    .expect("trunc program");
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
                primitive_id: 38,
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
                63,
                63,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::Identity,
            ),
            (
                63,
                63,
                LiftMode::Scalar,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                63,
                63,
                LiftMode::Vector,
                ScalarType::Double,
                Conversion::PromoteIntToDouble,
            ),
            (
                63,
                63,
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
                        primitive_id: 38,
                        ..
                    }
                )
            })
            .and_then(|node| node.cardinality),
        Some(Cardinality::DynamicVector)
    );
}

#[test]
fn trunc_fractional_signs_integral_boundaries_and_exact_results_are_public_semantics() {
    for (source, expected) in [
        ("trunc[0.0]", 0x0000_0000_0000_0000),
        ("trunc[-0.0]", 0x8000_0000_0000_0000),
        ("trunc[inf]", 0x7ff0_0000_0000_0000),
        ("trunc[-inf]", 0xfff0_0000_0000_0000),
        ("trunc[nan]", CANONICAL_NAN_BITS),
    ] {
        assert_eq!(double(source).to_bits(), expected, "{source}");
    }

    for (source, expected_bits) in [
        ("trunc[1.5]", 0x3ff0_0000_0000_0000),
        ("trunc[-1.5]", 0xbff0_0000_0000_0000),
        ("trunc[0.5]", 0x0000_0000_0000_0000),
        ("trunc[-0.5]", 0x8000_0000_0000_0000),
        ("trunc[5e-324]", 0x0000_0000_0000_0000),
        ("trunc[-5e-324]", 0x8000_0000_0000_0000),
        ("trunc[4503599627370495.5]", 0x432f_ffff_ffff_fffe),
        ("trunc[-4503599627370495.5]", 0xc32f_ffff_ffff_fffe),
        ("trunc[4503599627370496.0]", 0x4330_0000_0000_0000),
        ("trunc[4503599627370497.0]", 0x4330_0000_0000_0001),
        ("trunc[-42.0]", 0xc045_0000_0000_0000),
        ("trunc[9223372036854775807]", 0x43e0_0000_0000_0000),
        ("trunc[1.7976931348623157e308]", 0x7fef_ffff_ffff_ffff),
    ] {
        assert_exact(source, expected_bits);
    }

    let static_vector = evaluate_expression("trunc[(0.5 -0.5 2.0)]")
        .expect("static trunc vector")
        .value;
    let Value::DoubleVector(values) = static_vector else {
        panic!("trunc vector returned {static_vector:?}");
    };
    assert_eq!(
        values
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        vec![
            0x0000_0000_0000_0000,
            0x8000_0000_0000_0000,
            0x4000_0000_0000_0000,
        ]
    );

    let mut dynamic_results = evaluate_source_with_arguments(
        "parameters[count Int]\ntrunc[iota[count]]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic trunc vector")
    .values;
    let dynamic = dynamic_results.pop().expect("dynamic trunc root");
    let Value::DoubleVector(values) = dynamic else {
        panic!("dynamic trunc returned {dynamic:?}");
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
fn trunc_resources_failures_cleanup_and_diagnostics_are_exact() {
    assert_bounded_usage(
        "trunc[0]",
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
        "trunc[(0.0 1.0 2.0)]",
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
            "trunc",
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
            "trunc",
        ),
    ] {
        assert_profile_limit_refusal(
            "trunc[(0.0 1.0 2.0)]",
            limits,
            expected_limit,
            expected_producer,
            Some(0),
        );
    }

    assert_allocation_refusal("trunc[(0.0 1.0)]", "trunc", Some(0));
    assert_empty_double_vector("trunc[Double()]", 0, 0);

    for (source, kind) in [
        ("trunc[true]", ErrorKind::TypeError),
        ("trunc[1.0 2.0]", ErrorKind::ArityError),
        ("trunc[[1.0]]", ErrorKind::TypeError),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, kind, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("trunc"), "{source}");
    }
}

#[test]
fn trunc_fwir_roundtrip_malformed_identities_and_version_are_checked() {
    let bytes = compile_source_to_fwir("trunc[(0.0 1.0)]\n", &FwirEncodeOptions::default())
        .expect("trunc artifact");
    assert_canonical_roundtrip(&bytes, "trunc");

    let node = selected_node(&bytes, 38, "trunc");
    assert_identity_mismatches(&bytes, node, &[(24, 37), (28, 62), (32, 62)], "trunc");
    assert_backend_native_feature_required(&bytes);
    assert_semantic_minor_zero_rejected(&bytes, "trunc");
}
