use crate::parser::{CallSyntax, Expr, ExprKind, Program};
use crate::resources::ResourceContext;
use crate::semantic_registry::{
    Conversion, StructuralBehavior, conversion, descriptors, primitive_from_name,
};
use crate::strict_float::{self, Binary64Operation};
use crate::{
    DomainErrorContext, DomainErrorReason, Error, ErrorKind, ScalarType, SourceLocation, Type,
    Value,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct TypeInfo {
    pub value_type: Type,
    pub known_length: Option<usize>,
    pub location: SourceLocation,
}

pub(crate) fn analyze(program: &Program) -> Result<Vec<TypeInfo>, Error> {
    resolve_names(program)?;
    let parameter_types: Vec<TypeInfo> = program
        .parameters
        .iter()
        .map(|parameter| TypeInfo {
            value_type: Type::Scalar(parameter.scalar_type),
            known_length: None,
            location: parameter.span.begin,
        })
        .collect();
    for root in &program.roots {
        let _ = infer(root, &parameter_types, None, false)?;
    }
    program
        .roots
        .iter()
        .map(|root| infer(root, &parameter_types, None, true))
        .collect()
}

pub(crate) fn analyze_for_lowering(program: &Program) -> Result<Vec<TypeInfo>, Error> {
    resolve_names(program)?;
    let mut parameter_types = Vec::new();
    parameter_types
        .try_reserve(program.parameters.len())
        .map_err(|_| analysis_allocation_error(SourceLocation::start()))?;
    for parameter in &program.parameters {
        parameter_types.push(TypeInfo {
            value_type: Type::Scalar(parameter.scalar_type),
            known_length: None,
            location: parameter.span.begin,
        });
    }
    for root in &program.roots {
        if let Some(error) = first_arity_error(root, &parameter_types, None)? {
            return Err(error);
        }
    }
    analyze(program)
}

fn first_arity_error(
    expression: &Expr,
    parameters: &[TypeInfo],
    placeholder: Option<&TypeInfo>,
) -> Result<Option<Error>, Error> {
    match &expression.kind {
        ExprKind::Call {
            name,
            syntax,
            arguments,
            ..
        } => {
            for argument in arguments {
                if let Some(error) = first_arity_error(argument, parameters, placeholder)? {
                    return Ok(Some(error));
                }
            }
            let actual = match syntax {
                CallSyntax::Direct => arguments.len(),
                CallSyntax::Prefix => {
                    let Some(argument) = arguments.first() else {
                        return Ok(None);
                    };
                    let Ok(inferred) = infer(argument, parameters, placeholder, false) else {
                        return Ok(None);
                    };
                    match inferred.value_type {
                        Type::Tuple(elements) => elements.len(),
                        Type::RepeatedTuple { .. } | Type::Scalar(_) | Type::Vector(_) => {
                            arguments.len()
                        }
                    }
                }
            };
            arity_error(name, actual, expression.span.begin)
        }
        ExprKind::Tuple(elements) => {
            for element in elements {
                if let Some(error) = first_arity_error(element, parameters, placeholder)? {
                    return Ok(Some(error));
                }
            }
            Ok(None)
        }
        ExprKind::Fanout { operand, branches } => {
            if let Some(error) = first_arity_error(operand, parameters, placeholder)? {
                return Ok(Some(error));
            }
            let operand_type = infer(operand, parameters, placeholder, false).ok();
            for branch in branches {
                if let Some(error) = first_arity_error(branch, parameters, operand_type.as_ref())? {
                    return Ok(Some(error));
                }
            }
            Ok(None)
        }
        ExprKind::UnaryChain {
            leaf,
            leaf_span,
            steps,
        } => {
            let mut current = Some(TypeInfo {
                value_type: leaf.value_type(),
                known_length: None,
                location: leaf_span.begin,
            });
            for step in steps {
                if current.is_none() {
                    break;
                }
                if let Some(error) = arity_error(&step.name, 1, step.span.begin)? {
                    return Ok(Some(error));
                }
                current = current.and_then(|current| {
                    select_call(
                        &step.name,
                        std::slice::from_ref(&current),
                        step.span.begin,
                        false,
                    )
                    .ok()
                });
            }
            Ok(None)
        }
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::DeepTuple { .. }
        | ExprKind::Parameter(_)
        | ExprKind::Placeholder
        | ExprKind::UnresolvedName { .. } => Ok(None),
    }
}

fn arity_error(
    name: &str,
    actual: usize,
    location: SourceLocation,
) -> Result<Option<Error>, Error> {
    let Ok(primitive) = primitive_from_name(name) else {
        return Ok(None);
    };
    if descriptors(primitive).any(|signature| signature.parameters.len() == actual) {
        return Ok(None);
    }
    let Some(expected) = descriptors(primitive)
        .next()
        .map(|signature| signature.parameters.len())
    else {
        return Ok(None);
    };
    let mut accepted = Vec::new();
    accepted
        .try_reserve(1)
        .map_err(|_| analysis_allocation_error(location))?;
    accepted.push(expected);
    let mut error = Error::new(
        ErrorKind::ArityError,
        location,
        format!("{name} received {actual} argument(s); accepted arity {expected}"),
    );
    error.primitive = Some(name.to_owned());
    error.actual_arity = Some(actual);
    error.expected_arity = accepted;
    Ok(Some(error))
}

fn analysis_allocation_error(location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::ResourceError,
        location,
        "analysis failed: allocation_unavailable",
    )
}

