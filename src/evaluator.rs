use crate::parser::{
    CallSyntax, Expr, ExprKind, Program, first_tuple_location, parse, program_contains_tuple,
    validate_parameter_declarations,
};
use crate::primitive::{ApplicationArgument, analyze, apply, resolve_names};
use crate::resources::ResourceContext;
use crate::{
    AllocationFailureInjection, ArgumentErrorContext, ArgumentErrorReason, Error, ErrorKind,
    ExecutionProfile, ParameterErrorContext, ParameterErrorReason, ResourceLimits,
    ResourceObserver, ResourceUsage, ScalarType, SourceLocation, Value, format_value,
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
    let _ = ResourceContext::new(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
    )?;
    let program = parse(source)?;
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
    resolve_names(&program)?;
    validate_tuple_profile(&program, configuration)?;
    let _ = analyze(&program)?;
    let result = evaluate_program(&program, &[], configuration, observer)?;
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
    let _ = ResourceContext::new(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
    )?;
    let program = parse(source)?;
    validate_parameter_declarations(&program)?;
    resolve_names(&program)?;
    validate_tuple_profile(&program, configuration)?;
    let _types = analyze(&program)?;
    validate_typed_arguments(&program, arguments)?;
    evaluate_program(&program, arguments, configuration, observer)
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

pub fn evaluate_runner_source(
    source: &str,
    arguments: &[&str],
) -> Result<RunnerEvaluationResult, Error> {
    let program = parse(source)?;
    validate_parameter_declarations(&program)?;
    resolve_names(&program)?;
    let _types = analyze(&program)?;
    let decoded = decode_arguments(&program, arguments)?;
    let result = evaluate_program(&program, &decoded, EvaluationConfiguration::default(), None)?;
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

fn evaluate_program(
    program: &Program,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: Option<ResourceObserver>,
) -> Result<ProgramResult, Error> {
    let mut resources = ResourceContext::new_with_observer(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
        observer,
    )?;
    let mut roots = Vec::new();
    roots.try_reserve_exact(program.roots.len()).map_err(|_| {
        Error::new(
            ErrorKind::ResourceError,
            SourceLocation::start(),
            "program failed: allocation_unavailable",
        )
    })?;
    for root in &program.roots {
        match evaluate_expr(root, arguments, None, &mut resources) {
            Ok(evaluated) => roots.push(evaluated.into_owned()),
            Err(mut error) => {
                for value in roots.iter().rev() {
                    resources.release(value);
                }
                error.usage = Some(resources.usage);
                return Err(error);
            }
        }
    }
    Ok(ProgramResult {
        values: roots,
        usage: resources.usage,
    })
}

enum RuntimeValue<'a> {
    Borrowed(&'a Value),
    Owned(Value),
}

struct Evaluated<'a> {
    value: RuntimeValue<'a>,
    accounted: bool,
    static_length: Option<usize>,
    location: SourceLocation,
    tuple_elements: Option<Vec<(Option<usize>, SourceLocation)>>,
}

#[derive(Clone, Copy)]
struct PlaceholderInfo<'a> {
    value: &'a Value,
    static_length: Option<usize>,
    location: SourceLocation,
    tuple_elements: Option<&'a [(Option<usize>, SourceLocation)]>,
}

impl Evaluated<'_> {
    fn as_value(&self) -> &Value {
        match &self.value {
            RuntimeValue::Borrowed(value) => value,
            RuntimeValue::Owned(value) => value,
        }
    }

    fn into_owned(self) -> Value {
        match self.value {
            RuntimeValue::Borrowed(value) => value.clone(),
            RuntimeValue::Owned(value) => value,
        }
    }
}

