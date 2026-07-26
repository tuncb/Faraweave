use faraweave::{
    AllocationFailureInjection, CompilerConfiguration, ErrorKind, EvaluationConfiguration,
    ExecutionProfile, NativePlatform, ResourceLimits, ScalarType, Type, Value, evaluate_expression,
    evaluate_expression_with_configuration, evaluate_source, format_type, format_value,
    select_c_compiler,
};

fn formatted(source: &str) -> String {
    format_value(&evaluate_expression(source).expect(source).value).expect("format")
}

#[test]
fn s16_complete_elementwise_matrix() {
    let cases = [
        ("inc[(1 2)]", "(2 3)"),
        ("dec[(1.0 2.5)]", "(0.0 1.5)"),
        ("neg[(1 -2)]", "(-1 2)"),
        ("abs[(-1.0 -2.5)]", "(1.0 2.5)"),
        ("add[10 (1 2)]", "(11 12)"),
        ("sub[(10 20) 2]", "(8 18)"),
        ("mul[(2 3) (4 5)]", "(8 15)"),
        ("equals[(1 2) 2.0]", "(false true)"),
        ("not_equals[(true false) true]", "(false true)"),
        ("not[(true false)]", "(false true)"),
        ("and[(true false) true]", "(true false)"),
        ("or[false (true false)]", "(true false)"),
        ("odd[(-3 -2)]", "(true false)"),
        ("even[(-3 -2)]", "(false true)"),
        ("is_positive[(-0.0 inf nan)]", "(false true false)"),
        ("is_negative[(0.0 -inf nan)]", "(false true false)"),
        ("less_than[(1 3) 2.0]", "(true false)"),
        ("greater_than[2.0 (1 3)]", "(true false)"),
    ];
    for (source, expected) in cases {
        assert_eq!(formatted(source), expected, "{source}");
    }
}

#[test]
fn s16_empty_singleton_promotion_and_shape_contracts() {
    assert_eq!(formatted("add[Int() 2.0]"), "()");
    assert_eq!(formatted("equals[Double() Int()]"), "()");
    assert_eq!(formatted("add[(1) 2]"), "(3)");
    let shape = evaluate_expression("add[(1) (2 3)]").expect_err("shape mismatch");
    assert_eq!(shape.kind, ErrorKind::ShapeMismatch);
    let type_error = evaluate_expression("add[(1) (true false)]").expect_err("type mismatch");
    assert_eq!(type_error.kind, ErrorKind::TypeError);
}

#[test]
fn issue54_predicates_ordering_and_nan_contracts() {
    assert_eq!(formatted("odd[-9223372036854775807]"), "true");
    assert_eq!(formatted("even[-9223372036854775808]"), "true");
    assert_eq!(formatted("equals[nan nan]"), "false");
    assert_eq!(formatted("not_equals[nan nan]"), "true");
    assert_eq!(formatted("less_than[nan inf]"), "false");
    assert_eq!(formatted("greater_than[inf nan]"), "false");
    assert_eq!(formatted("equals[-0.0 0.0]"), "true");
}

#[test]
fn canonical_binary64_format_boundaries() {
    let cases = [
        (f64::from_bits(1), "5e-324"),
        (
            f64::from_bits(0x000f_ffff_ffff_ffff),
            "2.225073858507201e-308",
        ),
        (f64::MIN_POSITIVE, "2.2250738585072014e-308"),
        (f64::MAX, "1.7976931348623157e308"),
        (1.0e20, "1e20"),
        (1.0e-7, "1e-7"),
        (9_007_199_254_740_992.0, "9.007199254740992e15"),
        (999_999.0, "999999.0"),
        (1_000_000.0, "1e6"),
        (0.0001, "0.0001"),
        (
            f64::from_bits(0x3f1a_36e2_eb1c_432c),
            "9.999999999999999e-5",
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(
            format_value(&Value::Double(value)).expect("format"),
            expected
        );
    }
}

#[test]
fn checked_arithmetic_has_no_partial_result() {
    for source in [
        "inc[9223372036854775807]",
        "dec[-9223372036854775808]",
        "neg[-9223372036854775808]",
        "abs[-9223372036854775808]",
        "add[9223372036854775807 1]",
        "sub[-9223372036854775808 1]",
        "mul[9223372036854775807 2]",
        "inc[(1 9223372036854775807 3)]",
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::DomainError, "{source}");
    }
    assert!(evaluate_source("1\ninc[9223372036854775807]\n").is_err());
}

#[test]
fn tup_structural_format_spread_and_direct_preservation() {
    assert_eq!(formatted("[]"), "[]");
    assert_eq!(formatted("[1 [2 3] (4 5)]"), "[1 [2 3] (4 5)]");
    assert_eq!(formatted("add [1 2]"), "3");
    assert_eq!(
        evaluate_expression("add[[1 2]]")
            .expect_err("direct tuple")
            .kind,
        ErrorKind::ArityError
    );
    assert_eq!(
        format_type(&Type::Tuple(vec![
            Type::Scalar(ScalarType::Int),
            Type::Vector(ScalarType::Double),
        ])),
        "Tuple<Int, Vector<Double>>"
    );
}

#[test]
fn fan_stable_id_matrix() {
    assert_eq!(formatted("fanout[1 {inc[_]}]"), "[2]");
    assert_eq!(
        formatted("fanout[iota[3] {inc[_]} {add[_ 10]}]"),
        "[(2 3 4) (11 12 13)]"
    );
    assert_eq!(
        evaluate_expression("fanout[1 {inc[1]}]")
            .expect_err("placeholder count")
            .kind,
        ErrorKind::SyntaxError
    );
    assert_eq!(
        evaluate_expression("fanout[fanout[1 {inc[_]}] {inc[_]}]")
            .expect_err("nested")
            .kind,
        ErrorKind::SyntaxError
    );
}

