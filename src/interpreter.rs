use crate::evaluator::{EvaluationConfiguration, ProgramResult};
use crate::primitive::{
    SelectedApplicationArgument, apply_implementation, apply_reference_consumer_implementation,
    implementation_name,
};
use crate::resources::ResourceContext;
use crate::{
    ArgumentErrorContext, ArgumentErrorReason, ConstantRecord, Edge, Error, ErrorKind, Feature,
    NodeIndex, NodeKind, OriginIndex, OwnershipMode, ReleaseAfter, ResourceObserver,
    ScalarConstant, ScalarType, SourceLocation, SourceSpan, TypeRecord, Value, ValueAccess,
    VerifiedProgram,
};

enum Slot<'a> {
    Borrowed(&'a Value),
    Alias(NodeIndex),
    Owned { value: Value, accounted: bool },
}

struct Interpreter<'a> {
    raw: &'a crate::RawProgram,
    parameters: &'a [Value],
    slots: Vec<Option<Slot<'a>>>,
    branch_nodes: Vec<bool>,
    releases: Vec<Vec<usize>>,
    resources: ResourceContext,
}

pub fn evaluate_verified_program(
    program: &VerifiedProgram,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
) -> Result<ProgramResult, Error> {
    evaluate_verified_program_observed(program, arguments, configuration, None)
}

pub fn evaluate_verified_program_with_observer(
    program: &VerifiedProgram,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: ResourceObserver,
) -> Result<ProgramResult, Error> {
    evaluate_verified_program_observed(program, arguments, configuration, Some(observer))
}

fn evaluate_verified_program_observed(
    program: &VerifiedProgram,
    arguments: &[Value],
    configuration: EvaluationConfiguration,
    observer: Option<ResourceObserver>,
) -> Result<ProgramResult, Error> {
    let raw = program.as_raw();
    let mut resources = ResourceContext::new_with_observer(
        configuration.profile,
        configuration.limits,
        configuration.allocation_failure,
        observer,
    )?;
    validate_profile(raw, &mut resources)?;
    validate_arguments(raw, arguments)?;

    let mut slots = Vec::new();
    slots
        .try_reserve_exact(raw.nodes.len())
        .map_err(|_| execution_allocation_error(SourceLocation::start(), "program"))?;
    slots.resize_with(raw.nodes.len(), || None);

    let mut branch_nodes = Vec::new();
    branch_nodes
        .try_reserve_exact(raw.nodes.len())
        .map_err(|_| execution_allocation_error(SourceLocation::start(), "program"))?;
    branch_nodes.resize(raw.nodes.len(), false);
    for branch in &raw.branches {
        for index in range_indices(branch.nodes) {
            if let Some(marker) = branch_nodes.get_mut(index) {
                *marker = true;
            }
        }
    }

    let mut releases = Vec::new();
    releases
        .try_reserve_exact(raw.nodes.len())
        .map_err(|_| execution_allocation_error(SourceLocation::start(), "program"))?;
    releases.resize_with(raw.nodes.len(), Vec::new);
    for ownership in &raw.ownership {
        if let ReleaseAfter::Node(node) = ownership.release_after {
            let sink = node.0 as usize;
            let owner = ownership.owner.0 as usize;
            let Some(at_sink) = releases.get_mut(sink) else {
                return Err(execution_invariant_error(SourceLocation::start()));
            };
            at_sink
                .try_reserve(1)
                .map_err(|_| execution_allocation_error(SourceLocation::start(), "program"))?;
            at_sink.push(owner);
        }
    }

    let mut interpreter = Interpreter {
        raw,
        parameters: arguments,
        slots,
        branch_nodes,
        releases,
        resources,
    };
    let execution = interpreter.execute();
    match execution {
        Ok(values) => Ok(ProgramResult {
            values,
            usage: interpreter.resources.usage,
        }),
        Err(mut error) => {
            interpreter.cleanup_all();
            error.usage = Some(interpreter.resources.usage);
            Err(error)
        }
    }
}

impl<'a> Interpreter<'a> {
    fn execute(&mut self) -> Result<Vec<Value>, Error> {
        for index in 0..self.raw.nodes.len() {
            if !self.branch_nodes[index] {
                self.execute_node(index)?;
            }
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.raw.roots.len())
            .map_err(|_| execution_allocation_error(SourceLocation::start(), "program"))?;
        for root in &self.raw.roots {
            values.push(self.take_output(root.node)?);
        }
        Ok(values)
    }