fn evaluate_expr<'a>(
    expression: &Expr,
    parameters: &'a [Value],
    placeholder: Option<PlaceholderInfo<'a>>,
    resources: &mut ResourceContext,
) -> Result<Evaluated<'a>, Error> {
    match &expression.kind {
        ExprKind::Literal(value) => Ok(Evaluated {
            value: RuntimeValue::Owned(value.clone()),
            accounted: false,
            static_length: None,
            location: expression.span.begin,
            tuple_elements: None,
        }),
        ExprKind::Vector(element_type, elements) => {
            let bytes = resources.admit_vector(
                *element_type,
                elements.len(),
                expression.span.begin,
                "vector_literal",
            )?;
            let value = match build_literal_vector(*element_type, elements) {
                Ok(value) => value,
                Err(error) => {
                    resources.refund(bytes);
                    return Err(error);
                }
            };
            Ok(Evaluated {
                value: RuntimeValue::Owned(value),
                accounted: !elements.is_empty(),
                static_length: Some(elements.len()),
                location: expression.span.begin,
                tuple_elements: None,
            })
        }
        ExprKind::Tuple(elements) => {
            let mut values = Vec::new();
            let mut tuple_elements = Vec::new();
            values.try_reserve_exact(elements.len()).map_err(|_| {
                Error::new(
                    ErrorKind::ResourceError,
                    expression.span.begin,
                    "tuple_literal failed: allocation_unavailable",
                )
            })?;
            tuple_elements
                .try_reserve_exact(elements.len())
                .map_err(|_| literal_allocation_error())?;
            for element in elements {
                match evaluate_expr(element, parameters, placeholder, resources) {
                    Ok(value) => {
                        tuple_elements.push((value.static_length, value.location));
                        values.push(value.into_owned());
                    }
                    Err(error) => {
                        for value in values.iter().rev() {
                            resources.release(value);
                        }
                        return Err(error);
                    }
                }
            }
            if let Err(error) =
                resources.admit_tuple(elements.len(), expression.span.begin, "tuple_literal")
            {
                for value in values.iter().rev() {
                    resources.release(value);
                }
                return Err(error);
            }
            Ok(Evaluated {
                value: RuntimeValue::Owned(Value::Tuple(values.into())),
                accounted: !elements.is_empty(),
                static_length: Some(elements.len()),
                location: expression.span.begin,
                tuple_elements: Some(tuple_elements),
            })
        }
        ExprKind::DeepTuple { depth, leaf } => {
            let mut value = leaf.clone();
            let mut accounted = false;
            for _ in 0..*depth {
                let admitted =
                    match resources.admit_tuple(1, expression.span.begin, "tuple_literal") {
                        Ok(admitted) => admitted,
                        Err(error) => {
                            if accounted {
                                resources.release(&value);
                            }
                            return Err(error);
                        }
                    };
                let mut elements = Vec::new();
                if elements.try_reserve_exact(1).is_err() {
                    resources.refund(admitted);
                    if accounted {
                        resources.release(&value);
                    }
                    return Err(literal_allocation_error());
                }
                elements.push(value);
                value = Value::Tuple(elements.into());
                accounted = true;
            }
            Ok(Evaluated {
                value: RuntimeValue::Owned(value),
                accounted,
                static_length: Some(1),
                location: expression.span.begin,
                tuple_elements: None,
            })
        }
        ExprKind::UnaryChain {
            leaf,
            leaf_span,
            steps,
        } => {
            let mut current = Evaluated {
                value: RuntimeValue::Owned(leaf.clone()),
                accounted: false,
                static_length: None,
                location: leaf_span.begin,
                tuple_elements: None,
            };
            for step in steps {
                let arguments = [ApplicationArgument {
                    value: current.as_value(),
                    static_length: current.static_length,
                    location: current.location,
                }];
                let applied = apply(&step.name, &arguments, step.span.begin, resources);
                if current.accounted {
                    resources.release(current.as_value());
                }
                let (value, accounted) = applied?;
                let static_length = value.is_vector().then_some(current.static_length).flatten();
                current = Evaluated {
                    value: RuntimeValue::Owned(value),
                    accounted,
                    static_length,
                    location: step.span.begin,
                    tuple_elements: None,
                };
            }
            Ok(current)
        }
        ExprKind::Parameter(index) => parameters
            .get(*index)
            .map(|value| Evaluated {
                value: RuntimeValue::Borrowed(value),
                accounted: false,
                static_length: None,
                location: expression.span.begin,
                tuple_elements: None,
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::ArgumentError,
                    expression.span.begin,
                    "missing parameter value",
                )
            }),
        ExprKind::Placeholder => placeholder
            .map(|placeholder| Evaluated {
                value: RuntimeValue::Borrowed(placeholder.value),
                accounted: false,
                static_length: placeholder.static_length,
                location: placeholder.location,
                tuple_elements: placeholder.tuple_elements.map(<[_]>::to_vec),
            })
            .ok_or_else(|| {
                Error::new(
                    ErrorKind::SyntaxError,
                    expression.span.begin,
                    "placeholder has no fanout operand",
                )
            }),
        ExprKind::UnresolvedName { name, name_span } => Err(Error::at_span(
            ErrorKind::UnknownPrimitive,
            *name_span,
            format!("unknown primitive '{name}'"),
        )),
        ExprKind::Call {
            name,
            syntax,
            arguments,
            ..
        } => {
            let mut evaluated = Vec::new();
            for argument in arguments {
                match evaluate_expr(argument, parameters, placeholder, resources) {
                    Ok(value) => evaluated.push(value),
                    Err(error) => {
                        release_evaluated(&evaluated, resources);
                        return Err(error);
                    }
                }
            }
            let semantic = semantic_values(*syntax, &evaluated);
            let result_static_length = semantic
                .iter()
                .find_map(|argument| argument.value.is_vector().then_some(argument.static_length))
                .flatten();
            let applied = apply(name, &semantic, expression.span.begin, resources);
            release_evaluated(&evaluated, resources);
            let (result, accounted) = applied?;
            Ok(Evaluated {
                value: RuntimeValue::Owned(result),
                accounted,
                static_length: result_static_length,
                location: expression.span.begin,
                tuple_elements: None,
            })
        }
        ExprKind::Fanout { operand, branches } => {
            let operand = evaluate_expr(operand, parameters, placeholder, resources)?;
            let mut results = Vec::new();
            let mut tuple_elements = Vec::new();
            results.try_reserve_exact(branches.len()).map_err(|_| {
                Error::new(
                    ErrorKind::ResourceError,
                    expression.span.begin,
                    "fanout failed: allocation_unavailable",
                )
            })?;
            tuple_elements
                .try_reserve_exact(branches.len())
                .map_err(|_| literal_allocation_error())?;
            for branch in branches {
                let placeholder = PlaceholderInfo {
                    value: operand.as_value(),
                    static_length: operand.static_length,
                    location: operand.location,
                    tuple_elements: operand.tuple_elements.as_deref(),
                };
                match evaluate_expr(branch, parameters, Some(placeholder), resources) {
                    Ok(value) => {
                        tuple_elements.push((value.static_length, value.location));
                        results.push(value.into_owned());
                    }
                    Err(error) => {
                        for value in results.iter().rev() {
                            resources.release(value);
                        }
                        if operand.accounted {
                            resources.release(operand.as_value());
                        }
                        return Err(error);
                    }
                }
            }
            if let Err(error) =
                resources.admit_tuple(branches.len(), expression.span.begin, "fanout")
            {
                for value in results.iter().rev() {
                    resources.release(value);
                }
                if operand.accounted {
                    resources.release(operand.as_value());
                }
                return Err(error);
            }
            if operand.accounted {
                resources.release(operand.as_value());
            }
            Ok(Evaluated {
                value: RuntimeValue::Owned(Value::Tuple(results.into())),
                accounted: true,
                static_length: Some(branches.len()),
                location: expression.span.begin,
                tuple_elements: Some(tuple_elements),
            })
        }
    }
}

