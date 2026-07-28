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
    ParameterErrorReason, ResourceLimits, ResourceObserver, ResourceUsage, SourceLocation, Value,
    VerifiedProgram, format_value,
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
    let mut formatted = Vec::new();
    formatted
        .try_reserve_exact(result.values.len())
        .map_err(|_| {
            Error::new(
                ErrorKind::FormattingError,
                SourceLocation::start(),
                "unable to allocate formatted output",
            )
        })?;
    for value in &result.values {
        formatted.push(format_value(value)?);
    }
    Ok(RunnerEvaluationResult {
        values: result.values,
        formatted,
        usage: result.usage,
    })
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
