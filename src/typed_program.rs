use crate::ScalarType;
use std::collections::TryReserveError;

pub const SUPPORTED_SEMANTIC_MAJOR: u16 = 1;
pub const SUPPORTED_SEMANTIC_MINOR: u16 = 1;

macro_rules! index_type {
    ($name:ident) => {
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        pub struct $name(pub u32);
    };
}

index_type!(SourceUnitIndex);
index_type!(ParameterIndex);
index_type!(TypeIndex);
index_type!(ConstantIndex);
index_type!(NodeIndex);
index_type!(OriginIndex);
index_type!(RootIndex);

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct IndexRange {
    pub start: u32,
    pub count: u32,
}

impl IndexRange {
    pub fn checked_end(self) -> Option<u32> {
        self.start.checked_add(self.count)
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ProgramRanges {
    pub features: IndexRange,
    pub source_units: IndexRange,
    pub parameters: IndexRange,
    pub types: IndexRange,
    pub type_elements: IndexRange,
    pub constants: IndexRange,
    pub constant_elements: IndexRange,
    pub nodes: IndexRange,
    pub edges: IndexRange,
    pub shape_checks: IndexRange,
    pub origins: IndexRange,
    pub branches: IndexRange,
    pub ownership: IndexRange,
    pub roots: IndexRange,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModuleMetadata {
    pub semantic_major: u16,
    pub semantic_minor: u16,
    pub parameter_header_origin: Option<OriginIndex>,
    pub ranges: ProgramRanges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum Feature {
    StableSemanticIds = 1,
    Tuples = 2,
    PrefixSpread = 3,
    FanOut = 4,
    BackendNativeMathV1 = 7,
}

impl Feature {
    pub const fn numeric(self) -> u16 {
        self as u16
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct SourceUnit {
    pub diagnostic_name: String,
    pub byte_length: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginPosition {
    pub offset: u32,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginSpan {
    pub begin: OriginPosition,
    pub end: OriginPosition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Origin {
    pub source_unit: SourceUnitIndex,
    pub span: OriginSpan,
}

#[derive(Debug, Eq, PartialEq)]
pub struct Parameter {
    pub slot: u32,
    pub name: String,
    pub scalar_type: ScalarType,
    pub declaration_origin: OriginIndex,
    pub name_origin: OriginIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeRecord {
    Scalar(ScalarType),
    Vector(ScalarType),
    Tuple { elements: IndexRange },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ScalarConstant {
    Bool(bool),
    Int(i64),
    DoubleBits(u64),
}

impl Eq for ScalarConstant {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConstantRecord {
    Scalar(ScalarConstant),
    Vector {
        element_type: ScalarType,
        elements: IndexRange,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Cardinality {
    StaticScalar,
    StaticVector(u32),
    DynamicVector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Conversion {
    Identity,
    PromoteIntToDouble,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ValueAccess {
    WholeValue,
    TupleElement(u32),
    FanOutOperandBorrow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnershipMode {
    OwnedInput,
    ImmutableBorrow,
    InfallibleTransfer,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Edge {
    pub producer: NodeIndex,
    pub argument_position: u32,
    pub access: ValueAccess,
    pub cardinality: Option<Cardinality>,
    pub conversion: Conversion,
    pub ownership: OwnershipMode,
    pub origin: OriginIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LiftMode {
    Scalar,
    Vector,
    DynamicVector,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ShapePlan {
    pub static_anchor: Option<u32>,
    pub dynamic_checks: IndexRange,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NodeKind {
    Constant {
        constant: ConstantIndex,
    },
    ParameterBorrow {
        parameter: ParameterIndex,
    },
    TupleConstruct,
    SelectedApply {
        primitive_id: u16,
        signature_id: u16,
        implementation_id: u16,
        primitive_origin: OriginIndex,
        lift: LiftMode,
        result_element_type: ScalarType,
        shape: ShapePlan,
    },
    PrefixSpreadPrepare,
    FanOut {
        branches: IndexRange,
        keyword_origin: OriginIndex,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub result_type: TypeIndex,
    pub cardinality: Option<Cardinality>,
    pub edges: IndexRange,
    pub origin: OriginIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FanOutBranch {
    pub nodes: IndexRange,
    pub root: NodeIndex,
    pub placeholder_origin: OriginIndex,
    pub origin: OriginIndex,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReleaseAfter {
    Node(NodeIndex),
    Root(RootIndex),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Ownership {
    pub owner: NodeIndex,
    pub release_after: ReleaseAfter,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Root {
    pub node: NodeIndex,
    pub origin: OriginIndex,
}

#[derive(Debug, Eq, PartialEq)]
pub struct RawProgram {
    pub module: ModuleMetadata,
    pub features: Vec<u16>,
    pub source_units: Vec<SourceUnit>,
    pub parameters: Vec<Parameter>,
    pub types: Vec<TypeRecord>,
    pub type_elements: Vec<TypeIndex>,
    pub constants: Vec<ConstantRecord>,
    pub constant_elements: Vec<ScalarConstant>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub shape_checks: Vec<u32>,
    pub origins: Vec<Origin>,
    pub branches: Vec<FanOutBranch>,
    pub ownership: Vec<Ownership>,
    pub roots: Vec<Root>,
}

impl RawProgram {
    pub fn verify(self) -> Result<VerifiedProgram, VerifyError> {
        verify_program(&self, VerifyAllocationFailureInjection::none())?;
        Ok(VerifiedProgram { raw: self })
    }

    pub fn verify_with_allocation_failure(
        self,
        injection: VerifyAllocationFailureInjection,
    ) -> Result<VerifiedProgram, VerifyError> {
        verify_program(&self, injection)?;
        Ok(VerifiedProgram { raw: self })
    }
}

#[derive(Debug, Eq, PartialEq)]
pub struct VerifiedProgram {
    raw: RawProgram,
}

impl VerifiedProgram {
    pub fn as_raw(&self) -> &RawProgram {
        &self.raw
    }

    pub fn module(&self) -> &ModuleMetadata {
        &self.raw.module
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecordKind {
    Module,
    Feature,
    SourceUnit,
    Parameter,
    Type,
    TypeElement,
    Constant,
    ConstantElement,
    Origin,
    Node,
    Edge,
    ShapeCheck,
    Branch,
    Ownership,
    Root,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Invariant {
    UnsupportedVersion,
    UnknownFeature,
    DuplicateFeature,
    RangeOverflow,
    RangeMismatch,
    IndexOutOfBounds,
    InvalidRecord,
    NonPostorderReference,
    UnreachableNode,
    AmbiguousOwnership,
    InvalidSemanticIdentity,
    InconsistentResultMetadata,
    MissingFeature,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MalformedProgram {
    pub invariant: Invariant,
    pub record: RecordKind,
    pub index: Option<u32>,
    pub field: &'static str,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum VerifyAllocationSite {
    DynamicShapeScratch,
    ReachabilityBits,
    ReachabilityWorklist,
    FanOutBorrowContext,
    OwnershipSinks,
    OwnershipLastUse,
    OwnershipRootOwner,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct VerifyAllocationFailureInjection {
    fail_at: Option<VerifyAllocationSite>,
}

impl VerifyAllocationFailureInjection {
    pub const fn none() -> Self {
        Self { fail_at: None }
    }

    pub const fn at(site: VerifyAllocationSite) -> Self {
        Self {
            fail_at: Some(site),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerifyError {
    MalformedProgram(MalformedProgram),
    AllocationUnavailable { site: VerifyAllocationSite },
}

impl std::fmt::Display for VerifyError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MalformedProgram(error) => write!(
                formatter,
                "malformed program: {:?} at {:?} {:?}.{}",
                error.invariant, error.record, error.index, error.field
            ),
            Self::AllocationUnavailable { site } => {
                write!(
                    formatter,
                    "typed-program verification allocation unavailable at {site:?}"
                )
            }
        }
    }
}

impl std::error::Error for VerifyError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Arena {
    Feature,
    SourceUnit,
    Parameter,
    Type,
    TypeElement,
    Constant,
    ConstantElement,
    Node,
    Edge,
    ShapeCheck,
    Origin,
    Branch,
    Ownership,
    Root,
}

#[derive(Debug)]
pub enum BuildError {
    CountOverflow { arena: Arena },
    AllocationUnavailable { arena: Arena },
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "typed-program build failed: {self:?}")
    }
}

impl std::error::Error for BuildError {}

fn reserve_one<T>(
    values: &mut Vec<T>,
    arena: Arena,
    reservation_attempts: &mut u32,
    fail_at_reservation: Option<u32>,
) -> Result<u32, BuildError> {
    let index = u32::try_from(values.len()).map_err(|_| BuildError::CountOverflow { arena })?;
    let attempt = *reservation_attempts;
    *reservation_attempts = reservation_attempts
        .checked_add(1)
        .ok_or(BuildError::CountOverflow { arena })?;
    if fail_at_reservation == Some(attempt) {
        return Err(BuildError::AllocationUnavailable { arena });
    }
    values
        .try_reserve(1)
        .map_err(|_: TryReserveError| BuildError::AllocationUnavailable { arena })?;
    Ok(index)
}

#[derive(Debug)]
pub struct RawProgramBuilder {
    raw: RawProgram,
    reservation_attempts: u32,
    fail_at_reservation: Option<u32>,
}

macro_rules! push_method {
    ($name:ident, $field:ident, $value:ty, $arena:ident, $index:ident) => {
        pub fn $name(&mut self, value: $value) -> Result<$index, BuildError> {
            let index = reserve_one(
                &mut self.raw.$field,
                Arena::$arena,
                &mut self.reservation_attempts,
                self.fail_at_reservation,
            )?;
            self.raw.$field.push(value);
            Ok($index(index))
        }
    };
    ($name:ident, $field:ident, $value:ty, $arena:ident) => {
        pub fn $name(&mut self, value: $value) -> Result<u32, BuildError> {
            let index = reserve_one(
                &mut self.raw.$field,
                Arena::$arena,
                &mut self.reservation_attempts,
                self.fail_at_reservation,
            )?;
            self.raw.$field.push(value);
            Ok(index)
        }
    };
}

impl RawProgramBuilder {
    pub fn new() -> Self {
        Self {
            raw: RawProgram {
                module: ModuleMetadata {
                    semantic_major: SUPPORTED_SEMANTIC_MAJOR,
                    semantic_minor: 0,
                    parameter_header_origin: None,
                    ranges: ProgramRanges::default(),
                },
                features: Vec::new(),
                source_units: Vec::new(),
                parameters: Vec::new(),
                types: Vec::new(),
                type_elements: Vec::new(),
                constants: Vec::new(),
                constant_elements: Vec::new(),
                nodes: Vec::new(),
                edges: Vec::new(),
                shape_checks: Vec::new(),
                origins: Vec::new(),
                branches: Vec::new(),
                ownership: Vec::new(),
                roots: Vec::new(),
            },
            reservation_attempts: 0,
            fail_at_reservation: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_reservation_failure_at(ordinal: u32) -> Self {
        let mut builder = Self::new();
        builder.fail_at_reservation = Some(ordinal);
        builder
    }

    pub fn set_parameter_header_origin(&mut self, origin: OriginIndex) {
        self.raw.module.parameter_header_origin = Some(origin);
    }

    pub(crate) fn set_semantic_minor(&mut self, semantic_minor: u16) {
        self.raw.module.semantic_minor = semantic_minor;
    }

    pub(crate) fn finish_preview_type_elements(&self) -> Result<u32, BuildError> {
        checked_count(self.raw.type_elements.len() as u64, Arena::TypeElement)
    }

    pub(crate) fn finish_preview_constant_elements(&self) -> Result<u32, BuildError> {
        checked_count(
            self.raw.constant_elements.len() as u64,
            Arena::ConstantElement,
        )
    }

    pub(crate) fn finish_preview_edges(&self) -> Result<u32, BuildError> {
        checked_count(self.raw.edges.len() as u64, Arena::Edge)
    }

    pub(crate) fn finish_preview_shape_checks(&self) -> Result<u32, BuildError> {
        checked_count(self.raw.shape_checks.len() as u64, Arena::ShapeCheck)
    }

    pub(crate) fn finish_preview_nodes(&self) -> Result<u32, BuildError> {
        checked_count(self.raw.nodes.len() as u64, Arena::Node)
    }

    pub(crate) fn finish_preview_branches(&self) -> Result<u32, BuildError> {
        checked_count(self.raw.branches.len() as u64, Arena::Branch)
    }

    push_method!(push_feature, features, u16, Feature);
    push_method!(
        push_source_unit,
        source_units,
        SourceUnit,
        SourceUnit,
        SourceUnitIndex
    );
    push_method!(
        push_parameter,
        parameters,
        Parameter,
        Parameter,
        ParameterIndex
    );
    push_method!(push_type, types, TypeRecord, Type, TypeIndex);
    push_method!(push_type_element, type_elements, TypeIndex, TypeElement);
    push_method!(
        push_constant,
        constants,
        ConstantRecord,
        Constant,
        ConstantIndex
    );
    push_method!(
        push_constant_element,
        constant_elements,
        ScalarConstant,
        ConstantElement
    );
    push_method!(push_node, nodes, Node, Node, NodeIndex);
    push_method!(push_edge, edges, Edge, Edge);
    push_method!(push_shape_check, shape_checks, u32, ShapeCheck);
    push_method!(push_origin, origins, Origin, Origin, OriginIndex);
    push_method!(push_branch, branches, FanOutBranch, Branch);
    push_method!(push_ownership, ownership, Ownership, Ownership);
    push_method!(push_root, roots, Root, Root, RootIndex);

    pub fn finish(mut self) -> Result<RawProgram, BuildError> {
        self.raw.module.ranges = ProgramRanges {
            features: whole_range(self.raw.features.len(), Arena::Feature)?,
            source_units: whole_range(self.raw.source_units.len(), Arena::SourceUnit)?,
            parameters: whole_range(self.raw.parameters.len(), Arena::Parameter)?,
            types: whole_range(self.raw.types.len(), Arena::Type)?,
            type_elements: whole_range(self.raw.type_elements.len(), Arena::TypeElement)?,
            constants: whole_range(self.raw.constants.len(), Arena::Constant)?,
            constant_elements: whole_range(
                self.raw.constant_elements.len(),
                Arena::ConstantElement,
            )?,
            nodes: whole_range(self.raw.nodes.len(), Arena::Node)?,
            edges: whole_range(self.raw.edges.len(), Arena::Edge)?,
            shape_checks: whole_range(self.raw.shape_checks.len(), Arena::ShapeCheck)?,
            origins: whole_range(self.raw.origins.len(), Arena::Origin)?,
            branches: whole_range(self.raw.branches.len(), Arena::Branch)?,
            ownership: whole_range(self.raw.ownership.len(), Arena::Ownership)?,
            roots: whole_range(self.raw.roots.len(), Arena::Root)?,
        };
        Ok(self.raw)
    }
}

impl Default for RawProgramBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn whole_range(length: usize, arena: Arena) -> Result<IndexRange, BuildError> {
    Ok(IndexRange {
        start: 0,
        count: checked_count(length as u64, arena)?,
    })
}

fn checked_count(length: u64, arena: Arena) -> Result<u32, BuildError> {
    u32::try_from(length).map_err(|_| BuildError::CountOverflow { arena })
}

fn verify_program(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    if program.module.semantic_major != SUPPORTED_SEMANTIC_MAJOR
        || program.module.semantic_minor > SUPPORTED_SEMANTIC_MINOR
    {
        return Err(malformed(
            Invariant::UnsupportedVersion,
            RecordKind::Module,
            None,
            "semantic_version",
        ));
    }
    let mut previous_feature = None;
    for (index, feature) in program.features.iter().copied().enumerate() {
        if ![
            Feature::StableSemanticIds.numeric(),
            Feature::Tuples.numeric(),
            Feature::PrefixSpread.numeric(),
            Feature::FanOut.numeric(),
            Feature::BackendNativeMathV1.numeric(),
        ]
        .contains(&feature)
        {
            return Err(malformed(
                Invariant::UnknownFeature,
                RecordKind::Feature,
                u32::try_from(index).ok(),
                "id",
            ));
        }
        if previous_feature.is_some_and(|previous| previous >= feature) {
            return Err(malformed(
                Invariant::DuplicateFeature,
                RecordKind::Feature,
                checked_index(index),
                "id",
            ));
        }
        previous_feature = Some(feature);
    }
    if program.module.semantic_minor == 0
        && program
            .features
            .binary_search(&Feature::BackendNativeMathV1.numeric())
            .is_ok()
    {
        return Err(malformed(
            Invariant::UnsupportedVersion,
            RecordKind::Module,
            None,
            "semantic_version",
        ));
    }
    verify_module_ranges(program)?;
    verify_parameters(program)?;
    verify_types(program)?;
    verify_constants(program)?;
    verify_sources_and_origins(program)?;
    verify_node_and_edge_references(program)?;
    verify_reachability(program, injection)?;
    verify_node_metadata(program, injection)?;
    verify_semantic_ownership(program, injection)?;
    verify_roots_and_features(program)?;
    Ok(())
}

fn checked_index(index: usize) -> Option<u32> {
    u32::try_from(index).ok()
}

fn malformed(
    invariant: Invariant,
    record: RecordKind,
    index: Option<u32>,
    field: &'static str,
) -> VerifyError {
    VerifyError::MalformedProgram(MalformedProgram {
        invariant,
        record,
        index,
        field,
    })
}

fn allocation_error(site: VerifyAllocationSite) -> VerifyError {
    VerifyError::AllocationUnavailable { site }
}

fn injected(
    injection: VerifyAllocationFailureInjection,
    site: VerifyAllocationSite,
) -> Result<(), VerifyError> {
    if injection.fail_at == Some(site) {
        Err(allocation_error(site))
    } else {
        Ok(())
    }
}

fn exact_range(
    range: IndexRange,
    length: usize,
    record: RecordKind,
    field: &'static str,
) -> Result<(), VerifyError> {
    let expected = u32::try_from(length)
        .map_err(|_| malformed(Invariant::RangeOverflow, record, None, field))?;
    let end = range
        .checked_end()
        .ok_or_else(|| malformed(Invariant::RangeOverflow, record, None, field))?;
    if range.start != 0 || end != expected {
        return Err(malformed(Invariant::RangeMismatch, record, None, field));
    }
    Ok(())
}

fn verify_module_ranges(program: &RawProgram) -> Result<(), VerifyError> {
    let ranges = &program.module.ranges;
    exact_range(
        ranges.features,
        program.features.len(),
        RecordKind::Module,
        "ranges.features",
    )?;
    exact_range(
        ranges.source_units,
        program.source_units.len(),
        RecordKind::Module,
        "ranges.source_units",
    )?;
    exact_range(
        ranges.parameters,
        program.parameters.len(),
        RecordKind::Module,
        "ranges.parameters",
    )?;
    exact_range(
        ranges.types,
        program.types.len(),
        RecordKind::Module,
        "ranges.types",
    )?;
    exact_range(
        ranges.type_elements,
        program.type_elements.len(),
        RecordKind::Module,
        "ranges.type_elements",
    )?;
    exact_range(
        ranges.constants,
        program.constants.len(),
        RecordKind::Module,
        "ranges.constants",
    )?;
    exact_range(
        ranges.constant_elements,
        program.constant_elements.len(),
        RecordKind::Module,
        "ranges.constant_elements",
    )?;
    exact_range(
        ranges.nodes,
        program.nodes.len(),
        RecordKind::Module,
        "ranges.nodes",
    )?;
    exact_range(
        ranges.edges,
        program.edges.len(),
        RecordKind::Module,
        "ranges.edges",
    )?;
    exact_range(
        ranges.shape_checks,
        program.shape_checks.len(),
        RecordKind::Module,
        "ranges.shape_checks",
    )?;
    exact_range(
        ranges.origins,
        program.origins.len(),
        RecordKind::Module,
        "ranges.origins",
    )?;
    exact_range(
        ranges.branches,
        program.branches.len(),
        RecordKind::Module,
        "ranges.branches",
    )?;
    exact_range(
        ranges.ownership,
        program.ownership.len(),
        RecordKind::Module,
        "ranges.ownership",
    )?;
    exact_range(
        ranges.roots,
        program.roots.len(),
        RecordKind::Module,
        "ranges.roots",
    )
}

fn in_bounds(index: u32, length: usize) -> bool {
    usize::try_from(index).is_ok_and(|index| index < length)
}

fn range_bounds(
    range: IndexRange,
    length: usize,
    record: RecordKind,
    index: u32,
    field: &'static str,
) -> Result<std::ops::Range<usize>, VerifyError> {
    let end = range
        .checked_end()
        .ok_or_else(|| malformed(Invariant::RangeOverflow, record, Some(index), field))?;
    let start = usize::try_from(range.start)
        .map_err(|_| malformed(Invariant::RangeOverflow, record, Some(index), field))?;
    let end = usize::try_from(end)
        .map_err(|_| malformed(Invariant::RangeOverflow, record, Some(index), field))?;
    if end > length {
        return Err(malformed(
            Invariant::IndexOutOfBounds,
            record,
            Some(index),
            field,
        ));
    }
    Ok(start..end)
}

fn verify_sources_and_origins(program: &RawProgram) -> Result<(), VerifyError> {
    for (index, source) in program.source_units.iter().enumerate() {
        if source.diagnostic_name.is_empty() {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::SourceUnit,
                checked_index(index),
                "diagnostic_name",
            ));
        }
    }
    for (index, origin) in program.origins.iter().enumerate() {
        let record_index = checked_index(index);
        if !in_bounds(origin.source_unit.0, program.source_units.len()) {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Origin,
                record_index,
                "source_unit",
            ));
        }
        let begin = origin.span.begin;
        let end = origin.span.end;
        let source = &program.source_units[origin.source_unit.0 as usize];
        let source_end = source.byte_length.checked_add(1).ok_or_else(|| {
            malformed(
                Invariant::RangeOverflow,
                RecordKind::SourceUnit,
                Some(origin.source_unit.0),
                "byte_length",
            )
        })?;
        if begin.offset == 0
            || begin.line == 0
            || begin.column == 0
            || end.offset == 0
            || end.line == 0
            || end.column == 0
            || begin.offset > end.offset
            || end.offset > source_end
            || end.line < begin.line
            || (end.line == begin.line && end.column < begin.column)
            || (begin.offset == end.offset
                && (begin.line != end.line || begin.column != end.column))
        {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Origin,
                record_index,
                "span",
            ));
        }
    }
    Ok(())
}

fn verify_parameters(program: &RawProgram) -> Result<(), VerifyError> {
    match (
        program.parameters.is_empty(),
        program.module.parameter_header_origin,
    ) {
        (true, None) => {}
        (_, Some(origin)) if in_bounds(origin.0, program.origins.len()) => {}
        (_, Some(_)) => {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Module,
                None,
                "parameter_header_origin",
            ));
        }
        (false, None) => {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Module,
                None,
                "parameter_header_origin",
            ));
        }
    }
    for (index, parameter) in program.parameters.iter().enumerate() {
        let record_index = checked_index(index);
        if parameter.slot != u32::try_from(index).unwrap_or(u32::MAX) {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Parameter,
                record_index,
                "slot",
            ));
        }
        if parameter.name.is_empty() {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Parameter,
                record_index,
                "name",
            ));
        }
        for (field, origin) in [
            ("declaration_origin", parameter.declaration_origin),
            ("name_origin", parameter.name_origin),
        ] {
            if !in_bounds(origin.0, program.origins.len()) {
                return Err(malformed(
                    Invariant::IndexOutOfBounds,
                    RecordKind::Parameter,
                    record_index,
                    field,
                ));
            }
        }
    }
    Ok(())
}

fn verify_types(program: &RawProgram) -> Result<(), VerifyError> {
    let mut next_element = 0_u32;
    for (index, record) in program.types.iter().copied().enumerate() {
        let record_index = u32::try_from(index).unwrap_or(u32::MAX);
        if let TypeRecord::Tuple { elements } = record {
            if elements.start != next_element {
                return Err(malformed(
                    Invariant::RangeMismatch,
                    RecordKind::Type,
                    Some(record_index),
                    "elements",
                ));
            }
            let bounds = range_bounds(
                elements,
                program.type_elements.len(),
                RecordKind::Type,
                record_index,
                "elements",
            )?;
            for element in &program.type_elements[bounds] {
                if element.0 >= record_index {
                    return Err(malformed(
                        Invariant::NonPostorderReference,
                        RecordKind::Type,
                        Some(record_index),
                        "elements",
                    ));
                }
            }
            next_element = elements.checked_end().ok_or_else(|| {
                malformed(
                    Invariant::RangeOverflow,
                    RecordKind::Type,
                    Some(record_index),
                    "elements",
                )
            })?;
        }
    }
    if usize::try_from(next_element).ok() != Some(program.type_elements.len()) {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::TypeElement,
            None,
            "owner",
        ));
    }
    Ok(())
}

fn scalar_kind(constant: ScalarConstant) -> ScalarType {
    match constant {
        ScalarConstant::Bool(_) => ScalarType::Bool,
        ScalarConstant::Int(_) => ScalarType::Int,
        ScalarConstant::DoubleBits(_) => ScalarType::Double,
    }
}

fn canonical_constant(constant: ScalarConstant) -> bool {
    let ScalarConstant::DoubleBits(bits) = constant else {
        return true;
    };
    let is_nan =
        bits & 0x7ff0_0000_0000_0000 == 0x7ff0_0000_0000_0000 && bits & 0x000f_ffff_ffff_ffff != 0;
    !is_nan || bits == 0x7ff8_0000_0000_0000
}

fn verify_constants(program: &RawProgram) -> Result<(), VerifyError> {
    let mut next_element = 0_u32;
    for (index, constant) in program.constants.iter().copied().enumerate() {
        let record_index = u32::try_from(index).unwrap_or(u32::MAX);
        match constant {
            ConstantRecord::Scalar(value) => {
                if !canonical_constant(value) {
                    return Err(malformed(
                        Invariant::InvalidRecord,
                        RecordKind::Constant,
                        Some(record_index),
                        "value",
                    ));
                }
            }
            ConstantRecord::Vector {
                element_type,
                elements,
            } => {
                if elements.start != next_element {
                    return Err(malformed(
                        Invariant::RangeMismatch,
                        RecordKind::Constant,
                        Some(record_index),
                        "elements",
                    ));
                }
                let bounds = range_bounds(
                    elements,
                    program.constant_elements.len(),
                    RecordKind::Constant,
                    record_index,
                    "elements",
                )?;
                for value in &program.constant_elements[bounds] {
                    if scalar_kind(*value) != element_type || !canonical_constant(*value) {
                        return Err(malformed(
                            Invariant::InvalidRecord,
                            RecordKind::Constant,
                            Some(record_index),
                            "elements",
                        ));
                    }
                }
                next_element = elements.checked_end().ok_or_else(|| {
                    malformed(
                        Invariant::RangeOverflow,
                        RecordKind::Constant,
                        Some(record_index),
                        "elements",
                    )
                })?;
            }
        }
    }
    if usize::try_from(next_element).ok() != Some(program.constant_elements.len()) {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::ConstantElement,
            None,
            "owner",
        ));
    }
    Ok(())
}

fn type_record(program: &RawProgram, index: TypeIndex) -> Option<TypeRecord> {
    usize::try_from(index.0)
        .ok()
        .and_then(|index| program.types.get(index))
        .copied()
}

fn edge_type(program: &RawProgram, edge: Edge) -> Option<TypeIndex> {
    let producer = usize::try_from(edge.producer.0)
        .ok()
        .and_then(|index| program.nodes.get(index))?;
    match edge.access {
        ValueAccess::WholeValue | ValueAccess::FanOutOperandBorrow => Some(producer.result_type),
        ValueAccess::TupleElement(element) => {
            let TypeRecord::Tuple { elements } = type_record(program, producer.result_type)? else {
                return None;
            };
            if element >= elements.count {
                return None;
            }
            let offset = elements.start.checked_add(element)?;
            usize::try_from(offset)
                .ok()
                .and_then(|index| program.type_elements.get(index))
                .copied()
        }
    }
}

fn cardinality_matches_type(
    program: &RawProgram,
    type_index: TypeIndex,
    cardinality: Option<Cardinality>,
) -> bool {
    match type_record(program, type_index) {
        Some(TypeRecord::Scalar(_)) => cardinality == Some(Cardinality::StaticScalar),
        Some(TypeRecord::Vector(_)) => matches!(
            cardinality,
            Some(Cardinality::StaticVector(_) | Cardinality::DynamicVector)
        ),
        Some(TypeRecord::Tuple { .. }) => cardinality.is_none(),
        None => false,
    }
}

fn tuple_element_cardinality(
    program: &RawProgram,
    mut prepared: NodeIndex,
    element: u32,
) -> Option<Option<Cardinality>> {
    loop {
        let node = program.nodes.get(prepared.0 as usize)?;
        if !matches!(node.kind, NodeKind::PrefixSpreadPrepare) {
            return None;
        }
        let prepared_edges = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            prepared.0,
            "edges",
        )
        .ok()?;
        let owner = program.edges.get(prepared_edges.start)?.producer;
        let owner_node = program.nodes.get(owner.0 as usize)?;
        match owner_node.kind {
            NodeKind::TupleConstruct => {
                let owner_edges = range_bounds(
                    owner_node.edges,
                    program.edges.len(),
                    RecordKind::Node,
                    owner.0,
                    "edges",
                )
                .ok()?;
                return program
                    .edges
                    .get(owner_edges.start.checked_add(element as usize)?)
                    .map(|edge| edge.cardinality);
            }
            NodeKind::FanOut { branches, .. } => {
                let branch = program
                    .branches
                    .get(branches.start.checked_add(element)? as usize)?;
                return program
                    .nodes
                    .get(branch.root.0 as usize)
                    .map(|root| root.cardinality);
            }
            NodeKind::PrefixSpreadPrepare => prepared = owner,
            _ => return None,
        }
    }
}

fn scalar_container(program: &RawProgram, index: TypeIndex) -> Option<(ScalarType, bool)> {
    match type_record(program, index)? {
        TypeRecord::Scalar(scalar) => Some((scalar, false)),
        TypeRecord::Vector(scalar) => Some((scalar, true)),
        TypeRecord::Tuple { .. } => None,
    }
}

fn verify_node_and_edge_references(program: &RawProgram) -> Result<(), VerifyError> {
    let mut next_edge = 0_u32;
    let mut next_shape_check = 0_u32;
    let mut next_branch = 0_u32;
    for (index, node) in program.nodes.iter().copied().enumerate() {
        let node_index = u32::try_from(index).unwrap_or(u32::MAX);
        if !in_bounds(node.result_type.0, program.types.len()) {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Node,
                Some(node_index),
                "result_type",
            ));
        }
        if !in_bounds(node.origin.0, program.origins.len()) {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Node,
                Some(node_index),
                "origin",
            ));
        }
        if node.edges.start != next_edge {
            return Err(malformed(
                Invariant::RangeMismatch,
                RecordKind::Node,
                Some(node_index),
                "edges",
            ));
        }
        let edge_bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            node_index,
            "edges",
        )?;
        let edges = &program.edges[edge_bounds];
        for (offset, edge) in edges.iter().copied().enumerate() {
            let edge_index = node.edges.start.saturating_add(offset as u32);
            if edge.producer.0 >= node_index {
                return Err(malformed(
                    Invariant::NonPostorderReference,
                    RecordKind::Edge,
                    Some(edge_index),
                    "producer",
                ));
            }
            if edge.argument_position != u32::try_from(offset + 1).unwrap_or(u32::MAX) {
                return Err(malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Edge,
                    Some(edge_index),
                    "argument_position",
                ));
            }
            if !in_bounds(edge.origin.0, program.origins.len()) {
                return Err(malformed(
                    Invariant::IndexOutOfBounds,
                    RecordKind::Edge,
                    Some(edge_index),
                    "origin",
                ));
            }
            if edge_type(program, edge).is_none() {
                return Err(malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Edge,
                    Some(edge_index),
                    "access",
                ));
            }
        }
        if matches!(node.kind, NodeKind::SelectedApply { .. }) {
            verify_prefix_spread_group(program, node_index, edges)?;
        }
        match node.kind {
            NodeKind::Constant { constant } => {
                if !in_bounds(constant.0, program.constants.len()) {
                    return Err(malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "constant",
                    ));
                }
            }
            NodeKind::ParameterBorrow { parameter } => {
                if !in_bounds(parameter.0, program.parameters.len()) {
                    return Err(malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "parameter",
                    ));
                }
            }
            NodeKind::SelectedApply {
                primitive_origin, ..
            } => {
                if !in_bounds(primitive_origin.0, program.origins.len()) {
                    return Err(malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "primitive_origin",
                    ));
                }
            }
            NodeKind::FanOut {
                branches,
                keyword_origin,
            } => {
                if !in_bounds(keyword_origin.0, program.origins.len()) {
                    return Err(malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "keyword_origin",
                    ));
                }
                verify_fan_out_references(program, node_index, branches)?;
            }
            NodeKind::TupleConstruct | NodeKind::PrefixSpreadPrepare => {}
        }
        next_edge = node.edges.checked_end().ok_or_else(|| {
            malformed(
                Invariant::RangeOverflow,
                RecordKind::Node,
                Some(node_index),
                "edges",
            )
        })?;
        if let NodeKind::SelectedApply { shape, .. } = node.kind {
            if shape.dynamic_checks.start != next_shape_check {
                return Err(malformed(
                    Invariant::RangeMismatch,
                    RecordKind::Node,
                    Some(node_index),
                    "shape.dynamic_checks",
                ));
            }
            next_shape_check = shape.dynamic_checks.checked_end().ok_or_else(|| {
                malformed(
                    Invariant::RangeOverflow,
                    RecordKind::Node,
                    Some(node_index),
                    "shape.dynamic_checks",
                )
            })?;
        }
        if let NodeKind::FanOut { branches, .. } = node.kind {
            if branches.start != next_branch {
                return Err(malformed(
                    Invariant::RangeMismatch,
                    RecordKind::Node,
                    Some(node_index),
                    "branches",
                ));
            }
            next_branch = branches.checked_end().ok_or_else(|| {
                malformed(
                    Invariant::RangeOverflow,
                    RecordKind::Node,
                    Some(node_index),
                    "branches",
                )
            })?;
        }
    }
    if usize::try_from(next_edge).ok() != Some(program.edges.len()) {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::Edge,
            None,
            "owner",
        ));
    }
    if usize::try_from(next_shape_check).ok() != Some(program.shape_checks.len()) {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::ShapeCheck,
            None,
            "shape_check.owner",
        ));
    }
    if usize::try_from(next_branch).ok() != Some(program.branches.len()) {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::Branch,
            None,
            "owner",
        ));
    }
    Ok(())
}

