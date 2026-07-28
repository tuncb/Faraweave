use crate::parser::parse;
use crate::parser::{CallSyntax, Expr, ExprKind, Program, validate_parameter_declarations};
use crate::primitive::resolve_names;
use crate::semantic_registry::{
    Conversion as RegistryConversion, SemanticDescriptor, StructuralBehavior, conversion,
    descriptors, is_backend_native_math_primitive, primitive_from_name,
};
use crate::typed_program::{
    BuildError, Cardinality, ConstantRecord, Edge, FanOutBranch, Feature, IndexRange, LiftMode,
    Node, NodeIndex, NodeKind, Origin, OriginIndex, OriginPosition, OriginSpan, Ownership,
    OwnershipMode, Parameter, RawProgramBuilder, ReleaseAfter, Root, RootIndex, ScalarConstant,
    ShapePlan, SourceUnit, TypeIndex, TypeRecord, ValueAccess, VerifiedProgram, VerifyError,
};
use crate::{Error, ErrorKind, ScalarType, SourceLocation, SourceSpan, Type, Value};
use std::fmt::Write as _;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub(crate) enum CompileError {
    Source(Error),
    Build(BuildError),
    Verify(VerifyError),
}

type LowerError = CompileError;

impl std::fmt::Display for CompileError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Source(error) => error.fmt(formatter),
            Self::Build(error) => error.fmt(formatter),
            Self::Verify(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileError {}

impl CompileError {
    pub(crate) fn into_evaluation_error(self) -> Error {
        match self {
            Self::Source(error) => error,
            Self::Build(BuildError::CountOverflow { .. }) => Error::new(
                ErrorKind::ResourceError,
                SourceLocation::start(),
                "source compilation failed: size_overflow",
            ),
            Self::Build(BuildError::AllocationUnavailable { .. })
            | Self::Verify(VerifyError::AllocationUnavailable { .. }) => Error::new(
                ErrorKind::ResourceError,
                SourceLocation::start(),
                "source compilation failed: allocation_unavailable",
            ),
            Self::Verify(VerifyError::MalformedProgram(_)) => Error::new(
                ErrorKind::TypeError,
                SourceLocation::start(),
                "source compilation produced an invalid verified program",
            ),
        }
    }
}

impl From<Error> for CompileError {
    fn from(error: Error) -> Self {
        Self::Source(error)
    }
}

impl From<BuildError> for CompileError {
    fn from(error: BuildError) -> Self {
        Self::Build(error)
    }
}

impl From<VerifyError> for CompileError {
    fn from(error: VerifyError) -> Self {
        Self::Verify(error)
    }
}

struct Lowerer {
    builder: RawProgramBuilder,
    source_unit: crate::SourceUnitIndex,
    parameter_types: Vec<TypeIndex>,
    parameter_scalar_types: Vec<ScalarType>,
    needs_ids: bool,
    needs_tuples: bool,
    needs_spread: bool,
    needs_fan_out: bool,
    needs_backend_native_math: bool,
    placeholder: Option<Lowered>,
    releases: Vec<Option<ReleaseAfter>>,
    first_shape_error: Option<Error>,
    diagnostics: DiagnosticReservations,
}

#[derive(Default)]
struct DiagnosticReservations {
    refuse_next: bool,
}

impl DiagnosticReservations {
    fn try_reserve(
        &mut self,
        message: &mut String,
        capacity: usize,
        arena: crate::Arena,
    ) -> Result<(), LowerError> {
        if std::mem::take(&mut self.refuse_next) {
            return Err(LowerError::Build(BuildError::AllocationUnavailable {
                arena,
            }));
        }
        message
            .try_reserve_exact(capacity)
            .map_err(|_| LowerError::Build(BuildError::AllocationUnavailable { arena }))
    }
}

#[derive(Clone)]
struct Lowered {
    node: NodeIndex,
    result_type: TypeIndex,
    cardinality: Option<Cardinality>,
    origin: OriginIndex,
    location: SourceLocation,
    borrowed: bool,
    value_type: Type,
    tuple_elements: Vec<TupleElement>,
    access: ValueAccess,
}

#[derive(Clone)]
struct TupleElement {
    value_type: Type,
    cardinality: Option<Cardinality>,
    origin: OriginIndex,
    location: SourceLocation,
}

pub(crate) fn compile_source_with_name(
    source: &str,
    diagnostic_name: &str,
) -> Result<VerifiedProgram, CompileError> {
    let program = parse(source)?;
    compile_parsed_source_with_name(source, diagnostic_name, &program)
}

pub(crate) fn compile_parsed_source(
    source: &str,
    program: &Program,
) -> Result<VerifiedProgram, CompileError> {
    compile_parsed_source_with_name(source, "<source>", program)
}

pub(crate) fn compile_parsed_source_with_name(
    source: &str,
    diagnostic_name: &str,
    program: &Program,
) -> Result<VerifiedProgram, CompileError> {
    validate_parameter_declarations(program)?;
    resolve_names(program)?;
    validate_program_arities(program)?;
    lower_program(source, diagnostic_name, program)
}

#[derive(Clone, Copy)]
enum StructuralValue {
    NonTuple,
    Tuple(usize),
}

fn validate_program_arities(program: &Program) -> Result<(), LowerError> {
    for root in &program.roots {
        let _ = validate_expr_arities(root, None)?;
    }
    Ok(())
}

fn validate_expr_arities(
    expression: &Expr,
    placeholder: Option<StructuralValue>,
) -> Result<StructuralValue, LowerError> {
    match &expression.kind {
        ExprKind::Call {
            name,
            syntax,
            arguments,
            ..
        } => {
            let mut spread_width = None;
            for argument in arguments {
                let value = validate_expr_arities(argument, placeholder)?;
                if arguments.len() == 1 {
                    spread_width = match value {
                        StructuralValue::Tuple(width) => Some(width),
                        StructuralValue::NonTuple => None,
                    };
                }
            }
            let actual = if *syntax == CallSyntax::Prefix {
                spread_width.unwrap_or(arguments.len())
            } else {
                arguments.len()
            };
            validate_arity(name, actual, expression.span.begin)?;
            Ok(StructuralValue::NonTuple)
        }
        ExprKind::Tuple(elements) => {
            for element in elements {
                let _ = validate_expr_arities(element, placeholder)?;
            }
            Ok(StructuralValue::Tuple(elements.len()))
        }
        ExprKind::Fanout { operand, branches } => {
            let operand = validate_expr_arities(operand, placeholder)?;
            for branch in branches {
                let _ = validate_expr_arities(branch, Some(operand))?;
            }
            Ok(StructuralValue::Tuple(branches.len()))
        }
        ExprKind::UnaryChain { steps, .. } => {
            for step in steps {
                validate_arity(&step.name, 1, step.span.begin)?;
            }
            Ok(StructuralValue::NonTuple)
        }
        ExprKind::Placeholder => placeholder.ok_or_else(|| {
            LowerError::Source(Error::at_span(
                ErrorKind::SyntaxError,
                expression.span,
                "placeholder has no fanout operand",
            ))
        }),
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::DeepTuple { .. }
        | ExprKind::Parameter(_)
        | ExprKind::UnresolvedName { .. } => Ok(StructuralValue::NonTuple),
    }
}

fn validate_arity(name: &str, actual: usize, location: SourceLocation) -> Result<(), LowerError> {
    let primitive = match primitive_from_name(name) {
        Ok(primitive) => primitive,
        Err(_) => {
            return Err(LowerError::Source(unknown_primitive_diagnostic(
                name, location,
            )?));
        }
    };
    if descriptors(primitive).any(|descriptor| descriptor.parameters.len() == actual) {
        return Ok(());
    }
    let mut accepted = Vec::new();
    for descriptor in descriptors(primitive) {
        if !accepted.contains(&descriptor.parameters.len()) {
            accepted.try_reserve(1).map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::Node,
                })
            })?;
            accepted.push(descriptor.parameters.len());
        }
    }
    accepted.sort_unstable();
    let accepted_capacity = accepted
        .len()
        .checked_mul(usize::BITS as usize / 3 + 2)
        .ok_or(LowerError::Build(BuildError::CountOverflow {
            arena: crate::Arena::Node,
        }))?;
    let mut accepted_text = String::new();
    accepted_text.try_reserve(accepted_capacity).map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Node,
        })
    })?;
    for (index, arity) in accepted.iter().enumerate() {
        if index != 0 {
            accepted_text.push(' ');
        }
        write!(&mut accepted_text, "{arity}").map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Node,
            })
        })?;
    }
    let message_capacity = name
        .len()
        .checked_add(accepted_text.len())
        .and_then(|length| length.checked_add(128))
        .ok_or(LowerError::Build(BuildError::CountOverflow {
            arena: crate::Arena::Node,
        }))?;
    let mut message = String::new();
    message.try_reserve_exact(message_capacity).map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Node,
        })
    })?;
    write!(
        &mut message,
        "{name} received {actual} argument(s); accepted arity{} {accepted_text}",
        if accepted.len() == 1 { "" } else { " values" },
    )
    .map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Node,
        })
    })?;
    let mut error = Error::new(ErrorKind::ArityError, location, message);
    error.primitive = Some(try_clone_string(name, crate::Arena::Node)?);
    error.actual_arity = Some(actual);
    error.expected_arity = accepted;
    Err(LowerError::Source(error))
}

