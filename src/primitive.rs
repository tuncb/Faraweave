use crate::parser::{Expr, ExprKind, Program};
use crate::resources::ResourceContext;
use crate::semantic_registry::{
    AdmissionSequence, ApplicationPlan, ResultCardinality, ScalarKernel, StructuralBehavior,
    WorkAdmission, application_plan_from_numeric, implementation_from_numeric, primitive_from_name,
};
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
        ExprKind::Connected { templates, operand } => {
            for template in templates {
                for argument in &template.arguments {
                    validate_names(argument)?;
                }
            }
            validate_names(operand)?;
            for template in templates.iter().rev() {
                if primitive_from_name(&template.name).is_err() {
                    return Err(Error::at_span(
                        ErrorKind::UnknownPrimitive,
                        template.name_span,
                        format!("unknown primitive '{}'", template.name),
                    ));
                }
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
    application_plan_id: u16,
    arguments: &[SelectedApplicationArgument<'_>],
    lift: crate::LiftMode,
    result_type: ScalarType,
    location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let descriptor = implementation_from_numeric(implementation_id)
        .map_err(|_| type_runtime_error("selected implementation", location))?;
    let application_plan = application_plan_from_numeric(application_plan_id)
        .map_err(|_| type_runtime_error("selected application plan", location))?;
    if descriptor.application_plan != application_plan {
        return Err(type_runtime_error("selected application plan", location));
    }
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
            admitted_work(application_plan, length, arguments, producer, location)?,
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

    if matches!(
        descriptor.kernel,
        ScalarKernel::LengthBoolVector
            | ScalarKernel::LengthIntVector
            | ScalarKernel::LengthDoubleVector
    ) {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerScalar
            || result_type != ScalarType::Int
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let length = match (descriptor.kernel, argument.value) {
            (ScalarKernel::LengthBoolVector, Value::BoolVector(values)) => values.len(),
            (ScalarKernel::LengthIntVector, Value::IntVector(values)) => values.len(),
            (ScalarKernel::LengthDoubleVector, Value::DoubleVector(values)) => values.len(),
            _ => return Err(type_runtime_error(producer, location)),
        };
        let work = admitted_work(application_plan, 1, arguments, producer, location)?;
        return apply_vector_length(length, work, location, producer, resources)
            .map(|value| (value, false));
    }

    if matches!(
        descriptor.kernel,
        ScalarKernel::SortBoolVector | ScalarKernel::SortIntVector | ScalarKernel::SortDoubleVector
    ) {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerVector
            || result_type != descriptor.result
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let work = admitted_work(
            application_plan,
            argument.value.len(),
            arguments,
            producer,
            location,
        )?;
        return apply_vector_sort(
            descriptor.kernel,
            argument.value,
            work,
            location,
            producer,
            resources,
        )
        .map(|value| (value, true));
    }

    if matches!(
        descriptor.kernel,
        ScalarKernel::SumIntVector | ScalarKernel::SumDoubleVector
    ) {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerScalar
            || result_type != descriptor.result
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let work = admitted_work(application_plan, 1, arguments, producer, location)?;
        return apply_vector_sum(
            descriptor.kernel,
            argument.value,
            work,
            location,
            producer,
            resources,
        )
        .map(|value| (value, false));
    }

    if descriptor.kernel == ScalarKernel::AllOfBoolVector {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerScalar
            || result_type != ScalarType::Bool
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let work = admitted_work(application_plan, 1, arguments, producer, location)?;
        return apply_vector_all_of(argument.value, work, location, producer, resources)
            .map(|value| (value, false));
    }

    if descriptor.kernel == ScalarKernel::AnyOfBoolVector {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerScalar
            || result_type != ScalarType::Bool
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let work = admitted_work(application_plan, 1, arguments, producer, location)?;
        return apply_vector_any_of(argument.value, work, location, producer, resources)
            .map(|value| (value, false));
    }

    if descriptor.kernel == ScalarKernel::NoneOfBoolVector {
        let [argument] = arguments else {
            return Err(type_runtime_error(producer, location));
        };
        if lift != crate::LiftMode::ContainerScalar
            || result_type != ScalarType::Bool
            || argument.conversion != crate::Conversion::Identity
        {
            return Err(type_runtime_error(producer, location));
        }
        let work = admitted_work(application_plan, 1, arguments, producer, location)?;
        return apply_vector_none_of(argument.value, work, location, producer, resources)
            .map(|value| (value, false));
    }

    if matches!(
        lift,
        crate::LiftMode::ContainerScalar | crate::LiftMode::ContainerVector
    ) {
        return Err(type_runtime_error("container implementation", location));
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
        resources.admit_vector_with_work(
            result_type,
            count,
            admitted_work(application_plan, count, arguments, producer, location)?,
            location,
            producer,
        )?
    } else {
        resources.charge_work(
            admitted_work(application_plan, count, arguments, producer, location)?,
            location,
            producer,
        )?;
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

#[allow(clippy::too_many_arguments)]
pub(crate) fn apply_reference_consumer_implementation(
    implementation_id: u16,
    application_plan_id: u16,
    reference: &crate::OperationReference,
    arguments: &[SelectedApplicationArgument<'_>],
    lift: crate::LiftMode,
    result_type: ScalarType,
    location: SourceLocation,
    reference_location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let descriptor = implementation_from_numeric(implementation_id)
        .map_err(|_| type_runtime_error("selected implementation", location))?;
    let application_plan = application_plan_from_numeric(application_plan_id)
        .map_err(|_| type_runtime_error("selected application plan", location))?;
    let producer = descriptor.primitive_name;
    if descriptor.behavior == StructuralBehavior::VectorFilter {
        return apply_vector_filter_consumer(
            descriptor,
            application_plan,
            reference,
            arguments,
            lift,
            result_type,
            location,
            reference_location,
            resources,
        );
    }
    let valid_lift = match descriptor.behavior {
        StructuralBehavior::Foldl => lift == crate::LiftMode::ContainerScalar,
        StructuralBehavior::Scanl => lift == crate::LiftMode::ContainerVector,
        _ => false,
    };
    if descriptor.application_plan != application_plan
        || !valid_lift
        || descriptor.result != result_type
    {
        return Err(type_runtime_error(producer, location));
    }
    let [initializer, vector] = arguments else {
        return Err(type_runtime_error(producer, location));
    };
    if vector.conversion != crate::Conversion::Identity {
        return Err(type_runtime_error(producer, location));
    }
    let reducer = implementation_from_numeric(reference.implementation_id)
        .map_err(|_| type_runtime_error("referenced operation", reference_location))?;
    if reducer.primitive_id.numeric() != reference.primitive_id
        || reducer.signature_id.numeric() != reference.signature_id
        || reducer.behavior != StructuralBehavior::Elementwise
        || reducer.result != result_type
        || reducer.parameters.len() != 2
        || reducer.parameters.iter().any(|operand| {
            operand.consumption != crate::semantic_registry::OperandConsumption::Elementwise
                || operand.element_type != result_type
        })
    {
        return Err(type_runtime_error(
            "referenced operation",
            reference_location,
        ));
    }
    let mut accumulator = match (initializer.conversion, initializer.value) {
        (crate::Conversion::Identity, Value::Bool(value)) if result_type == ScalarType::Bool => {
            Value::Bool(*value)
        }
        (crate::Conversion::Identity, Value::Int(value)) if result_type == ScalarType::Int => {
            Value::Int(*value)
        }
        (crate::Conversion::Identity, Value::Double(value))
            if result_type == ScalarType::Double =>
        {
            Value::Double(*value)
        }
        (crate::Conversion::PromoteIntToDouble, Value::Int(value))
            if result_type == ScalarType::Double =>
        {
            Value::Double(strict_float::int_to_binary64(*value))
        }
        _ => return Err(type_runtime_error(producer, location)),
    };
    let length = match (descriptor.kernel, vector.value) {
        (ScalarKernel::FoldlBool, Value::BoolVector(values)) => values.len(),
        (ScalarKernel::FoldlInt, Value::IntVector(values)) => values.len(),
        (ScalarKernel::FoldlDouble, Value::DoubleVector(values)) => values.len(),
        (ScalarKernel::ScanlBool, Value::BoolVector(values)) => values.len(),
        (ScalarKernel::ScanlInt, Value::IntVector(values)) => values.len(),
        (ScalarKernel::ScanlDouble, Value::DoubleVector(values)) => values.len(),
        _ => return Err(type_runtime_error(producer, location)),
    };
    let work = admitted_work(application_plan, 1, arguments, producer, location)?;
    if work != length {
        return Err(type_runtime_error(producer, location));
    }
    if descriptor.behavior == StructuralBehavior::Foldl {
        resources.charge_work(work, location, producer)?;
        for index in 0..length {
            let element = vector_element(vector.value, index, producer, location)?;
            let operands = [accumulator, element];
            accumulator = invoke_kernel(
                reducer.kernel,
                reducer.primitive_name,
                &operands,
                result_type,
                reference_location,
                Some(index),
            )?;
        }
        return Ok((accumulator, false));
    }

    let output_length = scan_output_length(length, location, producer, resources)?;
    let admitted =
        resources.admit_vector_with_work(result_type, output_length, work, location, producer)?;
    let mut output = match allocate_scan_output(result_type, output_length) {
        Ok(output) => output,
        Err(()) => {
            resources.refund(admitted);
            return Err(allocation_error(producer, location));
        }
    };
    if write_scan_output(&mut output, 0, &accumulator).is_err() {
        resources.release(&output);
        return Err(type_runtime_error(producer, location));
    }
    for index in 0..length {
        let element = match vector_element(vector.value, index, producer, location) {
            Ok(element) => element,
            Err(error) => {
                resources.release(&output);
                return Err(error);
            }
        };
        let operands = [accumulator, element];
        accumulator = match invoke_kernel(
            reducer.kernel,
            reducer.primitive_name,
            &operands,
            result_type,
            reference_location,
            Some(index),
        ) {
            Ok(value) => value,
            Err(error) => {
                resources.release(&output);
                return Err(error);
            }
        };
        if write_scan_output(&mut output, index + 1, &accumulator).is_err() {
            resources.release(&output);
            return Err(type_runtime_error(producer, location));
        }
    }
    Ok((output, true))
}

#[allow(clippy::too_many_arguments)]
fn apply_vector_filter_consumer(
    descriptor: &crate::semantic_registry::SemanticDescriptor,
    application_plan: ApplicationPlan,
    reference: &crate::OperationReference,
    arguments: &[SelectedApplicationArgument<'_>],
    lift: crate::LiftMode,
    result_type: ScalarType,
    location: SourceLocation,
    reference_location: SourceLocation,
    resources: &mut ResourceContext,
) -> Result<(Value, bool), Error> {
    let producer = descriptor.primitive_name;
    if descriptor.application_plan != application_plan
        || application_plan.result_cardinality != ResultCardinality::SubsetOfOperand(1)
        || application_plan.resources.work != WorkAdmission::OperandCardinality(1)
        || application_plan.resources.sequence != AdmissionSequence::WorkThenResult
        || lift != crate::LiftMode::ContainerVector
        || descriptor.result != result_type
    {
        return Err(type_runtime_error(producer, location));
    }
    let [vector] = arguments else {
        return Err(type_runtime_error(producer, location));
    };
    if vector.conversion != crate::Conversion::Identity {
        return Err(type_runtime_error(producer, location));
    }
    let predicate = implementation_from_numeric(reference.implementation_id)
        .map_err(|_| type_runtime_error("referenced operation", reference_location))?;
    if predicate.primitive_id.numeric() != reference.primitive_id
        || predicate.signature_id.numeric() != reference.signature_id
        || !crate::semantic_registry::is_total_unary_predicate(predicate)
        || predicate
            .parameters
            .first()
            .is_none_or(|parameter| parameter.element_type != result_type)
    {
        return Err(type_runtime_error(
            "referenced operation",
            reference_location,
        ));
    }
    let length = match (descriptor.kernel, vector.value) {
        (ScalarKernel::FilterBool, Value::BoolVector(values)) => values.len(),
        (ScalarKernel::FilterInt, Value::IntVector(values)) => values.len(),
        (ScalarKernel::FilterDouble, Value::DoubleVector(values)) => values.len(),
        _ => return Err(type_runtime_error(producer, location)),
    };
    let work = admitted_work(application_plan, 0, arguments, producer, location)?;
    if work != length {
        return Err(type_runtime_error(producer, location));
    }
    resources.charge_work(work, location, producer)?;
    let mut kept = 0_usize;
    for index in 0..length {
        let element = vector_element(vector.value, index, producer, location)?;
        let predicate_result = invoke_kernel(
            predicate.kernel,
            predicate.primitive_name,
            &[element],
            ScalarType::Bool,
            reference_location,
            Some(index),
        )?;
        match predicate_result {
            Value::Bool(true) => {
                kept = kept
                    .checked_add(1)
                    .ok_or_else(|| resources.size_overflow(Some(length), location, producer))?;
            }
            Value::Bool(false) => {}
            _ => {
                return Err(type_runtime_error(
                    "referenced operation",
                    reference_location,
                ));
            }
        }
    }
    let admitted = resources.admit_vector(result_type, kept, location, producer)?;
    let mut output = match allocate_filter_output(result_type, kept) {
        Ok(output) => output,
        Err(()) => {
            resources.refund(admitted);
            return Err(allocation_error(producer, location));
        }
    };
    for index in 0..length {
        let element = match vector_element(vector.value, index, producer, location) {
            Ok(element) => element,
            Err(error) => {
                resources.release(&output);
                return Err(error);
            }
        };
        let predicate_result = match invoke_kernel(
            predicate.kernel,
            predicate.primitive_name,
            std::slice::from_ref(&element),
            ScalarType::Bool,
            reference_location,
            Some(index),
        ) {
            Ok(result) => result,
            Err(error) => {
                resources.release(&output);
                return Err(error);
            }
        };
        match predicate_result {
            Value::Bool(true) => {
                if push_filter_output(&mut output, element).is_err() {
                    resources.release(&output);
                    return Err(type_runtime_error(producer, location));
                }
            }
            Value::Bool(false) => {}
            _ => {
                resources.release(&output);
                return Err(type_runtime_error(
                    "referenced operation",
                    reference_location,
                ));
            }
        }
    }
    Ok((output, true))
}

fn allocate_filter_output(element_type: ScalarType, length: usize) -> Result<Value, ()> {
    match element_type {
        ScalarType::Bool => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            Ok(Value::BoolVector(values))
        }
        ScalarType::Int => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            Ok(Value::IntVector(values))
        }
        ScalarType::Double => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            Ok(Value::DoubleVector(values))
        }
    }
}

fn push_filter_output(output: &mut Value, value: Value) -> Result<(), ()> {
    match (output, value) {
        (Value::BoolVector(values), Value::Bool(value)) => {
            values.push(value);
            Ok(())
        }
        (Value::IntVector(values), Value::Int(value)) => {
            values.push(value);
            Ok(())
        }
        (Value::DoubleVector(values), Value::Double(value)) => {
            values.push(value);
            Ok(())
        }
        _ => Err(()),
    }
}

fn vector_element(
    vector: &Value,
    index: usize,
    producer: &str,
    location: SourceLocation,
) -> Result<Value, Error> {
    match vector {
        Value::BoolVector(values) => values.get(index).copied().map(Value::Bool),
        Value::IntVector(values) => values.get(index).copied().map(Value::Int),
        Value::DoubleVector(values) => values.get(index).copied().map(Value::Double),
        _ => None,
    }
    .ok_or_else(|| type_runtime_error(producer, location))
}

fn scan_output_length(
    input_length: usize,
    location: SourceLocation,
    producer: &str,
    resources: &ResourceContext,
) -> Result<usize, Error> {
    input_length
        .checked_add(1)
        .ok_or_else(|| resources.size_overflow(Some(input_length), location, producer))
}

fn allocate_scan_output(element_type: ScalarType, length: usize) -> Result<Value, ()> {
    match element_type {
        ScalarType::Bool => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            values.resize(length, false);
            Ok(Value::BoolVector(values))
        }
        ScalarType::Int => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            values.resize(length, 0);
            Ok(Value::IntVector(values))
        }
        ScalarType::Double => {
            let mut values = Vec::new();
            values.try_reserve_exact(length).map_err(|_| ())?;
            values.resize(length, 0.0);
            Ok(Value::DoubleVector(values))
        }
    }
}

fn write_scan_output(output: &mut Value, index: usize, value: &Value) -> Result<(), ()> {
    match (output, value) {
        (Value::BoolVector(values), Value::Bool(value)) => values
            .get_mut(index)
            .map(|destination| *destination = *value)
            .ok_or(()),
        (Value::IntVector(values), Value::Int(value)) => values
            .get_mut(index)
            .map(|destination| *destination = *value)
            .ok_or(()),
        (Value::DoubleVector(values), Value::Double(value)) => values
            .get_mut(index)
            .map(|destination| *destination = *value)
            .ok_or(()),
        _ => Err(()),
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
        descriptor.application_plan.id.numeric(),
        arguments,
        lift,
        result_type,
        location,
        resources,
    )
}

fn admitted_work(
    plan: ApplicationPlan,
    result_cardinality: usize,
    arguments: &[SelectedApplicationArgument<'_>],
    producer: &str,
    location: SourceLocation,
) -> Result<usize, Error> {
    match plan.resources.work {
        WorkAdmission::Constant(value) => {
            usize::try_from(value).map_err(|_| resource_size_error(producer, location))
        }
        WorkAdmission::ResultCardinality => Ok(result_cardinality),
        WorkAdmission::OperandCardinality(position) => position
            .checked_sub(1)
            .map(usize::from)
            .and_then(|index| arguments.get(index))
            .map(|argument| argument.value.len())
            .ok_or_else(|| type_runtime_error(producer, location)),
    }
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
    if let (ScalarKernel::DivInt, [Value::Int(left), Value::Int(right)]) = (kernel, operands) {
        if *right == 0 {
            return Err(integer_domain_error(
                producer,
                operands,
                result_type,
                location,
                element_index,
                DomainErrorReason::DivisionByZero,
            ));
        }
        return left.checked_div(*right).map(Value::Int).ok_or_else(|| {
            integer_domain_error(
                producer,
                operands,
                result_type,
                location,
                element_index,
                DomainErrorReason::IntegerOverflow,
            )
        });
    }

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
        (ScalarKernel::DivDouble, [Value::Double(left), Value::Double(right)]) => {
            Some(Value::Double(strict_float::arithmetic(
                *left,
                *right,
                Binary64Operation::Divide,
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
        (ScalarKernel::SqrtDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_sqrt(*value)))
        }
        (ScalarKernel::ExpDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_exp(*value)))
        }
        (ScalarKernel::LogDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_log(*value)))
        }
        (ScalarKernel::Log10Double, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_log10(*value)))
        }
        (ScalarKernel::SinDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_sin(*value)))
        }
        (ScalarKernel::CosDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_cos(*value)))
        }
        (ScalarKernel::TanDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_tan(*value)))
        }
        (ScalarKernel::FloorDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_floor(*value)))
        }
        (ScalarKernel::CeilDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_ceil(*value)))
        }
        (ScalarKernel::TruncDouble, [Value::Double(value)]) => {
            Some(Value::Double(strict_float::backend_native_trunc(*value)))
        }
        (
            ScalarKernel::DivInt
            | ScalarKernel::LengthBoolVector
            | ScalarKernel::LengthIntVector
            | ScalarKernel::LengthDoubleVector
            | ScalarKernel::SortBoolVector
            | ScalarKernel::SortIntVector
            | ScalarKernel::SortDoubleVector
            | ScalarKernel::SumIntVector
            | ScalarKernel::SumDoubleVector
            | ScalarKernel::AllOfBoolVector
            | ScalarKernel::AnyOfBoolVector
            | ScalarKernel::NoneOfBoolVector
            | ScalarKernel::FoldlBool
            | ScalarKernel::FoldlInt
            | ScalarKernel::FoldlDouble
            | ScalarKernel::ScanlBool
            | ScalarKernel::ScanlInt
            | ScalarKernel::ScanlDouble
            | ScalarKernel::IotaInt,
            _,
        ) => None,
        _ => None,
    };
    result.ok_or_else(|| {
        integer_domain_error(
            producer,
            operands,
            result_type,
            location,
            element_index,
            DomainErrorReason::IntegerOverflow,
        )
    })
}