    fn execute_node(&mut self, index: usize) -> Result<(), Error> {
        let node = *self
            .raw
            .nodes
            .get(index)
            .ok_or_else(|| execution_invariant_error(SourceLocation::start()))?;
        let location = self.origin_location(node.origin)?;
        match node.kind {
            NodeKind::Constant { constant } => {
                let (value, accounted) = self.build_constant(constant.0 as usize, location)?;
                self.put_owned(index, value, accounted)
            }
            NodeKind::ParameterBorrow { parameter } => {
                let value = self
                    .parameters
                    .get(parameter.0 as usize)
                    .ok_or_else(|| execution_invariant_error(location))?;
                self.put_slot(index, Slot::Borrowed(value))
            }
            NodeKind::TupleConstruct => self.execute_tuple(index, node, location),
            NodeKind::PrefixSpreadPrepare => self.execute_spread_prepare(index, node, location),
            NodeKind::SelectedApply {
                implementation_id,
                application_plan_id,
                operation_reference,
                lift,
                result_element_type,
                shape,
                ..
            } => self.execute_apply(
                index,
                node,
                implementation_id,
                application_plan_id,
                operation_reference,
                lift,
                result_element_type,
                shape,
                location,
            ),
            NodeKind::FanOut { branches, .. } => {
                self.execute_fan_out(index, node, branches, location)
            }
        }
    }

    fn execute_tuple(
        &mut self,
        index: usize,
        node: crate::Node,
        location: SourceLocation,
    ) -> Result<(), Error> {
        let edges = self.copy_edges(node.edges, location)?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(edges.len())
            .map_err(|_| execution_allocation_error(location, "tuple_literal"))?;
        self.resources
            .admit_tuple(edges.len(), location, "tuple_literal")?;
        for edge in &edges {
            let value = match edge.ownership {
                OwnershipMode::InfallibleTransfer => self.take_owned(edge.producer, location)?,
                OwnershipMode::ImmutableBorrow => self.edge_value(*edge)?.clone(),
                OwnershipMode::OwnedInput => return Err(execution_invariant_error(location)),
            };
            values.push(value);
        }
        self.put_owned(index, Value::Tuple(values.into()), !edges.is_empty())?;
        self.release_after(index);
        Ok(())
    }