fn verify_node_metadata(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    for (index, node) in program.nodes.iter().copied().enumerate() {
        let node_index = u32::try_from(index).unwrap_or(u32::MAX);
        let bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            node_index,
            "edges",
        )?;
        for (offset, edge) in program.edges[bounds.clone()].iter().copied().enumerate() {
            let edge_index = node.edges.start.saturating_add(offset as u32);
            let edge_type = edge_type(program, edge).ok_or_else(|| {
                malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Edge,
                    Some(edge_index),
                    "access",
                )
            })?;
            let expected_cardinality = match edge.access {
                ValueAccess::TupleElement(element) => {
                    tuple_element_cardinality(program, edge.producer, element)
                }
                ValueAccess::WholeValue | ValueAccess::FanOutOperandBorrow => {
                    Some(program.nodes[edge.producer.0 as usize].cardinality)
                }
            };
            if !cardinality_matches_type(program, edge_type, edge.cardinality)
                || expected_cardinality != Some(edge.cardinality)
            {
                return Err(malformed(
                    Invariant::InconsistentResultMetadata,
                    RecordKind::Edge,
                    Some(edge_index),
                    "cardinality",
                ));
            }
        }
        verify_node(program, node_index, node, &program.edges[bounds], injection)?;
        if let NodeKind::FanOut { branches, .. } = node.kind {
            verify_fan_out_result_metadata(program, node_index, node, branches)?;
        }
    }
    Ok(())
}