fn apply_vector_length(
    length: usize,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    resources.charge_work(work, location, producer)?;
    i64::try_from(length)
        .map(Value::Int)
        .map_err(|_| resources.size_overflow(Some(length), location, producer))
}

fn apply_vector_sort(
    kernel: ScalarKernel,
    input: &Value,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    let (element_type, length) = match (kernel, input) {
        (ScalarKernel::SortBoolVector, Value::BoolVector(values)) => {
            (ScalarType::Bool, values.len())
        }
        (ScalarKernel::SortIntVector, Value::IntVector(values)) => (ScalarType::Int, values.len()),
        (ScalarKernel::SortDoubleVector, Value::DoubleVector(values)) => {
            (ScalarType::Double, values.len())
        }
        _ => return Err(type_runtime_error(producer, location)),
    };
    let admitted =
        resources.admit_vector_with_work(element_type, length, work, location, producer)?;
    let result = match (kernel, input) {
        (ScalarKernel::SortBoolVector, Value::BoolVector(input)) => {
            let mut output = Vec::new();
            if output.try_reserve_exact(length).is_err() {
                resources.refund(admitted);
                return Err(allocation_error(producer, location));
            }
            output.extend_from_slice(input);
            output.sort_unstable();
            Value::BoolVector(output)
        }
        (ScalarKernel::SortIntVector, Value::IntVector(input)) => {
            let mut output = Vec::new();
            if output.try_reserve_exact(length).is_err() {
                resources.refund(admitted);
                return Err(allocation_error(producer, location));
            }
            output.extend_from_slice(input);
            output.sort_unstable();
            Value::IntVector(output)
        }
        (ScalarKernel::SortDoubleVector, Value::DoubleVector(input)) => {
            let mut output = Vec::new();
            if output.try_reserve_exact(length).is_err() {
                resources.refund(admitted);
                return Err(allocation_error(producer, location));
            }
            output.extend_from_slice(input);
            output.sort_unstable_by(f64::total_cmp);
            Value::DoubleVector(output)
        }
        _ => {
            resources.refund(admitted);
            return Err(type_runtime_error(producer, location));
        }
    };
    Ok(result)
}