fn unknown_primitive_diagnostic(name: &str, location: SourceLocation) -> Result<Error, LowerError> {
    let capacity =
        name.len()
            .checked_add(32)
            .ok_or(LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Node,
            }))?;
    let mut message = String::new();
    message.try_reserve_exact(capacity).map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Node,
        })
    })?;
    write!(&mut message, "unknown primitive '{name}'").map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Node,
        })
    })?;
    Ok(Error::new(ErrorKind::UnknownPrimitive, location, message))
}

fn lower_program(
    source: &str,
    diagnostic_name: &str,
    program: &Program,
) -> Result<VerifiedProgram, LowerError> {
    let builder = RawProgramBuilder::new();
    lower_program_with_builder(source, diagnostic_name, program, builder)
}

fn lower_program_with_builder(
    source: &str,
    diagnostic_name: &str,
    program: &Program,
    builder: RawProgramBuilder,
) -> Result<VerifiedProgram, LowerError> {
    lower_program_with_builder_and_diagnostics(
        source,
        diagnostic_name,
        program,
        builder,
        DiagnosticReservations::default(),
    )
}

fn lower_program_with_builder_and_diagnostics(
    source: &str,
    diagnostic_name: &str,
    program: &Program,
    mut builder: RawProgramBuilder,
    diagnostics: DiagnosticReservations,
) -> Result<VerifiedProgram, LowerError> {
    let byte_length = u32::try_from(source.len()).map_err(|_| {
        LowerError::Build(BuildError::CountOverflow {
            arena: crate::Arena::SourceUnit,
        })
    })?;
    let source_unit = builder.push_source_unit(SourceUnit {
        diagnostic_name: try_clone_string(diagnostic_name, crate::Arena::SourceUnit)?,
        byte_length,
    })?;
    let mut lowerer = Lowerer {
        builder,
        source_unit,
        parameter_types: Vec::new(),
        parameter_scalar_types: Vec::new(),
        needs_ids: false,
        needs_tuples: false,
        needs_spread: false,
        needs_fan_out: false,
        needs_backend_native_math: false,
        placeholder: None,
        releases: Vec::new(),
        first_shape_error: None,
        diagnostics,
    };

    if let Some(header) = program.parameter_header {
        let origin = lowerer.push_origin(header)?;
        lowerer.builder.set_parameter_header_origin(origin);
    }
    lowerer
        .parameter_types
        .try_reserve(program.parameters.len())
        .map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Type,
            })
        })?;
    lowerer
        .parameter_scalar_types
        .try_reserve(program.parameters.len())
        .map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Type,
            })
        })?;
    for (slot, parameter) in program.parameters.iter().enumerate() {
        let declaration_origin = lowerer.push_origin(parameter.span)?;
        let name_origin = lowerer.push_origin(parameter.name_span)?;
        let slot = u32::try_from(slot).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Parameter,
            })
        })?;
        lowerer.builder.push_parameter(Parameter {
            slot,
            name: try_clone_string(&parameter.name, crate::Arena::Parameter)?,
            scalar_type: parameter.scalar_type,
            declaration_origin,
            name_origin,
        })?;
        let result_type = lowerer
            .builder
            .push_type(TypeRecord::Scalar(parameter.scalar_type))?;
        lowerer.parameter_types.push(result_type);
        lowerer.parameter_scalar_types.push(parameter.scalar_type);
    }

    for (root_index, root) in program.roots.iter().enumerate() {
        let lowered = lowerer.lower_expr(root)?;
        let root_index = RootIndex(u32::try_from(root_index).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Root,
            })
        })?);
        if !lowered.borrowed {
            lowerer.set_release(lowered.node, ReleaseAfter::Root(root_index))?;
        }
        lowerer.builder.push_root(Root {
            node: lowered.node,
            origin: lowered.origin,
        })?;
    }
    if let Some(error) = lowerer.first_shape_error {
        return Err(LowerError::Source(error));
    }
    for (owner, release_after) in lowerer.releases.iter().copied().enumerate() {
        if let Some(release_after) = release_after {
            lowerer.builder.push_ownership(Ownership {
                owner: NodeIndex(u32::try_from(owner).map_err(|_| {
                    LowerError::Build(BuildError::CountOverflow {
                        arena: crate::Arena::Ownership,
                    })
                })?),
                release_after,
            })?;
        }
    }
    if lowerer.needs_ids {
        lowerer
            .builder
            .push_feature(Feature::StableSemanticIds.numeric())?;
    }
    if lowerer.needs_tuples {
        lowerer.builder.push_feature(Feature::Tuples.numeric())?;
    }
    if lowerer.needs_spread {
        lowerer
            .builder
            .push_feature(Feature::PrefixSpread.numeric())?;
    }
    if lowerer.needs_fan_out {
        lowerer.builder.push_feature(Feature::FanOut.numeric())?;
    }
    if lowerer.needs_backend_native_math {
        lowerer.builder.set_semantic_minor(1);
        lowerer
            .builder
            .push_feature(Feature::BackendNativeMathV1.numeric())?;
    }
    Ok(lowerer.builder.finish()?.verify()?)
}

