use faraweave::{
    AllocationFailureInjection, CompilerConfiguration, DomainErrorReason, ErrorKind,
    EvaluationConfiguration, ExecutionProfile, NativePlatform, ResourceLimits, ScalarType,
    SourceLocation, Type, Value, evaluate_expression, evaluate_expression_with_configuration,
    evaluate_source, format_type, format_value, select_c_compiler,
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
        ("div[(8 9 10) 2]", "(4 4 5)"),
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
    assert_eq!(formatted("div[Int() 0]"), "()");
    assert_eq!(formatted("div[Int() Double()]"), "()");
    assert_eq!(formatted("div[(7) 2]"), "(3)");
    let div_shape = evaluate_expression("div[(1) (0 1)]").expect_err("shape before domain");
    assert_eq!(div_shape.kind, ErrorKind::ShapeMismatch);
}

#[test]
fn div_integer_faults_and_strict_binary64_are_exact() {
    for (source, expected) in [
        ("div[7 3]", "2"),
        ("div[-7 3]", "-2"),
        ("div[7 -3]", "-2"),
        ("div[-7 -3]", "2"),
        ("div[1 2.0]", "0.5"),
        ("div[1.0 2]", "0.5"),
        ("div[(8 9 10) (2 3 5)]", "(4 3 2)"),
    ] {
        assert_eq!(formatted(source), expected, "{source}");
    }

    let scalar_zero = evaluate_expression("div[7 0]").expect_err("integer division by zero");
    assert_eq!(scalar_zero.kind, ErrorKind::DomainError);
    assert_eq!(scalar_zero.message, "div failed: division_by_zero");
    assert_eq!(scalar_zero.primitive.as_deref(), Some("div"));
    let scalar_context = scalar_zero.domain.expect("structured division domain");
    assert_eq!(scalar_context.reason, DomainErrorReason::DivisionByZero);
    assert_eq!(
        scalar_context.parameter_types,
        [ScalarType::Int, ScalarType::Int]
    );
    assert_eq!(scalar_context.result_type, ScalarType::Int);
    assert_eq!(scalar_context.operands, [Value::Int(7), Value::Int(0)]);
    assert_eq!(scalar_context.element_index, None);

    let lifted =
        evaluate_expression("div[(8 9 10) (2 0 0)]").expect_err("lowest integer division failure");
    assert_eq!(
        lifted.message,
        "div failed: division_by_zero at result index 1"
    );
    let lifted_context = lifted.domain.expect("lifted division context");
    assert_eq!(lifted_context.reason, DomainErrorReason::DivisionByZero);
    assert_eq!(lifted_context.operands, [Value::Int(9), Value::Int(0)]);
    assert_eq!(lifted_context.element_index, Some(1));

    let overflow =
        evaluate_expression("div[-9223372036854775808 -1]").expect_err("integer division overflow");
    assert_eq!(overflow.message, "div failed: integer_overflow");
    let overflow_context = overflow.domain.expect("overflow context");
    assert_eq!(overflow_context.reason, DomainErrorReason::IntegerOverflow);

    let first = evaluate_expression("div[(-9223372036854775808 4) (-1 0)]")
        .expect_err("lowest overflow precedes later zero");
    let first_context = first.domain.expect("first fault context");
    assert_eq!(first_context.reason, DomainErrorReason::IntegerOverflow);
    assert_eq!(first_context.element_index, Some(0));

    for (source, expected_bits) in [
        ("div[1.0 0.0]", 0x7ff0_0000_0000_0000),
        ("div[1 0.0]", 0x7ff0_0000_0000_0000),
        ("div[-1.0 0.0]", 0xfff0_0000_0000_0000),
        ("div[1.0 -0.0]", 0xfff0_0000_0000_0000),
        ("div[-0.0 2.0]", 0x8000_0000_0000_0000),
        ("div[1.0 inf]", 0x0000_0000_0000_0000),
        ("div[1e-323 2.0]", 0x0000_0000_0000_0001),
        ("div[0.0 0.0]", 0x7ff8_0000_0000_0000),
        ("div[inf inf]", 0x7ff8_0000_0000_0000),
        ("div[nan 1.0]", 0x7ff8_0000_0000_0000),
    ] {
        let value = evaluate_expression(source).expect(source).value;
        let Value::Double(value) = value else {
            panic!("{source} did not return Double");
        };
        assert_eq!(value.to_bits(), expected_bits, "{source}");
    }
}