fn apply_vector_sum(
    kernel: ScalarKernel,
    input: &Value,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    match (kernel, input) {
        (ScalarKernel::SumIntVector, Value::IntVector(values)) => {
            resources.charge_work(work, location, producer)?;
            let mut total = 0_i64;
            for (index, value) in values.iter().copied().enumerate() {
                let operands = [Value::Int(total), Value::Int(value)];
                total = total.checked_add(value).ok_or_else(|| {
                    integer_domain_error(
                        producer,
                        &operands,
                        ScalarType::Int,
                        location,
                        Some(index),
                        DomainErrorReason::IntegerOverflow,
                    )
                })?;
            }
            Ok(Value::Int(total))
        }
        (ScalarKernel::SumDoubleVector, Value::DoubleVector(values)) => {
            resources.charge_work(work, location, producer)?;
            let mut total = 0.0_f64;
            for value in values {
                total = strict_float::arithmetic(total, *value, Binary64Operation::Add);
            }
            Ok(Value::Double(total))
        }
        _ => Err(type_runtime_error(producer, location)),
    }
}

fn apply_vector_all_of(
    input: &Value,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    let Value::BoolVector(values) = input else {
        return Err(type_runtime_error(producer, location));
    };
    resources.charge_work(work, location, producer)?;
    Ok(Value::Bool(values.iter().all(|value| *value)))
}