impl Lowerer {
    fn register_node(&mut self, node: NodeIndex) -> Result<(), LowerError> {
        let expected = u32::try_from(self.releases.len()).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Ownership,
            })
        })?;
        if node.0 != expected {
            return Err(LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Ownership,
            }));
        }
        self.releases.try_reserve(1).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Ownership,
            })
        })?;
        self.releases.push(None);
        Ok(())
    }

    fn set_release(
        &mut self,
        owner: NodeIndex,
        release_after: ReleaseAfter,
    ) -> Result<(), LowerError> {
        let release = usize::try_from(owner.0)
            .ok()
            .and_then(|index| self.releases.get_mut(index))
            .ok_or_else(|| {
                LowerError::Build(BuildError::CountOverflow {
                    arena: crate::Arena::Ownership,
                })
            })?;
        if release.is_some() {
            return Err(LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Ownership,
            }));
        }
        *release = Some(release_after);
        Ok(())
    }

    fn push_origin(&mut self, span: SourceSpan) -> Result<OriginIndex, BuildError> {
        self.builder.push_origin(Origin {
            source_unit: self.source_unit,
            span: OriginSpan {
                begin: origin_position(span.begin)?,
                end: origin_position(span.end)?,
            },
        })
    }

    fn lower_expr(&mut self, expression: &Expr) -> Result<Lowered, LowerError> {
        let origin = self.push_origin(expression.span)?;
        match &expression.kind {
            ExprKind::Literal(value) => self.lower_scalar(value, origin, expression.span.begin),
            ExprKind::Vector(element_type, values) => {
                self.lower_vector(*element_type, values, origin, expression.span.begin)
            }
            ExprKind::Parameter(index) => {
                let result_type = self.parameter_types.get(*index).copied().ok_or_else(|| {
                    LowerError::Source(Error::at_span(
                        ErrorKind::ParameterError,
                        expression.span,
                        "invalid parameter reference",
                    ))
                })?;
                let parameter = crate::ParameterIndex(u32::try_from(*index).map_err(|_| {
                    LowerError::Build(BuildError::CountOverflow {
                        arena: crate::Arena::Parameter,
                    })
                })?);
                let node = self.builder.push_node(Node {
                    kind: NodeKind::ParameterBorrow { parameter },
                    result_type,
                    cardinality: Some(Cardinality::StaticScalar),
                    edges: IndexRange {
                        start: self.builder.finish_preview_edges()?,
                        count: 0,
                    },
                    origin,
                })?;
                self.register_node(node)?;
                Ok(Lowered {
                    node,
                    result_type,
                    cardinality: Some(Cardinality::StaticScalar),
                    origin,
                    location: expression.span.begin,
                    borrowed: true,
                    value_type: Type::Scalar(*self.parameter_scalar_types.get(*index).ok_or_else(
                        || {
                            LowerError::Source(Error::at_span(
                                ErrorKind::ParameterError,
                                expression.span,
                                "invalid parameter reference",
                            ))
                        },
                    )?),
                    tuple_elements: Vec::new(),
                    access: ValueAccess::WholeValue,
                })
            }
            ExprKind::Tuple(elements) => self.lower_tuple(elements, origin, expression.span.begin),
            ExprKind::DeepTuple { depth, leaf } => {
                self.lower_deep_tuple(*depth, leaf, origin, expression.span.begin)
            }
            ExprKind::UnaryChain {
                leaf,
                leaf_span,
                steps,
            } => {
                let leaf_origin = self.push_origin(*leaf_span)?;
                let mut current = self.lower_scalar(leaf, leaf_origin, leaf_span.begin)?;
                for step in steps {
                    let primitive_origin = self.push_origin(step.name_span)?;
                    let call_origin = self.push_origin(step.span)?;
                    let mut operands = Vec::new();
                    operands.try_reserve(1).map_err(|_| {
                        LowerError::Build(BuildError::AllocationUnavailable {
                            arena: crate::Arena::Edge,
                        })
                    })?;
                    operands.push(CallOperand::Whole(current));
                    current = self.lower_selected_call(
                        &step.name,
                        primitive_origin,
                        call_origin,
                        step.span.begin,
                        operands,
                    )?;
                }
                Ok(current)
            }
            ExprKind::Call {
                name,
                syntax,
                arguments,
                name_span,
            } => self.lower_call(
                name,
                *syntax,
                arguments,
                *name_span,
                origin,
                expression.span.begin,
            ),
            ExprKind::Placeholder => {
                let mut placeholder = self.placeholder.take().ok_or_else(|| {
                    LowerError::Source(Error::at_span(
                        ErrorKind::SyntaxError,
                        expression.span,
                        "placeholder has no fanout operand",
                    ))
                })?;
                placeholder.origin = origin;
                placeholder.location = expression.span.begin;
                placeholder.borrowed = true;
                placeholder.access = ValueAccess::FanOutOperandBorrow;
                Ok(placeholder)
            }
            ExprKind::Fanout { operand, branches } => {
                self.lower_fan_out(expression, operand, branches, origin)
            }
            _ => Err(LowerError::Source(Error::at_span(
                ErrorKind::TypeError,
                expression.span,
                "source construct is not yet lowerable",
            ))),
        }
    }

    fn lower_scalar(
        &mut self,
        value: &Value,
        origin: OriginIndex,
        location: SourceLocation,
    ) -> Result<Lowered, LowerError> {
        let scalar = scalar_constant(value).ok_or_else(|| {
            LowerError::Source(Error::new(
                ErrorKind::TypeError,
                SourceLocation::start(),
                "literal is not scalar",
            ))
        })?;
        let scalar_type = value.scalar_type().ok_or_else(|| {
            LowerError::Source(Error::new(
                ErrorKind::TypeError,
                SourceLocation::start(),
                "literal is not scalar",
            ))
        })?;
        let result_type = self.builder.push_type(TypeRecord::Scalar(scalar_type))?;
        let constant = self.builder.push_constant(ConstantRecord::Scalar(scalar))?;
        let node = self.builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange {
                start: self.builder.finish_preview_edges()?,
                count: 0,
            },
            origin,
        })?;
        self.register_node(node)?;
        Ok(Lowered {
            node,
            result_type,
            cardinality: Some(Cardinality::StaticScalar),
            origin,
            location,
            borrowed: false,
            value_type: Type::Scalar(scalar_type),
            tuple_elements: Vec::new(),
            access: ValueAccess::WholeValue,
        })
    }

    fn lower_vector(
        &mut self,
        element_type: ScalarType,
        values: &[Value],
        origin: OriginIndex,
        location: SourceLocation,
    ) -> Result<Lowered, LowerError> {
        let start = self.builder.finish_preview_constant_elements()?;
        for value in values {
            let scalar = scalar_constant(value).ok_or_else(|| {
                LowerError::Source(Error::new(
                    ErrorKind::TypeError,
                    SourceLocation::start(),
                    "vector element is not scalar",
                ))
            })?;
            self.builder.push_constant_element(scalar)?;
        }
        let count = u32::try_from(values.len()).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::ConstantElement,
            })
        })?;
        let result_type = self.builder.push_type(TypeRecord::Vector(element_type))?;
        let constant = self.builder.push_constant(ConstantRecord::Vector {
            element_type,
            elements: IndexRange { start, count },
        })?;
        let cardinality = Some(Cardinality::StaticVector(count));
        let node = self.builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type,
            cardinality,
            edges: IndexRange {
                start: self.builder.finish_preview_edges()?,
                count: 0,
            },
            origin,
        })?;
        self.register_node(node)?;
        Ok(Lowered {
            node,
            result_type,
            cardinality,
            origin,
            location,
            borrowed: false,
            value_type: Type::Vector(element_type),
            tuple_elements: Vec::new(),
            access: ValueAccess::WholeValue,
        })
    }

    fn lower_tuple(
        &mut self,
        elements: &[Expr],
        origin: OriginIndex,
        location: SourceLocation,
    ) -> Result<Lowered, LowerError> {
        let mut lowered = Vec::new();
        lowered.try_reserve(elements.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Node,
            })
        })?;
        for element in elements {
            lowered.push(self.lower_expr(element)?);
        }
        let type_start = self.builder.finish_preview_type_elements()?;
        for element in &lowered {
            self.builder.push_type_element(element.result_type)?;
        }
        let count = u32::try_from(lowered.len()).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::TypeElement,
            })
        })?;
        let result_type = self.builder.push_type(TypeRecord::Tuple {
            elements: IndexRange {
                start: type_start,
                count,
            },
        })?;
        let edge_start = self.builder.finish_preview_edges()?;
        for (position, element) in lowered.iter().enumerate() {
            let argument_position = u32::try_from(position)
                .ok()
                .and_then(|position| position.checked_add(1))
                .ok_or_else(|| {
                    LowerError::Build(BuildError::CountOverflow {
                        arena: crate::Arena::Edge,
                    })
                })?;
            self.builder.push_edge(Edge {
                producer: element.node,
                argument_position,
                access: ValueAccess::WholeValue,
                cardinality: element.cardinality,
                conversion: crate::Conversion::Identity,
                ownership: if element.borrowed {
                    OwnershipMode::ImmutableBorrow
                } else {
                    OwnershipMode::InfallibleTransfer
                },
                origin: element.origin,
            })?;
        }
        let node = self.builder.push_node(Node {
            kind: NodeKind::TupleConstruct,
            result_type,
            cardinality: None,
            edges: IndexRange {
                start: edge_start,
                count,
            },
            origin,
        })?;
        self.register_node(node)?;
        for element in &lowered {
            if !element.borrowed {
                self.set_release(element.node, ReleaseAfter::Node(node))?;
            }
        }
        self.needs_tuples = true;
        let mut tuple_elements = Vec::new();
        tuple_elements.try_reserve(lowered.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::TypeElement,
            })
        })?;
        for element in &lowered {
            tuple_elements.push(TupleElement {
                value_type: try_clone_type(&element.value_type)?,
                cardinality: element.cardinality,
                origin: element.origin,
                location,
            });
        }
        let mut value_types = Vec::new();
        value_types.try_reserve(lowered.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::TypeElement,
            })
        })?;
        for element in &lowered {
            value_types.push(try_clone_type(&element.value_type)?);
        }
        let value_type = Type::Tuple(value_types);
        Ok(Lowered {
            node,
            result_type,
            cardinality: None,
            origin,
            location,
            borrowed: false,
            value_type,
            tuple_elements,
            access: ValueAccess::WholeValue,
        })
    }

    fn lower_deep_tuple(
        &mut self,
        depth: usize,
        leaf: &Value,
        origin: OriginIndex,
        location: SourceLocation,
    ) -> Result<Lowered, LowerError> {
        let mut current = self.lower_scalar(leaf, origin, location)?;
        for _ in 0..depth {
            let type_start = self.builder.finish_preview_type_elements()?;
            self.builder.push_type_element(current.result_type)?;
            let result_type = self.builder.push_type(TypeRecord::Tuple {
                elements: IndexRange {
                    start: type_start,
                    count: 1,
                },
            })?;
            let edge_start = self.builder.finish_preview_edges()?;
            self.builder.push_edge(Edge {
                producer: current.node,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: current.cardinality,
                conversion: crate::Conversion::Identity,
                ownership: if current.borrowed {
                    OwnershipMode::ImmutableBorrow
                } else {
                    OwnershipMode::InfallibleTransfer
                },
                origin: current.origin,
            })?;
            let node = self.builder.push_node(Node {
                kind: NodeKind::TupleConstruct,
                result_type,
                cardinality: None,
                edges: IndexRange {
                    start: edge_start,
                    count: 1,
                },
                origin,
            })?;
            self.register_node(node)?;
            if !current.borrowed {
                self.set_release(current.node, ReleaseAfter::Node(node))?;
            }
            let (current_depth, leaf) = match current.value_type {
                Type::Scalar(leaf) => (0, leaf),
                Type::RepeatedTuple { depth, leaf } => (depth, leaf),
                _ => {
                    return Err(LowerError::Source(Error::new(
                        ErrorKind::TypeError,
                        SourceLocation::start(),
                        "deep tuple leaf is not scalar",
                    )));
                }
            };
            let next_depth = current_depth.checked_add(1).ok_or_else(|| {
                LowerError::Build(BuildError::CountOverflow {
                    arena: crate::Arena::Type,
                })
            })?;
            let child_type = if current_depth == 0 {
                Type::Scalar(leaf)
            } else {
                Type::RepeatedTuple {
                    depth: current_depth,
                    leaf,
                }
            };
            let mut tuple_elements = Vec::new();
            tuple_elements.try_reserve(1).map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::TypeElement,
                })
            })?;
            tuple_elements.push(TupleElement {
                value_type: child_type,
                cardinality: current.cardinality,
                origin: current.origin,
                location,
            });
            current = Lowered {
                node,
                result_type,
                cardinality: None,
                origin,
                location,
                borrowed: false,
                value_type: Type::RepeatedTuple {
                    depth: next_depth,
                    leaf,
                },
                tuple_elements,
                access: ValueAccess::WholeValue,
            };
        }
        self.needs_tuples |= depth != 0;
        Ok(current)
    }

    fn lower_call(
        &mut self,
        name: &str,
        syntax: CallSyntax,
        arguments: &[Expr],
        name_span: SourceSpan,
        origin: OriginIndex,
        location: SourceLocation,
    ) -> Result<Lowered, LowerError> {
        let primitive_origin = self.push_origin(name_span)?;
        let mut lowered = Vec::new();
        lowered.try_reserve(arguments.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Node,
            })
        })?;
        for argument in arguments {
            lowered.push(self.lower_expr(argument)?);
        }
        if syntax == CallSyntax::Prefix
            && lowered.len() == 1
            && matches!(lowered[0].value_type, Type::Tuple(_))
        {
            return self.lower_prefix_call(
                name,
                primitive_origin,
                origin,
                location,
                lowered.remove(0),
            );
        }
        let mut operands = Vec::new();
        operands.try_reserve(lowered.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Edge,
            })
        })?;
        for argument in lowered {
            operands.push(CallOperand::Whole(argument));
        }
        self.lower_selected_call(name, primitive_origin, origin, location, operands)
    }

    fn lower_prefix_call(
        &mut self,
        name: &str,
        primitive_origin: OriginIndex,
        origin: OriginIndex,
        location: SourceLocation,
        tuple: Lowered,
    ) -> Result<Lowered, LowerError> {
        let edge_start = self.builder.finish_preview_edges()?;
        self.builder.push_edge(Edge {
            producer: tuple.node,
            argument_position: 1,
            access: tuple.access,
            cardinality: None,
            conversion: crate::Conversion::Identity,
            ownership: if tuple.borrowed {
                OwnershipMode::ImmutableBorrow
            } else {
                OwnershipMode::InfallibleTransfer
            },
            origin: tuple.origin,
        })?;
        let prepare = self.builder.push_node(Node {
            kind: NodeKind::PrefixSpreadPrepare,
            result_type: tuple.result_type,
            cardinality: None,
            edges: IndexRange {
                start: edge_start,
                count: 1,
            },
            origin: tuple.origin,
        })?;
        self.register_node(prepare)?;
        if !tuple.borrowed {
            self.set_release(tuple.node, ReleaseAfter::Node(prepare))?;
        }
        let mut operands = Vec::new();
        operands
            .try_reserve(tuple.tuple_elements.len())
            .map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::Edge,
                })
            })?;
        for (element, metadata) in tuple.tuple_elements.into_iter().enumerate() {
            let element = u32::try_from(element).map_err(|_| {
                LowerError::Build(BuildError::CountOverflow {
                    arena: crate::Arena::Edge,
                })
            })?;
            operands.push(CallOperand::TupleElement {
                prepare,
                element,
                metadata,
            });
        }
        let result =
            self.lower_selected_call(name, primitive_origin, origin, location, operands)?;
        self.set_release(prepare, ReleaseAfter::Node(result.node))?;
        self.needs_spread = true;
        Ok(result)
    }

    fn lower_selected_call(
        &mut self,
        name: &str,
        primitive_origin: OriginIndex,
        origin: OriginIndex,
        location: SourceLocation,
        operands: Vec<CallOperand>,
    ) -> Result<Lowered, LowerError> {
        let descriptor = select_descriptor(name, &operands, location, &mut self.diagnostics)?;
        let edge_start = self.builder.finish_preview_edges()?;
        let mut static_anchor = None;
        let mut static_length = None;
        let mut any_vector = false;
        let mut any_dynamic = false;
        for (position, (operand, accepted)) in
            operands.iter().zip(descriptor.parameters).enumerate()
        {
            let (
                producer,
                access,
                value_type,
                operand_cardinality,
                operand_origin,
                borrowed,
                _operand_location,
            ) = operand.parts();
            let actual = scalar_element(value_type).ok_or_else(|| {
                LowerError::Source(Error::new(
                    ErrorKind::TypeError,
                    SourceLocation::start(),
                    "selected operand is not scalar or vector",
                ))
            })?;
            let conversion = match conversion(actual, *accepted) {
                Some(RegistryConversion::Identity) => crate::Conversion::Identity,
                Some(RegistryConversion::PromoteIntToDouble) => {
                    crate::Conversion::PromoteIntToDouble
                }
                None => {
                    return Err(LowerError::Source(Error::new(
                        ErrorKind::TypeError,
                        SourceLocation::start(),
                        "selected conversion is invalid",
                    )));
                }
            };
            if matches!(value_type, Type::Vector(_)) {
                any_vector = true;
                match operand_cardinality {
                    Some(Cardinality::StaticVector(length)) => {
                        if static_anchor.is_none() {
                            static_anchor = u32::try_from(position).ok();
                            static_length = Some(length);
                        }
                    }
                    Some(Cardinality::DynamicVector) => any_dynamic = true,
                    _ => {}
                }
            }
            self.builder.push_edge(Edge {
                producer,
                argument_position: u32::try_from(position + 1).map_err(|_| {
                    LowerError::Build(BuildError::CountOverflow {
                        arena: crate::Arena::Edge,
                    })
                })?,
                access,
                cardinality: operand_cardinality,
                conversion,
                ownership: if borrowed {
                    OwnershipMode::ImmutableBorrow
                } else {
                    OwnershipMode::OwnedInput
                },
                origin: operand_origin,
            })?;
        }
        if self.first_shape_error.is_none()
            && let Some((anchor_index, expected)) =
                operands
                    .iter()
                    .enumerate()
                    .find_map(|(index, operand)| match operand.parts().3 {
                        Some(Cardinality::StaticVector(length)) => Some((index, length)),
                        _ => None,
                    })
        {
            for (index, operand) in operands.iter().enumerate() {
                if index == anchor_index {
                    continue;
                }
                if let Some(Cardinality::StaticVector(actual)) = operand.parts().3
                    && actual != expected
                {
                    let message = static_shape_message(
                        name,
                        index + 1,
                        expected,
                        actual,
                        &mut self.diagnostics,
                    )?;
                    let mut error =
                        Error::new(ErrorKind::ShapeMismatch, operand.parts().6, message);
                    error.primitive = Some(try_clone_string(name, crate::Arena::Node)?);
                    error.argument_position = Some(index + 1);
                    let expected = usize::try_from(expected).map_err(|_| {
                        LowerError::Build(BuildError::CountOverflow {
                            arena: crate::Arena::ShapeCheck,
                        })
                    })?;
                    let actual = usize::try_from(actual).map_err(|_| {
                        LowerError::Build(BuildError::CountOverflow {
                            arena: crate::Arena::ShapeCheck,
                        })
                    })?;
                    let mut expected_shape = Vec::new();
                    expected_shape.try_reserve(1).map_err(|_| {
                        LowerError::Build(BuildError::AllocationUnavailable {
                            arena: crate::Arena::ShapeCheck,
                        })
                    })?;
                    expected_shape.push(expected);
                    let mut actual_shape = Vec::new();
                    actual_shape.try_reserve(1).map_err(|_| {
                        LowerError::Build(BuildError::AllocationUnavailable {
                            arena: crate::Arena::ShapeCheck,
                        })
                    })?;
                    actual_shape.push(actual);
                    error.expected_shape = Some(expected_shape);
                    error.actual_shape = Some(actual_shape);
                    self.first_shape_error = Some(error);
                    break;
                }
            }
        }
        let shape_start = self.builder.finish_preview_shape_checks()?;
        for (position, operand) in operands.iter().enumerate() {
            if matches!(operand.parts().3, Some(Cardinality::DynamicVector)) {
                self.builder.push_shape_check(
                    edge_start
                        .checked_add(u32::try_from(position).map_err(|_| {
                            LowerError::Build(BuildError::CountOverflow {
                                arena: crate::Arena::ShapeCheck,
                            })
                        })?)
                        .ok_or_else(|| {
                            LowerError::Build(BuildError::CountOverflow {
                                arena: crate::Arena::ShapeCheck,
                            })
                        })?,
                )?;
            }
        }
        let shape_count = operands
            .iter()
            .filter(|operand| matches!(operand.parts().3, Some(Cardinality::DynamicVector)))
            .count();
        let (record, value_type, cardinality, lift) =
            if descriptor.behavior == StructuralBehavior::Iota {
                (
                    TypeRecord::Vector(descriptor.result),
                    Type::Vector(descriptor.result),
                    Some(Cardinality::DynamicVector),
                    LiftMode::DynamicVector,
                )
            } else if any_vector {
                (
                    TypeRecord::Vector(descriptor.result),
                    Type::Vector(descriptor.result),
                    Some(if any_dynamic {
                        Cardinality::DynamicVector
                    } else {
                        Cardinality::StaticVector(static_length.unwrap_or(0))
                    }),
                    LiftMode::Vector,
                )
            } else {
                (
                    TypeRecord::Scalar(descriptor.result),
                    Type::Scalar(descriptor.result),
                    Some(Cardinality::StaticScalar),
                    LiftMode::Scalar,
                )
            };
        let result_type = self.builder.push_type(record)?;
        let node = self.builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: descriptor.primitive_id.numeric(),
                signature_id: descriptor.signature_id.numeric(),
                implementation_id: descriptor.implementation_id.numeric(),
                primitive_origin,
                lift,
                result_element_type: descriptor.result,
                shape: ShapePlan {
                    static_anchor,
                    dynamic_checks: IndexRange {
                        start: shape_start,
                        count: u32::try_from(shape_count).map_err(|_| {
                            LowerError::Build(BuildError::CountOverflow {
                                arena: crate::Arena::ShapeCheck,
                            })
                        })?,
                    },
                },
            },
            result_type,
            cardinality,
            edges: IndexRange {
                start: edge_start,
                count: u32::try_from(operands.len()).map_err(|_| {
                    LowerError::Build(BuildError::CountOverflow {
                        arena: crate::Arena::Edge,
                    })
                })?,
            },
            origin,
        })?;
        self.register_node(node)?;
        for operand in operands {
            if let CallOperand::Whole(lowered) = operand
                && !lowered.borrowed
            {
                self.set_release(lowered.node, ReleaseAfter::Node(node))?;
            }
        }
        self.needs_ids = true;
        self.needs_backend_native_math |=
            is_backend_native_math_primitive(descriptor.primitive_id.numeric());
        Ok(Lowered {
            node,
            result_type,
            cardinality,
            origin,
            location,
            borrowed: false,
            value_type,
            tuple_elements: Vec::new(),
            access: ValueAccess::WholeValue,
        })
    }

    fn lower_fan_out(
        &mut self,
        expression: &Expr,
        operand: &Expr,
        branches: &[Expr],
        origin: OriginIndex,
    ) -> Result<Lowered, LowerError> {
        let operand = self.lower_expr(operand)?;
        let branch_start = self.builder.finish_preview_branches()?;
        let mut branch_roots = Vec::new();
        let mut result_elements = Vec::new();
        branch_roots.try_reserve(branches.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Branch,
            })
        })?;
        result_elements.try_reserve(branches.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::TypeElement,
            })
        })?;
        for branch in branches {
            let nodes_start = self.builder.finish_preview_nodes()?;
            self.placeholder = Some(try_clone_lowered(&operand)?);
            let lowered = self.lower_expr(branch);
            self.placeholder = None;
            let lowered = lowered?;
            let nodes_end = self.builder.finish_preview_nodes()?;
            let placeholder_span = placeholder_span(branch).ok_or_else(|| {
                LowerError::Source(Error::at_span(
                    ErrorKind::SyntaxError,
                    branch.span,
                    "fanout branch has no placeholder",
                ))
            })?;
            let placeholder_origin = self.push_origin(placeholder_span)?;
            let branch_origin = self.push_origin(branch.span)?;
            self.builder.push_branch(FanOutBranch {
                nodes: IndexRange {
                    start: nodes_start,
                    count: nodes_end.checked_sub(nodes_start).ok_or_else(|| {
                        LowerError::Build(BuildError::CountOverflow {
                            arena: crate::Arena::Node,
                        })
                    })?,
                },
                root: lowered.node,
                placeholder_origin,
                origin: branch_origin,
            })?;
            result_elements.push(lowered.result_type);
            branch_roots.push(lowered);
        }
        let type_start = self.builder.finish_preview_type_elements()?;
        for result_type in result_elements {
            self.builder.push_type_element(result_type)?;
        }
        let branch_count = u32::try_from(branches.len()).map_err(|_| {
            LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Branch,
            })
        })?;
        let result_type = self.builder.push_type(TypeRecord::Tuple {
            elements: IndexRange {
                start: type_start,
                count: branch_count,
            },
        })?;
        let edge_start = self.builder.finish_preview_edges()?;
        self.builder.push_edge(Edge {
            producer: operand.node,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: operand.cardinality,
            conversion: crate::Conversion::Identity,
            ownership: if operand.borrowed {
                OwnershipMode::ImmutableBorrow
            } else {
                OwnershipMode::OwnedInput
            },
            origin: operand.origin,
        })?;
        let keyword_origin = self.push_origin(fanout_keyword_span(expression.span)?)?;
        let node = self.builder.push_node(Node {
            kind: NodeKind::FanOut {
                branches: IndexRange {
                    start: branch_start,
                    count: branch_count,
                },
                keyword_origin,
            },
            result_type,
            cardinality: None,
            edges: IndexRange {
                start: edge_start,
                count: 1,
            },
            origin,
        })?;
        self.register_node(node)?;
        if !operand.borrowed {
            self.set_release(operand.node, ReleaseAfter::Node(node))?;
        }
        for root in &branch_roots {
            self.set_release(root.node, ReleaseAfter::Node(node))?;
        }
        self.needs_tuples = true;
        self.needs_fan_out = true;
        let mut tuple_elements = Vec::new();
        let mut value_types = Vec::new();
        tuple_elements
            .try_reserve(branch_roots.len())
            .map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::TypeElement,
                })
            })?;
        value_types.try_reserve(branch_roots.len()).map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::TypeElement,
            })
        })?;
        for root in &branch_roots {
            tuple_elements.push(TupleElement {
                value_type: try_clone_type(&root.value_type)?,
                cardinality: root.cardinality,
                origin: root.origin,
                location: expression.span.begin,
            });
            value_types.push(try_clone_type(&root.value_type)?);
        }
        Ok(Lowered {
            node,
            result_type,
            cardinality: None,
            origin,
            location: expression.span.begin,
            borrowed: false,
            value_type: Type::Tuple(value_types),
            tuple_elements,
            access: ValueAccess::WholeValue,
        })
    }
}