fn verify_fan_out_references(
    program: &RawProgram,
    node_index: u32,
    branches: IndexRange,
) -> Result<(), VerifyError> {
    let bounds = range_bounds(
        branches,
        program.branches.len(),
        RecordKind::Node,
        node_index,
        "branches",
    )?;
    let mut previous_end = program
        .nodes
        .get(node_index as usize)
        .and_then(|node| {
            range_bounds(
                node.edges,
                program.edges.len(),
                RecordKind::Node,
                node_index,
                "edges",
            )
            .ok()
        })
        .and_then(|edges| program.edges[edges].first())
        .and_then(|edge| edge.producer.0.checked_add(1))
        .ok_or_else(|| {
            malformed(
                Invariant::InvalidRecord,
                RecordKind::Node,
                Some(node_index),
                "fan_out.operand",
            )
        })?;
    for (offset, branch) in program.branches[bounds].iter().copied().enumerate() {
        let branch_index = branches.start.saturating_add(offset as u32);
        let end = branch.nodes.checked_end().ok_or_else(|| {
            malformed(
                Invariant::RangeOverflow,
                RecordKind::Branch,
                Some(branch_index),
                "nodes",
            )
        })?;
        if branch.nodes.start != previous_end
            || branch.nodes.count == 0
            || end > node_index
            || branch.root.0 < branch.nodes.start
            || branch.root.0 >= end
        {
            return Err(malformed(
                Invariant::NonPostorderReference,
                RecordKind::Branch,
                Some(branch_index),
                "nodes",
            ));
        }
        if !in_bounds(branch.origin.0, program.origins.len())
            || !in_bounds(branch.placeholder_origin.0, program.origins.len())
        {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Branch,
                Some(branch_index),
                "origin",
            ));
        }
        previous_end = end;
    }
    if previous_end != node_index {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::Node,
            Some(node_index),
            "fan_out.region",
        ));
    }
    Ok(())
}

fn verify_fan_out_result_metadata(
    program: &RawProgram,
    node_index: u32,
    node: Node,
    branches: IndexRange,
) -> Result<(), VerifyError> {
    let Some(TypeRecord::Tuple { elements }) = type_record(program, node.result_type) else {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "result_type",
        ));
    };
    if elements.count != branches.count {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "result_type",
        ));
    }
    let bounds = range_bounds(
        branches,
        program.branches.len(),
        RecordKind::Node,
        node_index,
        "branches",
    )?;
    for (offset, branch) in program.branches[bounds].iter().enumerate() {
        let expected_type = program.type_elements[elements.start as usize + offset];
        if program
            .nodes
            .get(branch.root.0 as usize)
            .is_some_and(|root| root.result_type != expected_type)
        {
            return Err(malformed(
                Invariant::InconsistentResultMetadata,
                RecordKind::Branch,
                Some(branches.start.saturating_add(offset as u32)),
                "root",
            ));
        }
    }
    Ok(())
}

fn verify_node(
    program: &RawProgram,
    node_index: u32,
    node: Node,
    edges: &[Edge],
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    let inconsistent = |field| {
        malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            field,
        )
    };
    match node.kind {
        NodeKind::Constant { constant } => {
            let value = usize::try_from(constant.0)
                .ok()
                .and_then(|index| program.constants.get(index))
                .copied()
                .ok_or_else(|| {
                    malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "constant",
                    )
                })?;
            if !edges.is_empty() {
                return Err(inconsistent("edges"));
            }
            let expected = match value {
                ConstantRecord::Scalar(value) => (
                    TypeRecord::Scalar(scalar_kind(value)),
                    Cardinality::StaticScalar,
                ),
                ConstantRecord::Vector {
                    element_type,
                    elements,
                } => (
                    TypeRecord::Vector(element_type),
                    Cardinality::StaticVector(elements.count),
                ),
            };
            if type_record(program, node.result_type) != Some(expected.0)
                || node.cardinality != Some(expected.1)
            {
                return Err(inconsistent("result"));
            }
        }
        NodeKind::ParameterBorrow { parameter } => {
            let parameter = usize::try_from(parameter.0)
                .ok()
                .and_then(|index| program.parameters.get(index))
                .ok_or_else(|| {
                    malformed(
                        Invariant::IndexOutOfBounds,
                        RecordKind::Node,
                        Some(node_index),
                        "parameter",
                    )
                })?;
            if !edges.is_empty()
                || type_record(program, node.result_type)
                    != Some(TypeRecord::Scalar(parameter.scalar_type))
                || node.cardinality != Some(Cardinality::StaticScalar)
            {
                return Err(inconsistent("result"));
            }
        }
        NodeKind::TupleConstruct => {
            let Some(TypeRecord::Tuple { elements }) = type_record(program, node.result_type)
            else {
                return Err(inconsistent("result_type"));
            };
            if elements.count != u32::try_from(edges.len()).unwrap_or(u32::MAX)
                || node.cardinality.is_some()
            {
                return Err(inconsistent("cardinality"));
            }
            for (offset, edge) in edges.iter().copied().enumerate() {
                let expected_index = usize::try_from(elements.start)
                    .ok()
                    .and_then(|start| start.checked_add(offset));
                let expected = expected_index
                    .and_then(|index| program.type_elements.get(index))
                    .copied()
                    .ok_or_else(|| inconsistent("elements"))?;
                if edge_type(program, edge) != Some(expected)
                    || edge.access != ValueAccess::WholeValue
                    || edge.conversion != Conversion::Identity
                {
                    return Err(inconsistent("elements"));
                }
            }
        }
        NodeKind::PrefixSpreadPrepare => {
            if edges.len() != 1
                || !matches!(
                    edges[0].access,
                    ValueAccess::WholeValue | ValueAccess::FanOutOperandBorrow
                )
                || edges[0].conversion != Conversion::Identity
                || !matches!(
                    type_record(program, node.result_type),
                    Some(TypeRecord::Tuple { .. })
                )
                || edge_type(program, edges[0]) != Some(node.result_type)
                || node.cardinality.is_some()
            {
                return Err(inconsistent("spread"));
            }
        }
        NodeKind::SelectedApply {
            primitive_id,
            signature_id,
            implementation_id,
            primitive_origin,
            lift,
            result_element_type,
            shape,
        } => {
            verify_apply(
                program,
                node_index,
                node,
                edges,
                primitive_id,
                signature_id,
                implementation_id,
                primitive_origin,
                lift,
                result_element_type,
                shape,
                injection,
            )?;
        }
        NodeKind::FanOut { .. } => {
            if node.cardinality.is_some() {
                return Err(inconsistent("cardinality"));
            }
        }
    }
    Ok(())
}

fn verify_prefix_spread_group(
    program: &RawProgram,
    node_index: u32,
    edges: &[Edge],
) -> Result<(), VerifyError> {
    let Some(first) = edges
        .iter()
        .find(|edge| matches!(edge.access, ValueAccess::TupleElement(_)))
    else {
        return Ok(());
    };
    let Some(prepare) = program.nodes.get(first.producer.0 as usize) else {
        return Ok(());
    };
    let Some(TypeRecord::Tuple { elements }) = type_record(program, prepare.result_type) else {
        return Err(malformed(
            Invariant::InvalidRecord,
            RecordKind::Node,
            Some(node_index),
            "spread",
        ));
    };
    let valid = matches!(prepare.kind, NodeKind::PrefixSpreadPrepare)
        && usize::try_from(elements.count).ok() == Some(edges.len())
        && edges.iter().copied().enumerate().all(|(offset, edge)| {
            edge.producer == first.producer
                && edge.access
                    == ValueAccess::TupleElement(u32::try_from(offset).unwrap_or(u32::MAX))
        });
    if !valid {
        return Err(malformed(
            Invariant::InvalidRecord,
            RecordKind::Node,
            Some(node_index),
            "spread",
        ));
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn verify_apply(
    program: &RawProgram,
    node_index: u32,
    node: Node,
    edges: &[Edge],
    primitive_id: u16,
    signature_id: u16,
    implementation_id: u16,
    primitive_origin: OriginIndex,
    lift: LiftMode,
    result_element_type: ScalarType,
    shape: ShapePlan,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    use crate::semantic_registry::{StructuralBehavior, implementation_from_numeric};
    let descriptor = implementation_from_numeric(implementation_id).map_err(|_| {
        malformed(
            Invariant::InvalidSemanticIdentity,
            RecordKind::Node,
            Some(node_index),
            "implementation_id",
        )
    })?;
    if !in_bounds(primitive_origin.0, program.origins.len()) {
        return Err(malformed(
            Invariant::IndexOutOfBounds,
            RecordKind::Node,
            Some(node_index),
            "primitive_origin",
        ));
    }
    if descriptor.primitive_id.numeric() != primitive_id
        || descriptor.signature_id.numeric() != signature_id
        || descriptor.result != result_element_type
    {
        return Err(malformed(
            Invariant::InvalidSemanticIdentity,
            RecordKind::Node,
            Some(node_index),
            "semantic_identity",
        ));
    }
    if edges.len() != descriptor.parameters.len() {
        return Err(malformed(
            Invariant::InvalidRecord,
            RecordKind::Node,
            Some(node_index),
            "arity",
        ));
    }
    let mut any_vector = false;
    let mut any_dynamic = false;
    let mut static_length = None;
    let mut first_static = None;
    injected(injection, VerifyAllocationSite::DynamicShapeScratch)?;
    let mut expected_dynamic = Vec::new();
    expected_dynamic
        .try_reserve(edges.len())
        .map_err(|_| allocation_error(VerifyAllocationSite::DynamicShapeScratch))?;
    for (offset, (edge, accepted)) in edges
        .iter()
        .copied()
        .zip(descriptor.parameters.iter().copied())
        .enumerate()
    {
        let (actual, vector) = scalar_container(
            program,
            edge_type(program, edge).ok_or_else(|| {
                malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Edge,
                    Some(node.edges.start.saturating_add(offset as u32)),
                    "type",
                )
            })?,
        )
        .ok_or_else(|| {
            malformed(
                Invariant::InvalidRecord,
                RecordKind::Edge,
                Some(node.edges.start.saturating_add(offset as u32)),
                "type",
            )
        })?;
        let valid_conversion = match edge.conversion {
            Conversion::Identity => actual == accepted,
            Conversion::PromoteIntToDouble => {
                actual == ScalarType::Int && accepted == ScalarType::Double
            }
        };
        if !valid_conversion {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Edge,
                Some(node.edges.start.saturating_add(offset as u32)),
                "conversion",
            ));
        }
        if vector {
            any_vector = true;
            match edge.cardinality {
                Some(Cardinality::StaticVector(length)) => {
                    if first_static.is_none() {
                        first_static = Some(offset as u32);
                        static_length = Some(length);
                    } else if static_length != Some(length) {
                        return Err(malformed(
                            Invariant::InconsistentResultMetadata,
                            RecordKind::Node,
                            Some(node_index),
                            "static_shape",
                        ));
                    }
                }
                Some(Cardinality::DynamicVector) => {
                    any_dynamic = true;
                    expected_dynamic.push(offset as u32);
                }
                _ => {
                    return Err(malformed(
                        Invariant::InconsistentResultMetadata,
                        RecordKind::Edge,
                        Some(node.edges.start.saturating_add(offset as u32)),
                        "cardinality",
                    ));
                }
            }
        }
    }
    let dynamic_bounds = range_bounds(
        shape.dynamic_checks,
        program.shape_checks.len(),
        RecordKind::Node,
        node_index,
        "shape.dynamic_checks",
    )?;
    let actual_dynamic = &program.shape_checks[dynamic_bounds];
    for edge_index in &mut expected_dynamic {
        *edge_index = node.edges.start.saturating_add(*edge_index);
    }
    if shape.static_anchor != first_static || actual_dynamic != expected_dynamic {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "shape",
        ));
    }
    let (expected_type, expected_cardinality, expected_lift) = match descriptor.behavior {
        StructuralBehavior::Iota => (
            TypeRecord::Vector(descriptor.result),
            Cardinality::DynamicVector,
            LiftMode::DynamicVector,
        ),
        StructuralBehavior::Elementwise if any_vector => (
            TypeRecord::Vector(descriptor.result),
            if any_dynamic {
                Cardinality::DynamicVector
            } else {
                Cardinality::StaticVector(static_length.unwrap_or(0))
            },
            LiftMode::Vector,
        ),
        StructuralBehavior::Elementwise => (
            TypeRecord::Scalar(descriptor.result),
            Cardinality::StaticScalar,
            LiftMode::Scalar,
        ),
    };
    if type_record(program, node.result_type) != Some(expected_type)
        || node.cardinality != Some(expected_cardinality)
        || lift != expected_lift
    {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "result",
        ));
    }
    Ok(())
}