    fn execute_spread_prepare(
        &mut self,
        index: usize,
        node: crate::Node,
        location: SourceLocation,
    ) -> Result<(), Error> {
        let edge = *self
            .edges(node.edges)?
            .first()
            .ok_or_else(|| execution_invariant_error(location))?;
        let slot = match edge.ownership {
            OwnershipMode::InfallibleTransfer => {
                let producer = edge.producer.0 as usize;
                self.slots
                    .get_mut(producer)
                    .and_then(Option::take)
                    .ok_or_else(|| execution_invariant_error(location))?
            }
            OwnershipMode::ImmutableBorrow => Slot::Alias(edge.producer),
            OwnershipMode::OwnedInput => return Err(execution_invariant_error(location)),
        };
        self.put_slot(index, slot)?;
        self.release_after(index);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn execute_apply(
        &mut self,
        index: usize,
        node: crate::Node,
        implementation_id: u16,
        application_plan_id: u16,
        operation_reference: Option<crate::OperationReferenceIndex>,
        lift: crate::LiftMode,
        result_type: ScalarType,
        shape: crate::ShapePlan,
        location: SourceLocation,
    ) -> Result<(), Error> {
        self.validate_shape(node.edges, shape, implementation_id)?;
        let edges = self.copy_edges(node.edges, location)?;
        let mut arguments = Vec::new();
        arguments
            .try_reserve_exact(edges.len())
            .map_err(|_| execution_allocation_error(location, "application"))?;
        for edge in &edges {
            arguments.push(SelectedApplicationArgument {
                value: edge_value(self.raw, &self.slots, *edge)?,
                conversion: edge.conversion,
            });
        }
        let applied = if let Some(reference_index) = operation_reference {
            let reference = *self
                .raw
                .operation_references
                .get(reference_index.0 as usize)
                .ok_or_else(|| execution_invariant_error(location))?;
            let reference_location = self.origin_location(reference.origin)?;
            apply_reference_consumer_implementation(
                implementation_id,
                application_plan_id,
                &reference,
                &arguments,
                lift,
                result_type,
                location,
                reference_location,
                &mut self.resources,
            )
        } else {
            apply_implementation(
                implementation_id,
                application_plan_id,
                &arguments,
                lift,
                result_type,
                location,
                &mut self.resources,
            )
        };
        drop(arguments);
        let (value, accounted) = applied?;
        self.put_owned(index, value, accounted)?;
        self.release_after(index);
        Ok(())
    }

    fn execute_fan_out(
        &mut self,
        index: usize,
        node: crate::Node,
        branches: crate::IndexRange,
        location: SourceLocation,
    ) -> Result<(), Error> {
        let branch_records = self.copy_branches(branches, location)?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(branch_records.len())
            .map_err(|_| execution_allocation_error(location, "fanout"))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(branch_records.len())
            .map_err(|_| execution_allocation_error(location, "fanout"))?;
        for branch in &branch_records {
            for branch_node in range_indices(branch.nodes) {
                self.execute_node(branch_node)?;
            }
            roots.push(branch.root);
        }
        self.resources
            .admit_tuple(branch_records.len(), location, "fanout")?;
        for root in roots {
            values.push(self.take_owned(root, location)?);
        }
        self.put_owned(index, Value::Tuple(values.into()), true)?;
        self.release_after(index);
        let _ = node;
        Ok(())
    }

    fn validate_shape(
        &self,
        edges_range: crate::IndexRange,
        shape: crate::ShapePlan,
        implementation_id: u16,
    ) -> Result<(), Error> {
        let edges = self.edges(edges_range)?;
        let anchor = shape
            .static_anchor
            .map(|position| position as usize)
            .or_else(|| {
                edges
                    .iter()
                    .position(|edge| self.edge_value(*edge).is_ok_and(Value::is_vector))
            });
        let Some(anchor) = anchor else {
            return Ok(());
        };
        let expected = self
            .edge_value(
                *edges
                    .get(anchor)
                    .ok_or_else(|| execution_invariant_error(SourceLocation::start()))?,
            )?
            .len();
        let name = implementation_name(implementation_id).unwrap_or("application");
        for edge_index in range_indices(shape.dynamic_checks) {
            let edge = *self
                .raw
                .shape_checks
                .get(edge_index)
                .and_then(|edge| self.raw.edges.get(*edge as usize))
                .ok_or_else(|| execution_invariant_error(SourceLocation::start()))?;
            let actual = self.edge_value(edge)?.len();
            if actual != expected {
                let location = self.origin_location(edge.origin)?;
                let position = edge.argument_position as usize;
                let mut error = Error::new(
                    ErrorKind::ShapeMismatch,
                    location,
                    format!(
                        "{name} argument {position} expected shape [{expected}], got [{actual}]"
                    ),
                );
                error.primitive = Some(name.to_owned());
                error.argument_position = Some(position);
                error.expected_shape = Some(vec![expected]);
                error.actual_shape = Some(vec![actual]);
                return Err(error);
            }
        }
        Ok(())
    }

    fn build_constant(
        &mut self,
        index: usize,
        location: SourceLocation,
    ) -> Result<(Value, bool), Error> {
        match *self
            .raw
            .constants
            .get(index)
            .ok_or_else(|| execution_invariant_error(location))?
        {
            ConstantRecord::Scalar(value) => Ok((scalar_value(value), false)),
            ConstantRecord::Vector {
                element_type,
                elements,
            } => {
                let count = elements.count as usize;
                let admitted =
                    self.resources
                        .admit_vector(element_type, count, location, "vector_literal")?;
                let values = &self.raw.constant_elements
                    [elements.start as usize..elements.start as usize + count];
                match build_vector(element_type, values) {
                    Ok(value) => Ok((value, count != 0)),
                    Err(error) => {
                        self.resources.refund(admitted);
                        Err(error)
                    }
                }
            }
        }
    }

    fn edge_value(&self, edge: Edge) -> Result<&Value, Error> {
        edge_value(self.raw, &self.slots, edge)
    }

    fn take_owned(
        &mut self,
        producer: NodeIndex,
        location: SourceLocation,
    ) -> Result<Value, Error> {
        let index = producer.0 as usize;
        match self.slots.get_mut(index).and_then(Option::take) {
            Some(Slot::Owned { value, .. }) => Ok(value),
            Some(other) => {
                self.slots[index] = Some(other);
                Err(execution_invariant_error(location))
            }
            None => Err(execution_invariant_error(location)),
        }
    }

    fn take_output(&mut self, producer: NodeIndex) -> Result<Value, Error> {
        let mut index = producer.0 as usize;
        for _ in 0..=self.slots.len() {
            match self.slots.get(index).and_then(Option::as_ref) {
                Some(Slot::Alias(next)) => index = next.0 as usize,
                Some(Slot::Borrowed(value)) => return Ok((*value).clone()),
                Some(Slot::Owned { .. }) => {
                    return match self.slots.get_mut(index).and_then(Option::take) {
                        Some(Slot::Owned { value, .. }) => Ok(value),
                        _ => Err(execution_invariant_error(SourceLocation::start())),
                    };
                }
                None => return Err(execution_invariant_error(SourceLocation::start())),
            }
        }
        Err(execution_invariant_error(SourceLocation::start()))
    }

    fn put_owned(&mut self, index: usize, value: Value, accounted: bool) -> Result<(), Error> {
        self.put_slot(index, Slot::Owned { value, accounted })
    }

    fn put_slot(&mut self, index: usize, slot: Slot<'a>) -> Result<(), Error> {
        let destination = self
            .slots
            .get_mut(index)
            .ok_or_else(|| execution_invariant_error(SourceLocation::start()))?;
        if destination.is_some() {
            return Err(execution_invariant_error(SourceLocation::start()));
        }
        *destination = Some(slot);
        Ok(())
    }

    fn release_after(&mut self, sink: usize) {
        for owner in self.releases[sink].iter().rev().copied() {
            if let Some(Slot::Owned {
                value,
                accounted: true,
            }) = self.slots.get_mut(owner).and_then(Option::take)
            {
                self.resources.release(&value);
            } else if matches!(
                self.slots.get(owner).and_then(Option::as_ref),
                Some(Slot::Owned {
                    accounted: false,
                    ..
                })
            ) {
                self.slots[owner] = None;
            }
        }
    }

    fn cleanup_all(&mut self) {
        for slot in self.slots.iter_mut().rev() {
            if let Some(Slot::Owned {
                value,
                accounted: true,
            }) = slot.take()
            {
                self.resources.release(&value);
            }
        }
    }

    fn edges(&self, range: crate::IndexRange) -> Result<&[Edge], Error> {
        self.raw
            .edges
            .get(range.start as usize..range.start as usize + range.count as usize)
            .ok_or_else(|| execution_invariant_error(SourceLocation::start()))
    }

    fn branches(&self, range: crate::IndexRange) -> Result<&[crate::FanOutBranch], Error> {
        self.raw
            .branches
            .get(range.start as usize..range.start as usize + range.count as usize)
            .ok_or_else(|| execution_invariant_error(SourceLocation::start()))
    }

    fn copy_edges(
        &self,
        range: crate::IndexRange,
        location: SourceLocation,
    ) -> Result<Vec<Edge>, Error> {
        let source = self.edges(range)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(source.len())
            .map_err(|_| execution_allocation_error(location, "application"))?;
        result.extend_from_slice(source);
        Ok(result)
    }

    fn copy_branches(
        &self,
        range: crate::IndexRange,
        location: SourceLocation,
    ) -> Result<Vec<crate::FanOutBranch>, Error> {
        let source = self.branches(range)?;
        let mut result = Vec::new();
        result
            .try_reserve_exact(source.len())
            .map_err(|_| execution_allocation_error(location, "fanout"))?;
        result.extend_from_slice(source);
        Ok(result)
    }

    fn origin_location(&self, origin: OriginIndex) -> Result<SourceLocation, Error> {
        self.raw
            .origins
            .get(origin.0 as usize)
            .map(|origin| SourceLocation {
                offset: origin.span.begin.offset as usize,
                line: origin.span.begin.line as usize,
                column: origin.span.begin.column as usize,
            })
            .ok_or_else(|| execution_invariant_error(SourceLocation::start()))
    }
}

fn slot_value<'slots, 'values>(
    slots: &'slots [Option<Slot<'values>>],
    producer: NodeIndex,
) -> Result<&'slots Value, Error>
where
    'values: 'slots,
{
    let mut index = producer.0 as usize;
    for _ in 0..=slots.len() {
        match slots.get(index).and_then(Option::as_ref) {
            Some(Slot::Borrowed(value)) => return Ok(value),
            Some(Slot::Owned { value, .. }) => return Ok(value),
            Some(Slot::Alias(next)) => index = next.0 as usize,
            None => return Err(execution_invariant_error(SourceLocation::start())),
        }
    }
    Err(execution_invariant_error(SourceLocation::start()))
}

