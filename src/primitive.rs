use crate::parser::{Expr, ExprKind, Program};
use crate::resources::ResourceContext;
use crate::semantic_registry::{ScalarKernel, implementation_from_numeric, primitive_from_name};
use crate::strict_float::{self, Binary64Operation};
use crate::{
    DomainErrorContext, DomainErrorReason, Error, ErrorKind, ScalarType, SourceLocation, Value,
};

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
        | ExprKind::OperationReference { .. }
        | ExprKind::Placeholder => {}
    }
    Ok(())
}

pub(crate) struct SelectedApplicationArgument<'a> {
    pub value: &'a Value,
    pub conversion: crate::Conversion,
}

pub(crate) fn apply_implementation(
    implementation_id: u16,
    arguments: &[SelectedApplicationArgument<'_>],
    lift: crate::LiftMode,
    result_type: ScalarType,
    location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let descriptor = implementation_from_numeric(implementation_id)
        .map_err(|_| type_runtime_error("selected implementation", location))?;
    let producer = descriptor.primitive_name;
    if descriptor.kernel == ScalarKernel::IotaInt {
        let Some(SelectedApplicationArgument {
            value: Value::Int(bound),
            ..
        }) = arguments.first()
        else {
            return Err(type_runtime_error(producer, location));
        };
        let length = if *bound <= 0 {
            0
        } else {
            usize::try_from(*bound).map_err(|_| resource_size_error(producer, location))?
        };
        let admitted = resources.admit_vector_with_work(
            ScalarType::Int,
            length,
            length,
            location,
            producer,
        )?;
        let mut values = Vec::new();
        if values.try_reserve_exact(length).is_err() {
            resources.refund(admitted);
            return Err(allocation_error(producer, location));
        }
        for value in 1..=*bound {
            values.push(value);
        }
        return Ok((Value::IntVector(values), true));
    }

    let accounted = !matches!(lift, crate::LiftMode::Scalar);
    let count = if accounted {
        arguments
            .iter()
            .find(|argument| argument.value.is_vector())
            .map_or(0, |argument| argument.value.len())
    } else {
        1
    };
    let admitted = if accounted {
        resources.admit_vector_with_work(result_type, count, count, location, producer)?
    } else {
        resources.charge_work(count, location, producer)?;
        0
    };
    let mut scalar_results = Vec::new();
    if scalar_results.try_reserve_exact(count).is_err() {
        resources.refund(admitted);
        return Err(allocation_error(producer, location));
    }
    for index in 0..count {
        let mut operands = Vec::new();
        if operands.try_reserve_exact(arguments.len()).is_err() {
            resources.refund(admitted);
            return Err(allocation_error(producer, location));
        }
        for argument in arguments {
            let value = match scalar_at(argument.value, index) {
                Ok(value) => value,
                Err(error) => {
                    resources.refund(admitted);
                    return Err(error);
                }
            };
            operands.push(match (argument.conversion, value) {
                (crate::Conversion::Identity, value) => value,
                (crate::Conversion::PromoteIntToDouble, Value::Int(value)) => {
                    Value::Double(strict_float::int_to_binary64(value))
                }
                _ => {
                    resources.refund(admitted);
                    return Err(type_runtime_error(producer, location));
                }
            });
        }
        match invoke_kernel(
            descriptor.kernel,
            producer,
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
            .ok_or_else(|| type_runtime_error(producer, location));
    }
    match vector_from_scalars(result_type, scalar_results) {
        Ok(value) => Ok((value, true)),
        Err(error) => {
            resources.refund(admitted);
            Err(error)
        }
    }
}

#[allow(dead_code)]
pub(crate) fn apply_operation_reference(
    reference: &crate::OperationReference,
    arguments: &[SelectedApplicationArgument<'_>],
    lift: crate::LiftMode,
    result_type: ScalarType,
    location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let descriptor = implementation_from_numeric(reference.implementation_id)
        .map_err(|_| type_runtime_error("referenced operation", location))?;
    if descriptor.primitive_id.numeric() != reference.primitive_id
        || descriptor.signature_id.numeric() != reference.signature_id
        || descriptor.result != result_type
        || descriptor.parameters.len() != arguments.len()
    {
        return Err(type_runtime_error("referenced operation", location));
    }
    apply_implementation(
        reference.implementation_id,
        arguments,
        lift,
        result_type,
        location,
        resources,
    )
}

pub(crate) fn implementation_name(implementation_id: u16) -> Option<&'static str> {
    implementation_from_numeric(implementation_id)
        .ok()
        .map(|descriptor| descriptor.primitive_name)
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

fn invoke_kernel(
    kernel: ScalarKernel,
    producer: &str,
    operands: &[Value],
    result_type: ScalarType,
    location: SourceLocation,
    element_index: Option<usize>,
) -> Result<Value, Error> {
    let result = match (kernel, operands) {
        (ScalarKernel::IncInt, [Value::Int(value)]) => value.checked_add(1).map(Value::Int),
        (ScalarKernel::DecInt, [Value::Int(value)]) => value.checked_sub(1).map(Value::Int),
        (ScalarKernel::NegInt, [Value::Int(value)]) => value.checked_neg().map(Value::Int),
        (ScalarKernel::AbsInt, [Value::Int(value)]) => value.checked_abs().map(Value::Int),
        (ScalarKernel::AddInt, [Value::Int(left), Value::Int(right)]) => {
            left.checked_add(*right).map(Value::Int)
        }
        (ScalarKernel::SubInt, [Value::Int(left), Value::Int(right)]) => {
            left.checked_sub(*right).map(Value::Int)
        }
        (ScalarKernel::MulInt, [Value::Int(left), Value::Int(right)]) => {
            left.checked_mul(*right).map(Value::Int)
        }
        (ScalarKernel::IncDouble, [Value::Double(value)]) => Some(Value::Double(
            strict_float::arithmetic(*value, 1.0, Binary64Operation::Add),
        )),
        (ScalarKernel::DecDouble, [Value::Double(value)]) => Some(Value::Double(
            strict_float::arithmetic(*value, 1.0, Binary64Operation::Subtract),
        )),
        (ScalarKernel::NegDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::negate(*value)))
        }
        (ScalarKernel::AbsDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::absolute(*value)))
        }
        (ScalarKernel::AddDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Double(strict_float::arithmetic(
                *left,
                *right,
                Binary64Operation::Add,
            )))
        }
        (ScalarKernel::SubDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Double(strict_float::arithmetic(
                *left,
                *right,
                Binary64Operation::Subtract,
            )))
        }
        (ScalarKernel::MulDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Double(strict_float::arithmetic(
                *left,
                *right,
                Binary64Operation::Multiply,
            )))
        }
        (ScalarKernel::EqualsBool, [left, right])
        | (ScalarKernel::EqualsInt, [left, right])
        | (ScalarKernel::EqualsDouble, [left, right]) => Some(Value::Bool(equals(left, right))),
        (ScalarKernel::NotEqualsBool, [left, right])
        | (ScalarKernel::NotEqualsInt, [left, right])
        | (ScalarKernel::NotEqualsDouble, [left, right]) => Some(Value::Bool(!equals(left, right))),
        (ScalarKernel::NotBool, [Value::Bool(value)]) => Some(Value::Bool(!value)),
        (ScalarKernel::AndBool, [Value::Bool(left), Value::Bool(right)]) => {
            Some(Value::Bool(*left && *right))
        }
        (ScalarKernel::OrBool, [Value::Bool(left), Value::Bool(right)]) => {
            Some(Value::Bool(*left || *right))
        }
        (ScalarKernel::OddInt, [Value::Int(value)]) => Some(Value::Bool(value % 2 != 0)),
        (ScalarKernel::EvenInt, [Value::Int(value)]) => Some(Value::Bool(value % 2 == 0)),
        (ScalarKernel::IsPositiveInt, [Value::Int(value)]) => Some(Value::Bool(*value > 0)),
        (ScalarKernel::IsNegativeInt, [Value::Int(value)]) => Some(Value::Bool(*value < 0)),
        (ScalarKernel::IsPositiveDouble, [Value::Double(value)]) => {
            Some(Value::Bool(strict_float::is_positive(*value)))
        }
        (ScalarKernel::IsNegativeDouble, [Value::Double(value)]) => {
            Some(Value::Bool(strict_float::is_negative(*value)))
        }
        (ScalarKernel::LessThanInt, [Value::Int(left), Value::Int(right)]) => {
            Some(Value::Bool(left < right))
        }
        (ScalarKernel::GreaterThanInt, [Value::Int(left), Value::Int(right)]) => {
            Some(Value::Bool(left > right))
        }
        (ScalarKernel::LessThanDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Bool(strict_float::less_than(*left, *right)))
        }
        (ScalarKernel::GreaterThanDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Bool(strict_float::less_than(*right, *left)))
        }
        (ScalarKernel::IotaInt, _) => None,
        _ => None,
    };
    result.ok_or_else(|| {
        let mut error = Error::new(
            ErrorKind::DomainError,
            location,
            format!(
                "{producer} failed: integer_overflow{}",
                if let Some(index) = element_index {
                    format!(" at result index {index}")
                } else {
                    String::new()
                }
            ),
        );
        error.primitive = Some(producer.to_owned());
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AllocationFailureInjection, Conversion, ExecutionProfile, OperationReference,
        ResourceLimits,
    };

    fn context(limits: ResourceLimits) -> ResourceContext {
        match ResourceContext::new(
            ExecutionProfile::BoundedV2,
            limits,
            AllocationFailureInjection::default(),
        ) {
            Ok(context) => context,
            Err(error) => panic!("resource context failed: {error:?}"),
        }
    }

    #[test]
    fn stable_operation_reference_dispatch_uses_recorded_implementation_identity() {
        let left = Value::Int(2);
        let right = Value::Int(3);
        let arguments = [
            SelectedApplicationArgument {
                value: &left,
                conversion: Conversion::Identity,
            },
            SelectedApplicationArgument {
                value: &right,
                conversion: Conversion::Identity,
            },
        ];
        let reference = OperationReference {
            primitive_id: 5,
            signature_id: 9,
            implementation_id: 9,
            origin: crate::OriginIndex(0),
        };
        let mut resources = context(ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        });
        assert_eq!(
            apply_operation_reference(
                &reference,
                &arguments,
                crate::LiftMode::Scalar,
                ScalarType::Int,
                SourceLocation::start(),
                &mut resources,
            ),
            Ok((Value::Int(5), false))
        );
        assert_eq!(resources.usage.work_units, 1);

        let mut refused = context(ResourceLimits {
            max_work_units: Some(0),
            ..ResourceLimits::default()
        });
        let error = apply_operation_reference(
            &reference,
            &arguments,
            crate::LiftMode::Scalar,
            ScalarType::Int,
            SourceLocation::start(),
            &mut refused,
        )
        .expect_err("work refusal");
        assert_eq!(error.kind, ErrorKind::ResourceError);

        let invalid = OperationReference {
            implementation_id: 10,
            ..reference
        };
        let mut resources = context(ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        });
        assert_eq!(
            apply_operation_reference(
                &invalid,
                &arguments,
                crate::LiftMode::Scalar,
                ScalarType::Int,
                SourceLocation::start(),
                &mut resources,
            )
            .expect_err("mismatched identity")
            .kind,
            ErrorKind::TypeError
        );
    }
}