#[test]
fn div_bool_operands_are_rejected_with_exact_public_type_diagnostics() {
    let cases = [
        (
            "div[true 2]",
            1,
            5,
            vec![
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Int),
            ],
        ),
        (
            "div[2 false]",
            2,
            7,
            vec![
                Type::Scalar(ScalarType::Int),
                Type::Scalar(ScalarType::Bool),
            ],
        ),
        (
            "div[true false]",
            1,
            5,
            vec![
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Bool),
            ],
        ),
        (
            "div[(true false) 2]",
            1,
            5,
            vec![
                Type::Vector(ScalarType::Bool),
                Type::Scalar(ScalarType::Int),
            ],
        ),
        (
            "div[2.0 (true false)]",
            2,
            9,
            vec![
                Type::Scalar(ScalarType::Double),
                Type::Vector(ScalarType::Bool),
            ],
        ),
        (
            "div[(1 2) false]",
            2,
            11,
            vec![
                Type::Vector(ScalarType::Int),
                Type::Scalar(ScalarType::Bool),
            ],
        ),
    ];

    for (source, position, offset, actual_types) in cases {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.kind.diagnostic_name(), "TypeError", "{source}");
        assert_eq!(error.primitive.as_deref(), Some("div"), "{source}");
        assert_eq!(error.argument_position, Some(position), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset,
                line: 1,
                column: offset,
            },
            "{source}"
        );
        assert_eq!(error.span, None, "{source}");
        assert_eq!(
            error.message,
            format!(
                "div arguments do not match an accepted signature; first unsupported argument is {position}"
            ),
            "{source}"
        );
        assert!(error.expected_types.is_empty(), "{source}");
        assert_eq!(error.actual_types, actual_types, "{source}");
    }
}