fn edge_value<'slots, 'values>(
    raw: &crate::RawProgram,
    slots: &'slots [Option<Slot<'values>>],
    edge: Edge,
) -> Result<&'slots Value, Error>
where
    'values: 'slots,
{
    let value = slot_value(slots, edge.producer)?;
    match edge.access {
        ValueAccess::WholeValue | ValueAccess::FanOutOperandBorrow => Ok(value),
        ValueAccess::TupleElement(element) => {
            let Value::Tuple(values) = value else {
                return Err(execution_invariant_error(origin_location(raw, edge.origin)));
            };
            values
                .get(element as usize)
                .ok_or_else(|| execution_invariant_error(origin_location(raw, edge.origin)))
        }
    }
}

fn origin_location(raw: &crate::RawProgram, origin: OriginIndex) -> SourceLocation {
    raw.origins
        .get(origin.0 as usize)
        .map(|origin| SourceLocation {
            offset: origin.span.begin.offset as usize,
            line: origin.span.begin.line as usize,
            column: origin.span.begin.column as usize,
        })
        .unwrap_or_else(SourceLocation::start)
}

fn range_indices(range: crate::IndexRange) -> std::ops::Range<usize> {
    range.start as usize..range.start as usize + range.count as usize
}

fn scalar_value(value: ScalarConstant) -> Value {
    match value {
        ScalarConstant::Bool(value) => Value::Bool(value),
        ScalarConstant::Int(value) => Value::Int(value),
        ScalarConstant::DoubleBits(value) => Value::Double(f64::from_bits(value)),
    }
}

