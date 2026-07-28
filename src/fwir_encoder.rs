use crate::{
    Cardinality, ConstantRecord, Conversion, LiftMode, NodeKind, OwnershipMode, ReleaseAfter,
    ScalarConstant, ScalarType, TypeRecord, ValueAccess, VerifiedProgram,
};
use std::collections::TryReserveError;
use std::io::{self, Write};

const MAGIC: &[u8; 8] = b"FWIR\r\n\x1a\n";
const HEADER_SIZE: u64 = 32;
const DIRECTORY_ENTRY_SIZE: u64 = 24;
const MANDATORY_IDENTITY_FLAGS: u16 = 3;
const NONE: u32 = u32::MAX;
const MAX_SECTIONS: usize = 17;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FwirProducerMetadata {
    WithoutSourceDigest,
    Sha256([u8; 32]),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FwirEncodeOptions {
    pub producer_metadata: Option<FwirProducerMetadata>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FwirEncodeAllocationSite {
    StringPool,
    Artifact,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FwirEncodeAllocationFailureInjection {
    fail_at: Option<FwirEncodeAllocationSite>,
}

impl FwirEncodeAllocationFailureInjection {
    pub const fn none() -> Self {
        Self { fail_at: None }
    }

    #[doc(hidden)]
    pub const fn at(site: FwirEncodeAllocationSite) -> Self {
        Self {
            fail_at: Some(site),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FwirOutputOperation {
    Write,
    Flush,
    Publish,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FwirEncodeError {
    CountOverflow {
        field: &'static str,
    },
    SizeOverflow {
        field: &'static str,
    },
    AllocationUnavailable {
        site: FwirEncodeAllocationSite,
    },
    InvalidProducerVersion,
    Output {
        operation: FwirOutputOperation,
        kind: io::ErrorKind,
    },
}

impl std::fmt::Display for FwirEncodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "FWIR v1 encoding failed: {self:?}")
    }
}

impl std::error::Error for FwirEncodeError {}

#[derive(Clone, Copy, Debug, Default)]
struct Section {
    id: u16,
    record_size: u32,
    length: u64,
}

#[derive(Clone, Copy, Debug)]
struct Preflight {
    sections: [Section; MAX_SECTIONS],
    section_count: usize,
    string_count: u32,
    string_reference_count: usize,
    total_size: usize,
}

pub fn encode_fwir(
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
) -> Result<Vec<u8>, FwirEncodeError> {
    encode_fwir_with_allocation_failure(
        program,
        options,
        FwirEncodeAllocationFailureInjection::none(),
    )
}

#[doc(hidden)]
pub fn encode_fwir_with_allocation_failure(
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
    injection: FwirEncodeAllocationFailureInjection,
) -> Result<Vec<u8>, FwirEncodeError> {
    let plan = preflight(program, options)?;
    let strings = collect_strings(program, &plan, injection)?;
    let mut output = Vec::new();
    reserve_exact(
        &mut output,
        plan.total_size,
        FwirEncodeAllocationSite::Artifact,
        injection,
    )?;
    encode_header_and_directory(&mut output, &plan)?;
    encode_sections(&mut output, program, options, &strings)?;
    if output.len() != plan.total_size {
        return Err(FwirEncodeError::SizeOverflow {
            field: "encoded_artifact",
        });
    }
    Ok(output)
}

pub fn write_fwir(
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
    output: &mut impl Write,
) -> Result<(), FwirEncodeError> {
    let bytes = encode_fwir(program, options)?;
    output
        .write_all(&bytes)
        .map_err(|error| FwirEncodeError::Output {
            operation: FwirOutputOperation::Write,
            kind: error.kind(),
        })?;
    output.flush().map_err(|error| FwirEncodeError::Output {
        operation: FwirOutputOperation::Flush,
        kind: error.kind(),
    })
}

/// Encodes completely before calling a publication operation.
///
/// The callback must atomically commit the supplied complete byte slice or
/// return an error without publishing it.
pub fn encode_fwir_with_atomic_publication(
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
    publish: impl FnOnce(&[u8]) -> io::Result<()>,
) -> Result<(), FwirEncodeError> {
    let bytes = encode_fwir(program, options)?;
    publish(&bytes).map_err(|error| FwirEncodeError::Output {
        operation: FwirOutputOperation::Publish,
        kind: error.kind(),
    })
}

fn reserve_exact<T>(
    values: &mut Vec<T>,
    count: usize,
    site: FwirEncodeAllocationSite,
    injection: FwirEncodeAllocationFailureInjection,
) -> Result<(), FwirEncodeError> {
    if injection.fail_at == Some(site) {
        return Err(FwirEncodeError::AllocationUnavailable { site });
    }
    values
        .try_reserve_exact(count)
        .map_err(|_: TryReserveError| FwirEncodeError::AllocationUnavailable { site })
}

fn checked_count(value: usize, field: &'static str) -> Result<u32, FwirEncodeError> {
    u32::try_from(value).map_err(|_| FwirEncodeError::CountOverflow { field })
}

fn checked_mul(left: u64, right: u64, field: &'static str) -> Result<u64, FwirEncodeError> {
    left.checked_mul(right)
        .ok_or(FwirEncodeError::SizeOverflow { field })
}

fn checked_add(left: u64, right: u64, field: &'static str) -> Result<u64, FwirEncodeError> {
    left.checked_add(right)
        .ok_or(FwirEncodeError::SizeOverflow { field })
}

fn fixed_length(
    count: usize,
    record_size: u32,
    field: &'static str,
) -> Result<u64, FwirEncodeError> {
    let count = u64::from(checked_count(count, field)?);
    checked_mul(count, u64::from(record_size), field)
}

fn name_already_seen(
    program: &VerifiedProgram,
    source_limit: usize,
    parameter_limit: usize,
    name: &str,
) -> bool {
    let raw = program.as_raw();
    raw.source_units[..source_limit]
        .iter()
        .any(|source| source.diagnostic_name == name)
        || raw.parameters[..parameter_limit]
            .iter()
            .any(|parameter| parameter.name == name)
}

fn string_pool_shape(program: &VerifiedProgram) -> Result<(u32, usize, u64), FwirEncodeError> {
    let raw = program.as_raw();
    let reference_count = raw
        .source_units
        .len()
        .checked_add(raw.parameters.len())
        .ok_or(FwirEncodeError::CountOverflow {
            field: "string_references",
        })?;
    let mut count = 0_u32;
    let mut bytes = 0_u64;
    for (index, source) in raw.source_units.iter().enumerate() {
        if !name_already_seen(program, index, 0, &source.diagnostic_name) {
            checked_count(source.diagnostic_name.len(), "string")?;
            count = count
                .checked_add(1)
                .ok_or(FwirEncodeError::CountOverflow { field: "strings" })?;
            bytes = checked_add(
                bytes,
                u64::try_from(source.diagnostic_name.len())
                    .map_err(|_| FwirEncodeError::SizeOverflow { field: "strings" })?,
                "strings",
            )?;
        }
    }
    for (index, parameter) in raw.parameters.iter().enumerate() {
        if !name_already_seen(program, raw.source_units.len(), index, &parameter.name) {
            checked_count(parameter.name.len(), "string")?;
            count = count
                .checked_add(1)
                .ok_or(FwirEncodeError::CountOverflow { field: "strings" })?;
            bytes = checked_add(
                bytes,
                u64::try_from(parameter.name.len())
                    .map_err(|_| FwirEncodeError::SizeOverflow { field: "strings" })?,
                "strings",
            )?;
        }
    }
    u32::try_from(bytes).map_err(|_| FwirEncodeError::SizeOverflow {
        field: "string_bytes",
    })?;
    let descriptors = checked_mul(u64::from(count), 8, "strings")?;
    let payload = checked_add(checked_add(4, descriptors, "strings")?, bytes, "strings")?;
    Ok((count, reference_count, payload))
}

fn add_section(
    sections: &mut [Section; MAX_SECTIONS],
    count: &mut usize,
    id: u16,
    record_size: u32,
    length: u64,
) {
    sections[*count] = Section {
        id,
        record_size,
        length,
    };
    *count += 1;
}

fn preflight(
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
) -> Result<Preflight, FwirEncodeError> {
    let raw = program.as_raw();
    checked_count(raw.features.len(), "features")?;
    checked_count(raw.source_units.len(), "source_units")?;
    checked_count(raw.parameters.len(), "parameters")?;
    checked_count(raw.types.len(), "types")?;
    checked_count(raw.type_elements.len(), "type_elements")?;
    checked_count(raw.constants.len(), "constants")?;
    checked_count(raw.constant_elements.len(), "constant_elements")?;
    checked_count(raw.origins.len(), "origins")?;
    checked_count(raw.edges.len(), "edges")?;
    checked_count(raw.shape_checks.len(), "shape_checks")?;
    checked_count(raw.branches.len(), "branches")?;
    checked_count(raw.nodes.len(), "nodes")?;
    checked_count(raw.ownership.len(), "ownership")?;
    checked_count(raw.roots.len(), "roots")?;

    let (string_count, string_reference_count, string_length) = string_pool_shape(program)?;
    let mut sections = [Section::default(); MAX_SECTIONS];
    let mut section_count = 0;
    add_section(&mut sections, &mut section_count, 1, 8, 8);
    let lengths = [
        (2, 4, fixed_length(raw.features.len(), 4, "features")?),
        (
            4,
            8,
            fixed_length(raw.source_units.len(), 8, "source_units")?,
        ),
        (5, 20, fixed_length(raw.parameters.len(), 20, "parameters")?),
        (6, 12, fixed_length(raw.types.len(), 12, "types")?),
        (
            7,
            4,
            fixed_length(raw.type_elements.len(), 4, "type_elements")?,
        ),
        (8, 20, fixed_length(raw.constants.len(), 20, "constants")?),
        (
            9,
            12,
            fixed_length(raw.constant_elements.len(), 12, "constant_elements")?,
        ),
        (10, 28, fixed_length(raw.origins.len(), 28, "origins")?),
        (11, 24, fixed_length(raw.edges.len(), 24, "edges")?),
        (
            12,
            4,
            fixed_length(raw.shape_checks.len(), 4, "shape_checks")?,
        ),
        (13, 20, fixed_length(raw.branches.len(), 20, "branches")?),
        (14, 56, fixed_length(raw.nodes.len(), 56, "nodes")?),
        (15, 12, fixed_length(raw.ownership.len(), 12, "ownership")?),
        (16, 8, fixed_length(raw.roots.len(), 8, "roots")?),
    ];
    for (id, record_size, length) in lengths {
        if id == 4 && string_count != 0 {
            add_section(&mut sections, &mut section_count, 3, 0, string_length);
        }
        if length != 0 {
            add_section(&mut sections, &mut section_count, id, record_size, length);
        }
    }
    if let Some(metadata) = options.producer_metadata {
        let version = crate::VERSION.as_bytes();
        if version.is_empty() || !version.is_ascii() || version.first() == Some(&b'v') {
            return Err(FwirEncodeError::InvalidProducerVersion);
        }
        checked_count(version.len(), "producer_version")?;
        let digest_length = match metadata {
            FwirProducerMetadata::WithoutSourceDigest => 0,
            FwirProducerMetadata::Sha256(_) => 32,
        };
        let producer_length = checked_add(
            checked_add(4, 9, "producer_metadata")?,
            checked_add(
                4,
                u64::try_from(version.len()).map_err(|_| FwirEncodeError::SizeOverflow {
                    field: "producer_metadata",
                })?,
                "producer_metadata",
            )?,
            "producer_metadata",
        )?;
        let producer_length = checked_add(producer_length, 4 + digest_length, "producer_metadata")?;
        add_section(&mut sections, &mut section_count, 32769, 0, producer_length);
    }
    let directory_length = checked_mul(
        u64::try_from(section_count)
            .map_err(|_| FwirEncodeError::CountOverflow { field: "sections" })?,
        DIRECTORY_ENTRY_SIZE,
        "directory",
    )?;
    let mut total = checked_add(HEADER_SIZE, directory_length, "artifact")?;
    for section in &sections[..section_count] {
        total = checked_add(total, section.length, "artifact")?;
    }
    let total_size =
        usize::try_from(total).map_err(|_| FwirEncodeError::SizeOverflow { field: "artifact" })?;
    checked_count(section_count, "sections")?;
    Ok(Preflight {
        sections,
        section_count,
        string_count,
        string_reference_count,
        total_size,
    })
}

fn collect_strings<'a>(
    program: &'a VerifiedProgram,
    plan: &Preflight,
    injection: FwirEncodeAllocationFailureInjection,
) -> Result<Vec<&'a str>, FwirEncodeError> {
    let mut strings = Vec::new();
    reserve_exact(
        &mut strings,
        usize::try_from(plan.string_count)
            .map_err(|_| FwirEncodeError::CountOverflow { field: "strings" })?,
        FwirEncodeAllocationSite::StringPool,
        injection,
    )?;
    for source in &program.as_raw().source_units {
        if !strings.contains(&source.diagnostic_name.as_str()) {
            strings.push(&source.diagnostic_name);
        }
    }
    for parameter in &program.as_raw().parameters {
        if !strings.contains(&parameter.name.as_str()) {
            strings.push(&parameter.name);
        }
    }
    strings.sort_unstable_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    if strings.len() > plan.string_reference_count
        || strings.len()
            != usize::try_from(plan.string_count)
                .map_err(|_| FwirEncodeError::CountOverflow { field: "strings" })?
    {
        return Err(FwirEncodeError::CountOverflow { field: "strings" });
    }
    Ok(strings)
}

fn put_u8(output: &mut Vec<u8>, value: u8) {
    output.push(value);
}

fn put_u16(output: &mut Vec<u8>, value: u16) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u32(output: &mut Vec<u8>, value: u32) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn put_u64(output: &mut Vec<u8>, value: u64) {
    output.extend_from_slice(&value.to_le_bytes());
}

fn encode_header_and_directory(
    output: &mut Vec<u8>,
    plan: &Preflight,
) -> Result<(), FwirEncodeError> {
    output.extend_from_slice(MAGIC);
    put_u16(output, 1);
    put_u16(output, 0);
    put_u32(output, 32);
    put_u16(output, 24);
    put_u16(output, 0);
    put_u32(output, checked_count(plan.section_count, "sections")?);
    put_u64(output, HEADER_SIZE);
    let directory_length = checked_mul(
        u64::try_from(plan.section_count)
            .map_err(|_| FwirEncodeError::CountOverflow { field: "sections" })?,
        DIRECTORY_ENTRY_SIZE,
        "directory",
    )?;
    let mut offset = checked_add(HEADER_SIZE, directory_length, "directory")?;
    for section in &plan.sections[..plan.section_count] {
        put_u16(output, section.id);
        put_u16(
            output,
            if section.id == 32769 {
                0
            } else {
                MANDATORY_IDENTITY_FLAGS
            },
        );
        put_u32(output, section.record_size);
        put_u64(output, offset);
        put_u64(output, section.length);
        offset = checked_add(offset, section.length, "artifact")?;
    }
    Ok(())
}

fn scalar_type(value: ScalarType) -> u8 {
    match value {
        ScalarType::Bool => 1,
        ScalarType::Int => 2,
        ScalarType::Double => 3,
    }
}

fn scalar_payload(value: ScalarConstant) -> (u8, u64) {
    match value {
        ScalarConstant::Bool(value) => (1, u64::from(value)),
        ScalarConstant::Int(value) => (2, u64::from_le_bytes(value.to_le_bytes())),
        ScalarConstant::DoubleBits(value) => (3, value),
    }
}

fn cardinality(value: Option<Cardinality>) -> (u8, u32) {
    match value {
        None => (0, 0),
        Some(Cardinality::StaticScalar) => (1, 0),
        Some(Cardinality::StaticVector(length)) => (2, length),
        Some(Cardinality::DynamicVector) => (3, 0),
    }
}

fn string_index(strings: &[&str], value: &str) -> Result<u32, FwirEncodeError> {
    let index = strings
        .binary_search_by(|candidate| candidate.as_bytes().cmp(value.as_bytes()))
        .map_err(|_| FwirEncodeError::CountOverflow {
            field: "string_reference",
        })?;
    checked_count(index, "string_reference")
}

fn encode_sections(
    output: &mut Vec<u8>,
    program: &VerifiedProgram,
    options: &FwirEncodeOptions,
    strings: &[&str],
) -> Result<(), FwirEncodeError> {
    let raw = program.as_raw();
    put_u16(output, raw.module.semantic_major);
    put_u16(output, raw.module.semantic_minor);
    put_u32(
        output,
        raw.module
            .parameter_header_origin
            .map_or(NONE, |origin| origin.0),
    );
    if !raw.features.is_empty() {
        for feature in &raw.features {
            put_u16(output, *feature);
            put_u8(output, 0);
            put_u8(output, 0);
        }
    }
    if !strings.is_empty() {
        put_u32(output, checked_count(strings.len(), "strings")?);
        let mut offset = 0_u32;
        for value in strings {
            put_u32(output, offset);
            let length = checked_count(value.len(), "string")?;
            put_u32(output, length);
            offset = offset
                .checked_add(length)
                .ok_or(FwirEncodeError::SizeOverflow { field: "strings" })?;
        }
        for value in strings {
            output.extend_from_slice(value.as_bytes());
        }
    }
    for source in &raw.source_units {
        put_u32(output, string_index(strings, &source.diagnostic_name)?);
        put_u32(output, source.byte_length);
    }
    for parameter in &raw.parameters {
        put_u32(output, parameter.slot);
        put_u32(output, string_index(strings, &parameter.name)?);
        put_u8(output, scalar_type(parameter.scalar_type));
        output.extend_from_slice(&[0; 3]);
        put_u32(output, parameter.declaration_origin.0);
        put_u32(output, parameter.name_origin.0);
    }
    for value_type in &raw.types {
        match value_type {
            TypeRecord::Scalar(value) => {
                put_u8(output, 1);
                put_u8(output, scalar_type(*value));
                put_u16(output, 0);
                put_u32(output, 0);
                put_u32(output, 0);
            }
            TypeRecord::Vector(value) => {
                put_u8(output, 2);
                put_u8(output, scalar_type(*value));
                put_u16(output, 0);
                put_u32(output, 0);
                put_u32(output, 0);
            }
            TypeRecord::Tuple { elements } => {
                put_u8(output, 3);
                put_u8(output, 0);
                put_u16(output, 0);
                put_u32(output, elements.start);
                put_u32(output, elements.count);
            }
        }
    }
    for element in &raw.type_elements {
        put_u32(output, element.0);
    }
    for constant in &raw.constants {
        match constant {
            ConstantRecord::Scalar(value) => {
                let (scalar, payload) = scalar_payload(*value);
                put_u8(output, 1);
                put_u8(output, scalar);
                put_u16(output, 0);
                put_u32(output, 0);
                put_u32(output, 0);
                put_u64(output, payload);
            }
            ConstantRecord::Vector {
                element_type,
                elements,
            } => {
                put_u8(output, 2);
                put_u8(output, scalar_type(*element_type));
                put_u16(output, 0);
                put_u32(output, elements.start);
                put_u32(output, elements.count);
                put_u64(output, 0);
            }
        }
    }
    for element in &raw.constant_elements {
        let (scalar, payload) = scalar_payload(*element);
        put_u8(output, scalar);
        output.extend_from_slice(&[0; 3]);
        put_u64(output, payload);
    }
    for origin in &raw.origins {
        put_u32(output, origin.source_unit.0);
        put_u32(output, origin.span.begin.offset);
        put_u32(output, origin.span.begin.line);
        put_u32(output, origin.span.begin.column);
        put_u32(output, origin.span.end.offset);
        put_u32(output, origin.span.end.line);
        put_u32(output, origin.span.end.column);
    }
    for edge in &raw.edges {
        put_u32(output, edge.producer.0);
        put_u32(output, edge.argument_position);
        let access = match edge.access {
            ValueAccess::WholeValue => (1, 0),
            ValueAccess::TupleElement(index) => (2, index),
            ValueAccess::FanOutOperandBorrow => (3, 0),
        };
        let (cardinality, cardinality_length) = cardinality(edge.cardinality);
        put_u8(output, access.0);
        put_u8(output, cardinality);
        put_u8(
            output,
            match edge.conversion {
                Conversion::Identity => 1,
                Conversion::PromoteIntToDouble => 2,
            },
        );
        put_u8(
            output,
            match edge.ownership {
                OwnershipMode::OwnedInput => 1,
                OwnershipMode::ImmutableBorrow => 2,
                OwnershipMode::InfallibleTransfer => 3,
            },
        );
        put_u32(output, access.1);
        put_u32(output, cardinality_length);
        put_u32(output, edge.origin.0);
    }
    for position in &raw.shape_checks {
        put_u32(output, *position);
    }
    for branch in &raw.branches {
        put_u32(output, branch.nodes.start);
        put_u32(output, branch.nodes.count);
        put_u32(output, branch.root.0);
        put_u32(output, branch.placeholder_origin.0);
        put_u32(output, branch.origin.0);
    }
    for node in &raw.nodes {
        let (cardinality, cardinality_length) = cardinality(node.cardinality);
        let (kind, lift, result_element_type, args) = match node.kind {
            NodeKind::Constant { constant } => (1, 0, 0, [constant.0, 0, 0, 0, 0, 0, 0, 0]),
            NodeKind::ParameterBorrow { parameter } => {
                (2, 0, 0, [parameter.0, 0, 0, 0, 0, 0, 0, 0])
            }
            NodeKind::TupleConstruct => (3, 0, 0, [0; 8]),
            NodeKind::SelectedApply {
                primitive_id,
                signature_id,
                implementation_id,
                primitive_origin,
                lift,
                result_element_type,
                shape,
            } => (
                4,
                match lift {
                    LiftMode::Scalar => 1,
                    LiftMode::Vector => 2,
                    LiftMode::DynamicVector => 3,
                },
                scalar_type(result_element_type),
                [
                    u32::from(primitive_id),
                    u32::from(signature_id),
                    u32::from(implementation_id),
                    primitive_origin.0,
                    shape.static_anchor.unwrap_or(NONE),
                    shape.dynamic_checks.start,
                    shape.dynamic_checks.count,
                    0,
                ],
            ),
            NodeKind::PrefixSpreadPrepare => (5, 0, 0, [0; 8]),
            NodeKind::FanOut {
                branches,
                keyword_origin,
            } => (
                6,
                0,
                0,
                [
                    branches.start,
                    branches.count,
                    keyword_origin.0,
                    0,
                    0,
                    0,
                    0,
                    0,
                ],
            ),
        };
        put_u8(output, kind);
        put_u8(output, cardinality);
        put_u8(output, lift);
        put_u8(output, result_element_type);
        put_u32(output, node.result_type.0);
        put_u32(output, cardinality_length);
        put_u32(output, node.edges.start);
        put_u32(output, node.edges.count);
        put_u32(output, node.origin.0);
        for argument in args {
            put_u32(output, argument);
        }
    }
    for ownership in &raw.ownership {
        put_u32(output, ownership.owner.0);
        match ownership.release_after {
            ReleaseAfter::Node(index) => {
                put_u8(output, 1);
                output.extend_from_slice(&[0; 3]);
                put_u32(output, index.0);
            }
            ReleaseAfter::Root(index) => {
                put_u8(output, 2);
                output.extend_from_slice(&[0; 3]);
                put_u32(output, index.0);
            }
        }
    }
    for root in &raw.roots {
        put_u32(output, root.node.0);
        put_u32(output, root.origin.0);
    }
    if let Some(metadata) = options.producer_metadata {
        put_u32(output, 9);
        output.extend_from_slice(b"faraweave");
        put_u32(
            output,
            checked_count(crate::VERSION.len(), "producer_version")?,
        );
        output.extend_from_slice(crate::VERSION.as_bytes());
        match metadata {
            FwirProducerMetadata::WithoutSourceDigest => {
                put_u16(output, 0);
                put_u16(output, 0);
            }
            FwirProducerMetadata::Sha256(digest) => {
                put_u16(output, 1);
                put_u16(output, 32);
                output.extend_from_slice(&digest);
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ConstantRecord, Node, NodeKind, Origin, OriginPosition, OriginSpan, Ownership,
        RawProgramBuilder, ReleaseAfter, Root, ScalarConstant, SourceUnit, TypeRecord,
    };

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("unexpected error: {error:?}"),
        }
    }

    fn example_bytes(name: &str) -> Vec<u8> {
        let text = match name {
            "empty" => include_str!("../spec/examples/fwir-v1-empty.hex"),
            "scalar-true" => include_str!("../spec/examples/fwir-v1-scalar-true.hex"),
            "complete" => include_str!("../spec/examples/fwir-v1-complete.hex"),
            _ => panic!("unknown example"),
        };
        must(
            text.split_ascii_whitespace()
                .map(|byte| u8::from_str_radix(byte, 16))
                .collect(),
        )
    }

    fn empty_program() -> VerifiedProgram {
        must(must(RawProgramBuilder::new().finish()).verify())
    }

    fn scalar_true_program() -> VerifiedProgram {
        let mut builder = RawProgramBuilder::new();
        let source_unit = must(builder.push_source_unit(SourceUnit {
            diagnostic_name: "example.fw".to_owned(),
            byte_length: 4,
        }));
        let value_type = must(builder.push_type(TypeRecord::Scalar(ScalarType::Bool)));
        let constant =
            must(builder.push_constant(ConstantRecord::Scalar(ScalarConstant::Bool(true))));
        let origin = must(builder.push_origin(Origin {
            source_unit,
            span: OriginSpan {
                begin: OriginPosition {
                    offset: 1,
                    line: 1,
                    column: 1,
                },
                end: OriginPosition {
                    offset: 5,
                    line: 1,
                    column: 5,
                },
            },
        }));
        let node = must(builder.push_node(Node {
            kind: NodeKind::Constant { constant },
            result_type: value_type,
            cardinality: Some(Cardinality::StaticScalar),
            edges: crate::IndexRange::default(),
            origin,
        }));
        must(builder.push_ownership(Ownership {
            owner: node,
            release_after: ReleaseAfter::Root(crate::RootIndex(0)),
        }));
        must(builder.push_root(Root { node, origin }));
        must(must(builder.finish()).verify())
    }

    fn read_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn read_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }

    fn read_u64(bytes: &[u8], offset: usize) -> u64 {
        u64::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
            bytes[offset + 4],
            bytes[offset + 5],
            bytes[offset + 6],
            bytes[offset + 7],
        ])
    }

    fn section(bytes: &[u8], wanted: u16) -> Option<&[u8]> {
        let count = read_u32(bytes, 20) as usize;
        for index in 0..count {
            let entry = 32 + index * 24;
            if read_u16(bytes, entry) == wanted {
                let offset = read_u64(bytes, entry + 8) as usize;
                let length = read_u64(bytes, entry + 16) as usize;
                return Some(&bytes[offset..offset + length]);
            }
        }
        None
    }

    fn complete_surface_program() -> VerifiedProgram {
        let source = "parameters[x Int]\n\
                      [true 1 2.0 (1 2) x]\n\
                      add [1 (2 3)]\n\
                      add[1 2.0]\n\
                      inc[(1 2)]\n\
                      iota[3]\n\
                      fanout[iota[3] {inc[_]} {add[_ 10]}]\n";
        must(crate::lowering::compile_source_with_name(
            source, "<source>",
        ))
    }

    #[test]
    fn exact_issue_10_examples_are_encoder_goldens() {
        assert_eq!(
            must(encode_fwir(&empty_program(), &FwirEncodeOptions::default())),
            example_bytes("empty")
        );
        assert_eq!(
            must(encode_fwir(
                &scalar_true_program(),
                &FwirEncodeOptions::default()
            )),
            example_bytes("scalar-true")
        );
    }

    #[test]
    fn every_semantic_section_and_node_opcode_is_encoded_deterministically() {
        let program = complete_surface_program();
        let first = must(encode_fwir(&program, &FwirEncodeOptions::default()));
        let second = must(encode_fwir(&program, &FwirEncodeOptions::default()));
        assert_eq!(first, second);
        assert_eq!(first, example_bytes("complete"));
        let section_ids: Vec<_> = (0..read_u32(&first, 20) as usize)
            .map(|index| read_u16(&first, 32 + index * 24))
            .collect();
        assert_eq!(section_ids, (1_u16..=16).collect::<Vec<_>>());
        let nodes = match section(&first, 14) {
            Some(value) => value,
            None => panic!("NODE section missing"),
        };
        assert_eq!(nodes.len() % 56, 0);
        let kinds: Vec<_> = nodes.chunks_exact(56).map(|record| record[0]).collect();
        for kind in 1_u8..=6 {
            assert!(kinds.contains(&kind), "node opcode {kind} missing");
        }
        let edges = match section(&first, 11) {
            Some(value) => value,
            None => panic!("EDGE section missing"),
        };
        assert!(edges.chunks_exact(24).any(|record| record[8] == 2));
        assert!(edges.chunks_exact(24).any(|record| record[8] == 3));
        assert!(edges.chunks_exact(24).any(|record| record[10] == 2));
        assert!(edges.chunks_exact(24).any(|record| record[11] == 3));
        assert!(section(&first, 12).is_some());
        assert!(section(&first, 13).is_some());
        assert!(section(&first, 15).is_some());
        assert_eq!(
            section(&first, 3),
            Some(
                &[
                    2, 0, 0, 0, 0, 0, 0, 0, 8, 0, 0, 0, 8, 0, 0, 0, 1, 0, 0, 0, b'<', b's', b'o',
                    b'u', b'r', b'c', b'e', b'>', b'x',
                ][..]
            )
        );
        let constants = match section(&first, 8) {
            Some(value) => value,
            None => panic!("CONS section missing"),
        };
        assert!(constants.chunks_exact(20).any(|record| {
            record[0] == 1 && record[1] == 3 && record[12..20] == 2.0_f64.to_bits().to_le_bytes()
        }));
        let constant_elements = match section(&first, 9) {
            Some(value) => value,
            None => panic!("COEL section missing"),
        };
        assert!(
            constant_elements
                .chunks_exact(12)
                .any(|record| record[0] == 2 && record[4..12] == 1_i64.to_le_bytes())
        );
    }

    #[test]
    fn producer_metadata_is_advisory_and_preserves_exact_digest_bytes() {
        let digest = [0xa5; 32];
        let bytes = must(encode_fwir(
            &empty_program(),
            &FwirEncodeOptions {
                producer_metadata: Some(FwirProducerMetadata::Sha256(digest)),
            },
        ));
        let count = read_u32(&bytes, 20) as usize;
        let entry = 32 + (count - 1) * 24;
        assert_eq!(read_u16(&bytes, entry), 32769);
        assert_eq!(read_u16(&bytes, entry + 2), 0);
        assert_eq!(read_u32(&bytes, entry + 4), 0);
        let producer = match section(&bytes, 32769) {
            Some(value) => value,
            None => panic!("PROD section missing"),
        };
        assert_eq!(read_u32(producer, 0), 9);
        assert_eq!(&producer[4..13], b"faraweave");
        let version_length = read_u32(producer, 13) as usize;
        assert_eq!(
            &producer[17..17 + version_length],
            crate::VERSION.as_bytes()
        );
        let digest_header = 17 + version_length;
        assert_eq!(read_u16(producer, digest_header), 1);
        assert_eq!(read_u16(producer, digest_header + 2), 32);
        assert_eq!(&producer[digest_header + 4..], &digest);
    }

    #[test]
    fn checked_preflight_reports_overflow_before_output_allocation() {
        assert_eq!(
            checked_add(u64::MAX, 1, "offset"),
            Err(FwirEncodeError::SizeOverflow { field: "offset" })
        );
        assert_eq!(
            checked_mul(u64::MAX, 2, "section"),
            Err(FwirEncodeError::SizeOverflow { field: "section" })
        );
        assert_eq!(
            fixed_length(usize::MAX, 56, "nodes"),
            Err(FwirEncodeError::CountOverflow { field: "nodes" })
        );
    }

    #[test]
    fn allocation_refusal_is_explicit_at_each_encoder_allocation() {
        let program = scalar_true_program();
        for site in [
            FwirEncodeAllocationSite::StringPool,
            FwirEncodeAllocationSite::Artifact,
        ] {
            assert_eq!(
                encode_fwir_with_allocation_failure(
                    &program,
                    &FwirEncodeOptions::default(),
                    FwirEncodeAllocationFailureInjection::at(site),
                ),
                Err(FwirEncodeError::AllocationUnavailable { site })
            );
        }
    }

    struct FailingWriter {
        fail_flush: bool,
    }

    impl Write for FailingWriter {
        fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
            if self.fail_flush {
                Ok(_buffer.len())
            } else {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "injected"))
            }
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::new(io::ErrorKind::WriteZero, "injected"))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn writer_and_atomic_publication_failures_are_explicit_and_transactional() {
        let program = empty_program();
        assert_eq!(
            write_fwir(
                &program,
                &FwirEncodeOptions::default(),
                &mut FailingWriter { fail_flush: false }
            ),
            Err(FwirEncodeError::Output {
                operation: FwirOutputOperation::Write,
                kind: io::ErrorKind::BrokenPipe,
            })
        );
        assert_eq!(
            write_fwir(
                &program,
                &FwirEncodeOptions::default(),
                &mut FailingWriter { fail_flush: true }
            ),
            Err(FwirEncodeError::Output {
                operation: FwirOutputOperation::Flush,
                kind: io::ErrorKind::WriteZero,
            })
        );

        let original = b"old artifact".to_vec();
        let mut published = original.clone();
        let result = encode_fwir_with_atomic_publication(
            &program,
            &FwirEncodeOptions::default(),
            |_complete_bytes| Err(io::Error::new(io::ErrorKind::PermissionDenied, "injected")),
        );
        assert_eq!(
            result,
            Err(FwirEncodeError::Output {
                operation: FwirOutputOperation::Publish,
                kind: io::ErrorKind::PermissionDenied,
            })
        );
        assert_eq!(published, original);
        must(encode_fwir_with_atomic_publication(
            &program,
            &FwirEncodeOptions::default(),
            |complete_bytes| {
                published = complete_bytes.to_vec();
                Ok(())
            },
        ));
        assert_eq!(
            published,
            must(encode_fwir(&program, &FwirEncodeOptions::default()))
        );
    }
}