pub(crate) fn resolve_names(program: &Program) -> Result<(), Error> {
    for root in &program.roots {
        validate_names(root)?;
    }
    Ok(())
}

fn validate_names(expression: &Expr) -> Result<(), Error> {
    match &expression.kind {
        ExprKind::Call {
            name,
            name_span,
            arguments,
            ..
        } => {
            for argument in arguments {
                validate_names(argument)?;
            }
            if primitive_from_name(name).is_err() {
                return Err(Error::at_span(
                    ErrorKind::UnknownPrimitive,
                    *name_span,
                    format!("unknown primitive '{name}'"),
                ));
            }
        }
        ExprKind::Tuple(elements) => {
            for element in elements {
                validate_names(element)?;
            }
        }
        ExprKind::DeepTuple { .. } => {}
        ExprKind::UnaryChain { steps, .. } => {
            for step in steps {
                if primitive_from_name(&step.name).is_err() {
                    return Err(Error::at_span(
                        ErrorKind::UnknownPrimitive,
                        step.name_span,
                        format!("unknown primitive '{}'", step.name),
                    ));
                }
            }
        }
        ExprKind::Fanout { operand, branches } => {
            validate_names(operand)?;
            for branch in branches {
                validate_names(branch)?;
            }
        }
        ExprKind::UnresolvedName { name, name_span } => {
            return Err(Error::at_span(
                ErrorKind::UnknownPrimitive,
                *name_span,
                format!("unknown primitive '{name}'"),
            ));
        }
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::Parameter(_)
        | ExprKind::Placeholder => {}
    }
    Ok(())
}

