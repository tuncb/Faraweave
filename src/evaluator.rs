use crate::interpreter::{
    decode_verified_arguments, evaluate_verified_program, evaluate_verified_program_with_observer,
};
use crate::lowering::compile_parsed_source;
use crate::parser::{
    Program, first_tuple_location, parse, program_contains_tuple, validate_parameter_declarations,
};
use crate::primitive::resolve_names;
use crate::resources::ResourceContext;
use crate::{
    AllocationFailureInjection, Error, ErrorKind, ExecutionProfile, ParameterErrorContext,
    ParameterErrorReason, ResourceLimits, ResourceObserver, ResourceUsage, RootPresentation,
    SourceLocation, Value, VerifiedProgram, format_value,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EvaluationConfiguration {
    pub profile: ExecutionProfile,
    pub limits: ResourceLimits,
    pub allocation_failure: AllocationFailureInjection,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ValueResult {
    pub value: Value,
    pub usage: ResourceUsage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ProgramResult {
    pub values: Vec<Value>,
    pub usage: ResourceUsage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RunnerEvaluationResult {
    pub values: Vec<Value>,
    pub formatted: Vec<String>,
    pub presentations: Vec<RootPresentation>,
    pub usage: ResourceUsage,
}

pub fn evaluate_expression(source: &str) -> Result<ValueResult, Error> {
    evaluate_expression_with_configuration(source, EvaluationConfiguration::default())
}

pub fn evaluate_expression_with_configuration(
    source: &str,
    configuration: EvaluationConfiguration,
) -> Result<ValueResult, Error> {
    evaluate_expression_observed(source, configuration, None)
}

pub fn evaluate_expression_with_observer(
    source: &str,
    configuration: EvaluationConfiguration,
    observer: ResourceObserver,
) -> Result<ValueResult, Error> {
    evaluate_expression_observed(source, configuration, Some(observer))
}

fn evaluate_expression_observed(
    source: &str,
    configuration: EvaluationConfiguration,
    observer: Option<ResourceObserver>,
) -> Result<ValueResult, Error> {
    validate_configuration(configuration)?;
    let parsed = parse(source)?;
    validate_expression_surface(&parsed)?;
    resolve_names(&parsed)?;
    validate_tuple_profile(&parsed, configuration)?;
    let program = compile_parsed_source(source, &parsed)
        .map_err(crate::lowering::CompileError::into_evaluation_error)?;
    let result = evaluate_compiled(&program, &[], configuration, observer)?;
    Ok(ValueResult {
        value: result.values.into_iter().next().ok_or_else(|| {
            Error::new(
                ErrorKind::EmptyExpression,
                SourceLocation::start(),
                "expected an expression",
            )
        })?,
        usage: result.usage,
    })
}

pub fn evaluate_source(source: &str) -> Result<ProgramResult, Error> {
    evaluate_source_with_configuration(source, EvaluationConfiguration::default())
}

pub fn evaluate_source_with_configuration(
    source: &str,
    configuration: EvaluationConfiguration,
) -> Result<ProgramResult, Error> {
    evaluate_source_with_arguments(source, &[], configuration)
}

pub fn evaluate_source_with_arguments(
    source: &str,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
) -> Result<ProgramResult, Error> {
    evaluate_source_with_arguments_observed(source, arguments, configuration, None)
}

pub fn evaluate_source_with_arguments_and_observer(
    source: &str,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: ResourceObserver,
) -> Result<ProgramResult, Error> {
    evaluate_source_with_arguments_observed(source, arguments, configuration, Some(observer))
}

fn evaluate_source_with_arguments_observed(
    source: &str,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: Option<ResourceObserver>,
) -> Result<ProgramResult, Error> {
    validate_configuration(configuration)?;
    let parsed = parse(source)?;
    validate_parameter_declarations(&parsed)?;
    resolve_names(&parsed)?;
    validate_tuple_profile(&parsed, configuration)?;
    let program = compile_parsed_source(source, &parsed)
        .map_err(crate::lowering::CompileError::into_evaluation_error)?;
    evaluate_compiled(&program, arguments, configuration, observer)
}

pub fn evaluate_runner_source(
    source: &str,
    arguments: &[&str],
) -> Result<RunnerEvaluationResult, Error> {
    let parsed = parse(source)?;
    validate_parameter_declarations(&parsed)?;
    resolve_names(&parsed)?;
    let program = compile_parsed_source(source, &parsed)
        .map_err(crate::lowering::CompileError::into_evaluation_error)?;
    let decoded = decode_verified_arguments(&program, arguments)?;
    let result = evaluate_verified_program(&program, &decoded, EvaluationConfiguration::default())?;
    let (formatted, presentations) = format_runner_values(&program, &result.values)?;
    Ok(RunnerEvaluationResult {
        values: result.values,
        formatted,
        presentations,
        usage: result.usage,
    })
}

pub(crate) fn format_runner_values(
    program: &VerifiedProgram,
    values: &[Value],
) -> Result<(Vec<String>, Vec<RootPresentation>), Error> {
    let mut formatted = Vec::new();
    formatted.try_reserve_exact(values.len()).map_err(|_| {
        Error::new(
            ErrorKind::FormattingError,
            SourceLocation::start(),
            "unable to allocate formatted output",
        )
    })?;
    let mut presentations = Vec::new();
    presentations.try_reserve_exact(values.len()).map_err(|_| {
        Error::new(
            ErrorKind::FormattingError,
            SourceLocation::start(),
            "unable to allocate formatted output",
        )
    })?;
    for (value, root) in values.iter().zip(&program.as_raw().roots) {
        let text = match root.presentation {
            RootPresentation::CanonicalValue => format_value(value)?,
            RootPresentation::RawString => {
                let Value::String(value) = value else {
                    return Err(Error::new(
                        ErrorKind::TypeError,
                        SourceLocation::start(),
                        "verified raw String presentation invariant failed",
                    ));
                };
                let mut text = String::new();
                text.try_reserve_exact(value.len()).map_err(|_| {
                    Error::new(
                        ErrorKind::FormattingError,
                        SourceLocation::start(),
                        "unable to allocate formatted output",
                    )
                })?;
                text.push_str(value);
                text
            }
        };
        formatted.push(text);
        presentations.push(root.presentation);
    }
    if formatted.len() != values.len() || presentations.len() != values.len() {
        return Err(Error::new(
            ErrorKind::TypeError,
            SourceLocation::start(),
            "verified root presentation count invariant failed",
        ));
    }
    Ok((formatted, presentations))
}

fn evaluate_compiled(
    program: &VerifiedProgram,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: Option<ResourceObserver>,
) -> Result<ProgramResult, Error> {
    match observer {
        Some(observer) => {
            evaluate_verified_program_with_observer(program, arguments, configuration, observer)
        }
        None => evaluate_verified_program(program, arguments, configuration),
    }
}

fn validate_configuration(configuration: EvaluationConfiguration) -> Result<(), Error> {
    ResourceContext::new(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
    )
    .map(|_| ())
}

fn validate_expression_surface(program: &Program) -> Result<(), Error> {
    if let Some(header) = program.parameter_header {
        let keyword = crate::SourceSpan {
            begin: header.begin,
            end: crate::SourceLocation {
                offset: header.begin.offset + "parameters".len(),
                line: header.begin.line,
                column: header.begin.column + "parameters".len(),
            },
        };
        let mut error = Error::at_span(ErrorKind::SyntaxError, keyword, "invalid parameter header");
        error.parameter = Some(ParameterErrorContext {
            reason: ParameterErrorReason::ProgramOnlyParameterHeader,
            primary_span: keyword,
            context_span: header,
            related_span: None,
        });
        return Err(error);
    }
    if program.roots.is_empty() {
        return Err(Error::new(
            ErrorKind::EmptyExpression,
            SourceLocation::start(),
            "expected one expression",
        ));
    }
    if program.roots.len() != 1 {
        return Err(Error::at_span(
            ErrorKind::SyntaxError,
            program.roots[1].span,
            "evaluate_expression accepts exactly one root expression",
        ));
    }
    Ok(())
}

fn validate_tuple_profile(
    program: &Program,
    configuration: EvaluationConfiguration,
) -> Result<(), Error> {
    if program_contains_tuple(program) {
        let first = first_tuple_location(program).unwrap_or_else(SourceLocation::start);
        ResourceContext::new(
            configuration.profile,
            configuration.limits,
            configuration.allocation_failure,
        )?
        .require_tuple_profile(first)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evaluates_complete_primitive_surface() {
        let cases = [
            ("inc[1]", "2"),
            ("dec[1]", "0"),
            ("neg[-5]", "5"),
            ("abs[-5]", "5"),
            ("add[1 2]", "3"),
            ("sub[5 2]", "3"),
            ("mul[3 4]", "12"),
            ("div[-7 3]", "-2"),
            ("div[1 2.0]", "0.5"),
            ("length[(true false true)]", "3"),
            ("length[(1 2 3 4)]", "4"),
            ("length[Double()]", "0"),
            ("sort[(true false true)]", "(false true true)"),
            ("sort[(3 -1 3 0)]", "(-1 0 3 3)"),
            ("sort[(nan 0.0 -0.0 -inf inf)]", "(-inf -0.0 0.0 inf nan)"),
            ("sum[(1 2 3)]", "6"),
            ("sum[(1.5 -0.5 2.0)]", "3.0"),
            ("sum[Double()]", "0.0"),
            ("all_of[Bool()]", "true"),
            ("all_of[(true true false)]", "false"),
            ("any_of[Bool()]", "false"),
            ("any_of[(false false true)]", "true"),
            ("none_of[Bool()]", "true"),
            ("none_of[(false false true)]", "false"),
            ("foldl[@and true Bool()]", "true"),
            ("foldl[@and true (true false true)]", "false"),
            ("foldl[@sub 20 (3 4 5)]", "8"),
            ("foldl[@add 1 (2.5 3.5)]", "7.0"),
            ("scanl[@and true Bool()]", "(true)"),
            (
                "scanl[@and true (true false true)]",
                "(true true false false)",
            ),
            ("scanl[@sub 20 (3 4 5)]", "(20 17 13 8)"),
            ("scanl[@add 1 (2.5 3.5)]", "(1.0 3.5 7.0)"),
            ("filter[@not (true false true)]", "(false)"),
            ("filter[@odd (1 2 3 4)]", "(1 3)"),
            ("filter[@is_positive (-1.0 0.0 2.0)]", "(2.0)"),
            ("equals[1 1.0]", "true"),
            ("not_equals[1 2]", "true"),
            ("not[false]", "true"),
            ("and[true false]", "false"),
            ("or[true false]", "true"),
            ("odd[-3]", "true"),
            ("even[-4]", "true"),
            ("is_positive[1.0]", "true"),
            ("is_negative[-1]", "true"),
            ("less_than[1 2.0]", "true"),
            ("greater_than[2 1]", "true"),
            ("iota[3]", "(1 2 3)"),
            ("sqrt[9]", "3.0"),
            ("exp[0]", "1.0"),
            ("log[1]", "0.0"),
            ("log10[1]", "0.0"),
            ("sin[0]", "0.0"),
            ("cos[0]", "1.0"),
            ("tan[-0.0]", "-0.0"),
            ("floor[-0.5]", "-1.0"),
            ("ceil[-0.5]", "-0.0"),
            ("trunc[-0.5]", "-0.0"),
        ];
        for (source, expected) in cases {
            let value = evaluate_expression(source).expect(source).value;
            assert_eq!(format_value(&value).expect("format"), expected, "{source}");
        }
    }

    #[test]
    fn lifting_and_tuples_are_canonical() {
        let result = evaluate_expression("fanout[iota[3] {inc[_]} {add[_ 10]}]").expect("fanout");
        assert_eq!(
            format_value(&result.value).expect("format"),
            "[(2 3 4) (11 12 13)]"
        );
        let spread = evaluate_expression("add [1 2]").expect("spread");
        assert_eq!(spread.value, Value::Int(3));
    }

    #[test]
    fn value_formatting_is_structural_and_direct_strings_are_raw() {
        let result = evaluate_expression(
            "format[\"{}|{}|{}|{}|{{ok}}\" \"Málaga\\0\" (true false) String() [1 [\"x\\n\" -0.0]]]",
        )
        .expect("format");
        let expected = "Málaga\0|(true false)|()|[1 [\"x\\n\" -0.0]]|{ok}";
        assert_eq!(result.value, Value::String(expected.to_owned()));
        assert_eq!(result.usage.work_units, expected.len());
    }

    #[test]
    fn format_covers_every_scalar_and_empty_or_singleton_vector_kind() {
        let result = evaluate_expression(
            "format[\"{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}|{}\" true -1 nan \"é\" Bool() (true) Int() (1) Double() (-0.0) String() (\"x\")]",
        )
        .expect("all value kinds");
        assert_eq!(
            result.value,
            Value::String("true|-1|nan|é|()|(true)|()|(1)|()|(-0.0)|()|(\"x\")".to_owned())
        );
    }

    #[test]
    fn format_templates_and_printf_position_fail_statically_at_the_authored_form() {
        for (source, kind, message, begin, end) in [
            (
                "format[]",
                ErrorKind::ArityError,
                "format requires a String literal template",
                1,
                7,
            ),
            (
                "format[1]",
                ErrorKind::TypeError,
                "format template must be a String literal",
                8,
                9,
            ),
            (
                "format[\"{\"]",
                ErrorKind::FormattingError,
                "malformed format template brace",
                8,
                11,
            ),
            (
                "format[\"}\" 1]",
                ErrorKind::FormattingError,
                "malformed format template brace",
                8,
                11,
            ),
            (
                "format[\"{}\"]",
                ErrorKind::ArityError,
                "format template has 1 placeholder(s) but received 0 interpolation argument(s)",
                8,
                12,
            ),
            (
                "format[\"literal\" 1]",
                ErrorKind::ArityError,
                "format template has 0 placeholder(s) but received 1 interpolation argument(s)",
                8,
                17,
            ),
            (
                "printf[]",
                ErrorKind::ArityError,
                "printf requires a String literal template",
                1,
                7,
            ),
            (
                "printf[\"{\"]",
                ErrorKind::FormattingError,
                "malformed printf template brace",
                8,
                11,
            ),
            (
                "add[printf[\"{}\" 1] 2]",
                ErrorKind::TypeError,
                "printf is valid only as a program root",
                5,
                11,
            ),
            (
                "let text = printf[\"x\"]\ntext",
                ErrorKind::TypeError,
                "printf is valid only as a program root",
                12,
                18,
            ),
        ] {
            let error = evaluate_source(source).expect_err(source);
            assert_eq!(error.kind, kind, "{source}");
            assert_eq!(error.message, message, "{source}");
            assert_eq!(error.location.offset, begin, "{source}");
            let span = error.span.expect("authored span");
            assert_eq!(
                (span.begin.offset, span.end.offset),
                (begin, end),
                "{source}"
            );
        }
        for (source, placeholders, supplied, begin, end) in [
            ("format[\"{}{}\" 1]", 2, 1, 8, 14),
            ("format[\"{}\" 1 2]", 1, 2, 8, 12),
        ] {
            let error = evaluate_source(source).expect_err(source);
            assert_eq!(error.kind, ErrorKind::ArityError, "{source}");
            assert_eq!(
                error.message,
                format!(
                    "format template has {placeholders} placeholder(s) but received {supplied} interpolation argument(s)"
                ),
                "{source}"
            );
            assert_eq!(error.actual_arity, Some(supplied), "{source}");
            assert_eq!(error.expected_arity, [placeholders], "{source}");
            let span = error.span.expect("literal span");
            assert_eq!(
                (span.begin.offset, span.end.offset),
                (begin, end),
                "{source}"
            );
        }
        let first = evaluate_source("format[\"{\"]\nformat[\"}\" 1]\n")
            .expect_err("first malformed template");
        assert_eq!(first.location.line, 1);
        assert_eq!(first.location.offset, 8);
    }

    #[test]
    fn runner_retains_root_presentation_for_atomic_publication() {
        let result =
            evaluate_runner_source("1\nprintf[\"raw={}\\0\" \"é\"]\nformat[\"{}\" 2]\n", &[])
                .expect("runner");
        assert_eq!(result.formatted, ["1", "raw=é\0", "\"2\""]);
        assert_eq!(
            result.presentations,
            [
                RootPresentation::CanonicalValue,
                RootPresentation::RawString,
                RootPresentation::CanonicalValue,
            ]
        );
    }

    #[test]
    fn format_composes_with_bindings_fanout_and_connected_completion() {
        let binding = evaluate_source("let value = [1 \"x\"]\nformat[\"bound={}\" value]\n")
            .expect("binding");
        assert_eq!(
            binding.values,
            [Value::String("bound=[1 \"x\"]".to_owned())]
        );

        let fanout =
            evaluate_expression("fanout[format[\"{}\" [1 \"x\"]] {length[_]} {equals[_ \"x\"]}]")
                .expect("fanout");
        assert_eq!(
            fanout.value,
            Value::Tuple(vec![Value::Int(7), Value::Bool(false)].into())
        );

        let connected = evaluate_expression("length[] format[\"{}\" \"é\"]").expect("connected");
        assert_eq!(connected.value, Value::Int(1));
    }

    #[test]
    fn checked_arithmetic_is_transactional() {
        let error = evaluate_source("1\ninc[9223372036854775807]\n").expect_err("overflow");
        assert_eq!(error.kind, ErrorKind::DomainError);
    }

    #[test]
    fn runner_static_failure_precedes_argument_decoding() {
        let error =
            evaluate_runner_source("parameters[value Int]\nadd[value]\n", &["not-an-integer"])
                .expect_err("static arity failure");
        assert_eq!(error.kind, ErrorKind::ArityError);
    }

    #[test]
    fn typed_api_static_failure_precedes_argument_validation() {
        let error = evaluate_source_with_arguments(
            "parameters[value Int]\nadd[value]\n",
            &[Value::Bool(true)],
            EvaluationConfiguration::default(),
        )
        .expect_err("static arity failure");
        assert_eq!(error.kind, ErrorKind::ArityError);
    }
}