enum CallOperand {
    Whole(Lowered),
    TupleElement {
        prepare: NodeIndex,
        element: u32,
        metadata: TupleElement,
    },
}

impl CallOperand {
    fn parts(
        &self,
    ) -> (
        NodeIndex,
        ValueAccess,
        &Type,
        Option<Cardinality>,
        OriginIndex,
        bool,
        SourceLocation,
    ) {
        match self {
            Self::Whole(lowered) => (
                lowered.node,
                lowered.access,
                &lowered.value_type,
                lowered.cardinality,
                lowered.origin,
                lowered.borrowed,
                lowered.location,
            ),
            Self::TupleElement {
                prepare,
                element,
                metadata,
            } => (
                *prepare,
                ValueAccess::TupleElement(*element),
                &metadata.value_type,
                metadata.cardinality,
                metadata.origin,
                true,
                metadata.location,
            ),
        }
    }
}

fn unsupported_signature_message(
    name: &str,
    first_unsupported: usize,
    diagnostics: &mut DiagnosticReservations,
) -> Result<String, LowerError> {
    let capacity =
        name.len()
            .checked_add(128)
            .ok_or(LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::Type,
            }))?;
    let mut message = String::new();
    diagnostics.try_reserve(&mut message, capacity, crate::Arena::Type)?;
    write!(
        &mut message,
        "{name} arguments do not match an accepted signature; first unsupported argument is {first_unsupported}"
    )
    .map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::Type,
        })
    })?;
    Ok(message)
}