fn infer(
    expression: &Expr,
    parameters: &[TypeInfo],
    placeholder: Option<&TypeInfo>,
    shapes: bool,
) -> Result<TypeInfo, Error> {
    match &expression.kind {
        ExprKind::Literal(value) => Ok(TypeInfo {
            value_type: value.value_type(),
            known_length: None,
            location: expression.span.begin,
        }),
        ExprKind::Vector(element_type, values) => Ok(TypeInfo {
            value_type: Type::Vector(*element_type),
            known_length: Some(values.len()),
            location: expression.span.begin,
        }),
        ExprKind::Tuple(elements) => {
            let mut types = Vec::with_capacity(elements.len());
            for element in elements {
                types.push(infer(element, parameters, placeholder, shapes)?.value_type);
            }
            Ok(TypeInfo {
                value_type: Type::Tuple(types),
                known_length: Some(elements.len()),
                location: expression.span.begin,
            })
        }
        ExprKind::DeepTuple { depth, leaf } => Ok(TypeInfo {
            value_type: Type::RepeatedTuple {
                depth: *depth,
                leaf: leaf
                    .scalar_type()
                    .ok_or_else(|| type_runtime_error("tuple_literal", expression.span.begin))?,
            },
            known_length: Some(1),
            location: expression.span.begin,
        }),
        ExprKind::UnaryChain {
            leaf,
            leaf_span,
            steps,
        } => {
            let mut current = TypeInfo {
                value_type: leaf.value_type(),
                known_length: None,
                location: leaf_span.begin,
            };
            for step in steps {
                current = select_call(
                    &step.name,
                    std::slice::from_ref(&current),
                    step.span.begin,
                    shapes,
                )?;
            }
            Ok(current)
        }
        ExprKind::Parameter(index) => parameters.get(*index).cloned().ok_or_else(|| {
            Error::at_span(
                ErrorKind::ParameterError,
                expression.span,
                "invalid parameter reference",
            )
        }),
        ExprKind::Placeholder => placeholder.cloned().ok_or_else(|| {
            Error::at_span(
                ErrorKind::SyntaxError,
                expression.span,
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
            let mut actual = Vec::with_capacity(arguments.len());
            for argument in arguments {
                actual.push(infer(argument, parameters, placeholder, shapes)?);
            }
            let actual = semantic_types(*syntax, actual);
            select_call(name, &actual, expression.span.begin, shapes)
        }
        ExprKind::Fanout { operand, branches } => {
            let operand_type = infer(operand, parameters, placeholder, shapes)?;
            let mut branch_types = Vec::with_capacity(branches.len());
            for branch in branches {
                branch_types
                    .push(infer(branch, parameters, Some(&operand_type), shapes)?.value_type);
            }
            Ok(TypeInfo {
                known_length: Some(branch_types.len()),
                value_type: Type::Tuple(branch_types),
                location: expression.span.begin,
            })
        }
    }
}

fn semantic_types(syntax: CallSyntax, mut actual: Vec<TypeInfo>) -> Vec<TypeInfo> {
    if syntax == CallSyntax::Prefix
        && actual.len() == 1
        && let Type::Tuple(elements) = &actual[0].value_type
    {
        return elements
            .iter()
            .cloned()
            .map(|value_type| TypeInfo {
                value_type,
                known_length: None,
                location: actual[0].location,
            })
            .collect();
    }
    actual.shrink_to_fit();
    actual
}

fn select_call(
    name: &str,
    actual: &[TypeInfo],
    location: SourceLocation,
    shapes: bool,
) -> Result<TypeInfo, Error> {
    let primitive = primitive_from_name(name).map_err(|_| {
        Error::new(
            ErrorKind::UnknownPrimitive,
            location,
            format!("unknown primitive '{name}'"),
        )
    })?;
    if !descriptors(primitive).any(|signature| signature.parameters.len() == actual.len()) {
        let mut accepted: Vec<usize> = descriptors(primitive)
            .map(|signature| signature.parameters.len())
            .collect();
        accepted.sort_unstable();
        accepted.dedup();
        let accepted_text = accepted
            .iter()
            .map(usize::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        let mut error = Error::new(
            ErrorKind::ArityError,
            location,
            format!(
                "{name} received {} argument(s); accepted arity{} {accepted_text}",
                actual.len(),
                if accepted.len() == 1 { "" } else { " values" },
            ),
        );
        error.primitive = Some(name.to_owned());
        error.actual_arity = Some(actual.len());
        error.expected_arity = accepted;
        return Err(error);
    }
    let structural_behavior = descriptors(primitive)
        .next()
        .map(|descriptor| descriptor.behavior)
        .ok_or_else(|| {
            Error::new(
                ErrorKind::UnknownPrimitive,
                location,
                format!("unknown primitive '{name}'"),
            )
        })?;
    if structural_behavior == StructuralBehavior::Iota
        && actual
            .first()
            .is_some_and(|argument| !matches!(argument.value_type, Type::Scalar(_)))
    {
        let position = 1;
        let mut error = Error::new(
            ErrorKind::TypeError,
            actual[0].location,
            "iota arguments do not match an accepted signature; first unsupported argument is 1",
        );
        error.primitive = Some(name.to_owned());
        error.argument_position = Some(position);
        error.actual_types = actual.iter().map(|info| info.value_type.clone()).collect();
        return Err(error);
    }
    let mut selected = None;
    let mut selected_cost = usize::MAX;
    for signature in descriptors(primitive) {
        if signature.parameters.len() != actual.len() {
            continue;
        }
        let mut cost = 0;
        let mut accepted = true;
        for (parameter, argument) in signature.parameters.iter().zip(actual) {
            let Some(actual_scalar) = scalar_element_type(&argument.value_type) else {
                accepted = false;
                break;
            };
            match conversion(actual_scalar, *parameter) {
                Some(Conversion::Identity) => {}
                Some(Conversion::PromoteIntToDouble) => cost += 1,
                None => {
                    accepted = false;
                    break;
                }
            }
        }
        if accepted && cost < selected_cost {
            selected = Some(signature);
            selected_cost = cost;
        }
    }
    let Some(selected) = selected else {
        let matched_prefix = descriptors(primitive)
            .filter(|signature| signature.parameters.len() == actual.len())
            .map(|signature| {
                signature
                    .parameters
                    .iter()
                    .zip(actual)
                    .take_while(|(parameter, argument)| {
                        scalar_element_type(&argument.value_type).is_some_and(|actual_type| {
                            conversion(actual_type, **parameter).is_some()
                        })
                    })
                    .count()
            })
            .max()
            .unwrap_or(0);
        let first_unsupported = (matched_prefix + 1).min(actual.len());
        let mut error = Error::new(
            ErrorKind::TypeError,
            actual[first_unsupported - 1].location,
            format!(
                "{name} arguments do not match an accepted signature; first unsupported argument is {first_unsupported}"
            ),
        );
        error.primitive = Some(name.to_owned());
        error.argument_position = Some(first_unsupported);
        error.actual_types = actual.iter().map(|info| info.value_type.clone()).collect();
        return Err(error);
    };
    let vectors: Vec<(usize, Option<usize>)> = actual
        .iter()
        .enumerate()
        .filter_map(|(index, argument)| {
            matches!(argument.value_type, Type::Vector(_)).then_some((index, argument.known_length))
        })
        .collect();
    if shapes
        && let Some((anchor_index, expected)) = vectors
            .iter()
            .find_map(|(index, length)| length.map(|length| (*index, length)))
    {
        for (index, length) in &vectors {
            if *index == anchor_index {
                continue;
            }
            if let Some(actual_length) = length
                && *actual_length != expected
            {
                let mut error = Error::new(
                    ErrorKind::ShapeMismatch,
                    actual[*index].location,
                    format!(
                        "{name} argument {} expected shape [{expected}], got [{actual_length}]",
                        index + 1
                    ),
                );
                error.primitive = Some(name.to_owned());
                error.argument_position = Some(index + 1);
                error.expected_shape = Some(vec![expected]);
                error.actual_shape = Some(vec![*actual_length]);
                return Err(error);
            }
        }
    }
    if selected.behavior == StructuralBehavior::Iota {
        return Ok(TypeInfo {
            value_type: Type::Vector(ScalarType::Int),
            known_length: None,
            location,
        });
    }
    let known_length = vectors.iter().find_map(|(_, length)| *length);
    Ok(TypeInfo {
        value_type: if vectors.is_empty() {
            Type::Scalar(selected.result)
        } else {
            Type::Vector(selected.result)
        },
        known_length,
        location,
    })
}

fn scalar_element_type(value_type: &Type) -> Option<ScalarType> {
    match value_type {
        Type::Scalar(scalar) | Type::Vector(scalar) => Some(*scalar),
        Type::Tuple(_) | Type::RepeatedTuple { .. } => None,
    }
}

pub(crate) struct ApplicationArgument<'a> {
    pub value: &'a Value,
    pub static_length: Option<usize>,
    pub location: SourceLocation,
}

pub(crate) fn apply(
    name: &str,
    arguments: &[ApplicationArgument<'_>],
    location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let actual: Vec<TypeInfo> = arguments
        .iter()
        .map(|argument| TypeInfo {
            value_type: argument.value.value_type(),
            known_length: argument.static_length,
            location: argument.location,
        })
        .collect();
    let selected = select_call(name, &actual, location, true)?;
    if name == "iota" {
        let Value::Int(bound) = arguments[0].value else {
            return Err(type_runtime_error(name, location));
        };
        let length = if *bound <= 0 {
            0
        } else {
            usize::try_from(*bound).map_err(|_| resource_size_error(name, location))?
        };
        let admitted =
            resources.admit_vector_with_work(ScalarType::Int, length, length, location, name)?;
        let mut values = Vec::new();
        if values.try_reserve_exact(length).is_err() {
            resources.refund(admitted);
            return Err(allocation_error(name, location));
        }
        for value in 1..=*bound {
            values.push(value);
        }
        return Ok((Value::IntVector(values), true));
    }
    let result_type = scalar_element_type(&selected.value_type)
        .ok_or_else(|| type_runtime_error(name, location))?;
    let vector_length = arguments
        .iter()
        .find(|argument| argument.value.is_vector())
        .map(|argument| argument.value.len());
    let count = vector_length.unwrap_or(1);
    let accounted = vector_length.is_some();
    if let Some((anchor_position, expected)) = arguments
        .iter()
        .enumerate()
        .find(|(_, argument)| argument.value.is_vector() && argument.static_length.is_some())
        .map(|(position, argument)| (position, argument.value.len()))
        .or_else(|| {
            arguments
                .iter()
                .enumerate()
                .find(|(_, argument)| argument.value.is_vector())
                .map(|(position, argument)| (position, argument.value.len()))
        })
    {
        for (position, argument) in arguments.iter().enumerate() {
            if position != anchor_position
                && argument.value.is_vector()
                && argument.value.len() != expected
            {
                let actual_length = argument.value.len();
                let mut error = Error::new(
                    ErrorKind::ShapeMismatch,
                    argument.location,
                    format!(
                        "{name} argument {} expected shape [{expected}], got [{actual_length}]",
                        position + 1
                    ),
                );
                error.primitive = Some(name.to_owned());
                error.argument_position = Some(position + 1);
                error.expected_shape = Some(vec![expected]);
                error.actual_shape = Some(vec![actual_length]);
                return Err(error);
            }
        }
    }
    let admitted = if accounted {
        resources.admit_vector_with_work(result_type, count, count, location, name)?
    } else {
        resources.charge_work(count, location, name)?;
        0
    };
    let mut scalar_results = Vec::new();
    if scalar_results.try_reserve_exact(count).is_err() {
        resources.refund(admitted);
        return Err(allocation_error(name, location));
    }
    for index in 0..count {
        let mut operands = Vec::new();
        if operands.try_reserve_exact(arguments.len()).is_err() {
            resources.refund(admitted);
            return Err(allocation_error(name, location));
        }
        for argument in arguments {
            match scalar_at(argument.value, index) {
                Ok(value) => operands.push(value),
                Err(error) => {
                    resources.refund(admitted);
                    return Err(error);
                }
            }
        }
        match invoke(
            name,
            &operands,
            result_type,
            location,
            accounted.then_some(index),
        ) {
            Ok(value) => scalar_results.push(value),
            Err(error) => {
                resources.refund(admitted);
                return Err(error);
            }
        }
    }
    if !accounted {
        return scalar_results
            .pop()
            .map(|value| (value, false))
            .ok_or_else(|| type_runtime_error(name, location));
    }
    match vector_from_scalars(result_type, scalar_results) {
        Ok(value) => Ok((value, true)),
        Err(error) => {
            resources.refund(admitted);
            Err(error)
        }
    }
}

fn scalar_at(value: &Value, index: usize) -> Result<Value, Error> {
    match value {
        Value::Bool(value) => Ok(Value::Bool(*value)),
        Value::Int(value) => Ok(Value::Int(*value)),
        Value::Double(value) => Ok(Value::Double(*value)),
        Value::BoolVector(values) => values
            .get(index)
            .copied()
            .map(Value::Bool)
            .ok_or_else(|| type_runtime_error("application", SourceLocation::start())),
        Value::IntVector(values) => values
            .get(index)
            .copied()
            .map(Value::Int)
            .ok_or_else(|| type_runtime_error("application", SourceLocation::start())),
        Value::DoubleVector(values) => values
            .get(index)
            .copied()
            .map(Value::Double)
            .ok_or_else(|| type_runtime_error("application", SourceLocation::start())),
        Value::Tuple(_) => Err(type_runtime_error("application", SourceLocation::start())),
    }
}

fn invoke(
    name: &str,
    operands: &[Value],
    result_type: ScalarType,
    location: SourceLocation,
    element_index: Option<usize>,
) -> Result<Value, Error> {
    let numeric_relation = matches!(
        name,
        "equals" | "not_equals" | "is_positive" | "is_negative" | "less_than" | "greater_than"
    );
    let promotion_type = if numeric_relation
        && operands
            .iter()
            .any(|operand| matches!(operand, Value::Double(_)))
    {
        ScalarType::Double
    } else {
        result_type
    };
    let converted: Vec<Value> = operands
        .iter()
        .map(|operand| promote(operand, promotion_type))
        .collect();
    let result = match (name, converted.as_slice()) {
        ("inc", [Value::Int(value)]) => value.checked_add(1).map(Value::Int),
        ("dec", [Value::Int(value)]) => value.checked_sub(1).map(Value::Int),
        ("neg", [Value::Int(value)]) => value.checked_neg().map(Value::Int),
        ("abs", [Value::Int(value)]) => value.checked_abs().map(Value::Int),
        ("add", [Value::Int(left), Value::Int(right)]) => left.checked_add(*right).map(Value::Int),
        ("sub", [Value::Int(left), Value::Int(right)]) => left.checked_sub(*right).map(Value::Int),
        ("mul", [Value::Int(left), Value::Int(right)]) => left.checked_mul(*right).map(Value::Int),
        ("inc", [Value::Double(value)]) => Some(Value::Double(strict_float::arithmetic(
            *value,
            1.0,
            Binary64Operation::Add,
        ))),
        ("dec", [Value::Double(value)]) => Some(Value::Double(strict_float::arithmetic(
            *value,
            1.0,
            Binary64Operation::Subtract,
        ))),
        ("neg", [Value::Double(value)]) => Some(Value::Double(strict_float::negate(*value))),
        ("abs", [Value::Double(value)]) => Some(Value::Double(strict_float::absolute(*value))),
        ("add", [Value::Double(left), Value::Double(right)]) => Some(Value::Double(
            strict_float::arithmetic(*left, *right, Binary64Operation::Add),
        )),
        ("sub", [Value::Double(left), Value::Double(right)]) => Some(Value::Double(
            strict_float::arithmetic(*left, *right, Binary64Operation::Subtract),
        )),
        ("mul", [Value::Double(left), Value::Double(right)]) => Some(Value::Double(
            strict_float::arithmetic(*left, *right, Binary64Operation::Multiply),
        )),
        ("equals", [left, right]) => Some(Value::Bool(equals(left, right))),
        ("not_equals", [left, right]) => Some(Value::Bool(!equals(left, right))),
        ("not", [Value::Bool(value)]) => Some(Value::Bool(!value)),
        ("and", [Value::Bool(left), Value::Bool(right)]) => Some(Value::Bool(*left && *right)),
        ("or", [Value::Bool(left), Value::Bool(right)]) => Some(Value::Bool(*left || *right)),
        ("odd", [Value::Int(value)]) => Some(Value::Bool(value % 2 != 0)),
        ("even", [Value::Int(value)]) => Some(Value::Bool(value % 2 == 0)),
        ("is_positive", [Value::Int(value)]) => Some(Value::Bool(*value > 0)),
        ("is_negative", [Value::Int(value)]) => Some(Value::Bool(*value < 0)),
        ("is_positive", [Value::Double(value)]) => {
            Some(Value::Bool(strict_float::is_positive(*value)))
        }
        ("is_negative", [Value::Double(value)]) => {
            Some(Value::Bool(strict_float::is_negative(*value)))
        }
        ("less_than", [Value::Int(left), Value::Int(right)]) => Some(Value::Bool(left < right)),
        ("greater_than", [Value::Int(left), Value::Int(right)]) => Some(Value::Bool(left > right)),
        ("less_than", [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Bool(strict_float::less_than(*left, *right)))
        }
        ("greater_than", [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Bool(strict_float::less_than(*right, *left)))
        }
        _ => None,
    };
    result.ok_or_else(|| {
        let mut error = Error::new(
            ErrorKind::DomainError,
            location,
            format!(
                "{name} failed: integer_overflow{}",
                if let Some(index) = element_index {
                    format!(" at result index {index}")
                } else {
                    String::new()
                }
            ),
        );
        error.primitive = Some(name.to_owned());
        error.domain = Some(DomainErrorContext {
            reason: DomainErrorReason::IntegerOverflow,
            parameter_types: operands.iter().filter_map(Value::scalar_type).collect(),
            result_type,
            operands: operands.to_vec(),
            element_index,
        });
        error
    })
}

fn promote(value: &Value, result_type: ScalarType) -> Value {
    let requires_double = result_type == ScalarType::Double;
    match value {
        Value::Int(integer) if requires_double => {
            Value::Double(strict_float::int_to_binary64(*integer))
        }
        _ => value.clone(),
    }
}

fn equals(left: &Value, right: &Value) -> bool {
    match (left, right) {
        (Value::Bool(left), Value::Bool(right)) => left == right,
        (Value::Int(left), Value::Int(right)) => left == right,
        (Value::Double(left), Value::Double(right)) => strict_float::equal(*left, *right),
        (Value::Int(left), Value::Double(right)) => {
            strict_float::equal(strict_float::int_to_binary64(*left), *right)
        }
        (Value::Double(left), Value::Int(right)) => {
            strict_float::equal(*left, strict_float::int_to_binary64(*right))
        }
        _ => false,
    }
}

fn vector_from_scalars(element_type: ScalarType, values: Vec<Value>) -> Result<Value, Error> {
    match element_type {
        ScalarType::Bool => values
            .into_iter()
            .map(|value| match value {
                Value::Bool(value) => Ok(value),
                _ => Err(type_runtime_error("application", SourceLocation::start())),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::BoolVector),
        ScalarType::Int => values
            .into_iter()
            .map(|value| match value {
                Value::Int(value) => Ok(value),
                _ => Err(type_runtime_error("application", SourceLocation::start())),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::IntVector),
        ScalarType::Double => values
            .into_iter()
            .map(|value| match value {
                Value::Double(value) => Ok(value),
                _ => Err(type_runtime_error("application", SourceLocation::start())),
            })
            .collect::<Result<Vec<_>, _>>()
            .map(Value::DoubleVector),
    }
}

fn type_runtime_error(name: &str, location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::TypeError,
        location,
        format!("{name} arguments do not match an accepted signature"),
    )
}

fn resource_size_error(name: &str, location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::ResourceError,
        location,
        format!("{name} failed: size_overflow"),
    )
}

fn allocation_error(name: &str, location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::ResourceError,
        location,
        format!("{name} failed: allocation_unavailable"),
    )
}