#[test]
fn resource_profiles_limits_and_ordinals() {
    let bounded = EvaluationConfiguration {
        profile: ExecutionProfile::BoundedV2,
        limits: ResourceLimits {
            max_vector_bytes: Some(16),
            max_tuple_table_bytes: Some(32),
            max_live_evaluation_bytes: Some(48),
            max_work_units: Some(2),
        },
        allocation_failure: AllocationFailureInjection::default(),
    };
    assert_eq!(
        format_value(
            &evaluate_expression_with_configuration("iota[2]", bounded)
                .expect("exact limit")
                .value
        )
        .expect("format"),
        "(1 2)"
    );
    assert_eq!(
        evaluate_expression_with_configuration("iota[3]", bounded)
            .expect_err("limit")
            .kind,
        ErrorKind::ResourceError
    );
    let injected = EvaluationConfiguration {
        profile: ExecutionProfile::TrustedLocalV2,
        limits: ResourceLimits::default(),
        allocation_failure: AllocationFailureInjection {
            fail_at_ordinal: Some(0),
        },
    };
    assert_eq!(
        evaluate_expression_with_configuration("iota[1]", injected)
            .expect_err("injected")
            .kind,
        ErrorKind::ResourceError
    );
}

#[test]
fn deep_unary_programs_use_iterative_parse_analysis_and_evaluation() {
    const DEPTH: usize = 4_000;

    let mut prefix = String::with_capacity(DEPTH * 4 + 1);
    for _ in 0..DEPTH {
        prefix.push_str("inc ");
    }
    prefix.push('1');
    let evaluated = evaluate_expression(&prefix).expect("4,000-deep prefix program");
    assert_eq!(evaluated.value, Value::Int(4_001));
    assert_eq!(evaluated.usage.work_units, DEPTH);

    let mut bracketed = String::with_capacity(DEPTH * 5 + 1);
    for _ in 0..DEPTH {
        bracketed.push_str("inc[");
    }
    bracketed.push('1');
    for _ in 0..DEPTH {
        bracketed.push(']');
    }
    let evaluated = evaluate_expression(&bracketed).expect("4,000-deep bracket program");
    assert_eq!(evaluated.value, Value::Int(4_001));
    assert_eq!(evaluated.usage.work_units, DEPTH);

    bracketed.pop();
    let error = evaluate_expression(&bracketed).expect_err("missing deep close");
    assert_eq!(error.kind, ErrorKind::SyntaxError);
    assert_eq!(error.message, "missing closing delimiter");
    let span = error.span.expect("deep syntax error span");
    assert_eq!(span.begin.offset, bracketed.len() + 1);
    assert_eq!(span.begin, span.end);
}

#[test]
fn deep_structural_values_and_types_format_and_drop_iteratively() {
    const DEPTH: usize = 4_096;
    let source = format!("{}7{}", "[".repeat(DEPTH), "]".repeat(DEPTH));
    let evaluated = evaluate_expression(&source).expect("4,096-deep tuple");
    assert_eq!(
        format_value(&evaluated.value).expect("deep value formatting"),
        source
    );
    assert_eq!(evaluated.usage.allocation_attempts, DEPTH);
    assert_eq!(
        format_type(&Type::RepeatedTuple {
            depth: DEPTH,
            leaf: ScalarType::Int,
        })
        .len(),
        "Tuple<".len() * DEPTH + "Int".len() + DEPTH
    );
}

#[test]
fn typed_public_api_parameter_contract() {
    let source = "parameters[n Int scale Double enabled Bool]\nn\nscale\nenabled\n";
    let result = faraweave::evaluate_source_with_arguments(
        source,
        &[Value::Int(-5), Value::Double(2.5), Value::Bool(true)],
        EvaluationConfiguration::default(),
    )
    .expect("typed binding");
    assert_eq!(
        result.values,
        vec![Value::Int(-5), Value::Double(2.5), Value::Bool(true)]
    );
    let missing =
        faraweave::evaluate_source_with_arguments(source, &[], EvaluationConfiguration::default())
            .expect_err("missing");
    assert_eq!(missing.kind, ErrorKind::ArgumentError);
}

#[test]
fn native_compiler_selection_is_explicit_then_environment_then_platform() {
    let explicit = select_c_compiler(
        Some("explicit compiler"),
        Some("environment compiler"),
        NativePlatform::GccLike,
    )
    .expect("explicit compiler");
    assert_eq!(
        explicit.configuration,
        CompilerConfiguration::ExplicitOption
    );
    assert_eq!(explicit.executable, "explicit compiler");

    let environment =
        select_c_compiler(None, Some("environment compiler"), NativePlatform::GccLike)
            .expect("environment compiler");
    assert_eq!(
        environment.configuration,
        CompilerConfiguration::Environment
    );
    assert_eq!(environment.executable, "environment compiler");

    let unix = select_c_compiler(None, None, NativePlatform::GccLike).expect("Unix fallback");
    assert_eq!(unix.configuration, CompilerConfiguration::Fallback);
    assert_eq!(unix.executable, "cc");
    let windows =
        select_c_compiler(None, None, NativePlatform::WindowsMsvc).expect("Windows fallback");
    assert_eq!(windows.executable, "cl.exe");

    assert!(select_c_compiler(Some(""), Some("cc"), NativePlatform::GccLike).is_err());
}