fn static_shape_message(
    name: &str,
    argument_position: usize,
    expected: u32,
    actual: u32,
    diagnostics: &mut DiagnosticReservations,
) -> Result<String, LowerError> {
    let capacity =
        name.len()
            .checked_add(128)
            .ok_or(LowerError::Build(BuildError::CountOverflow {
                arena: crate::Arena::ShapeCheck,
            }))?;
    let mut message = String::new();
    diagnostics.try_reserve(&mut message, capacity, crate::Arena::ShapeCheck)?;
    write!(
        &mut message,
        "{name} argument {argument_position} expected shape [{expected}], got [{actual}]"
    )
    .map_err(|_| {
        LowerError::Build(BuildError::AllocationUnavailable {
            arena: crate::Arena::ShapeCheck,
        })
    })?;
    Ok(message)
}

fn select_descriptor(
    name: &str,
    operands: &[CallOperand],
    location: SourceLocation,
    diagnostics: &mut DiagnosticReservations,
) -> Result<&'static SemanticDescriptor, LowerError> {
    let primitive = match primitive_from_name(name) {
        Ok(primitive) => primitive,
        Err(_) => {
            return Err(LowerError::Source(unknown_primitive_diagnostic(
                name, location,
            )?));
        }
    };
    if !descriptors(primitive).any(|descriptor| descriptor.parameters.len() == operands.len()) {
        return validate_arity(name, operands.len(), location).and_then(|()| {
            Err(LowerError::Source(Error::new(
                ErrorKind::ArityError,
                location,
                "arity validation did not reject an invalid call",
            )))
        });
    }
    if descriptors(primitive)
        .next()
        .is_some_and(|descriptor| descriptor.behavior == StructuralBehavior::Iota)
        && operands
            .first()
            .is_some_and(|operand| !matches!(operand.parts().2, Type::Scalar(_)))
    {
        let message = unsupported_signature_message(name, 1, diagnostics)?;
        let mut error = Error::new(ErrorKind::TypeError, operands[0].parts().6, message);
        error.primitive = Some(try_clone_string(name, crate::Arena::Node)?);
        error.argument_position = Some(1);
        error
            .actual_types
            .try_reserve(operands.len())
            .map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::Type,
                })
            })?;
        for operand in operands {
            error.actual_types.push(try_clone_type(operand.parts().2)?);
        }
        return Err(LowerError::Source(error));
    }
    let selected = descriptors(primitive)
        .filter(|descriptor| descriptor.parameters.len() == operands.len())
        .filter_map(|descriptor| {
            let mut cost = 0;
            for (operand, accepted) in operands.iter().zip(descriptor.parameters) {
                let actual = scalar_element(operand.parts().2)?;
                match conversion(actual, *accepted)? {
                    RegistryConversion::Identity => {}
                    RegistryConversion::PromoteIntToDouble => cost += 1,
                }
            }
            Some((cost, descriptor))
        })
        .min_by_key(|(cost, _)| *cost)
        .map(|(_, descriptor)| descriptor);
    let Some(selected) = selected else {
        let matched_prefix = descriptors(primitive)
            .filter(|descriptor| descriptor.parameters.len() == operands.len())
            .map(|descriptor| {
                descriptor
                    .parameters
                    .iter()
                    .zip(operands)
                    .take_while(|(accepted, operand)| {
                        scalar_element(operand.parts().2)
                            .is_some_and(|actual| conversion(actual, **accepted).is_some())
                    })
                    .count()
            })
            .max()
            .unwrap_or(0);
        let first_unsupported = (matched_prefix + 1).min(operands.len());
        let message = unsupported_signature_message(name, first_unsupported, diagnostics)?;
        let mut error = Error::new(
            ErrorKind::TypeError,
            operands[first_unsupported - 1].parts().6,
            message,
        );
        error.primitive = Some(try_clone_string(name, crate::Arena::Node)?);
        error.argument_position = Some(first_unsupported);
        error
            .actual_types
            .try_reserve(operands.len())
            .map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::Type,
                })
            })?;
        for operand in operands {
            error.actual_types.push(try_clone_type(operand.parts().2)?);
        }
        return Err(LowerError::Source(error));
    };
    Ok(selected)
}