#[test]
fn length_accepts_all_vector_types_empty_and_dynamic_cardinalities() {
    for (source, expected) in [
        ("length[(true false true)]", "3"),
        ("length[(7 -3 11 0)]", "4"),
        ("length[(1.0 -0.0 inf nan)]", "4"),
        ("length[Bool()]", "0"),
        ("length[Int()]", "0"),
        ("length[Double()]", "0"),
        ("length iota 5", "5"),
    ] {
        assert_eq!(formatted(source), expected, "{source}");
    }

    let parameterized = faraweave::evaluate_source_with_arguments(
        "parameters[n Int]\nlength iota n\n",
        &[Value::Int(6)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic parameterized length");
    assert_eq!(parameterized.values, [Value::Int(6)]);
}

#[test]
fn length_rejects_scalar_and_tuple_inputs_with_exact_static_diagnostics() {
    for (source, actual_type) in [
        ("length[1]", Type::Scalar(ScalarType::Int)),
        (
            "length[[1 2]]",
            Type::Tuple(vec![
                Type::Scalar(ScalarType::Int),
                Type::Scalar(ScalarType::Int),
            ]),
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("length"), "{source}");
        assert_eq!(error.argument_position, Some(1), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset: 8,
                line: 1,
                column: 8,
            },
            "{source}"
        );
        assert_eq!(
            error.message,
            "length arguments do not match an accepted signature; first unsupported argument is 1",
            "{source}"
        );
        assert_eq!(error.actual_types, [actual_type], "{source}");
        assert!(error.expected_types.is_empty(), "{source}");
    }
}

#[test]
fn sort_covers_exhaustive_small_bools_integer_edges_and_total_double_order() {
    for length in 0..=6 {
        for mask in 0..(1_usize << length) {
            let source = if length == 0 {
                "sort[Bool()]".to_owned()
            } else {
                let values = (0..length)
                    .map(|index| {
                        if mask & (1_usize << index) == 0 {
                            "false"
                        } else {
                            "true"
                        }
                    })
                    .collect::<Vec<_>>()
                    .join(" ");
                format!("sort[({values})]")
            };
            let mut expected = vec![false; length - mask.count_ones() as usize];
            expected.extend(std::iter::repeat_n(true, mask.count_ones() as usize));
            assert_eq!(
                evaluate_expression(&source).expect(&source).value,
                Value::BoolVector(expected),
                "{source}"
            );
        }
    }

    for (source, expected) in [
        (
            "sort[(9223372036854775807 0 -9223372036854775808 7 -3)]",
            vec![i64::MIN, -3, 0, 7, i64::MAX],
        ),
        ("sort[(3 3 2 1 1)]", vec![1, 1, 2, 3, 3]),
        ("sort[(1 2 3 4)]", vec![1, 2, 3, 4]),
        ("sort[(4 3 2 1)]", vec![1, 2, 3, 4]),
    ] {
        assert_eq!(
            evaluate_expression(source).expect(source).value,
            Value::IntVector(expected),
            "{source}"
        );
    }

    let doubles =
        evaluate_expression("sort[(nan inf -0.0 1.0 -inf 0.0 -1.0 nan)]").expect("double sort");
    let Value::DoubleVector(doubles) = doubles.value else {
        panic!("double sort result changed type");
    };
    assert_eq!(
        doubles
            .iter()
            .map(|value| value.to_bits())
            .collect::<Vec<_>>(),
        [
            f64::NEG_INFINITY.to_bits(),
            (-1.0_f64).to_bits(),
            (-0.0_f64).to_bits(),
            0.0_f64.to_bits(),
            1.0_f64.to_bits(),
            f64::INFINITY.to_bits(),
            0x7ff8_0000_0000_0000,
            0x7ff8_0000_0000_0000,
        ]
    );
}

#[test]
fn sort_accepts_empty_singleton_and_dynamic_vectors_and_rejects_nonvectors() {
    for (source, expected) in [
        ("sort[Bool()]", "()"),
        ("sort[Int()]", "()"),
        ("sort[Double()]", "()"),
        ("sort[(true)]", "(true)"),
        ("sort[(-7)]", "(-7)"),
        ("sort[(-0.0)]", "(-0.0)"),
        ("sort iota 5", "(1 2 3 4 5)"),
    ] {
        assert_eq!(formatted(source), expected, "{source}");
    }

    let dynamic = faraweave::evaluate_source_with_arguments(
        "parameters[n Int]\nsort iota n\n",
        &[Value::Int(6)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic parameterized sort");
    assert_eq!(dynamic.values, [Value::IntVector(vec![1, 2, 3, 4, 5, 6])]);

    for (source, actual_type) in [
        ("sort[1]", Type::Scalar(ScalarType::Int)),
        (
            "sort[[1 2]]",
            Type::Tuple(vec![
                Type::Scalar(ScalarType::Int),
                Type::Scalar(ScalarType::Int),
            ]),
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("sort"), "{source}");
        assert_eq!(error.argument_position, Some(1), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset: 6,
                line: 1,
                column: 6,
            },
            "{source}"
        );
        assert_eq!(
            error.message,
            "sort arguments do not match an accepted signature; first unsupported argument is 1",
            "{source}"
        );
        assert_eq!(error.actual_types, [actual_type], "{source}");
        assert!(error.expected_types.is_empty(), "{source}");
    }
}

#[test]
fn sum_accepts_numeric_empty_nonempty_and_dynamic_vectors_and_rejects_other_values() {
    for (source, expected) in [
        ("sum[Int()]", "0"),
        ("sum[Double()]", "0.0"),
        ("sum[(1 2 3 -4)]", "2"),
        ("sum[(1.5 -0.5 2.0)]", "3.0"),
        ("sum iota 5", "15"),
    ] {
        assert_eq!(formatted(source), expected, "{source}");
    }

    let dynamic = faraweave::evaluate_source_with_arguments(
        "parameters[n Int]\nsum iota n\n",
        &[Value::Int(6)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic parameterized sum");
    assert_eq!(dynamic.values, [Value::Int(21)]);

    for (source, actual_type) in [
        ("sum[1]", Type::Scalar(ScalarType::Int)),
        ("sum[(true false)]", Type::Vector(ScalarType::Bool)),
        (
            "sum[[1 2]]",
            Type::Tuple(vec![
                Type::Scalar(ScalarType::Int),
                Type::Scalar(ScalarType::Int),
            ]),
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("sum"), "{source}");
        assert_eq!(error.argument_position, Some(1), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset: 5,
                line: 1,
                column: 5,
            },
            "{source}"
        );
        assert_eq!(
            error.message,
            "sum arguments do not match an accepted signature; first unsupported argument is 1",
            "{source}"
        );
        assert_eq!(error.actual_types, [actual_type], "{source}");
        assert!(error.expected_types.is_empty(), "{source}");
    }
}

#[test]
fn sum_int_overflow_reports_the_first_reduction_step_and_operands() {
    for (source, expected_index, expected_operands) in [
        (
            "sum[(9223372036854775807 1 -1)]",
            1,
            vec![Value::Int(i64::MAX), Value::Int(1)],
        ),
        (
            "sum[(-9223372036854775808 -1 1)]",
            1,
            vec![Value::Int(i64::MIN), Value::Int(-1)],
        ),
        (
            "sum[(9223372036854775807 -1 1 1)]",
            3,
            vec![Value::Int(i64::MAX), Value::Int(1)],
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::DomainError, "{source}");
        assert_eq!(
            error.message,
            format!("sum failed: integer_overflow at result index {expected_index}"),
            "{source}"
        );
        assert_eq!(error.primitive.as_deref(), Some("sum"), "{source}");
        let domain = error.domain.expect("structured sum overflow");
        assert_eq!(domain.reason, DomainErrorReason::IntegerOverflow);
        assert_eq!(domain.parameter_types, [ScalarType::Int, ScalarType::Int]);
        assert_eq!(domain.result_type, ScalarType::Int);
        assert_eq!(domain.operands, expected_operands);
        assert_eq!(domain.element_index, Some(expected_index));
    }
}

#[test]
fn sum_double_is_left_to_right_strict_and_preserves_special_value_bits() {
    for (source, expected_bits) in [
        ("sum[Double()]", 0.0_f64.to_bits()),
        ("sum[(-0.0)]", 0.0_f64.to_bits()),
        ("sum[(1.0 -1.0)]", 0.0_f64.to_bits()),
        ("sum[(1e16 -1e16 1.0)]", 1.0_f64.to_bits()),
        ("sum[(1e16 1.0 -1e16)]", 0.0_f64.to_bits()),
        ("sum[(5e-324 5e-324)]", 2),
        ("sum[(inf 1.0)]", f64::INFINITY.to_bits()),
        ("sum[(-inf -1.0)]", f64::NEG_INFINITY.to_bits()),
        ("sum[(inf -inf)]", 0x7ff8_0000_0000_0000),
        ("sum[(nan 1.0)]", 0x7ff8_0000_0000_0000),
    ] {
        let value = evaluate_expression(source).expect(source).value;
        let Value::Double(value) = value else {
            panic!("sum Double result changed type");
        };
        assert_eq!(value.to_bits(), expected_bits, "{source}");
    }
}

#[test]
fn all_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_false_position() {
    assert_eq!(formatted("all_of[Bool()]"), "true");
    assert_eq!(formatted("all_of[(true true true true)]"), "true");
    for source in [
        "all_of[(false true true true)]",
        "all_of[(true false true true)]",
        "all_of[(true true false true)]",
        "all_of[(true true true false)]",
    ] {
        assert_eq!(formatted(source), "false", "{source}");
    }

    for (count, expected) in [(0, true), (1, true), (6, true)] {
        let result = faraweave::evaluate_source_with_arguments(
            "parameters[n Int]\nall_of equals[iota n iota n]\n",
            &[Value::Int(count)],
            EvaluationConfiguration::default(),
        )
        .expect("dynamic parameterized all_of");
        assert_eq!(result.values, [Value::Bool(expected)], "count {count}");
    }
    let dynamic_false = faraweave::evaluate_source_with_arguments(
        "parameters[n Int]\nall_of equals[iota n (1 0 3)]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic false all_of");
    assert_eq!(dynamic_false.values, [Value::Bool(false)]);
}

#[test]
fn all_of_rejects_non_bool_vectors_scalars_and_tuples_statically() {
    for (source, actual_type) in [
        ("all_of[true]", Type::Scalar(ScalarType::Bool)),
        ("all_of[(1 2)]", Type::Vector(ScalarType::Int)),
        ("all_of[(1.0 2.0)]", Type::Vector(ScalarType::Double)),
        (
            "all_of[[true false]]",
            Type::Tuple(vec![
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Bool),
            ]),
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("all_of"), "{source}");
        assert_eq!(error.argument_position, Some(1), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset: 8,
                line: 1,
                column: 8,
            },
            "{source}"
        );
        assert_eq!(
            error.message,
            "all_of arguments do not match an accepted signature; first unsupported argument is 1",
            "{source}"
        );
        assert_eq!(error.actual_types, [actual_type], "{source}");
        assert!(error.expected_types.is_empty(), "{source}");
    }
}

#[test]
fn any_of_accepts_empty_static_and_dynamic_bool_vectors_and_every_true_position() {
    assert_eq!(formatted("any_of[Bool()]"), "false");
    assert_eq!(formatted("any_of[(false false false false)]"), "false");
    for source in [
        "any_of[(true false false false)]",
        "any_of[(false true false false)]",
        "any_of[(false false true false)]",
        "any_of[(false false false true)]",
    ] {
        assert_eq!(formatted(source), "true", "{source}");
    }

    for (count, expected) in [(0, false), (1, true), (6, true)] {
        let result = faraweave::evaluate_source_with_arguments(
            "parameters[n Int]\nany_of equals[iota n iota n]\n",
            &[Value::Int(count)],
            EvaluationConfiguration::default(),
        )
        .expect("dynamic parameterized any_of");
        assert_eq!(result.values, [Value::Bool(expected)], "count {count}");
    }
    let dynamic_false = faraweave::evaluate_source_with_arguments(
        "parameters[n Int]\nany_of not equals[iota n iota n]\n",
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
    )
    .expect("dynamic false any_of");
    assert_eq!(dynamic_false.values, [Value::Bool(false)]);
}

#[test]
fn any_of_rejects_non_bool_vectors_scalars_and_tuples_statically() {
    for (source, actual_type) in [
        ("any_of[false]", Type::Scalar(ScalarType::Bool)),
        ("any_of[(1 2)]", Type::Vector(ScalarType::Int)),
        ("any_of[(1.0 2.0)]", Type::Vector(ScalarType::Double)),
        (
            "any_of[[true false]]",
            Type::Tuple(vec![
                Type::Scalar(ScalarType::Bool),
                Type::Scalar(ScalarType::Bool),
            ]),
        ),
    ] {
        let error = evaluate_expression(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::TypeError, "{source}");
        assert_eq!(error.primitive.as_deref(), Some("any_of"), "{source}");
        assert_eq!(error.argument_position, Some(1), "{source}");
        assert_eq!(
            error.location,
            SourceLocation {
                offset: 8,
                line: 1,
                column: 8,
            },
            "{source}"
        );
        assert_eq!(
            error.message,
            "any_of arguments do not match an accepted signature; first unsupported argument is 1",
            "{source}"
        );
        assert_eq!(error.actual_types, [actual_type], "{source}");
        assert!(error.expected_types.is_empty(), "{source}");
    }
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
fn line_comments_preserve_evaluation_and_compact_deep_paths() {
    let program = evaluate_source("# prologue\r\n1# first\r\ninc[# argument\n1]# eof")
        .expect("commented program");
    assert_eq!(program.values, vec![Value::Int(1), Value::Int(2)]);
    assert!(
        evaluate_source("# comment-only 🦀")
            .expect("comment-only program")
            .values
            .is_empty()
    );

    const DEPTH: usize = 256;
    let mut unary = String::new();
    for _ in 0..DEPTH {
        unary.push_str("inc[# layer 🦀\r\n");
    }
    unary.push('1');
    for _ in 0..DEPTH {
        unary.push(']');
    }
    assert_eq!(
        evaluate_expression(&unary)
            .expect("commented deep unary")
            .value,
        Value::Int(DEPTH as i64 + 1)
    );

    const TUPLE_DEPTH: usize = 4_000;
    let mut tuple = String::with_capacity(TUPLE_DEPTH * 2 + 32);
    for _ in 0..TUPLE_DEPTH {
        tuple.push('[');
    }
    tuple.push('1');
    tuple.push_str("# closing trivia 🦀\r\n");
    for _ in 0..TUPLE_DEPTH {
        tuple.push(']');
    }
    let allocation_error = evaluate_expression_with_configuration(
        &tuple,
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect_err("commented deep tuple allocation refusal");
    assert_eq!(allocation_error.kind, ErrorKind::ResourceError);
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