fn apply_vector_any_of(
    input: &Value,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    let Value::BoolVector(values) = input else {
        return Err(type_runtime_error(producer, location));
    };
    resources.charge_work(work, location, producer)?;
    Ok(Value::Bool(values.iter().any(|value| *value)))
}

fn apply_vector_none_of(
    input: &Value,
    work: usize,
    location: SourceLocation,
    producer: &str,
    resources: &mut ResourceContext,
) -> Result<Value, Error> {
    let Value::BoolVector(values) = input else {
        return Err(type_runtime_error(producer, location));
    };
    resources.charge_work(work, location, producer)?;
    Ok(Value::Bool(values.iter().all(|value| !*value)))
}

fn integer_domain_error(
    producer: &str,
    operands: &[Value],
    result_type: ScalarType,
    location: SourceLocation,
    element_index: Option<usize>,
    reason: DomainErrorReason,
) -> Error {
    let reason_name = match reason {
        DomainErrorReason::IntegerOverflow => "integer_overflow",
        DomainErrorReason::DivisionByZero => "division_by_zero",
    };
    let mut error = Error::new(
        ErrorKind::DomainError,
        location,
        format!(
            "{producer} failed: {reason_name}{}",
            if let Some(index) = element_index {
                format!(" at result index {index}")
            } else {
                String::new()
            }
        ),
    );
    error.primitive = Some(producer.to_owned());
    error.domain = Some(DomainErrorContext {
        reason,
        parameter_types: operands.iter().filter_map(Value::scalar_type).collect(),
        result_type,
        operands: operands.to_vec(),
        element_index,
    });
    error
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

    #[test]
    fn vector_length_maximum_and_overflow_seams_charge_before_conversion() {
        let mut resources = context(ResourceLimits {
            max_work_units: Some(2),
            ..ResourceLimits::default()
        });
        let representable = usize::try_from(i64::MAX).unwrap_or(usize::MAX);
        assert_eq!(
            apply_vector_length(
                representable,
                1,
                SourceLocation::start(),
                "length",
                &mut resources
            ),
            Ok(Value::Int(i64::try_from(representable).unwrap_or(i64::MAX)))
        );
        assert_eq!(resources.usage.work_units, 1);

        #[cfg(target_pointer_width = "64")]
        {
            let unrepresentable = usize::try_from(i64::MAX)
                .ok()
                .and_then(|maximum| maximum.checked_add(1))
                .unwrap_or(usize::MAX);
            let error = apply_vector_length(
                unrepresentable,
                1,
                SourceLocation::start(),
                "length",
                &mut resources,
            )
            .expect_err("unrepresentable vector length");
            assert_eq!(error.kind, ErrorKind::ResourceError);
            let context = error.resource.expect("structured size overflow");
            assert_eq!(context.reason, crate::ResourceErrorReason::SizeOverflow);
            assert_eq!(context.requested_elements, Some(unrepresentable));
            assert_eq!(resources.usage.work_units, 2);
            assert_eq!(resources.usage.allocation_attempts, 0);
        }
    }

    #[test]
    fn scan_output_length_rejects_n_plus_one_overflow_before_admission() {
        let resources = context(ResourceLimits {
            max_work_units: Some(usize::MAX),
            ..ResourceLimits::default()
        });
        let error = scan_output_length(usize::MAX, SourceLocation::start(), "scanl", &resources)
            .expect_err("n + 1 overflow");
        assert_eq!(error.kind, ErrorKind::ResourceError);
        let context = error.resource.expect("structured size overflow");
        assert_eq!(context.reason, crate::ResourceErrorReason::SizeOverflow);
        assert_eq!(context.requested_elements, Some(usize::MAX));
        assert_eq!(resources.usage.work_units, 0);
        assert_eq!(resources.usage.allocation_attempts, 0);
    }
}