fn scalar_element(value_type: &Type) -> Option<ScalarType> {
    match value_type {
        Type::Scalar(scalar) | Type::Vector(scalar) => Some(*scalar),
        Type::Tuple(_) | Type::RepeatedTuple { .. } => None,
    }
}

fn try_clone_string(value: &str, arena: crate::Arena) -> Result<String, LowerError> {
    let mut cloned = String::new();
    cloned
        .try_reserve_exact(value.len())
        .map_err(|_| LowerError::Build(BuildError::AllocationUnavailable { arena }))?;
    cloned.push_str(value);
    Ok(cloned)
}

fn try_clone_type(value_type: &Type) -> Result<Type, LowerError> {
    match value_type {
        Type::Scalar(scalar) => Ok(Type::Scalar(*scalar)),
        Type::Vector(scalar) => Ok(Type::Vector(*scalar)),
        Type::RepeatedTuple { depth, leaf } => Ok(Type::RepeatedTuple {
            depth: *depth,
            leaf: *leaf,
        }),
        Type::Tuple(elements) => {
            let mut cloned = Vec::new();
            cloned.try_reserve(elements.len()).map_err(|_| {
                LowerError::Build(BuildError::AllocationUnavailable {
                    arena: crate::Arena::TypeElement,
                })
            })?;
            for element in elements {
                cloned.push(try_clone_type(element)?);
            }
            Ok(Type::Tuple(cloned))
        }
    }
}

fn try_clone_lowered(lowered: &Lowered) -> Result<Lowered, LowerError> {
    let mut tuple_elements = Vec::new();
    tuple_elements
        .try_reserve(lowered.tuple_elements.len())
        .map_err(|_| {
            LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::TypeElement,
            })
        })?;
    for element in &lowered.tuple_elements {
        tuple_elements.push(TupleElement {
            value_type: try_clone_type(&element.value_type)?,
            cardinality: element.cardinality,
            origin: element.origin,
            location: element.location,
        });
    }
    Ok(Lowered {
        node: lowered.node,
        result_type: lowered.result_type,
        cardinality: lowered.cardinality,
        origin: lowered.origin,
        location: lowered.location,
        borrowed: lowered.borrowed,
        value_type: try_clone_type(&lowered.value_type)?,
        tuple_elements,
        access: lowered.access,
    })
}

fn placeholder_span(expression: &Expr) -> Option<SourceSpan> {
    match &expression.kind {
        ExprKind::Placeholder => Some(expression.span),
        ExprKind::Call { arguments, .. } | ExprKind::Tuple(arguments) => {
            arguments.iter().find_map(placeholder_span)
        }
        ExprKind::Fanout { operand, branches } => {
            placeholder_span(operand).or_else(|| branches.iter().find_map(placeholder_span))
        }
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::DeepTuple { .. }
        | ExprKind::UnaryChain { .. }
        | ExprKind::Parameter(_)
        | ExprKind::UnresolvedName { .. } => None,
    }
}

fn fanout_keyword_span(span: SourceSpan) -> Result<SourceSpan, BuildError> {
    let end_offset = span
        .begin
        .offset
        .checked_add(6)
        .ok_or(BuildError::CountOverflow {
            arena: crate::Arena::Origin,
        })?;
    let end_column = span
        .begin
        .column
        .checked_add(6)
        .ok_or(BuildError::CountOverflow {
            arena: crate::Arena::Origin,
        })?;
    Ok(SourceSpan {
        begin: span.begin,
        end: SourceLocation {
            offset: end_offset,
            line: span.begin.line,
            column: end_column,
        },
    })
}

fn origin_position(location: SourceLocation) -> Result<OriginPosition, BuildError> {
    let convert = |value| {
        u32::try_from(value).map_err(|_| BuildError::CountOverflow {
            arena: crate::Arena::Origin,
        })
    };
    Ok(OriginPosition {
        offset: convert(location.offset)?,
        line: convert(location.line)?,
        column: convert(location.column)?,
    })
}