#[derive(Debug, Default, Eq, PartialEq)]
struct FanOutContextVisits {
    fan_outs: u64,
    branch_nodes: u64,
    edges: u64,
}

#[derive(Debug, Default, Eq, PartialEq)]
struct NestedFanOutVisits {
    nodes: u64,
    region_nodes: u64,
    branches: u64,
}

fn verify_no_nested_fan_out(program: &RawProgram) -> Result<(), VerifyError> {
    verify_no_nested_fan_out_with_visits(program, &mut NestedFanOutVisits::default())
}

fn verify_no_nested_fan_out_with_visits(
    program: &RawProgram,
    visits: &mut NestedFanOutVisits,
) -> Result<(), VerifyError> {
    for (node_index, node) in program.nodes.iter().copied().enumerate() {
        visits.nodes = visits.nodes.saturating_add(1);
        let NodeKind::FanOut { branches, .. } = node.kind else {
            continue;
        };
        let edge_bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            node_index as u32,
            "edges",
        )?;
        let operand = program.edges[edge_bounds]
            .first()
            .map(|edge| edge.producer.0)
            .ok_or_else(|| {
                malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Node,
                    Some(node_index as u32),
                    "fan_out.operand",
                )
            })?;
        let mut nested = None;
        for (offset, candidate) in program.nodes[operand as usize..node_index]
            .iter()
            .enumerate()
        {
            visits.region_nodes = visits.region_nodes.saturating_add(1);
            if matches!(candidate.kind, NodeKind::FanOut { .. }) {
                nested = Some(operand.saturating_add(offset as u32));
                break;
            }
        }
        if let Some(nested) = nested {
            let branch_bounds = range_bounds(
                branches,
                program.branches.len(),
                RecordKind::Node,
                node_index as u32,
                "branches",
            )?;
            for (offset, branch) in program.branches[branch_bounds].iter().copied().enumerate() {
                visits.branches = visits.branches.saturating_add(1);
                if branch
                    .nodes
                    .checked_end()
                    .is_some_and(|end| nested >= branch.nodes.start && nested < end)
                {
                    return Err(malformed(
                        Invariant::InvalidRecord,
                        RecordKind::Branch,
                        Some(branches.start.saturating_add(offset as u32)),
                        "nested_fan_out",
                    ));
                }
            }
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Branch,
                Some(branches.start),
                "nested_fan_out",
            ));
        }
    }
    Ok(())
}

fn build_fan_out_borrow_context(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
    visits: &mut FanOutContextVisits,
) -> Result<Vec<bool>, VerifyError> {
    injected(injection, VerifyAllocationSite::FanOutBorrowContext)?;
    let mut context = filled_verify_vec(
        program.edges.len(),
        false,
        VerifyAllocationSite::FanOutBorrowContext,
    )?;
    for (fan_out_index, node) in program.nodes.iter().copied().enumerate() {
        let NodeKind::FanOut { branches, .. } = node.kind else {
            continue;
        };
        visits.fan_outs = visits.fan_outs.saturating_add(1);
        let edge_bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            fan_out_index as u32,
            "edges",
        )?;
        let Some(operand) = program.edges[edge_bounds].first() else {
            continue;
        };
        let branch_bounds = range_bounds(
            branches,
            program.branches.len(),
            RecordKind::Node,
            fan_out_index as u32,
            "branches",
        )?;
        for (branch_offset, branch) in program.branches[branch_bounds].iter().copied().enumerate() {
            let branch_index = branches.start.saturating_add(branch_offset as u32);
            let node_bounds = range_bounds(
                branch.nodes,
                program.nodes.len(),
                RecordKind::Branch,
                branch_index,
                "nodes",
            )?;
            for (node_offset, branch_node) in program.nodes[node_bounds].iter().copied().enumerate()
            {
                visits.branch_nodes = visits.branch_nodes.saturating_add(1);
                let branch_node_index = branch.nodes.start.saturating_add(node_offset as u32);
                let edge_bounds = range_bounds(
                    branch_node.edges,
                    program.edges.len(),
                    RecordKind::Node,
                    branch_node_index,
                    "edges",
                )?;
                for edge_index in edge_bounds {
                    visits.edges = visits.edges.saturating_add(1);
                    let edge = program.edges[edge_index];
                    if edge.access == ValueAccess::FanOutOperandBorrow
                        && edge.producer == operand.producer
                    {
                        context[edge_index] = true;
                    }
                }
            }
        }
    }
    Ok(context)
}

fn verify_fan_out(
    program: &RawProgram,
    node_index: u32,
    node: Node,
    edges: &[Edge],
    branches: IndexRange,
) -> Result<(), VerifyError> {
    let NodeKind::FanOut { keyword_origin, .. } = node.kind else {
        return Err(malformed(
            Invariant::InvalidRecord,
            RecordKind::Node,
            Some(node_index),
            "kind",
        ));
    };
    if !in_bounds(keyword_origin.0, program.origins.len()) {
        return Err(malformed(
            Invariant::IndexOutOfBounds,
            RecordKind::Node,
            Some(node_index),
            "keyword_origin",
        ));
    }
    let operand_is_parameter = edges
        .first()
        .and_then(|edge| program.nodes.get(edge.producer.0 as usize))
        .is_some_and(|node| matches!(node.kind, NodeKind::ParameterBorrow { .. }));
    if branches.count == 0
        || edges.len() != 1
        || edges[0].access != ValueAccess::WholeValue
        || edges[0].conversion != Conversion::Identity
        || if operand_is_parameter {
            edges[0].ownership != OwnershipMode::ImmutableBorrow
        } else {
            edges[0].ownership != OwnershipMode::OwnedInput
        }
    {
        return Err(malformed(
            Invariant::InvalidRecord,
            RecordKind::Node,
            Some(node_index),
            "fan_out",
        ));
    }
    let Some(TypeRecord::Tuple { elements }) = type_record(program, node.result_type) else {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "result_type",
        ));
    };
    if elements.count != branches.count {
        return Err(malformed(
            Invariant::InconsistentResultMetadata,
            RecordKind::Node,
            Some(node_index),
            "result_type",
        ));
    }
    let branch_bounds = range_bounds(
        branches,
        program.branches.len(),
        RecordKind::Node,
        node_index,
        "branches",
    )?;
    let operand = edges[0].producer;
    let mut previous_end = operand.0.checked_add(1).ok_or_else(|| {
        malformed(
            Invariant::RangeOverflow,
            RecordKind::Node,
            Some(node_index),
            "fan_out.operand",
        )
    })?;
    for (offset, branch) in program.branches[branch_bounds].iter().copied().enumerate() {
        let branch_index = branches.start.saturating_add(offset as u32);
        if branch.nodes.start != previous_end
            || branch.nodes.count == 0
            || branch
                .nodes
                .checked_end()
                .is_none_or(|end| end > node_index)
            || branch.root.0 < branch.nodes.start
            || branch
                .nodes
                .checked_end()
                .is_none_or(|end| branch.root.0 >= end)
        {
            return Err(malformed(
                Invariant::NonPostorderReference,
                RecordKind::Branch,
                Some(branch_index),
                "nodes",
            ));
        }
        if !in_bounds(branch.origin.0, program.origins.len())
            || !in_bounds(branch.placeholder_origin.0, program.origins.len())
        {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Branch,
                Some(branch_index),
                "origin",
            ));
        }
        if !matches!(
            program.nodes[branch.root.0 as usize].kind,
            NodeKind::SelectedApply { .. }
        ) {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Branch,
                Some(branch_index),
                "root_kind",
            ));
        }
        let start = branch.nodes.start as usize;
        let end = branch.nodes.checked_end().unwrap_or(branch.nodes.start) as usize;
        let mut placeholders = 0_u32;
        for branch_node in &program.nodes[start..end] {
            if matches!(branch_node.kind, NodeKind::FanOut { .. }) {
                return Err(malformed(
                    Invariant::InvalidRecord,
                    RecordKind::Branch,
                    Some(branch_index),
                    "nested_fan_out",
                ));
            }
            let edge_start = branch_node.edges.start as usize;
            let edge_end = branch_node
                .edges
                .checked_end()
                .unwrap_or(branch_node.edges.start) as usize;
            for edge in &program.edges[edge_start..edge_end] {
                if edge.access == ValueAccess::FanOutOperandBorrow {
                    if !matches!(
                        branch_node.kind,
                        NodeKind::SelectedApply { .. } | NodeKind::PrefixSpreadPrepare
                    ) {
                        return Err(malformed(
                            Invariant::InvalidRecord,
                            RecordKind::Branch,
                            Some(branch_index),
                            "placeholder_aggregate",
                        ));
                    }
                    if edge.producer != operand || edge.ownership != OwnershipMode::ImmutableBorrow
                    {
                        return Err(malformed(
                            Invariant::AmbiguousOwnership,
                            RecordKind::Branch,
                            Some(branch_index),
                            "placeholder",
                        ));
                    }
                    placeholders = placeholders.saturating_add(1);
                }
            }
        }
        if placeholders != 1 {
            return Err(malformed(
                Invariant::InvalidRecord,
                RecordKind::Branch,
                Some(branch_index),
                "placeholder",
            ));
        }
        let expected_type = program.type_elements[elements.start as usize + offset];
        if program.nodes[branch.root.0 as usize].result_type != expected_type {
            return Err(malformed(
                Invariant::InconsistentResultMetadata,
                RecordKind::Branch,
                Some(branch_index),
                "root",
            ));
        }
        previous_end = branch.nodes.checked_end().unwrap_or(previous_end);
    }
    if previous_end != node_index {
        return Err(malformed(
            Invariant::RangeMismatch,
            RecordKind::Node,
            Some(node_index),
            "fan_out.region",
        ));
    }
    Ok(())
}

fn verify_reachability(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    injected(injection, VerifyAllocationSite::ReachabilityBits)?;
    let mut reachable = Vec::new();
    reachable
        .try_reserve_exact(program.nodes.len())
        .map_err(|_| allocation_error(VerifyAllocationSite::ReachabilityBits))?;
    reachable.resize(program.nodes.len(), false);
    injected(injection, VerifyAllocationSite::ReachabilityWorklist)?;
    let mut work = Vec::new();
    work.try_reserve(program.roots.len())
        .map_err(|_| allocation_error(VerifyAllocationSite::ReachabilityWorklist))?;
    for (root_index, root) in program.roots.iter().enumerate().rev() {
        if let Some(node) = root_traversal_node(program, root_index, *root) {
            push_verify_work(&mut work, node)?;
        }
    }
    while let Some(node_index) = work.pop() {
        let index = node_index.0 as usize;
        if reachable.get(index).copied().unwrap_or(false) {
            continue;
        }
        let Some(node) = program.nodes.get(index) else {
            continue;
        };
        reachable[index] = true;
        if let NodeKind::FanOut { branches, .. } = node.kind
            && let Ok(bounds) = range_bounds(
                branches,
                program.branches.len(),
                RecordKind::Node,
                node_index.0,
                "branches",
            )
        {
            for branch in program.branches[bounds].iter().rev() {
                push_verify_work(&mut work, branch.root)?;
            }
        }
        if let Ok(bounds) = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            node_index.0,
            "edges",
        ) {
            for edge in program.edges[bounds].iter().rev() {
                push_verify_work(&mut work, edge.producer)?;
            }
        }
    }
    if let Some(index) = reachable.iter().position(|reachable| !reachable) {
        return Err(malformed(
            Invariant::UnreachableNode,
            RecordKind::Node,
            checked_index(index),
            "reachability",
        ));
    }
    Ok(())
}

fn root_traversal_node(program: &RawProgram, root_index: usize, root: Root) -> Option<NodeIndex> {
    if in_bounds(root.node.0, program.nodes.len()) {
        return Some(root.node);
    }
    let root_index = u32::try_from(root_index).ok()?;
    let mut owners = program
        .ownership
        .iter()
        .filter(|ownership| {
            ownership.release_after == ReleaseAfter::Root(RootIndex(root_index))
                && in_bounds(ownership.owner.0, program.nodes.len())
        })
        .map(|ownership| ownership.owner);
    let owner = owners.next()?;
    owners.next().is_none().then_some(owner)
}

fn push_verify_work(work: &mut Vec<NodeIndex>, node: NodeIndex) -> Result<(), VerifyError> {
    work.try_reserve(1)
        .map_err(|_| allocation_error(VerifyAllocationSite::ReachabilityWorklist))?;
    work.push(node);
    Ok(())
}

fn filled_verify_vec<T: Clone>(
    length: usize,
    value: T,
    site: VerifyAllocationSite,
) -> Result<Vec<T>, VerifyError> {
    let mut values = Vec::new();
    values
        .try_reserve_exact(length)
        .map_err(|_| allocation_error(site))?;
    values.resize(length, value);
    Ok(values)
}