fn release_evaluated(evaluated: &[Evaluated<'_>], resources: &mut ResourceContext) {
    for argument in evaluated.iter().rev() {
        if argument.accounted {
            resources.release(argument.as_value());
        }
    }
}

fn semantic_values<'a>(
    syntax: CallSyntax,
    evaluated: &'a [Evaluated<'a>],
) -> Vec<ApplicationArgument<'a>> {
    if syntax == CallSyntax::Prefix
        && evaluated.len() == 1
        && let Value::Tuple(elements) = evaluated[0].as_value()
    {
        return elements
            .iter()
            .enumerate()
            .map(|(index, value)| {
                let metadata = evaluated[0]
                    .tuple_elements
                    .as_deref()
                    .and_then(|metadata| metadata.get(index))
                    .copied()
                    .unwrap_or((None, evaluated[0].location));
                ApplicationArgument {
                    value,
                    static_length: metadata.0,
                    location: metadata.1,
                }
            })
            .collect();
    }
    evaluated
        .iter()
        .map(|value| ApplicationArgument {
            value: value.as_value(),
            static_length: value.static_length,
            location: value.location,
        })
        .collect()
}

fn build_literal_vector(element_type: ScalarType, elements: &[Value]) -> Result<Value, Error> {
    match element_type {
        ScalarType::Bool => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| literal_allocation_error())?;
            for element in elements {
                if let Value::Bool(value) = element {
                    values.push(*value);
                }
            }
            Ok(Value::BoolVector(values))
        }
        ScalarType::Int => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| literal_allocation_error())?;
            for element in elements {
                if let Value::Int(value) = element {
                    values.push(*value);
                }
            }
            Ok(Value::IntVector(values))
        }
        ScalarType::Double => {
            let mut values = Vec::new();
            values
                .try_reserve_exact(elements.len())
                .map_err(|_| literal_allocation_error())?;
            for element in elements {
                if let Value::Double(value) = element {
                    values.push(*value);
                }
            }
            Ok(Value::DoubleVector(values))
        }
    }
}