fn scalar_constant(value: &Value) -> Option<ScalarConstant> {
    match value {
        Value::Bool(value) => Some(ScalarConstant::Bool(*value)),
        Value::Int(value) => Some(ScalarConstant::Int(*value)),
        Value::Double(value) => Some(ScalarConstant::DoubleBits(value.to_bits())),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn debug_digest(program: &VerifiedProgram) -> u64 {
        let text = format!("{:?}", program.as_raw());
        text.bytes().fold(0xcbf2_9ce4_8422_2325, |hash, byte| {
            (hash ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3)
        })
    }

    fn must_compile(source: &str) -> VerifiedProgram {
        match compile_source_with_name(source, "<source>") {
            Ok(program) => program,
            Err(error) => panic!("compile failed: {error:?}"),
        }
    }

    fn must_source_error(source: &str) -> Error {
        match compile_source_with_name(source, "<source>") {
            Err(CompileError::Source(error)) => error,
            Ok(_) => panic!("source unexpectedly compiled"),
            Err(error) => panic!("expected source diagnostic, got {error:?}"),
        }
    }

    #[test]
    fn lowers_scalar_vector_parameter_and_tuple() {
        let program = must_compile("parameters[value Int]\n[1 (2 3) value]\n");
        let raw = program.as_raw();
        assert_eq!(raw.parameters.len(), 1);
        assert_eq!(raw.roots.len(), 1);
        assert_eq!(raw.nodes.len(), 4);
        assert_eq!(raw.features, vec![Feature::Tuples.numeric()]);
        assert!(matches!(raw.nodes[0].kind, NodeKind::Constant { .. }));
        assert!(matches!(raw.nodes[1].kind, NodeKind::Constant { .. }));
        assert!(matches!(
            raw.nodes[2].kind,
            NodeKind::ParameterBorrow {
                parameter: crate::ParameterIndex(0)
            }
        ));
        assert!(matches!(raw.nodes[3].kind, NodeKind::TupleConstruct));
    }

    #[test]
    fn preserves_empty_program_and_explicit_empty_parameter_header() {
        assert!(must_compile("").as_raw().roots.is_empty());
        let explicit = must_compile("parameters[]\n");
        assert!(explicit.as_raw().parameters.is_empty());
        assert!(explicit.module().parameter_header_origin.is_some());
    }

    #[test]
    fn selected_calls_record_stable_ids_conversions_cardinality_and_origins() {
        let program = must_compile("add[1 2.0]\ninc[(1 2)]\niota[3]\n");
        let raw = program.as_raw();
        let applies: Vec<_> = raw
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::SelectedApply {
                    primitive_id,
                    signature_id,
                    implementation_id,
                    lift,
                    shape,
                    ..
                } => Some((
                    node,
                    primitive_id,
                    signature_id,
                    implementation_id,
                    lift,
                    shape,
                )),
                _ => None,
            })
            .collect();
        assert_eq!(applies.len(), 3);
        assert_eq!((applies[0].1, applies[0].2, applies[0].3), (5, 10, 10));
        assert_eq!(
            raw.edges[0].conversion,
            crate::Conversion::PromoteIntToDouble
        );
        assert_eq!(raw.edges[1].conversion, crate::Conversion::Identity);
        assert_eq!(applies[1].4, LiftMode::Vector);
        assert_eq!(applies[1].0.cardinality, Some(Cardinality::StaticVector(2)));
        assert_eq!(applies[1].5.static_anchor, Some(0));
        assert_eq!(applies[2].4, LiftMode::DynamicVector);
        assert_eq!(applies[2].0.cardinality, Some(Cardinality::DynamicVector));
        assert_ne!(raw.edges[0].origin, raw.nodes[2].origin);
    }

    #[test]
    fn prefix_spread_preserves_immediate_element_metadata() {
        let program = must_compile("add [1 (2 3)]\n");
        let raw = program.as_raw();
        let prepare = raw
            .nodes
            .iter()
            .position(|node| matches!(node.kind, NodeKind::PrefixSpreadPrepare));
        let apply = raw
            .nodes
            .iter()
            .position(|node| matches!(node.kind, NodeKind::SelectedApply { .. }));
        assert_eq!(prepare, Some(3));
        assert_eq!(apply, Some(4));
        assert_eq!(raw.edges[3].cardinality, Some(Cardinality::StaticScalar));
        assert_eq!(raw.edges[4].cardinality, Some(Cardinality::StaticVector(2)));
        assert_ne!(raw.edges[3].origin, raw.edges[4].origin);
    }

    #[test]
    fn fan_out_substitutes_placeholder_type_and_records_regions_and_borrows() {
        let program = must_compile("fanout[iota[3] {inc[_]} {add[_ 10]}]\n");
        let raw = program.as_raw();
        assert_eq!(
            raw.features,
            vec![
                Feature::StableSemanticIds.numeric(),
                Feature::Tuples.numeric(),
                Feature::FanOut.numeric(),
            ]
        );
        assert_eq!(raw.branches.len(), 2);
        assert_eq!(raw.branches[0].nodes, IndexRange { start: 2, count: 1 });
        assert_eq!(raw.branches[1].nodes, IndexRange { start: 3, count: 2 });
        assert_eq!(raw.edges[1].access, ValueAccess::FanOutOperandBorrow);
        assert_eq!(raw.edges[2].access, ValueAccess::FanOutOperandBorrow);
        assert_eq!(raw.edges[1].ownership, OwnershipMode::ImmutableBorrow);
        assert_eq!(raw.nodes[2].cardinality, Some(Cardinality::DynamicVector));
        assert_eq!(raw.nodes[4].cardinality, Some(Cardinality::DynamicVector));
        assert_eq!(raw.shape_checks, vec![1, 2]);
        let NodeKind::SelectedApply { shape: first, .. } = raw.nodes[2].kind else {
            panic!("first branch is not an application");
        };
        let NodeKind::SelectedApply { shape: second, .. } = raw.nodes[4].kind else {
            panic!("second branch is not an application");
        };
        assert_eq!(first.static_anchor, None);
        assert_eq!(first.dynamic_checks, IndexRange { start: 0, count: 1 });
        assert_eq!(second.static_anchor, None);
        assert_eq!(second.dynamic_checks, IndexRange { start: 1, count: 1 });
        assert_ne!(
            raw.branches[0].placeholder_origin,
            raw.branches[1].placeholder_origin
        );

        let mixed = must_compile("add[(1 2) iota[2]]\n");
        let mixed_raw = mixed.as_raw();
        let Some(mixed_apply) = mixed_raw.nodes.iter().find(|node| {
            matches!(
                node.kind,
                NodeKind::SelectedApply {
                    primitive_id: 5,
                    ..
                }
            )
        }) else {
            panic!("missing mixed-shape add");
        };
        let NodeKind::SelectedApply { shape, .. } = mixed_apply.kind else {
            panic!("mixed-shape node has wrong kind");
        };
        assert_eq!(shape.static_anchor, Some(0));
        assert_eq!(shape.dynamic_checks, IndexRange { start: 0, count: 1 });
        assert_eq!(mixed_raw.shape_checks, vec![2]);
    }

    #[test]
    fn fan_out_prefix_placeholder_borrows_prepare_and_preserves_elements() {
        let program = must_compile("fanout[[1 (2 3)] {add _}]\n");
        let raw = program.as_raw();
        let prepare_index = raw
            .nodes
            .iter()
            .position(|node| matches!(node.kind, NodeKind::PrefixSpreadPrepare));
        let Some(prepare_index) = prepare_index else {
            panic!("missing prefix prepare");
        };
        let prepare = raw.nodes[prepare_index];
        let prepare_edge = raw.edges[prepare.edges.start as usize];
        assert_eq!(prepare_edge.access, ValueAccess::FanOutOperandBorrow);
        assert_eq!(prepare_edge.ownership, OwnershipMode::ImmutableBorrow);
        let apply = raw.nodes.iter().find(|node| {
            matches!(
                node.kind,
                NodeKind::SelectedApply {
                    primitive_id: 5,
                    ..
                }
            )
        });
        let Some(apply) = apply else {
            panic!("missing add application");
        };
        let first = raw.edges[apply.edges.start as usize];
        let second = raw.edges[apply.edges.start as usize + 1];
        assert_eq!(first.cardinality, Some(Cardinality::StaticScalar));
        assert_eq!(second.cardinality, Some(Cardinality::StaticVector(2)));
        assert_ne!(first.origin, second.origin);
    }

    #[test]
    fn deep_tuple_lowers_iteratively_to_fully_expanded_records() {
        let join = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let depth = 4_000;
                let mut source = String::new();
                source.reserve(depth * 2 + 2);
                for _ in 0..depth {
                    source.push('[');
                }
                source.push('1');
                for _ in 0..depth {
                    source.push(']');
                }
                let program = must_compile(&source);
                assert_eq!(program.as_raw().nodes.len(), depth + 1);
                assert_eq!(program.as_raw().types.len(), depth + 1);
                assert_eq!(
                    program
                        .as_raw()
                        .types
                        .iter()
                        .filter(|record| matches!(record, TypeRecord::Tuple { .. }))
                        .count(),
                    depth
                );

                let unary_depth = 4_000;
                let mut unary = String::new();
                unary.reserve(unary_depth * 4 + 2);
                for _ in 0..unary_depth {
                    unary.push_str("inc ");
                }
                unary.push('1');
                let unary_program = must_compile(&unary);
                assert_eq!(unary_program.as_raw().nodes.len(), unary_depth + 1);
                assert_eq!(
                    unary_program
                        .as_raw()
                        .nodes
                        .iter()
                        .filter(|node| matches!(
                            node.kind,
                            NodeKind::SelectedApply {
                                implementation_id: 1,
                                ..
                            }
                        ))
                        .count(),
                    unary_depth
                );
            });
        let Ok(join) = join else {
            panic!("failed to spawn reduced-stack lowering thread");
        };
        assert!(join.join().is_ok());
    }

    #[test]
    fn allocation_refusal_reaches_every_lowering_arena() {
        let source = "parameters[x Int]\n\
                      add [x 1]\n\
                      inc[(1 2)]\n\
                      fanout[iota[x] {add[_ iota[1]]}]\n";
        let program = match parse(source) {
            Ok(program) => program,
            Err(error) => panic!("parse failed: {error:?}"),
        };
        let mut seen = Vec::new();
        for ordinal in 0..1_000 {
            match lower_program_with_builder(
                source,
                "<source>",
                &program,
                RawProgramBuilder::with_reservation_failure_at(ordinal),
            ) {
                Err(LowerError::Build(BuildError::AllocationUnavailable { arena })) => {
                    if !seen.contains(&arena) {
                        seen.push(arena);
                    }
                }
                Ok(_) => break,
                Err(error) => panic!("unexpected lowering result: {error:?}"),
            }
        }
        let expected = [
            crate::Arena::Feature,
            crate::Arena::SourceUnit,
            crate::Arena::Parameter,
            crate::Arena::Type,
            crate::Arena::TypeElement,
            crate::Arena::Constant,
            crate::Arena::ConstantElement,
            crate::Arena::Node,
            crate::Arena::Edge,
            crate::Arena::ShapeCheck,
            crate::Arena::Origin,
            crate::Arena::Branch,
            crate::Arena::Ownership,
            crate::Arena::Root,
        ];
        for arena in expected {
            assert!(seen.contains(&arena), "missing injected arena {arena:?}");
        }
        assert_eq!(seen.len(), expected.len());
    }

    #[test]
    fn lowering_materializes_the_only_typed_selection_decisions() {
        let program = must_compile(
            "equals[true false]\n\
             equals[1 2]\n\
             equals[1 2.0]\n\
             equals [1.0 2]\n\
             iota[3]\n",
        );
        let selected: Vec<(u16, u16, u16)> = program
            .as_raw()
            .nodes
            .iter()
            .filter_map(|node| match node.kind {
                NodeKind::SelectedApply {
                    primitive_id,
                    signature_id,
                    implementation_id,
                    ..
                } => Some((primitive_id, signature_id, implementation_id)),
                _ => None,
            })
            .collect();
        assert_eq!(
            selected,
            vec![
                (8, 15, 15),
                (8, 16, 16),
                (8, 17, 17),
                (8, 17, 17),
                (19, 34, 34),
            ]
        );

        let fan_out_spread = must_compile("add fanout[1 {inc[_]} {inc[_]}]\n");
        assert!(fan_out_spread.as_raw().nodes.iter().any(|node| matches!(
            node.kind,
            NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                ..
            }
        )));

        let rejected = must_source_error("iota[(1 2)]\n");
        assert_eq!(rejected.kind, ErrorKind::TypeError);
        assert_eq!(rejected.argument_position, Some(1));
        assert_eq!(rejected.actual_types, vec![Type::Vector(ScalarType::Int)]);
        assert_eq!(
            rejected.message,
            "iota arguments do not match an accepted signature; first unsupported argument is 1"
        );

        let deep_tuple = must_source_error("inc [[[[1]]]]\n");
        assert_eq!(deep_tuple.kind, ErrorKind::TypeError);
        assert_eq!(deep_tuple.argument_position, Some(1));
    }

    #[test]
    fn unsupported_signature_diagnostic_allocation_refusal_is_explicit() {
        let source = "inc[true]\n";
        let program = match parse(source) {
            Ok(program) => program,
            Err(error) => panic!("parse failed: {error:?}"),
        };
        let result = lower_program_with_builder_and_diagnostics(
            source,
            "<source>",
            &program,
            RawProgramBuilder::new(),
            DiagnosticReservations { refuse_next: true },
        );
        assert!(matches!(
            result,
            Err(LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Type,
            }))
        ));
    }

    #[test]
    fn iota_type_diagnostic_allocation_refusal_is_explicit() {
        let source = "iota[(1 2)]\n";
        let program = match parse(source) {
            Ok(program) => program,
            Err(error) => panic!("parse failed: {error:?}"),
        };
        let result = lower_program_with_builder_and_diagnostics(
            source,
            "<source>",
            &program,
            RawProgramBuilder::new(),
            DiagnosticReservations { refuse_next: true },
        );
        assert!(matches!(
            result,
            Err(LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::Type,
            }))
        ));
    }

    #[test]
    fn static_shape_diagnostic_allocation_refusal_is_explicit() {
        let source = "add[(1 2) (3 4 5)]\n";
        let program = match parse(source) {
            Ok(program) => program,
            Err(error) => panic!("parse failed: {error:?}"),
        };
        let result = lower_program_with_builder_and_diagnostics(
            source,
            "<source>",
            &program,
            RawProgramBuilder::new(),
            DiagnosticReservations { refuse_next: true },
        );
        assert!(matches!(
            result,
            Err(LowerError::Build(BuildError::AllocationUnavailable {
                arena: crate::Arena::ShapeCheck,
            }))
        ));
    }

    #[test]
    fn whole_program_static_precedence_is_arity_then_type_then_shape() {
        let cross_root = must_source_error("inc[true]\nadd[1]\n");
        assert_eq!(cross_root.kind, ErrorKind::ArityError);
        assert_eq!(cross_root.location.line, 2);

        let unary_cross_root = must_source_error("inc[true]\nadd 1\n");
        assert_eq!(unary_cross_root.kind, ErrorKind::ArityError);
        assert_eq!(unary_cross_root.location.line, 2);

        let parent = must_source_error("add[inc[true]]\n");
        assert_eq!(parent.kind, ErrorKind::ArityError);
        assert_eq!(parent.location.offset, 1);

        let fan_out = must_source_error("fanout[1 {add[_ true]} {add[_]}]\n");
        assert_eq!(fan_out.kind, ErrorKind::ArityError);
        assert_eq!(fan_out.location.column, 25);

        let type_before_shape = must_source_error("add[(1 2) (3 4 5)]\ninc[true]\n");
        assert_eq!(type_before_shape.kind, ErrorKind::TypeError);
        assert_eq!(type_before_shape.location.line, 2);

        let arity_before_shape = must_source_error("add[(1 2) (3 4 5)]\nadd[1]\n");
        assert_eq!(arity_before_shape.kind, ErrorKind::ArityError);
        assert_eq!(arity_before_shape.location.line, 2);

        let shape = must_source_error("add[(1 2) (3 4 5)]\n");
        assert_eq!(shape.kind, ErrorKind::ShapeMismatch);
        assert_eq!(shape.location.column, 11);
        assert_eq!(shape.argument_position, Some(2));
        assert_eq!(shape.expected_shape, Some(vec![2]));
        assert_eq!(shape.actual_shape, Some(vec![3]));
    }

    #[test]
    fn invalid_source_returns_original_diagnostic_and_no_program() {
        let duplicate = must_source_error("parameters[x Int x Double]\nx\n");
        assert_eq!(duplicate.kind, ErrorKind::ParameterError);
        assert_eq!(
            duplicate.parameter.as_ref().map(|context| context.reason),
            Some(crate::ParameterErrorReason::DuplicateParameterName)
        );

        let unknown = must_source_error("add[]\nmissing[1]\n");
        assert_eq!(unknown.kind, ErrorKind::UnknownPrimitive);
        assert_eq!(unknown.location.line, 2);
    }

    #[test]
    fn exact_ir_golden_digests_cover_every_source_construct() {
        let matrix = must_compile(
            "parameters[x Int]\n\
             true\n\
             Int()\n\
             (1 2)\n\
             [1 x]\n\
             inc[1]\n\
             add [1 2]\n\
             fanout[[1 2] {add _}]\n",
        );
        assert_eq!(debug_digest(&matrix), 16_420_725_918_700_611_398);

        let depth = 256;
        let mut deep = String::new();
        deep.reserve(depth * 2 + 2);
        for _ in 0..depth {
            deep.push('[');
        }
        deep.push('1');
        for _ in 0..depth {
            deep.push(']');
        }
        assert_eq!(
            debug_digest(&must_compile(&deep)),
            17_055_416_865_788_300_019
        );

        let mut unary = String::new();
        unary.reserve(depth * 4 + 2);
        for _ in 0..depth {
            unary.push_str("inc ");
        }
        unary.push('1');
        assert_eq!(
            debug_digest(&must_compile(&unary)),
            16_033_402_538_388_638_440
        );
    }

    #[test]
    fn shape_plan_and_static_shape_diagnostic_are_exact() {
        let program = must_compile("add[iota[2] (1 2)]\n");
        let raw = program.as_raw();
        let Some(apply) = raw.nodes.iter().find(|node| {
            matches!(
                node.kind,
                NodeKind::SelectedApply {
                    primitive_id: 5,
                    ..
                }
            )
        }) else {
            panic!("missing final add");
        };
        let NodeKind::SelectedApply { shape, .. } = apply.kind else {
            panic!("final add has wrong kind");
        };
        assert_eq!(shape.static_anchor, Some(1));
        assert_eq!(shape.dynamic_checks, IndexRange { start: 0, count: 1 });
        assert_eq!(raw.shape_checks, vec![1]);

        let mismatch = must_source_error("add[(1 2) (3 4 5)]\n");
        assert_eq!(mismatch.kind, ErrorKind::ShapeMismatch);
        assert_eq!(mismatch.primitive.as_deref(), Some("add"));
        assert_eq!(mismatch.argument_position, Some(2));
        assert_eq!(mismatch.expected_shape, Some(vec![2]));
        assert_eq!(mismatch.actual_shape, Some(vec![3]));
        assert_eq!(
            mismatch.location,
            SourceLocation {
                offset: 11,
                line: 1,
                column: 11,
            }
        );
        assert_eq!(mismatch.span, None);
    }
}