fn verify_semantic_ownership(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    verify_no_nested_fan_out(program)?;
    let mut fan_out_visits = FanOutContextVisits::default();
    let fan_out_borrow_context =
        build_fan_out_borrow_context(program, injection, &mut fan_out_visits)?;
    for (consumer_index, node) in program.nodes.iter().copied().enumerate() {
        let bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            consumer_index as u32,
            "edges",
        )?;
        for (offset, edge) in program.edges[bounds.clone()].iter().copied().enumerate() {
            let producer_kind = program.nodes[edge.producer.0 as usize].kind;
            let parameter = matches!(producer_kind, NodeKind::ParameterBorrow { .. });
            let edge_index = node.edges.start as usize + offset;
            let valid = if matches!(producer_kind, NodeKind::PrefixSpreadPrepare) {
                matches!(
                    (node.kind, edge.access, edge.ownership),
                    (
                        NodeKind::SelectedApply { .. },
                        ValueAccess::TupleElement(_),
                        OwnershipMode::ImmutableBorrow
                    )
                )
            } else {
                match (node.kind, edge.access) {
                    (NodeKind::TupleConstruct, ValueAccess::WholeValue) => {
                        if parameter {
                            edge.ownership == OwnershipMode::ImmutableBorrow
                        } else {
                            edge.ownership == OwnershipMode::InfallibleTransfer
                        }
                    }
                    (NodeKind::PrefixSpreadPrepare, ValueAccess::WholeValue) => {
                        edge.ownership == OwnershipMode::InfallibleTransfer
                    }
                    (NodeKind::PrefixSpreadPrepare, ValueAccess::FanOutOperandBorrow) => {
                        edge.ownership == OwnershipMode::ImmutableBorrow
                            && fan_out_borrow_context
                                .get(edge_index)
                                .copied()
                                .unwrap_or(false)
                    }
                    (NodeKind::SelectedApply { .. }, ValueAccess::WholeValue) => {
                        if parameter {
                            edge.ownership == OwnershipMode::ImmutableBorrow
                        } else {
                            edge.ownership == OwnershipMode::OwnedInput
                        }
                    }
                    (NodeKind::SelectedApply { .. }, ValueAccess::TupleElement(_)) => false,
                    (NodeKind::SelectedApply { .. }, ValueAccess::FanOutOperandBorrow) => {
                        edge.ownership == OwnershipMode::ImmutableBorrow
                            && fan_out_borrow_context
                                .get(edge_index)
                                .copied()
                                .unwrap_or(false)
                    }
                    (NodeKind::FanOut { .. }, ValueAccess::WholeValue) => {
                        if parameter {
                            edge.ownership == OwnershipMode::ImmutableBorrow
                        } else {
                            edge.ownership == OwnershipMode::OwnedInput
                        }
                    }
                    _ => false,
                }
            };
            if !valid {
                return Err(malformed(
                    Invariant::AmbiguousOwnership,
                    RecordKind::Edge,
                    Some(node.edges.start.saturating_add(offset as u32)),
                    "ownership",
                ));
            }
        }
        if let NodeKind::FanOut { branches, .. } = node.kind {
            verify_fan_out(
                program,
                consumer_index as u32,
                node,
                &program.edges[bounds],
                branches,
            )?;
        }
    }
    verify_ownership(program, injection)
}

fn verify_ownership(
    program: &RawProgram,
    injection: VerifyAllocationFailureInjection,
) -> Result<(), VerifyError> {
    injected(injection, VerifyAllocationSite::OwnershipSinks)?;
    let mut sinks = filled_verify_vec(
        program.nodes.len(),
        0_u32,
        VerifyAllocationSite::OwnershipSinks,
    )?;
    injected(injection, VerifyAllocationSite::OwnershipLastUse)?;
    let mut last_use = filled_verify_vec(
        program.nodes.len(),
        None,
        VerifyAllocationSite::OwnershipLastUse,
    )?;
    for (consumer, node) in program.nodes.iter().enumerate() {
        let bounds = range_bounds(
            node.edges,
            program.edges.len(),
            RecordKind::Node,
            consumer as u32,
            "edges",
        )?;
        for edge in &program.edges[bounds] {
            let producer = edge.producer.0 as usize;
            if matches!(program.nodes[producer].kind, NodeKind::PrefixSpreadPrepare)
                && edge.access == ValueAccess::TupleElement(0)
                && last_use[producer].is_some_and(|previous| previous != NodeIndex(consumer as u32))
            {
                return Err(malformed(
                    Invariant::AmbiguousOwnership,
                    RecordKind::Edge,
                    Some(node.edges.start),
                    "ownership",
                ));
            }
            last_use[producer] = Some(NodeIndex(consumer as u32));
            if edge.ownership != OwnershipMode::ImmutableBorrow {
                sinks[producer] = sinks[producer].saturating_add(1);
            }
        }
    }
    for (fanout_index, node) in program.nodes.iter().enumerate() {
        if let NodeKind::FanOut { branches, .. } = node.kind {
            let bounds = range_bounds(
                branches,
                program.branches.len(),
                RecordKind::Node,
                fanout_index as u32,
                "branches",
            )?;
            for branch in &program.branches[bounds] {
                sinks[branch.root.0 as usize] = sinks[branch.root.0 as usize].saturating_add(1);
                last_use[branch.root.0 as usize] = Some(NodeIndex(fanout_index as u32));
            }
        }
    }
    injected(injection, VerifyAllocationSite::OwnershipRootOwner)?;
    let mut root_owner = filled_verify_vec(
        program.nodes.len(),
        None,
        VerifyAllocationSite::OwnershipRootOwner,
    )?;
    for (root_index, root) in program.roots.iter().enumerate() {
        let Some(root_node) = root_traversal_node(program, root_index, *root) else {
            continue;
        };
        if !matches!(
            program.nodes[root_node.0 as usize].kind,
            NodeKind::ParameterBorrow { .. }
        ) {
            sinks[root_node.0 as usize] = sinks[root_node.0 as usize].saturating_add(1);
            root_owner[root_node.0 as usize] = Some(RootIndex(root_index as u32));
        }
    }
    for (index, node) in program.nodes.iter().enumerate() {
        if matches!(node.kind, NodeKind::PrefixSpreadPrepare)
            && sinks[index] == 0
            && last_use[index].is_some()
        {
            sinks[index] = 1;
        }
        let expected = if matches!(node.kind, NodeKind::ParameterBorrow { .. }) {
            0
        } else {
            1
        };
        if sinks[index] != expected {
            return Err(malformed(
                Invariant::AmbiguousOwnership,
                RecordKind::Node,
                checked_index(index),
                "owner",
            ));
        }
    }
    if program.ownership.len()
        != program
            .nodes
            .iter()
            .filter(|node| !matches!(node.kind, NodeKind::ParameterBorrow { .. }))
            .count()
    {
        return Err(malformed(
            Invariant::AmbiguousOwnership,
            RecordKind::Ownership,
            None,
            "count",
        ));
    }
    let mut expected_owner = 0_usize;
    for (index, ownership) in program.ownership.iter().copied().enumerate() {
        while expected_owner < program.nodes.len()
            && matches!(
                program.nodes[expected_owner].kind,
                NodeKind::ParameterBorrow { .. }
            )
        {
            expected_owner += 1;
        }
        if ownership.owner.0 as usize != expected_owner {
            return Err(malformed(
                Invariant::AmbiguousOwnership,
                RecordKind::Ownership,
                checked_index(index),
                "owner",
            ));
        }
        let valid_release = if let Some(root) = root_owner[expected_owner] {
            ownership.release_after == ReleaseAfter::Root(root)
        } else {
            last_use[expected_owner]
                .is_some_and(|node| ownership.release_after == ReleaseAfter::Node(node))
        };
        if !valid_release {
            return Err(malformed(
                Invariant::AmbiguousOwnership,
                RecordKind::Ownership,
                checked_index(index),
                "release_after",
            ));
        }
        expected_owner += 1;
    }
    Ok(())
}

fn has_feature(program: &RawProgram, feature: Feature) -> bool {
    program.features.binary_search(&feature.numeric()).is_ok()
}