fn literal_allocation_error() -> Error {
    Error::new(
        ErrorKind::ResourceError,
        SourceLocation::start(),
        "vector_literal failed: allocation_unavailable",
    )
}

fn validate_typed_arguments(program: &Program, arguments: &[Value]) -> Result<(), Error> {
    if arguments.len() != program.parameters.len() {
        let reason = if arguments.len() < program.parameters.len() {
            ArgumentErrorReason::Missing
        } else {
            ArgumentErrorReason::Extra
        };
        let position = arguments.len().min(program.parameters.len()) + 1;
        return Err(argument_error(program, reason, arguments.len(), position));
    }
    for (index, (parameter, argument)) in program.parameters.iter().zip(arguments).enumerate() {
        if contains_noncanonical_nan(argument)? {
            let mut error = argument_error(
                program,
                ArgumentErrorReason::InvalidTypedValue,
                arguments.len(),
                index + 1,
            );
            if let Some(context) = &mut error.argument {
                context.actual_container = Some(if argument.is_scalar() {
                    "scalar"
                } else if argument.is_vector() {
                    "vector"
                } else {
                    "tuple"
                });
                context.actual_type = argument.scalar_type();
                context.invalid_value_invariant = Some("noncanonical_nan");
            }
            return Err(error);
        }
        if !argument.is_scalar() {
            let mut error = argument_error(
                program,
                ArgumentErrorReason::ContainerMismatch,
                arguments.len(),
                index + 1,
            );
            if let Some(context) = &mut error.argument {
                context.actual_container = Some(if argument.is_vector() {
                    "vector"
                } else {
                    "tuple"
                });
            }
            return Err(error);
        }
        if argument.scalar_type() != Some(parameter.scalar_type) {
            let mut error = argument_error(
                program,
                ArgumentErrorReason::TypeMismatch,
                arguments.len(),
                index + 1,
            );
            if let Some(context) = &mut error.argument {
                context.actual_container = Some("scalar");
                context.actual_type = argument.scalar_type();
            }
            return Err(error);
        }
    }
    Ok(())
}

fn contains_noncanonical_nan(value: &Value) -> Result<bool, Error> {
    let mut pending = Vec::new();
    pending.try_reserve(1).map_err(|_| {
        Error::new(
            ErrorKind::ResourceError,
            SourceLocation::start(),
            "argument validation failed: allocation_unavailable",
        )
    })?;
    pending.push(value);
    while let Some(current) = pending.pop() {
        match current {
            Value::Double(value) if value.is_nan() => {
                if value.to_bits() != 0x7ff8_0000_0000_0000 {
                    return Ok(true);
                }
            }
            Value::DoubleVector(values) => {
                if values
                    .iter()
                    .any(|value| value.is_nan() && value.to_bits() != 0x7ff8_0000_0000_0000)
                {
                    return Ok(true);
                }
            }
            Value::Tuple(values) => {
                pending.try_reserve(values.len()).map_err(|_| {
                    Error::new(
                        ErrorKind::ResourceError,
                        SourceLocation::start(),
                        "argument validation failed: allocation_unavailable",
                    )
                })?;
                pending.extend(values.iter());
            }
            Value::Bool(_)
            | Value::Int(_)
            | Value::BoolVector(_)
            | Value::IntVector(_)
            | Value::Double(_) => {}
        }
    }
    Ok(false)
}