fn build_vector(element_type: ScalarType, values: &[ScalarConstant]) -> Result<Value, Error> {
    match element_type {
        ScalarType::Bool => {
            let mut result = Vec::new();
            result.try_reserve_exact(values.len()).map_err(|_| {
                execution_allocation_error(SourceLocation::start(), "vector_literal")
            })?;
            for value in values {
                let ScalarConstant::Bool(value) = value else {
                    return Err(execution_invariant_error(SourceLocation::start()));
                };
                result.push(*value);
            }
            Ok(Value::BoolVector(result))
        }
        ScalarType::Int => {
            let mut result = Vec::new();
            result.try_reserve_exact(values.len()).map_err(|_| {
                execution_allocation_error(SourceLocation::start(), "vector_literal")
            })?;
            for value in values {
                let ScalarConstant::Int(value) = value else {
                    return Err(execution_invariant_error(SourceLocation::start()));
                };
                result.push(*value);
            }
            Ok(Value::IntVector(result))
        }
        ScalarType::Double => {
            let mut result = Vec::new();
            result.try_reserve_exact(values.len()).map_err(|_| {
                execution_allocation_error(SourceLocation::start(), "vector_literal")
            })?;
            for value in values {
                let ScalarConstant::DoubleBits(value) = value else {
                    return Err(execution_invariant_error(SourceLocation::start()));
                };
                result.push(f64::from_bits(*value));
            }
            Ok(Value::DoubleVector(result))
        }
    }
}

fn validate_profile(raw: &crate::RawProgram, resources: &mut ResourceContext) -> Result<(), Error> {
    if !raw.features.contains(&Feature::Tuples.numeric()) {
        return Ok(());
    }
    let location = raw
        .nodes
        .iter()
        .filter(|node| {
            raw.types
                .get(node.result_type.0 as usize)
                .is_some_and(|record| matches!(record, TypeRecord::Tuple { .. }))
        })
        .filter_map(|node| raw.origins.get(node.origin.0 as usize))
        .min_by_key(|origin| origin.span.begin.offset)
        .map(|origin| SourceLocation {
            offset: origin.span.begin.offset as usize,
            line: origin.span.begin.line as usize,
            column: origin.span.begin.column as usize,
        })
        .unwrap_or_else(SourceLocation::start);
    resources.require_tuple_profile(location)
}