fn verify_roots_and_features(program: &RawProgram) -> Result<(), VerifyError> {
    for (index, root) in program.roots.iter().enumerate() {
        if !in_bounds(root.node.0, program.nodes.len()) {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Root,
                checked_index(index),
                "node",
            ));
        }
        if !in_bounds(root.origin.0, program.origins.len()) {
            return Err(malformed(
                Invariant::IndexOutOfBounds,
                RecordKind::Root,
                checked_index(index),
                "origin",
            ));
        }
    }
    let needs_ids = program
        .nodes
        .iter()
        .any(|node| matches!(node.kind, NodeKind::SelectedApply { .. }));
    let needs_tuples = program
        .types
        .iter()
        .any(|record| matches!(record, TypeRecord::Tuple { .. }));
    let needs_spread = program
        .nodes
        .iter()
        .any(|node| matches!(node.kind, NodeKind::PrefixSpreadPrepare))
        || program
            .edges
            .iter()
            .any(|edge| matches!(edge.access, ValueAccess::TupleElement(_)));
    let needs_fan_out = program
        .nodes
        .iter()
        .any(|node| matches!(node.kind, NodeKind::FanOut { .. }))
        || program
            .edges
            .iter()
            .any(|edge| matches!(edge.access, ValueAccess::FanOutOperandBorrow));
    let needs_backend_native_math = program.nodes.iter().any(|node| {
        matches!(
            node.kind,
            NodeKind::SelectedApply {
                primitive_id,
                ..
            } if crate::semantic_registry::is_backend_native_math_primitive(
                primitive_id
            )
        )
    });
    for (required, needed, field) in [
        (Feature::StableSemanticIds, needs_ids, "stable_semantic_ids"),
        (Feature::Tuples, needs_tuples, "tuples"),
        (Feature::PrefixSpread, needs_spread, "prefix_spread"),
        (Feature::FanOut, needs_fan_out, "fan_out"),
        (
            Feature::BackendNativeMathV1,
            needs_backend_native_math,
            "backend_native_math_v1",
        ),
    ] {
        if needed && !has_feature(program, required) {
            return Err(malformed(
                Invariant::MissingFeature,
                RecordKind::Module,
                None,
                field,
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("fixture construction failed: {error:?}"),
        }
    }

    fn source_fixture(builder: &mut RawProgramBuilder) -> OriginIndex {
        let source = must(builder.push_source_unit(SourceUnit {
            diagnostic_name: "fixture.bennu".to_owned(),
            byte_length: 32,
        }));
        must(builder.push_origin(Origin {
            source_unit: source,
            span: OriginSpan {
                begin: OriginPosition {
                    offset: 1,
                    line: 1,
                    column: 1,
                },
                end: OriginPosition {
                    offset: 2,
                    line: 1,
                    column: 2,
                },
            },
        }))
    }

    fn scalar_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(7))));
        let node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_ownership(Ownership {
            owner: node,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root { node, origin }));
        must(builder.finish())
    }

    fn parameter_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        let origin = source_fixture(&mut builder);
        builder.set_parameter_header_origin(origin);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let parameter = must(builder.push_parameter(Parameter {
            slot: 0,
            name: "value".to_owned(),
            scalar_type: ScalarType::Int,
            declaration_origin: origin,
            name_origin: origin,
        }));
        let node = must(builder.push_node(Node {
            kind: NodeKind::ParameterBorrow { parameter },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_root(Root { node, origin }));
        must(builder.finish())
    }

    fn tuple_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::Tuples.numeric()));
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        must(builder.push_type_element(int_type));
        must(builder.push_type_element(int_type));
        let tuple_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 2 },
        }));
        let first = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let second = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(2))));
        let first_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: first },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        let second_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: second },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: first_node,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::InfallibleTransfer,
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: second_node,
            argument_position: 2,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::InfallibleTransfer,
            origin,
        }));
        let tuple = must(builder.push_node(Node {
            kind: NodeKind::TupleConstruct,
            result_type: tuple_type,
            cardinality: None,
            edges: IndexRange { start: 0, count: 2 },
            origin,
        }));
        for owner in [first_node, second_node] {
            must(builder.push_ownership(Ownership {
                owner,
                release_after: ReleaseAfter::Node(tuple),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: tuple,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: tuple,
            origin,
        }));
        must(builder.finish())
    }

    fn vector_apply_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::StableSemanticIds.numeric()));
        let origin = source_fixture(&mut builder);
        let vector_type = must(builder.push_type(TypeRecord::Vector(ScalarType::Int)));
        for value in [1_i64, 2, 3, 4] {
            must(builder.push_constant_element(ScalarConstant::Int(value)));
        }
        let left = must(builder.push_constant(ConstantRecord::Vector {
            element_type: ScalarType::Int,
            elements: IndexRange { start: 0, count: 2 },
        }));
        let right = must(builder.push_constant(ConstantRecord::Vector {
            element_type: ScalarType::Int,
            elements: IndexRange { start: 2, count: 2 },
        }));
        let left_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: left },
            result_type: vector_type,
            cardinality: Some(Cardinality::StaticVector(2)),
            edges: IndexRange::default(),
            origin,
        }));
        let right_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: right },
            result_type: vector_type,
            cardinality: Some(Cardinality::StaticVector(2)),
            edges: IndexRange::default(),
            origin,
        }));
        for (position, producer) in [(1, left_node), (2, right_node)] {
            must(builder.push_edge(Edge {
                producer,
                argument_position: position,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::StaticVector(2)),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::OwnedInput,
                origin,
            }));
        }
        let apply = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                primitive_origin: origin,
                lift: LiftMode::Vector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: Some(0),
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: vector_type,
            cardinality: Some(Cardinality::StaticVector(2)),
            edges: IndexRange { start: 0, count: 2 },
            origin,
        }));
        for owner in [left_node, right_node] {
            must(builder.push_ownership(Ownership {
                owner,
                release_after: ReleaseAfter::Node(apply),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: apply,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: apply,
            origin,
        }));
        must(builder.finish())
    }

    fn iota_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::StableSemanticIds.numeric()));
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let vector_type = must(builder.push_type(TypeRecord::Vector(ScalarType::Int)));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(4))));
        let bound = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: bound,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        let iota = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 19,
                signature_id: 34,
                implementation_id: 34,
                primitive_origin: origin,
                lift: LiftMode::DynamicVector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: vector_type,
            cardinality: Some(Cardinality::DynamicVector),
            edges: IndexRange { start: 0, count: 1 },
            origin,
        }));
        must(builder.push_ownership(Ownership {
            owner: bound,
            release_after: ReleaseAfter::Node(iota),
        }));
        must(builder.push_ownership(Ownership {
            owner: iota,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root { node: iota, origin }));
        must(builder.finish())
    }

    fn dynamic_shape_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::StableSemanticIds.numeric()));
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let vector_type = must(builder.push_type(TypeRecord::Vector(ScalarType::Int)));
        let left_constant =
            must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(2))));
        let right_constant =
            must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(3))));
        let left_bound = must(builder.push_node(Node {
            kind: NodeKind::Constant {
                constant: left_constant,
            },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        let right_bound = must(builder.push_node(Node {
            kind: NodeKind::Constant {
                constant: right_constant,
            },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: left_bound,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        let left = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 19,
                signature_id: 34,
                implementation_id: 34,
                primitive_origin: origin,
                lift: LiftMode::DynamicVector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: vector_type,
            cardinality: Some(Cardinality::DynamicVector),
            edges: IndexRange { start: 0, count: 1 },
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: right_bound,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        let right = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 19,
                signature_id: 34,
                implementation_id: 34,
                primitive_origin: origin,
                lift: LiftMode::DynamicVector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: vector_type,
            cardinality: Some(Cardinality::DynamicVector),
            edges: IndexRange { start: 1, count: 1 },
            origin,
        }));
        for (position, producer) in [(1, left), (2, right)] {
            must(builder.push_edge(Edge {
                producer,
                argument_position: position,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::DynamicVector),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::OwnedInput,
                origin,
            }));
        }
        must(builder.push_shape_check(2));
        must(builder.push_shape_check(3));
        let apply = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                primitive_origin: origin,
                lift: LiftMode::Vector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange { start: 0, count: 2 },
                },
            },
            result_type: vector_type,
            cardinality: Some(Cardinality::DynamicVector),
            edges: IndexRange { start: 2, count: 2 },
            origin,
        }));
        for (owner, release_after) in [
            (left_bound, left),
            (right_bound, right),
            (left, apply),
            (right, apply),
        ] {
            must(builder.push_ownership(Ownership {
                owner,
                release_after: ReleaseAfter::Node(release_after),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: apply,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: apply,
            origin,
        }));
        must(builder.finish())
    }

    fn prefix_spread_program() -> RawProgram {
        let mut program = tuple_program();
        program.features = vec![
            Feature::StableSemanticIds.numeric(),
            Feature::Tuples.numeric(),
            Feature::PrefixSpread.numeric(),
        ];
        program.module.ranges.features.count = 3;
        let tuple = NodeIndex(2);
        program.edges.push(Edge {
            producer: tuple,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: None,
            conversion: Conversion::Identity,
            ownership: OwnershipMode::InfallibleTransfer,
            origin: OriginIndex(0),
        });
        program.nodes.push(Node {
            kind: NodeKind::PrefixSpreadPrepare,
            result_type: TypeIndex(1),
            cardinality: None,
            edges: IndexRange { start: 2, count: 1 },
            origin: OriginIndex(0),
        });
        for element in 0..2 {
            program.edges.push(Edge {
                producer: NodeIndex(3),
                argument_position: element + 1,
                access: ValueAccess::TupleElement(element),
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::ImmutableBorrow,
                origin: OriginIndex(0),
            });
        }
        program.nodes.push(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                primitive_origin: OriginIndex(0),
                lift: LiftMode::Scalar,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: TypeIndex(0),
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange { start: 3, count: 2 },
            origin: OriginIndex(0),
        });
        program.roots[0].node = NodeIndex(4);
        program.ownership[2].release_after = ReleaseAfter::Node(NodeIndex(3));
        program.ownership.push(Ownership {
            owner: NodeIndex(3),
            release_after: ReleaseAfter::Node(NodeIndex(4)),
        });
        program.ownership.push(Ownership {
            owner: NodeIndex(4),
            release_after: ReleaseAfter::Root(RootIndex(0)),
        });
        program.module.ranges.nodes.count = 5;
        program.module.ranges.edges.count = 5;
        program.module.ranges.ownership.count = 5;
        program
    }

    fn heterogeneous_prefix_spread_program() -> RawProgram {
        let mut program = prefix_spread_program();
        program
            .types
            .insert(1, TypeRecord::Scalar(ScalarType::Double));
        program.type_elements[1] = TypeIndex(1);
        program.constants[1] =
            ConstantRecord::Scalar(ScalarConstant::DoubleBits(2.0_f64.to_bits()));
        program.nodes[1].result_type = TypeIndex(1);
        for node in &mut program.nodes[2..4] {
            node.result_type = TypeIndex(2);
        }
        program.nodes[4].result_type = TypeIndex(1);
        let NodeKind::SelectedApply {
            ref mut signature_id,
            ref mut implementation_id,
            ref mut result_element_type,
            ..
        } = program.nodes[4].kind
        else {
            panic!("fixture apply kind changed");
        };
        *signature_id = 10;
        *implementation_id = 10;
        *result_element_type = ScalarType::Double;
        program.edges[3].conversion = Conversion::PromoteIntToDouble;
        program.module.ranges.types.count = 3;
        program
    }

    fn shared_prefix_spread_program() -> RawProgram {
        let mut program = prefix_spread_program();
        let second_edge_start = program.edges.len() as u32;
        for element in 0..2 {
            program.edges.push(Edge {
                producer: NodeIndex(3),
                argument_position: element + 1,
                access: ValueAccess::TupleElement(element),
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::ImmutableBorrow,
                origin: OriginIndex(0),
            });
        }
        let mut second_apply = program.nodes[4];
        second_apply.edges = IndexRange {
            start: second_edge_start,
            count: 2,
        };
        program.nodes.push(second_apply);
        program.ownership.push(Ownership {
            owner: NodeIndex(5),
            release_after: ReleaseAfter::Root(RootIndex(1)),
        });
        program.roots.push(Root {
            node: NodeIndex(5),
            origin: OriginIndex(0),
        });
        program.module.ranges.nodes.count = 6;
        program.module.ranges.edges.count = 7;
        program.module.ranges.ownership.count = 6;
        program.module.ranges.roots.count = 2;
        program
    }

    fn fan_out_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [Feature::StableSemanticIds, Feature::Tuples, Feature::FanOut] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        must(builder.push_type_element(int_type));
        let tuple_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 1 },
        }));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let operand = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::FanOutOperandBorrow,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::ImmutableBorrow,
            origin,
        }));
        let branch_root = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 1,
                signature_id: 1,
                implementation_id: 1,
                primitive_origin: origin,
                lift: LiftMode::Scalar,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange { start: 0, count: 1 },
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        must(builder.push_branch(FanOutBranch {
            nodes: IndexRange { start: 1, count: 1 },
            root: branch_root,
            placeholder_origin: origin,
            origin,
        }));
        let fan_out = must(builder.push_node(Node {
            kind: NodeKind::FanOut {
                branches: IndexRange { start: 0, count: 1 },
                keyword_origin: origin,
            },
            result_type: tuple_type,
            cardinality: None,
            edges: IndexRange { start: 1, count: 1 },
            origin,
        }));
        for owner in [operand, branch_root] {
            must(builder.push_ownership(Ownership {
                owner,
                release_after: ReleaseAfter::Node(fan_out),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: fan_out,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: fan_out,
            origin,
        }));
        must(builder.finish())
    }

    fn push_test_inc(
        builder: &mut RawProgramBuilder,
        producer: NodeIndex,
        access: ValueAccess,
        ownership: OwnershipMode,
        int_type: TypeIndex,
        origin: OriginIndex,
    ) -> NodeIndex {
        let edge_start = builder.raw.edges.len() as u32;
        must(builder.push_edge(Edge {
            producer,
            argument_position: 1,
            access,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership,
            origin,
        }));
        must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 1,
                signature_id: 1,
                implementation_id: 1,
                primitive_origin: origin,
                lift: LiftMode::Scalar,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange {
                start: edge_start,
                count: 1,
            },
            origin,
        }))
    }

    fn two_branch_nested_fan_out_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [
            Feature::StableSemanticIds,
            Feature::Tuples,
            Feature::PrefixSpread,
            Feature::FanOut,
        ] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        must(builder.push_type_element(int_type));
        let inner_result_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 1 },
        }));
        must(builder.push_type_element(int_type));
        must(builder.push_type_element(int_type));
        let outer_result_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 1, count: 2 },
        }));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let operand = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));

        let mut outer_branches = Vec::new();
        let mut branch_roots = Vec::new();
        let mut releases = Vec::new();
        must(outer_branches.try_reserve_exact(2));
        must(branch_roots.try_reserve_exact(2));
        must(releases.try_reserve_exact(10));
        for _ in 0..2 {
            let branch_start = builder.raw.nodes.len() as u32;
            let inner_operand = push_test_inc(
                &mut builder,
                operand,
                ValueAccess::FanOutOperandBorrow,
                OwnershipMode::ImmutableBorrow,
                int_type,
                origin,
            );
            let inner_branch = push_test_inc(
                &mut builder,
                inner_operand,
                ValueAccess::FanOutOperandBorrow,
                OwnershipMode::ImmutableBorrow,
                int_type,
                origin,
            );
            must(builder.push_branch(FanOutBranch {
                nodes: IndexRange {
                    start: inner_branch.0,
                    count: 1,
                },
                root: inner_branch,
                placeholder_origin: origin,
                origin,
            }));
            let inner_edge_start = builder.raw.edges.len() as u32;
            must(builder.push_edge(Edge {
                producer: inner_operand,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::OwnedInput,
                origin,
            }));
            let inner_fan_out = must(builder.push_node(Node {
                kind: NodeKind::FanOut {
                    branches: IndexRange {
                        start: builder.raw.branches.len() as u32 - 1,
                        count: 1,
                    },
                    keyword_origin: origin,
                },
                result_type: inner_result_type,
                cardinality: None,
                edges: IndexRange {
                    start: inner_edge_start,
                    count: 1,
                },
                origin,
            }));
            let prepare_edge_start = builder.raw.edges.len() as u32;
            must(builder.push_edge(Edge {
                producer: inner_fan_out,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: None,
                conversion: Conversion::Identity,
                ownership: OwnershipMode::InfallibleTransfer,
                origin,
            }));
            let prepare = must(builder.push_node(Node {
                kind: NodeKind::PrefixSpreadPrepare,
                result_type: inner_result_type,
                cardinality: None,
                edges: IndexRange {
                    start: prepare_edge_start,
                    count: 1,
                },
                origin,
            }));
            let root = push_test_inc(
                &mut builder,
                prepare,
                ValueAccess::TupleElement(0),
                OwnershipMode::ImmutableBorrow,
                int_type,
                origin,
            );
            outer_branches.push(FanOutBranch {
                nodes: IndexRange {
                    start: branch_start,
                    count: builder.raw.nodes.len() as u32 - branch_start,
                },
                root,
                placeholder_origin: origin,
                origin,
            });
            branch_roots.push(root);
            releases.extend([
                (inner_operand, inner_fan_out),
                (inner_branch, inner_fan_out),
                (inner_fan_out, prepare),
                (prepare, root),
            ]);
        }
        for branch in outer_branches {
            must(builder.push_branch(branch));
        }
        let outer_edge_start = builder.raw.edges.len() as u32;
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        let outer_fan_out = must(builder.push_node(Node {
            kind: NodeKind::FanOut {
                branches: IndexRange { start: 2, count: 2 },
                keyword_origin: origin,
            },
            result_type: outer_result_type,
            cardinality: None,
            edges: IndexRange {
                start: outer_edge_start,
                count: 1,
            },
            origin,
        }));
        must(builder.push_ownership(Ownership {
            owner: operand,
            release_after: ReleaseAfter::Node(outer_fan_out),
        }));
        let mut release_index = 0;
        for node_index in 1..outer_fan_out.0 {
            let owner = NodeIndex(node_index);
            let release_after = if branch_roots.contains(&owner) {
                outer_fan_out
            } else {
                let (_, release) = releases[release_index];
                release_index += 1;
                release
            };
            must(builder.push_ownership(Ownership {
                owner,
                release_after: ReleaseAfter::Node(release_after),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: outer_fan_out,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: outer_fan_out,
            origin,
        }));
        must(builder.finish())
    }

    fn sibling_fan_out_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [Feature::StableSemanticIds, Feature::Tuples, Feature::FanOut] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        must(builder.push_type_element(int_type));
        let tuple_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 1 },
        }));
        let mut fan_outs = Vec::new();
        must(fan_outs.try_reserve_exact(2));
        for value in [1_i64, 2_i64] {
            let constant =
                must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(value))));
            let operand = must(builder.push_node(Node {
                kind: NodeKind::Constant { constant },
                result_type: int_type,
                cardinality: Some(Cardinality::StaticScalar),
                edges: IndexRange {
                    start: builder.raw.edges.len() as u32,
                    count: 0,
                },
                origin,
            }));
            let branch_root = push_test_inc(
                &mut builder,
                operand,
                ValueAccess::FanOutOperandBorrow,
                OwnershipMode::ImmutableBorrow,
                int_type,
                origin,
            );
            let branch_index = builder.raw.branches.len() as u32;
            must(builder.push_branch(FanOutBranch {
                nodes: IndexRange {
                    start: branch_root.0,
                    count: 1,
                },
                root: branch_root,
                placeholder_origin: origin,
                origin,
            }));
            let edge_start = builder.raw.edges.len() as u32;
            must(builder.push_edge(Edge {
                producer: operand,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::OwnedInput,
                origin,
            }));
            let fan_out = must(builder.push_node(Node {
                kind: NodeKind::FanOut {
                    branches: IndexRange {
                        start: branch_index,
                        count: 1,
                    },
                    keyword_origin: origin,
                },
                result_type: tuple_type,
                cardinality: None,
                edges: IndexRange {
                    start: edge_start,
                    count: 1,
                },
                origin,
            }));
            fan_outs.push((operand, branch_root, fan_out));
        }
        for (root_index, (operand, branch_root, fan_out)) in fan_outs.into_iter().enumerate() {
            must(builder.push_ownership(Ownership {
                owner: operand,
                release_after: ReleaseAfter::Node(fan_out),
            }));
            must(builder.push_ownership(Ownership {
                owner: branch_root,
                release_after: ReleaseAfter::Node(fan_out),
            }));
            must(builder.push_ownership(Ownership {
                owner: fan_out,
                release_after: ReleaseAfter::Root(RootIndex(root_index as u32)),
            }));
            must(builder.push_root(Root {
                node: fan_out,
                origin,
            }));
        }
        must(builder.finish())
    }

    fn fan_out_prefix_spread_program() -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [
            Feature::StableSemanticIds,
            Feature::Tuples,
            Feature::PrefixSpread,
            Feature::FanOut,
        ] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        for _ in 0..2 {
            must(builder.push_type_element(int_type));
        }
        let operand_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 2 },
        }));
        must(builder.push_type_element(int_type));
        let result_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 2, count: 1 },
        }));
        let first = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let second = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(2))));
        let first_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: first },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        let second_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant: second },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        let tuple_edges = builder.raw.edges.len() as u32;
        for (position, producer) in [(1, first_node), (2, second_node)] {
            must(builder.push_edge(Edge {
                producer,
                argument_position: position,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::InfallibleTransfer,
                origin,
            }));
        }
        let operand = must(builder.push_node(Node {
            kind: NodeKind::TupleConstruct,
            result_type: operand_type,
            cardinality: None,
            edges: IndexRange {
                start: tuple_edges,
                count: 2,
            },
            origin,
        }));
        let prepare_edge = builder.raw.edges.len() as u32;
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::FanOutOperandBorrow,
            cardinality: None,
            conversion: Conversion::Identity,
            ownership: OwnershipMode::ImmutableBorrow,
            origin,
        }));
        let prepare = must(builder.push_node(Node {
            kind: NodeKind::PrefixSpreadPrepare,
            result_type: operand_type,
            cardinality: None,
            edges: IndexRange {
                start: prepare_edge,
                count: 1,
            },
            origin,
        }));
        let apply_edges = builder.raw.edges.len() as u32;
        for element in 0..2 {
            must(builder.push_edge(Edge {
                producer: prepare,
                argument_position: element + 1,
                access: ValueAccess::TupleElement(element),
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::ImmutableBorrow,
                origin,
            }));
        }
        let branch_root = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                primitive_origin: origin,
                lift: LiftMode::Scalar,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: None,
                    dynamic_checks: IndexRange::default(),
                },
            },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange {
                start: apply_edges,
                count: 2,
            },
            origin,
        }));
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: None,
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        must(builder.push_branch(FanOutBranch {
            nodes: IndexRange { start: 3, count: 2 },
            root: branch_root,
            placeholder_origin: origin,
            origin,
        }));
        let fan_out = must(builder.push_node(Node {
            kind: NodeKind::FanOut {
                branches: IndexRange { start: 0, count: 1 },
                keyword_origin: origin,
            },
            result_type,
            cardinality: None,
            edges: IndexRange {
                start: apply_edges + 2,
                count: 1,
            },
            origin,
        }));
        for (owner, release_after) in [
            (first_node, ReleaseAfter::Node(operand)),
            (second_node, ReleaseAfter::Node(operand)),
            (operand, ReleaseAfter::Node(fan_out)),
            (prepare, ReleaseAfter::Node(branch_root)),
            (branch_root, ReleaseAfter::Node(fan_out)),
            (fan_out, ReleaseAfter::Root(RootIndex(0))),
        ] {
            must(builder.push_ownership(Ownership {
                owner,
                release_after,
            }));
        }
        must(builder.push_root(Root {
            node: fan_out,
            origin,
        }));
        must(builder.finish())
    }

    fn verify_error(program: RawProgram, invariant: Invariant) {
        match program.verify() {
            Ok(_) => panic!("fixture unexpectedly verified"),
            Err(VerifyError::MalformedProgram(error)) => {
                assert_eq!(error.invariant, invariant, "{error:?}");
            }
            Err(error) => panic!("expected malformed program, got {error:?}"),
        }
    }

    #[test]
    fn valid_fixtures_cover_every_node_and_edge_family() {
        for program in [
            scalar_program(),
            parameter_program(),
            tuple_program(),
            vector_apply_program(),
            iota_program(),
            dynamic_shape_program(),
            prefix_spread_program(),
            fan_out_program(),
        ] {
            assert!(program.verify().is_ok());
        }
    }

    #[test]
    fn version_feature_and_table_range_invariants_are_deterministic() {
        let mut version = scalar_program();
        version.module.semantic_major = 2;
        verify_error(version, Invariant::UnsupportedVersion);

        let mut unknown = scalar_program();
        unknown.features.push(99);
        unknown.module.ranges.features.count = 1;
        verify_error(unknown, Invariant::UnknownFeature);

        let mut duplicate = scalar_program();
        duplicate.features = vec![1, 1];
        duplicate.module.ranges.features.count = 2;
        verify_error(duplicate, Invariant::DuplicateFeature);

        let mut overflow = scalar_program();
        overflow.module.ranges.nodes = IndexRange {
            start: u32::MAX,
            count: 1,
        };
        verify_error(overflow, Invariant::RangeOverflow);

        let mut mismatch = scalar_program();
        mismatch.module.ranges.nodes.count = 2;
        verify_error(mismatch, Invariant::RangeMismatch);
    }

    #[test]
    fn provenance_parameter_type_and_constant_invariants_are_rejected() {
        let mut origin = scalar_program();
        origin.origins[0].span.begin.offset = 0;
        verify_error(origin, Invariant::InvalidRecord);

        let mut parameter = parameter_program();
        parameter.parameters[0].slot = 1;
        verify_error(parameter, Invariant::InvalidRecord);

        let mut header = parameter_program();
        header.module.parameter_header_origin = None;
        verify_error(header, Invariant::InvalidRecord);

        let mut forward_type = tuple_program();
        forward_type.type_elements[0] = TypeIndex(1);
        verify_error(forward_type, Invariant::NonPostorderReference);

        let mut constant = scalar_program();
        constant.constants[0] =
            ConstantRecord::Scalar(ScalarConstant::DoubleBits(0x7ff0_0000_0000_0001));
        verify_error(constant, Invariant::InvalidRecord);
    }

    fn verify_origin_span_error(program: RawProgram) {
        assert_eq!(
            program.verify(),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::InvalidRecord,
                record: RecordKind::Origin,
                index: Some(0),
                field: "span",
            }))
        );
    }

    #[test]
    fn origin_spans_follow_semantic_source_position_order() {
        let mut line_reversal = scalar_program();
        line_reversal.origins[0].span.begin.line = 2;
        line_reversal.origins[0].span.begin.column = 8;
        verify_origin_span_error(line_reversal);

        let mut column_reversal = scalar_program();
        column_reversal.origins[0].span.begin.column = 3;
        verify_origin_span_error(column_reversal);

        let mut cross_line_reset = scalar_program();
        cross_line_reset.origins[0].span.begin.column = 8;
        cross_line_reset.origins[0].span.end.line = 2;
        cross_line_reset.origins[0].span.end.column = 1;
        assert!(cross_line_reset.verify().is_ok());

        let mut empty_cross_line = scalar_program();
        empty_cross_line.origins[0].span.end.offset = empty_cross_line.origins[0].span.begin.offset;
        empty_cross_line.origins[0].span.end.line = 2;
        empty_cross_line.origins[0].span.end.column = 1;
        verify_origin_span_error(empty_cross_line);

        let mut empty_column_mismatch = scalar_program();
        empty_column_mismatch.origins[0].span.end.offset =
            empty_column_mismatch.origins[0].span.begin.offset;
        verify_origin_span_error(empty_column_mismatch);

        let mut empty = scalar_program();
        empty.origins[0].span.end = empty.origins[0].span.begin;
        assert!(empty.verify().is_ok());
    }

    #[test]
    fn node_edge_postorder_reachability_and_ownership_are_rejected() {
        let mut forward = scalar_program();
        forward.edges.push(Edge {
            producer: NodeIndex(0),
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin: OriginIndex(0),
        });
        forward.nodes[0].edges.count = 1;
        forward.module.ranges.edges.count = 1;
        verify_error(forward, Invariant::NonPostorderReference);

        let mut orphan = scalar_program();
        orphan.nodes.push(orphan.nodes[0]);
        orphan.module.ranges.nodes.count = 2;
        verify_error(orphan, Invariant::UnreachableNode);

        let mut alias = scalar_program();
        alias.roots.push(alias.roots[0]);
        alias.module.ranges.roots.count = 2;
        verify_error(alias, Invariant::AmbiguousOwnership);

        let mut bad_edge = tuple_program();
        bad_edge.edges[0].ownership = OwnershipMode::OwnedInput;
        verify_error(bad_edge, Invariant::AmbiguousOwnership);

        let mut stray_fan_out_borrow = iota_program();
        stray_fan_out_borrow.edges[0].access = ValueAccess::FanOutOperandBorrow;
        stray_fan_out_borrow.edges[0].ownership = OwnershipMode::ImmutableBorrow;
        assert_eq!(
            stray_fan_out_borrow.verify(),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::AmbiguousOwnership,
                record: RecordKind::Edge,
                index: Some(0),
                field: "ownership",
            }))
        );

        let mut cycle = scalar_program();
        cycle.edges.push(Edge {
            producer: NodeIndex(1),
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: None,
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin: OriginIndex(0),
        });
        cycle.nodes[0].edges.count = 1;
        cycle.nodes.push(Node {
            kind: NodeKind::PrefixSpreadPrepare,
            result_type: TypeIndex(0),
            cardinality: None,
            edges: IndexRange { start: 1, count: 0 },
            origin: OriginIndex(0),
        });
        cycle.module.ranges.nodes.count = 2;
        cycle.module.ranges.edges.count = 1;
        verify_error(cycle, Invariant::NonPostorderReference);
    }

    #[test]
    fn identity_result_root_and_feature_invariants_are_rejected() {
        let mut implementation = vector_apply_program();
        let NodeKind::SelectedApply {
            ref mut implementation_id,
            ..
        } = implementation.nodes[2].kind
        else {
            panic!("fixture node kind changed");
        };
        *implementation_id = 35;
        verify_error(implementation, Invariant::InvalidSemanticIdentity);

        let mut bad_primitive_origin = vector_apply_program();
        let NodeKind::SelectedApply {
            primitive_origin: ref mut origin,
            ..
        } = bad_primitive_origin.nodes[2].kind
        else {
            panic!("fixture node kind changed");
        };
        *origin = OriginIndex(9);
        verify_error(bad_primitive_origin, Invariant::IndexOutOfBounds);

        let mut result = vector_apply_program();
        result.nodes[2].cardinality = Some(Cardinality::StaticVector(3));
        verify_error(result, Invariant::InconsistentResultMetadata);

        let mut shape = dynamic_shape_program();
        shape.shape_checks.swap(0, 1);
        verify_error(shape, Invariant::InconsistentResultMetadata);

        let mut root = scalar_program();
        root.roots[0].origin = OriginIndex(9);
        verify_error(root, Invariant::IndexOutOfBounds);

        let mut root_node = scalar_program();
        root_node.roots[0].node = NodeIndex(9);
        verify_error(root_node, Invariant::IndexOutOfBounds);

        let mut feature = tuple_program();
        feature.features.clear();
        feature.module.ranges.features.count = 0;
        verify_error(feature, Invariant::MissingFeature);
    }

    #[test]
    fn fan_out_region_placeholder_and_result_invariants_are_rejected() {
        let mut placeholder = fan_out_program();
        placeholder.edges[0].access = ValueAccess::WholeValue;
        placeholder.edges[0].ownership = OwnershipMode::OwnedInput;
        verify_error(placeholder, Invariant::InvalidRecord);

        let mut region = fan_out_program();
        region.branches[0].nodes.count = 2;
        verify_error(region, Invariant::NonPostorderReference);

        let mut result = fan_out_program();
        result.nodes[2].result_type = TypeIndex(0);
        verify_error(result, Invariant::InconsistentResultMetadata);

        let mut bad_keyword_origin = fan_out_program();
        let NodeKind::FanOut {
            keyword_origin: ref mut origin,
            ..
        } = bad_keyword_origin.nodes[2].kind
        else {
            panic!("fixture node kind changed");
        };
        *origin = OriginIndex(9);
        verify_error(bad_keyword_origin, Invariant::IndexOutOfBounds);
    }

    #[test]
    fn builder_reports_synthetic_refusal_and_checked_count_overflow() {
        let mut builder = RawProgramBuilder::with_reservation_failure_at(0);
        assert!(matches!(
            builder.push_feature(Feature::Tuples.numeric()),
            Err(BuildError::AllocationUnavailable {
                arena: Arena::Feature
            })
        ));
        assert!(matches!(
            checked_count(u64::from(u32::MAX) + 1, Arena::Node),
            Err(BuildError::CountOverflow { arena: Arena::Node })
        ));
    }

    fn vector_prefix_spread_program(left_dynamic: bool, right_dynamic: bool) -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [
            Feature::StableSemanticIds,
            Feature::Tuples,
            Feature::PrefixSpread,
        ] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let vector_type = must(builder.push_type(TypeRecord::Vector(ScalarType::Int)));
        must(builder.push_type_element(vector_type));
        must(builder.push_type_element(vector_type));
        let tuple_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange { start: 0, count: 2 },
        }));

        let mut elements = Vec::new();
        let mut releases = Vec::new();
        for dynamic in [left_dynamic, right_dynamic] {
            if dynamic {
                let bound_value =
                    must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(2))));
                let bound = must(builder.push_node(Node {
                    kind: NodeKind::Constant {
                        constant: bound_value,
                    },
                    result_type: int_type,
                    cardinality: Some(Cardinality::StaticScalar),
                    edges: IndexRange {
                        start: builder.raw.edges.len() as u32,
                        count: 0,
                    },
                    origin,
                }));
                let edge_start = builder.raw.edges.len() as u32;
                must(builder.push_edge(Edge {
                    producer: bound,
                    argument_position: 1,
                    access: ValueAccess::WholeValue,
                    cardinality: Some(Cardinality::StaticScalar),
                    conversion: Conversion::Identity,
                    ownership: OwnershipMode::OwnedInput,
                    origin,
                }));
                let vector = must(builder.push_node(Node {
                    kind: NodeKind::SelectedApply {
                        primitive_id: 19,
                        signature_id: 34,
                        implementation_id: 34,
                        primitive_origin: origin,
                        lift: LiftMode::DynamicVector,
                        result_element_type: ScalarType::Int,
                        shape: ShapePlan {
                            static_anchor: None,
                            dynamic_checks: IndexRange::default(),
                        },
                    },
                    result_type: vector_type,
                    cardinality: Some(Cardinality::DynamicVector),
                    edges: IndexRange {
                        start: edge_start,
                        count: 1,
                    },
                    origin,
                }));
                releases.push((bound, ReleaseAfter::Node(vector)));
                elements.push((vector, Cardinality::DynamicVector));
            } else {
                let element_start = builder.raw.constant_elements.len() as u32;
                must(builder.push_constant_element(ScalarConstant::Int(1)));
                must(builder.push_constant_element(ScalarConstant::Int(2)));
                let value = must(builder.push_constant(ConstantRecord::Vector {
                    element_type: ScalarType::Int,
                    elements: IndexRange {
                        start: element_start,
                        count: 2,
                    },
                }));
                let vector = must(builder.push_node(Node {
                    kind: NodeKind::Constant { constant: value },
                    result_type: vector_type,
                    cardinality: Some(Cardinality::StaticVector(2)),
                    edges: IndexRange {
                        start: builder.raw.edges.len() as u32,
                        count: 0,
                    },
                    origin,
                }));
                elements.push((vector, Cardinality::StaticVector(2)));
            }
        }
        let tuple_edge_start = builder.raw.edges.len() as u32;
        for (position, (producer, cardinality)) in elements.iter().copied().enumerate() {
            must(builder.push_edge(Edge {
                producer,
                argument_position: position as u32 + 1,
                access: ValueAccess::WholeValue,
                cardinality: Some(cardinality),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::InfallibleTransfer,
                origin,
            }));
        }
        let tuple = must(builder.push_node(Node {
            kind: NodeKind::TupleConstruct,
            result_type: tuple_type,
            cardinality: None,
            edges: IndexRange {
                start: tuple_edge_start,
                count: 2,
            },
            origin,
        }));
        for (element, _) in &elements {
            releases.push((*element, ReleaseAfter::Node(tuple)));
        }
        let prepare_edge = builder.raw.edges.len() as u32;
        must(builder.push_edge(Edge {
            producer: tuple,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: None,
            conversion: Conversion::Identity,
            ownership: OwnershipMode::InfallibleTransfer,
            origin,
        }));
        let prepare = must(builder.push_node(Node {
            kind: NodeKind::PrefixSpreadPrepare,
            result_type: tuple_type,
            cardinality: None,
            edges: IndexRange {
                start: prepare_edge,
                count: 1,
            },
            origin,
        }));
        releases.push((tuple, ReleaseAfter::Node(prepare)));
        let apply_edges = builder.raw.edges.len() as u32;
        for (position, (_, cardinality)) in elements.iter().copied().enumerate() {
            must(builder.push_edge(Edge {
                producer: prepare,
                argument_position: position as u32 + 1,
                access: ValueAccess::TupleElement(position as u32),
                cardinality: Some(cardinality),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::ImmutableBorrow,
                origin,
            }));
            if cardinality == Cardinality::DynamicVector {
                must(builder.push_shape_check(apply_edges.saturating_add(position as u32)));
            }
        }
        let any_dynamic = left_dynamic || right_dynamic;
        let first_static = if left_dynamic {
            if right_dynamic { None } else { Some(1) }
        } else {
            Some(0)
        };
        let apply = must(builder.push_node(Node {
            kind: NodeKind::SelectedApply {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                primitive_origin: origin,
                lift: LiftMode::Vector,
                result_element_type: ScalarType::Int,
                shape: ShapePlan {
                    static_anchor: first_static,
                    dynamic_checks: IndexRange {
                        start: 0,
                        count: u32::from(left_dynamic) + u32::from(right_dynamic),
                    },
                },
            },
            result_type: vector_type,
            cardinality: Some(if any_dynamic {
                Cardinality::DynamicVector
            } else {
                Cardinality::StaticVector(2)
            }),
            edges: IndexRange {
                start: apply_edges,
                count: 2,
            },
            origin,
        }));
        releases.push((prepare, ReleaseAfter::Node(apply)));
        releases.push((apply, ReleaseAfter::Root(RootIndex(0))));
        releases.sort_by_key(|(owner, _)| owner.0);
        for (owner, release_after) in releases {
            must(builder.push_ownership(Ownership {
                owner,
                release_after,
            }));
        }
        must(builder.push_root(Root {
            node: apply,
            origin,
        }));
        must(builder.finish())
    }

    #[test]
    fn tuple_element_cardinality_drives_all_vector_shape_combinations() {
        assert!(heterogeneous_prefix_spread_program().verify().is_ok());
        for combination in [(false, false), (true, false), (true, true)] {
            let result = vector_prefix_spread_program(combination.0, combination.1).verify();
            assert!(result.is_ok(), "{combination:?}: {result:?}");
        }

        let mut cardinality = vector_prefix_spread_program(false, false);
        let last = cardinality.edges.len() - 1;
        cardinality.edges[last].cardinality = Some(Cardinality::DynamicVector);
        verify_error(cardinality, Invariant::InconsistentResultMetadata);

        let mut bad_shape = vector_prefix_spread_program(true, false);
        let Some(last_node) = bad_shape.nodes.last_mut() else {
            panic!("fixture apply missing");
        };
        let NodeKind::SelectedApply { ref mut shape, .. } = last_node.kind else {
            panic!("fixture apply kind changed");
        };
        shape.static_anchor = Some(0);
        verify_error(bad_shape, Invariant::InconsistentResultMetadata);
    }

    #[test]
    fn prefix_spread_grouping_order_and_aliases_are_explicit() {
        let mut duplicate = prefix_spread_program();
        duplicate.edges[4].access = ValueAccess::TupleElement(0);
        verify_error(duplicate, Invariant::InvalidRecord);

        let mut permuted = prefix_spread_program();
        permuted.edges[3].access = ValueAccess::TupleElement(1);
        permuted.edges[4].access = ValueAccess::TupleElement(0);
        verify_error(permuted, Invariant::InvalidRecord);

        let mut partial = prefix_spread_program();
        partial.edges.remove(4);
        partial.nodes[4].edges.count = 1;
        partial.module.ranges.edges.count = 4;
        verify_error(partial, Invariant::InvalidRecord);

        let mut mixed = prefix_spread_program();
        mixed.edges[4].producer = NodeIndex(2);
        verify_error(mixed, Invariant::InvalidRecord);

        assert_eq!(
            shared_prefix_spread_program().verify(),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::AmbiguousOwnership,
                record: RecordKind::Edge,
                index: Some(5),
                field: "ownership",
            }))
        );
    }

    #[test]
    fn verifier_allocation_refusal_is_distinct_at_every_scratch_site() {
        for site in [
            VerifyAllocationSite::DynamicShapeScratch,
            VerifyAllocationSite::ReachabilityBits,
            VerifyAllocationSite::ReachabilityWorklist,
            VerifyAllocationSite::FanOutBorrowContext,
            VerifyAllocationSite::OwnershipSinks,
            VerifyAllocationSite::OwnershipLastUse,
            VerifyAllocationSite::OwnershipRootOwner,
        ] {
            assert_eq!(
                dynamic_shape_program()
                    .verify_with_allocation_failure(VerifyAllocationFailureInjection::at(site),),
                Err(VerifyError::AllocationUnavailable { site })
            );
        }
    }

    fn parameterized_tuple_program(ownership: OwnershipMode) -> RawProgram {
        let mut program = tuple_program();
        program.module.parameter_header_origin = Some(OriginIndex(0));
        program.parameters.push(Parameter {
            slot: 0,
            name: "input".to_owned(),
            scalar_type: ScalarType::Int,
            declaration_origin: OriginIndex(0),
            name_origin: OriginIndex(0),
        });
        program.module.ranges.parameters.count = 1;
        program.nodes[0].kind = NodeKind::ParameterBorrow {
            parameter: ParameterIndex(0),
        };
        program.edges[0].ownership = ownership;
        program.ownership.remove(0);
        program.module.ranges.ownership.count -= 1;
        program
    }

    fn parameterized_fan_out_program(ownership: OwnershipMode) -> RawProgram {
        let mut program = fan_out_program();
        program.module.parameter_header_origin = Some(OriginIndex(0));
        program.parameters.push(Parameter {
            slot: 0,
            name: "input".to_owned(),
            scalar_type: ScalarType::Int,
            declaration_origin: OriginIndex(0),
            name_origin: OriginIndex(0),
        });
        program.module.ranges.parameters.count = 1;
        program.nodes[0].kind = NodeKind::ParameterBorrow {
            parameter: ParameterIndex(0),
        };
        program.edges[1].ownership = ownership;
        program.ownership.remove(0);
        program.module.ranges.ownership.count -= 1;
        program
    }

    #[test]
    fn parameter_borrows_materialize_in_tuples_and_feed_fan_out_without_ownership() {
        assert!(
            parameterized_tuple_program(OwnershipMode::ImmutableBorrow)
                .verify()
                .is_ok()
        );
        verify_error(
            parameterized_tuple_program(OwnershipMode::InfallibleTransfer),
            Invariant::AmbiguousOwnership,
        );

        assert!(
            parameterized_fan_out_program(OwnershipMode::ImmutableBorrow)
                .verify()
                .is_ok()
        );
        verify_error(
            parameterized_fan_out_program(OwnershipMode::OwnedInput),
            Invariant::AmbiguousOwnership,
        );
    }

    #[test]
    fn verifier_category_winners_follow_the_normative_order() {
        let mut version_over_range = scalar_program();
        version_over_range.module.semantic_major = 2;
        version_over_range.module.ranges.nodes.count = 2;
        verify_error(version_over_range, Invariant::UnsupportedVersion);

        let mut range_over_record = parameter_program();
        range_over_record.module.ranges.nodes.count = 2;
        range_over_record.parameters[0].slot = 9;
        verify_error(range_over_record, Invariant::RangeMismatch);

        let mut record_over_reference = parameter_program();
        record_over_reference.parameters[0].slot = 9;
        record_over_reference.nodes[0].result_type = TypeIndex(99);
        verify_error(record_over_reference, Invariant::InvalidRecord);

        let mut reference_over_metadata = vector_apply_program();
        reference_over_metadata.edges[0].producer = NodeIndex(2);
        reference_over_metadata.nodes[2].cardinality = Some(Cardinality::StaticVector(9));
        verify_error(reference_over_metadata, Invariant::NonPostorderReference);

        let mut metadata_over_ownership = vector_apply_program();
        metadata_over_ownership.nodes[2].cardinality = Some(Cardinality::StaticVector(9));
        metadata_over_ownership.edges[0].ownership = OwnershipMode::ImmutableBorrow;
        verify_error(
            metadata_over_ownership,
            Invariant::InconsistentResultMetadata,
        );

        let mut ownership_over_features = tuple_program();
        ownership_over_features.edges[0].ownership = OwnershipMode::OwnedInput;
        ownership_over_features.features.clear();
        ownership_over_features.module.ranges.features.count = 0;
        verify_error(ownership_over_features, Invariant::AmbiguousOwnership);
    }

    #[test]
    fn fan_out_rejects_non_apply_roots_and_placeholder_aggregates() {
        assert!(fan_out_program().verify().is_ok());
        let prefix = fan_out_prefix_spread_program().verify();
        assert!(prefix.is_ok(), "{prefix:?}");

        let mut constant_root = fan_out_program();
        constant_root.nodes[1].kind = NodeKind::Constant {
            constant: ConstantIndex(0),
        };
        constant_root.nodes[1].edges = IndexRange::default();
        constant_root.edges.remove(0);
        constant_root.nodes[2].edges.start = 0;
        constant_root.module.ranges.edges.count = 1;
        verify_error(constant_root, Invariant::InvalidRecord);

        let mut aggregate = fan_out_prefix_spread_program();
        aggregate.nodes[3].kind = NodeKind::TupleConstruct;
        verify_error(aggregate, Invariant::InvalidRecord);
    }

    fn deep_tuple_program(depth: u32) -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::Tuples.numeric()));
        let origin = source_fixture(&mut builder);
        let mut current_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        for index in 0..depth {
            must(builder.push_type_element(current_type));
            current_type = must(builder.push_type(TypeRecord::Tuple {
                elements: IndexRange {
                    start: index,
                    count: 1,
                },
            }));
        }
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let mut current_node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: TypeIndex(0),
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        for index in 0..depth {
            must(builder.push_edge(Edge {
                producer: current_node,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: if index == 0 {
                    Some(Cardinality::StaticScalar)
                } else {
                    None
                },
                conversion: Conversion::Identity,
                ownership: OwnershipMode::InfallibleTransfer,
                origin,
            }));
            let next_node = must(builder.push_node(Node {
                kind: NodeKind::TupleConstruct,
                result_type: TypeIndex(index + 1),
                cardinality: None,
                edges: IndexRange {
                    start: index,
                    count: 1,
                },
                origin,
            }));
            must(builder.push_ownership(Ownership {
                owner: current_node,
                release_after: ReleaseAfter::Node(next_node),
            }));
            current_node = next_node;
        }
        must(builder.push_ownership(Ownership {
            owner: current_node,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: current_node,
            origin,
        }));
        must(builder.finish())
    }

    fn deep_unary_program(depth: u32) -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::StableSemanticIds.numeric()));
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let mut current = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        for index in 0..depth {
            must(builder.push_edge(Edge {
                producer: current,
                argument_position: 1,
                access: ValueAccess::WholeValue,
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::OwnedInput,
                origin,
            }));
            let next = must(builder.push_node(Node {
                kind: NodeKind::SelectedApply {
                    primitive_id: 1,
                    signature_id: 1,
                    implementation_id: 1,
                    primitive_origin: origin,
                    lift: LiftMode::Scalar,
                    result_element_type: ScalarType::Int,
                    shape: ShapePlan {
                        static_anchor: None,
                        dynamic_checks: IndexRange::default(),
                    },
                },
                result_type: int_type,
                cardinality: Some(Cardinality::StaticScalar),
                edges: IndexRange {
                    start: index,
                    count: 1,
                },
                origin,
            }));
            must(builder.push_ownership(Ownership {
                owner: current,
                release_after: ReleaseAfter::Node(next),
            }));
            current = next;
        }
        must(builder.push_ownership(Ownership {
            owner: current,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: current,
            origin,
        }));
        must(builder.finish())
    }

    fn wide_fan_out_program(branch_count: u32) -> RawProgram {
        let mut builder = RawProgramBuilder::new();
        for feature in [Feature::StableSemanticIds, Feature::Tuples, Feature::FanOut] {
            must(builder.push_feature(feature.numeric()));
        }
        let origin = source_fixture(&mut builder);
        let int_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Int)));
        for _ in 0..branch_count {
            must(builder.push_type_element(int_type));
        }
        let tuple_type = must(builder.push_type(TypeRecord::Tuple {
            elements: IndexRange {
                start: 0,
                count: branch_count,
            },
        }));
        let constant = must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Int(1))));
        let operand = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: int_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: IndexRange::default(),
            origin,
        }));
        let mut branch_roots = Vec::new();
        must(branch_roots.try_reserve_exact(branch_count as usize));
        for index in 0..branch_count {
            must(builder.push_edge(Edge {
                producer: operand,
                argument_position: 1,
                access: ValueAccess::FanOutOperandBorrow,
                cardinality: Some(Cardinality::StaticScalar),
                conversion: Conversion::Identity,
                ownership: OwnershipMode::ImmutableBorrow,
                origin,
            }));
            let branch = must(builder.push_node(Node {
                kind: NodeKind::SelectedApply {
                    primitive_id: 1,
                    signature_id: 1,
                    implementation_id: 1,
                    primitive_origin: origin,
                    lift: LiftMode::Scalar,
                    result_element_type: ScalarType::Int,
                    shape: ShapePlan {
                        static_anchor: None,
                        dynamic_checks: IndexRange::default(),
                    },
                },
                result_type: int_type,
                cardinality: Some(Cardinality::StaticScalar),
                edges: IndexRange {
                    start: index,
                    count: 1,
                },
                origin,
            }));
            must(builder.push_branch(FanOutBranch {
                nodes: IndexRange {
                    start: index + 1,
                    count: 1,
                },
                root: branch,
                placeholder_origin: origin,
                origin,
            }));
            branch_roots.push(branch);
        }
        must(builder.push_edge(Edge {
            producer: operand,
            argument_position: 1,
            access: ValueAccess::WholeValue,
            cardinality: Some(Cardinality::StaticScalar),
            conversion: Conversion::Identity,
            ownership: OwnershipMode::OwnedInput,
            origin,
        }));
        let fan_out = must(builder.push_node(Node {
            kind: NodeKind::FanOut {
                branches: IndexRange {
                    start: 0,
                    count: branch_count,
                },
                keyword_origin: origin,
            },
            result_type: tuple_type,
            cardinality: None,
            edges: IndexRange {
                start: branch_count,
                count: 1,
            },
            origin,
        }));
        must(builder.push_ownership(Ownership {
            owner: operand,
            release_after: ReleaseAfter::Node(fan_out),
        }));
        for branch in branch_roots {
            must(builder.push_ownership(Ownership {
                owner: branch,
                release_after: ReleaseAfter::Node(fan_out),
            }));
        }
        must(builder.push_ownership(Ownership {
            owner: fan_out,
            release_after: ReleaseAfter::Root(RootIndex(0)),
        }));
        must(builder.push_root(Root {
            node: fan_out,
            origin,
        }));
        must(builder.finish())
    }

    #[test]
    fn fan_out_context_construction_visits_wide_regions_linearly() {
        for (width, expected) in [(64, 64_u64), (128, 128_u64)] {
            let program = wide_fan_out_program(width);
            let mut visits = FanOutContextVisits::default();
            let context = build_fan_out_borrow_context(
                &program,
                VerifyAllocationFailureInjection::none(),
                &mut visits,
            );
            assert!(context.is_ok());
            assert_eq!(
                visits,
                FanOutContextVisits {
                    fan_outs: 1,
                    branch_nodes: expected,
                    edges: expected,
                }
            );
        }

        let mut wrong_operand = wide_fan_out_program(2);
        wrong_operand.edges[1].producer = NodeIndex(1);
        assert_eq!(
            wrong_operand.verify(),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::AmbiguousOwnership,
                record: RecordKind::Edge,
                index: Some(1),
                field: "ownership",
            }))
        );
    }

    #[test]
    fn nested_fan_out_is_rejected_before_overlapping_context_scans() {
        assert!(sibling_fan_out_program().verify().is_ok());
        assert_eq!(
            two_branch_nested_fan_out_program().verify(),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::InvalidRecord,
                record: RecordKind::Branch,
                index: Some(2),
                field: "nested_fan_out",
            }))
        );

        for width in [64_u32, 128_u32] {
            let program = wide_fan_out_program(width);
            let mut visits = NestedFanOutVisits::default();
            assert!(verify_no_nested_fan_out_with_visits(&program, &mut visits).is_ok());
            assert_eq!(
                visits,
                NestedFanOutVisits {
                    nodes: u64::from(width) + 2,
                    region_nodes: u64::from(width) + 1,
                    branches: 0,
                }
            );
        }

        let mut program = wide_fan_out_program(4);
        program.nodes[1].kind = NodeKind::FanOut {
            branches: IndexRange::default(),
            keyword_origin: OriginIndex(0),
        };
        program.nodes[3].kind = NodeKind::FanOut {
            branches: IndexRange::default(),
            keyword_origin: OriginIndex(0),
        };
        program.edges[2].producer = NodeIndex(2);
        let mut visits = NestedFanOutVisits::default();
        assert_eq!(
            verify_no_nested_fan_out_with_visits(&program, &mut visits),
            Err(VerifyError::MalformedProgram(MalformedProgram {
                invariant: Invariant::InvalidRecord,
                record: RecordKind::Branch,
                index: Some(0),
                field: "nested_fan_out",
            }))
        );
        assert_eq!(
            visits,
            NestedFanOutVisits {
                nodes: 6,
                region_nodes: 4,
                branches: 1,
            }
        );
    }

    #[test]
    fn deep_structures_verify_on_a_reduced_stack() {
        let result = std::thread::Builder::new().stack_size(64 * 1024).spawn(|| {
            assert!(deep_tuple_program(4_000).verify().is_ok());
            assert!(deep_unary_program(4_000).verify().is_ok());
            assert!(wide_fan_out_program(2_000).verify().is_ok());
        });
        match result {
            Ok(handle) => assert!(handle.join().is_ok()),
            Err(error) => panic!("failed to create reduced-stack verifier thread: {error}"),
        }
    }
}