fn decode_arguments(program: &Program, arguments: &[&str]) -> Result<Vec<Value>, Error> {
    if arguments.len() != program.parameters.len() {
        let reason = if arguments.len() < program.parameters.len() {
            ArgumentErrorReason::Missing
        } else {
            ArgumentErrorReason::Extra
        };
        let position = arguments.len().min(program.parameters.len()) + 1;
        return Err(argument_error(program, reason, arguments.len(), position));
    }
    let mut decoded = Vec::new();
    for (index, (parameter, spelling)) in program.parameters.iter().zip(arguments).enumerate() {
        let value = match parameter.scalar_type {
            ScalarType::Bool => match *spelling {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                _ => Err(ArgumentErrorReason::InvalidLiteral),
            },
            ScalarType::Int => decode_int(spelling).map(Value::Int),
            ScalarType::Double => decode_double(spelling).map(Value::Double),
        };
        let value = match value {
            Ok(value) => value,
            Err(reason) => {
                return Err(argument_error(program, reason, arguments.len(), index + 1));
            }
        };
        decoded.push(value);
    }
    Ok(decoded)
}

fn decode_int(spelling: &str) -> Result<i64, ArgumentErrorReason> {
    let digits = spelling.strip_prefix('-').unwrap_or(spelling);
    if digits.is_empty()
        || !digits.bytes().all(|byte| byte.is_ascii_digit())
        || (digits.starts_with('0') && (digits.len() != 1 || spelling.starts_with('-')))
    {
        return Err(ArgumentErrorReason::InvalidLiteral);
    }
    spelling
        .parse()
        .map_err(|_| ArgumentErrorReason::OutOfRange)
}

fn decode_double(spelling: &str) -> Result<f64, ArgumentErrorReason> {
    match spelling {
        "inf" => return Ok(f64::INFINITY),
        "-inf" => return Ok(f64::NEG_INFINITY),
        "nan" => return Ok(f64::from_bits(0x7ff8_0000_0000_0000)),
        _ => {}
    }
    if !canonical_double_argument(spelling) {
        return Err(ArgumentErrorReason::InvalidLiteral);
    }
    let value: f64 = spelling
        .parse()
        .map_err(|_| ArgumentErrorReason::OutOfRange)?;
    value
        .is_finite()
        .then_some(value)
        .ok_or(ArgumentErrorReason::OutOfRange)
}

fn canonical_double_argument(spelling: &str) -> bool {
    let text = spelling.strip_prefix('-').unwrap_or(spelling);
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], Some(&text[index + 1..])),
        None => (text, None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    let has_exponent = exponent.is_some();
    let mut parts = mantissa.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.starts_with('0') && integer.len() != 1)
    {
        return false;
    }
    let has_fraction = fraction.is_some();
    if let Some(fraction) = fraction
        && (fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    has_fraction || has_exponent
}

fn argument_error(
    program: &Program,
    reason: ArgumentErrorReason,
    supplied_count: usize,
    position: usize,
) -> Error {
    let parameter = program.parameters.get(position.saturating_sub(1));
    let context = ArgumentErrorContext {
        reason,
        required_count: program.parameters.len(),
        supplied_count,
        position,
        parameter_name: parameter.map(|parameter| parameter.name.clone()),
        expected_type: parameter.map(|parameter| parameter.scalar_type),
        declaration_span: parameter.map(|parameter| parameter.span),
        actual_container: None,
        actual_type: None,
        invalid_value_invariant: None,
    };
    let location = parameter.map_or(SourceLocation::start(), |parameter| parameter.span.begin);
    let mut error = Error::new(ErrorKind::ArgumentError, location, reason.name());
    error.argument = Some(context);
    error
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
}