fn validate_arguments(raw: &crate::RawProgram, arguments: &[Value]) -> Result<(), Error> {
    if arguments.len() != raw.parameters.len() {
        let reason = if arguments.len() < raw.parameters.len() {
            ArgumentErrorReason::Missing
        } else {
            ArgumentErrorReason::Extra
        };
        return Err(argument_error(
            raw,
            reason,
            arguments.len(),
            arguments.len().min(raw.parameters.len()) + 1,
        ));
    }
    for (index, (parameter, argument)) in raw.parameters.iter().zip(arguments).enumerate() {
        if contains_noncanonical_nan(argument)? {
            let mut error = argument_error(
                raw,
                ArgumentErrorReason::InvalidTypedValue,
                arguments.len(),
                index + 1,
            );
            if let Some(context) = &mut error.argument {
                context.actual_container = Some(value_container(argument));
                context.actual_type = argument.scalar_type();
                context.invalid_value_invariant = Some("noncanonical_nan");
            }
            return Err(error);
        }
        if !argument.is_scalar() {
            let mut error = argument_error(
                raw,
                ArgumentErrorReason::ContainerMismatch,
                arguments.len(),
                index + 1,
            );
            if let Some(context) = &mut error.argument {
                context.actual_container = Some(value_container(argument));
            }
            return Err(error);
        }
        if argument.scalar_type() != Some(parameter.scalar_type) {
            let mut error = argument_error(
                raw,
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

pub(crate) fn decode_verified_arguments(
    program: &VerifiedProgram,
    arguments: &[&str],
) -> Result<Vec<Value>, Error> {
    let raw = program.as_raw();
    if arguments.len() != raw.parameters.len() {
        let reason = if arguments.len() < raw.parameters.len() {
            ArgumentErrorReason::Missing
        } else {
            ArgumentErrorReason::Extra
        };
        return Err(argument_error(
            raw,
            reason,
            arguments.len(),
            arguments.len().min(raw.parameters.len()) + 1,
        ));
    }
    let mut decoded = Vec::new();
    decoded
        .try_reserve_exact(raw.parameters.len())
        .map_err(|_| execution_allocation_error(SourceLocation::start(), "argument decoding"))?;
    for (index, (parameter, spelling)) in raw.parameters.iter().zip(arguments).enumerate() {
        let value = match parameter.scalar_type {
            ScalarType::Bool => match *spelling {
                "true" => Ok(Value::Bool(true)),
                "false" => Ok(Value::Bool(false)),
                _ => Err(ArgumentErrorReason::InvalidLiteral),
            },
            ScalarType::Int => decode_int(spelling).map(Value::Int),
            ScalarType::Double => decode_double(spelling).map(Value::Double),
        };
        match value {
            Ok(value) => decoded.push(value),
            Err(reason) => {
                return Err(argument_error(raw, reason, arguments.len(), index + 1));
            }
        }
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

fn contains_noncanonical_nan(value: &Value) -> Result<bool, Error> {
    let mut pending = Vec::new();
    pending
        .try_reserve(1)
        .map_err(|_| execution_allocation_error(SourceLocation::start(), "argument validation"))?;
    pending.push(value);
    while let Some(value) = pending.pop() {
        match value {
            Value::Double(value) if value.is_nan() && value.to_bits() != 0x7ff8_0000_0000_0000 => {
                return Ok(true);
            }
            Value::DoubleVector(values)
                if values
                    .iter()
                    .any(|value| value.is_nan() && value.to_bits() != 0x7ff8_0000_0000_0000) =>
            {
                return Ok(true);
            }
            Value::Tuple(values) => {
                pending.try_reserve(values.len()).map_err(|_| {
                    execution_allocation_error(SourceLocation::start(), "argument validation")
                })?;
                pending.extend(values.iter());
            }
            _ => {}
        }
    }
    Ok(false)
}

fn value_container(value: &Value) -> &'static str {
    if value.is_scalar() {
        "scalar"
    } else if value.is_vector() {
        "vector"
    } else {
        "tuple"
    }
}

fn argument_error(
    raw: &crate::RawProgram,
    reason: ArgumentErrorReason,
    supplied_count: usize,
    position: usize,
) -> Error {
    let parameter = raw.parameters.get(position.saturating_sub(1));
    let span = parameter.and_then(|parameter| {
        raw.origins
            .get(parameter.declaration_origin.0 as usize)
            .map(origin_span)
    });
    let context = ArgumentErrorContext {
        reason,
        required_count: raw.parameters.len(),
        supplied_count,
        position,
        parameter_name: parameter.map(|parameter| parameter.name.clone()),
        expected_type: parameter.map(|parameter| parameter.scalar_type),
        declaration_span: span,
        actual_container: None,
        actual_type: None,
        invalid_value_invariant: None,
    };
    let location = span.map_or_else(SourceLocation::start, |span| span.begin);
    let mut error = Error::new(ErrorKind::ArgumentError, location, reason.name());
    error.argument = Some(context);
    error
}

fn origin_span(origin: &crate::Origin) -> SourceSpan {
    SourceSpan {
        begin: SourceLocation {
            offset: origin.span.begin.offset as usize,
            line: origin.span.begin.line as usize,
            column: origin.span.begin.column as usize,
        },
        end: SourceLocation {
            offset: origin.span.end.offset as usize,
            line: origin.span.end.line as usize,
            column: origin.span.end.column as usize,
        },
    }
}

fn execution_allocation_error(location: SourceLocation, producer: &str) -> Error {
    Error::new(
        ErrorKind::ResourceError,
        location,
        format!("{producer} failed: allocation_unavailable"),
    )
}

fn execution_invariant_error(location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::TypeError,
        location,
        "verified program execution invariant failed",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        AllocationFailureInjection, ExecutionProfile, ResourceErrorReason, ResourceEvent,
        ResourceEventKind, ResourceLimits, ResourceUsage, evaluate_source_with_arguments,
        evaluate_source_with_arguments_and_observer,
    };
    use std::cell::RefCell;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct OwnedEvent {
        kind: ResourceEventKind,
        producer: String,
        requested_elements: Option<usize>,
        requested_bytes: Option<usize>,
        requested_work_units: usize,
        allocation_ordinal: Option<usize>,
        refusal_reason: Option<ResourceErrorReason>,
        usage: ResourceUsage,
    }

    thread_local! {
        static EVENTS: RefCell<Vec<OwnedEvent>> = const { RefCell::new(Vec::new()) };
    }

    fn observe(event: &ResourceEvent<'_>) {
        EVENTS.with(|events| {
            events.borrow_mut().push(OwnedEvent {
                kind: event.kind,
                producer: event.producer.to_owned(),
                requested_elements: event.requested_elements,
                requested_bytes: event.requested_bytes,
                requested_work_units: event.requested_work_units,
                allocation_ordinal: event.allocation_ordinal,
                refusal_reason: event.refusal_reason,
                usage: event.usage,
            });
        });
    }

    fn take_events() -> Vec<OwnedEvent> {
        EVENTS.with(|events| std::mem::take(&mut *events.borrow_mut()))
    }

    fn compile(source: &str) -> VerifiedProgram {
        match crate::lowering::compile_source_with_name(source, "<source>") {
            Ok(program) => program,
            Err(error) => panic!("{source:?} did not lower: {error}"),
        }
    }

    fn assert_public_route_matches_direct_ir(
        source: &str,
        arguments: &[Value],
        configuration: EvaluationConfiguration,
    ) {
        let public = evaluate_source_with_arguments(source, arguments, configuration);
        let verified = compile(source);
        let interpreted = evaluate_verified_program(&verified, arguments, configuration);
        assert_eq!(interpreted, public, "{source}");
    }

    #[test]
    fn selected_ir_matches_values_errors_tuples_spread_fanout_and_parameters() {
        for (source, arguments) in [
            ("", vec![]),
            ("inc[1]\n", vec![]),
            ("add[1 2.5]\n", vec![]),
            ("add[(1 2 3) 10]\n", vec![]),
            ("div[-7 3]\n", vec![]),
            ("div[(8 9 10) (2 0 5)]\n", vec![]),
            ("length[(true false true)]\n", vec![]),
            ("length iota 4\n", vec![]),
            ("sort[(3 1 2 1)]\n", vec![]),
            ("sort[(0.0 -0.0 -inf inf)]\n", vec![]),
            ("sum[(1 2 3)]\n", vec![]),
            ("sum[(1.5 -0.5 2.0)]\n", vec![]),
            ("all_of[Bool()]\n", vec![]),
            ("all_of[(true false true)]\n", vec![]),
            ("any_of[Bool()]\n", vec![]),
            ("any_of[(false true false)]\n", vec![]),
            ("none_of[Bool()]\n", vec![]),
            ("none_of[(false true false)]\n", vec![]),
            ("foldl[@sub 20 (3 4 5)]\n", vec![]),
            ("foldl[@add 1 (2.5 3.5)]\n", vec![]),
            ("scanl[@sub 20 (3 4 5)]\n", vec![]),
            ("scanl[@add 1 (2.5 3.5)]\n", vec![]),
            ("filter[@not (true false)]\n", vec![]),
            ("filter[@odd (1 2 3)]\n", vec![]),
            ("filter[@is_positive (-1.0 0.0 2.0)]\n", vec![]),
            ("add [1 2]\n", vec![]),
            ("[1 (2 3) true]\n", vec![]),
            ("fanout[iota[3] {inc[_]} {add[_ 10]}]\n", vec![]),
            (
                "parameters[x Int y Double]\nadd[x y]\nfanout[iota[x] {mul[_ 2]}]\n",
                vec![Value::Int(3), Value::Double(0.5)],
            ),
            ("inc[9223372036854775807]\n", vec![]),
            (
                "parameters[x Int y Int]\nadd[iota[x] iota[y]]\n",
                vec![Value::Int(2), Value::Int(3)],
            ),
        ] {
            assert_public_route_matches_direct_ir(
                source,
                &arguments,
                EvaluationConfiguration::default(),
            );
        }
    }

    #[test]
    fn every_selected_implementation_executes_by_stable_id() {
        for source in [
            "inc[1]\n",
            "inc[1.5]\n",
            "dec[1]\n",
            "dec[1.5]\n",
            "neg[1]\n",
            "neg[1.5]\n",
            "abs[-1]\n",
            "abs[-1.5]\n",
            "add[1 2]\n",
            "add[1.0 2.0]\n",
            "sub[2 1]\n",
            "sub[2.0 1.0]\n",
            "mul[2 3]\n",
            "mul[2.0 3.0]\n",
            "div[7 3]\n",
            "div[7.0 3.0]\n",
            "length[(true false)]\n",
            "length[(1 2)]\n",
            "length[(1.0 2.0)]\n",
            "sort[(true false)]\n",
            "sort[(2 1)]\n",
            "sort[(2.0 1.0)]\n",
            "sum[(1 2)]\n",
            "sum[(1.0 2.0)]\n",
            "all_of[(true false)]\n",
            "any_of[(true false)]\n",
            "none_of[(true false)]\n",
            "foldl[@and true (true false)]\n",
            "foldl[@sub 10 (1 2)]\n",
            "foldl[@add 1 (2.0 3.0)]\n",
            "scanl[@and true (true false)]\n",
            "scanl[@sub 10 (1 2)]\n",
            "scanl[@add 1 (2.0 3.0)]\n",
            "filter[@not (true false)]\n",
            "filter[@odd (1 2)]\n",
            "filter[@is_positive (-1.0 2.0)]\n",
            "equals[true false]\n",
            "equals[1 2]\n",
            "equals[1.0 2.0]\n",
            "not_equals[true false]\n",
            "not_equals[1 2]\n",
            "not_equals[1.0 2.0]\n",
            "not[true]\n",
            "and[true false]\n",
            "or[true false]\n",
            "odd[3]\n",
            "even[4]\n",
            "is_positive[1]\n",
            "is_positive[1.0]\n",
            "is_negative[-1]\n",
            "is_negative[-1.0]\n",
            "less_than[1 2]\n",
            "less_than[1.0 2.0]\n",
            "greater_than[2 1]\n",
            "greater_than[2.0 1.0]\n",
            "iota[3]\n",
        ] {
            assert_public_route_matches_direct_ir(source, &[], EvaluationConfiguration::default());
        }
    }

    #[test]
    fn typed_argument_validation_matches_before_execution() {
        let source = "parameters[x Double]\ninc[x]\n";
        for arguments in [
            vec![],
            vec![Value::Int(1)],
            vec![Value::DoubleVector(vec![1.0])],
            vec![Value::Double(f64::from_bits(0x7ff8_0000_0000_0001))],
            vec![Value::Double(1.5), Value::Double(2.5)],
        ] {
            assert_public_route_matches_direct_ir(
                source,
                &arguments,
                EvaluationConfiguration::default(),
            );
        }

        assert_public_route_matches_direct_ir(
            "parameters[x Int]\n[x]\n",
            &[],
            EvaluationConfiguration {
                profile: ExecutionProfile::TrustedLocalV1,
                ..EvaluationConfiguration::default()
            },
        );
    }

    #[test]
    fn public_observer_and_fault_ordinals_match_direct_ir() {
        let source = "fanout[iota[3] {inc[_]} {add[_ (10 20 30)]}]\nadd[(1 2 3) (4 5 6)]\n";
        for fail_at_ordinal in [None, Some(0), Some(1), Some(2), Some(3), Some(4)] {
            let configuration = EvaluationConfiguration {
                profile: ExecutionProfile::BoundedV2,
                limits: ResourceLimits {
                    max_vector_bytes: Some(128),
                    max_tuple_table_bytes: Some(128),
                    max_live_evaluation_bytes: Some(512),
                    max_work_units: Some(512),
                },
                allocation_failure: AllocationFailureInjection { fail_at_ordinal },
            };
            take_events();
            let public =
                evaluate_source_with_arguments_and_observer(source, &[], configuration, observe);
            let public_events = take_events();
            let verified = compile(source);
            let interpreted =
                evaluate_verified_program_with_observer(&verified, &[], configuration, observe);
            let interpreted_events = take_events();
            assert_eq!(interpreted, public, "fault {fail_at_ordinal:?}");
            assert_eq!(
                interpreted_events, public_events,
                "fault {fail_at_ordinal:?}"
            );
        }
    }

    #[test]
    fn public_resource_limit_winners_and_cleanup_usage_match_direct_ir() {
        let source = "[iota[3] iota[2]]\n";
        for limits in [
            ResourceLimits {
                max_vector_bytes: Some(16),
                max_tuple_table_bytes: Some(64),
                max_live_evaluation_bytes: Some(128),
                max_work_units: Some(128),
            },
            ResourceLimits {
                max_vector_bytes: Some(128),
                max_tuple_table_bytes: Some(16),
                max_live_evaluation_bytes: Some(128),
                max_work_units: Some(128),
            },
            ResourceLimits {
                max_vector_bytes: Some(128),
                max_tuple_table_bytes: Some(64),
                max_live_evaluation_bytes: Some(32),
                max_work_units: Some(128),
            },
            ResourceLimits {
                max_vector_bytes: Some(128),
                max_tuple_table_bytes: Some(64),
                max_live_evaluation_bytes: Some(128),
                max_work_units: Some(4),
            },
        ] {
            assert_public_route_matches_direct_ir(
                source,
                &[],
                EvaluationConfiguration {
                    profile: ExecutionProfile::BoundedV2,
                    limits,
                    allocation_failure: AllocationFailureInjection::default(),
                },
            );
        }
    }

    #[test]
    fn deep_flat_program_executes_without_recursive_evaluation() {
        let depth = 20_000usize;
        let mut source = String::new();
        for _ in 0..depth {
            source.push_str("inc ");
        }
        source.push('1');
        source.push('\n');
        let verified = compile(&source);
        let interpreted =
            evaluate_verified_program(&verified, &[], EvaluationConfiguration::default());
        let result = match interpreted {
            Ok(result) => result,
            Err(error) => panic!("deep IR execution failed: {error:?}"),
        };
        assert_eq!(result.values, [Value::Int(depth as i64 + 1)]);
        assert_eq!(result.usage.work_units, depth);
    }

    #[test]
    fn deep_tuple_execution_and_cleanup_are_iterative() {
        let depth = 4_096usize;
        let source = format!("{}7{}\n", "[".repeat(depth), "]".repeat(depth));
        let verified = compile(&source);
        let interpreted =
            evaluate_verified_program(&verified, &[], EvaluationConfiguration::default());
        let result = match interpreted {
            Ok(result) => result,
            Err(error) => panic!("deep tuple IR execution failed: {error:?}"),
        };
        assert_eq!(result.usage.allocation_attempts, depth);
        assert_eq!(result.usage.live_evaluation_bytes, depth * 16);
        drop(result);

        let configuration = EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits: ResourceLimits {
                max_vector_bytes: None,
                max_tuple_table_bytes: Some(16),
                max_live_evaluation_bytes: Some((depth - 1) * 16),
                max_work_units: None,
            },
            allocation_failure: AllocationFailureInjection::default(),
        };
        assert_public_route_matches_direct_ir(&source, &[], configuration);
    }
}
