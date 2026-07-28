use crate::{
    Cardinality, ConstantIndex, ConstantRecord, Conversion, Edge, FanOutBranch, Feature,
    IndexRange, LiftMode, ModuleMetadata, Node, NodeIndex, NodeKind, OperationReference, Origin,
    OriginIndex, OriginPosition, OriginSpan, Ownership, OwnershipMode, Parameter, ParameterIndex,
    ProgramRanges, RawProgram, ReleaseAfter, Root, RootIndex, ScalarConstant, ScalarType,
    ShapePlan, SourceUnit, SourceUnitIndex, TypeIndex, TypeRecord, ValueAccess, VerifiedProgram,
    VerifyAllocationFailureInjection, VerifyAllocationSite, VerifyError,
};
use std::collections::TryReserveError;

const MAGIC: &[u8; 8] = b"FWIR\r\n\x1a\n";
const HEADER_SIZE: usize = 32;
const DIRECTORY_ENTRY_SIZE: usize = 24;
const NONE: u32 = u32::MAX;
const PRODUCER_SECTION_ID: u16 = 32769;
const KNOWN_SECTION_SLOTS: usize = 19;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FwirDecodeLimits {
    pub max_artifact_bytes: usize,
    pub max_sections: u32,
    pub max_records_per_section: u32,
    pub max_total_records: u64,
    pub max_string_bytes: usize,
}

impl Default for FwirDecodeLimits {
    fn default() -> Self {
        Self {
            max_artifact_bytes: 64 * 1024 * 1024,
            max_sections: 64,
            max_records_per_section: 1_000_000,
            max_total_records: 4_000_000,
            max_string_bytes: 16 * 1024 * 1024,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FwirDecodeLimit {
    ArtifactBytes,
    Sections,
    RecordsPerSection,
    TotalRecords,
    StringBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FwirDecodeAllocationSite {
    Features,
    SourceUnits,
    SourceName,
    Parameters,
    ParameterName,
    Types,
    TypeElements,
    Constants,
    ConstantElements,
    Origins,
    OperationReferences,
    Edges,
    ShapeChecks,
    Branches,
    Nodes,
    Ownership,
    Roots,
    StringUse,
    Verifier(VerifyAllocationSite),
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct FwirDecodeAllocationFailureInjection {
    fail_at: Option<FwirDecodeAllocationSite>,
}

impl FwirDecodeAllocationFailureInjection {
    pub const fn none() -> Self {
        Self { fail_at: None }
    }

    #[doc(hidden)]
    pub const fn at(site: FwirDecodeAllocationSite) -> Self {
        Self {
            fail_at: Some(site),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FwirDecodeErrorKind {
    ArtifactTooLarge {
        actual: usize,
        limit: usize,
    },
    Truncated {
        needed_end: u64,
    },
    InvalidHeader {
        field: &'static str,
    },
    UnsupportedFormatVersion {
        major: u16,
        minor: u16,
    },
    UnknownMandatoryExtension {
        id: u16,
    },
    NonCanonicalDirectory {
        field: &'static str,
    },
    InvalidSectionLength,
    ResourceLimit {
        limit: FwirDecodeLimit,
        claimed: u64,
        configured: u64,
    },
    AllocationUnavailable {
        site: FwirDecodeAllocationSite,
    },
    InvalidUtf8,
    NonCanonicalRecord {
        field: &'static str,
    },
    MalformedProgram(VerifyError),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FwirDecodeError {
    pub kind: FwirDecodeErrorKind,
    pub offset: u64,
    pub section_id: Option<u16>,
    pub record_index: Option<u32>,
}

impl std::fmt::Display for FwirDecodeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "FWIR decoding failed at byte {} section {:?} record {:?}: {:?}",
            self.offset, self.section_id, self.record_index, self.kind
        )
    }
}

impl std::error::Error for FwirDecodeError {}

#[derive(Clone, Copy, Debug)]
struct Section {
    id: u16,
    offset: usize,
    end: usize,
    length: usize,
    record_size: usize,
}

impl Section {
    fn record_count(self) -> usize {
        self.length
            .checked_div(self.record_size)
            .map_or(0, |count| count)
    }
}

#[derive(Clone, Copy, Debug)]
struct DecodePlan {
    sections: [Option<Section>; KNOWN_SECTION_SLOTS],
    string_count: usize,
    format_minor: u16,
}

fn record_offset(section: Section, index: usize) -> usize {
    match index
        .checked_mul(section.record_size)
        .and_then(|relative| section.offset.checked_add(relative))
    {
        Some(offset) if offset <= section.end => offset,
        _ => section.end,
    }
}

fn record_error(
    section: Section,
    index: usize,
    relative_offset: usize,
    field: &'static str,
) -> FwirDecodeError {
    error(
        FwirDecodeErrorKind::NonCanonicalRecord { field },
        record_offset(section, index) + relative_offset,
        Some(section.id),
        u32::try_from(index).ok(),
    )
}

fn zero_bytes(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative_offset: usize,
    length: usize,
    field: &'static str,
) -> Result<(), FwirDecodeError> {
    let begin = record_offset(section, index) + relative_offset;
    let end = begin + length;
    if bytes
        .get(begin..end)
        .is_none_or(|value| value.iter().any(|byte| *byte != 0))
    {
        return Err(record_error(section, index, relative_offset, field));
    }
    Ok(())
}

fn scalar_type_from_tag(tag: u8) -> Option<ScalarType> {
    match tag {
        1 => Some(ScalarType::Bool),
        2 => Some(ScalarType::Int),
        3 => Some(ScalarType::Double),
        _ => None,
    }
}

fn scalar_constant_from_parts(tag: u8, payload: u64) -> Option<ScalarConstant> {
    match tag {
        1 if payload <= 1 => Some(ScalarConstant::Bool(payload == 1)),
        2 => Some(ScalarConstant::Int(i64::from_le_bytes(
            payload.to_le_bytes(),
        ))),
        3 if !f64::from_bits(payload).is_nan() || payload == 0x7ff8_0000_0000_0000 => {
            Some(ScalarConstant::DoubleBits(payload))
        }
        _ => None,
    }
}

fn cardinality_from_parts(tag: u8, length: u32) -> Option<Option<Cardinality>> {
    match (tag, length) {
        (0, 0) => Some(None),
        (1, 0) => Some(Some(Cardinality::StaticScalar)),
        (2, value) => Some(Some(Cardinality::StaticVector(value))),
        (3, 0) => Some(Some(Cardinality::DynamicVector)),
        _ => None,
    }
}

fn error(
    kind: FwirDecodeErrorKind,
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> FwirDecodeError {
    FwirDecodeError {
        kind,
        offset: usize_as_u64(offset),
        section_id,
        record_index,
    }
}

fn usize_as_u64(value: usize) -> u64 {
    u64::try_from(value).map_or(u64::MAX, |converted| converted)
}

fn read_bytes<const LENGTH: usize>(
    bytes: &[u8],
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> Result<[u8; LENGTH], FwirDecodeError> {
    let end = offset.checked_add(LENGTH).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: u64::MAX,
            },
            offset,
            section_id,
            record_index,
        )
    })?;
    let source = bytes.get(offset..end).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: usize_as_u64(end),
            },
            offset,
            section_id,
            record_index,
        )
    })?;
    let mut output = [0; LENGTH];
    output.copy_from_slice(source);
    Ok(output)
}

fn read_u8(
    bytes: &[u8],
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> Result<u8, FwirDecodeError> {
    Ok(read_bytes::<1>(bytes, offset, section_id, record_index)?[0])
}

fn read_u16(
    bytes: &[u8],
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> Result<u16, FwirDecodeError> {
    Ok(u16::from_le_bytes(read_bytes(
        bytes,
        offset,
        section_id,
        record_index,
    )?))
}

fn read_u32(
    bytes: &[u8],
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> Result<u32, FwirDecodeError> {
    Ok(u32::from_le_bytes(read_bytes(
        bytes,
        offset,
        section_id,
        record_index,
    )?))
}

fn read_u64(
    bytes: &[u8],
    offset: usize,
    section_id: Option<u16>,
    record_index: Option<u32>,
) -> Result<u64, FwirDecodeError> {
    Ok(u64::from_le_bytes(read_bytes(
        bytes,
        offset,
        section_id,
        record_index,
    )?))
}

fn known_section(id: u16) -> Option<(usize, u16, usize)> {
    match id {
        1 => Some((0, 3, 8)),
        2 => Some((1, 3, 4)),
        3 => Some((2, 3, 0)),
        4 => Some((3, 3, 8)),
        5 => Some((4, 3, 20)),
        6 => Some((5, 3, 12)),
        7 => Some((6, 3, 4)),
        8 => Some((7, 3, 20)),
        9 => Some((8, 3, 12)),
        10 => Some((9, 3, 28)),
        11 => Some((10, 3, 24)),
        12 => Some((11, 3, 4)),
        13 => Some((12, 3, 20)),
        14 => Some((13, 3, 56)),
        15 => Some((14, 3, 12)),
        16 => Some((15, 3, 8)),
        17 => Some((16, 3, 8)),
        18 => Some((17, 3, 16)),
        PRODUCER_SECTION_ID => Some((18, 0, 0)),
        _ => None,
    }
}

fn section(plan: &DecodePlan, id: u16) -> Option<Section> {
    known_section(id).and_then(|(slot, _, _)| plan.sections[slot])
}

fn checked_usize(
    value: u64,
    offset: usize,
    section_id: Option<u16>,
    field: &'static str,
) -> Result<usize, FwirDecodeError> {
    usize::try_from(value).map_err(|_| {
        error(
            FwirDecodeErrorKind::NonCanonicalDirectory { field },
            offset,
            section_id,
            None,
        )
    })
}

fn resource_limit(
    limit: FwirDecodeLimit,
    claimed: u64,
    configured: u64,
    offset: usize,
    section_id: Option<u16>,
) -> FwirDecodeError {
    error(
        FwirDecodeErrorKind::ResourceLimit {
            limit,
            claimed,
            configured,
        },
        offset,
        section_id,
        None,
    )
}

fn string_descriptor(
    bytes: &[u8],
    strings: Section,
    index: usize,
) -> Result<(usize, usize), FwirDecodeError> {
    let record_index = u32::try_from(index).ok();
    let descriptor = strings
        .offset
        .checked_add(4)
        .and_then(|value| {
            index
                .checked_mul(8)
                .and_then(|delta| value.checked_add(delta))
        })
        .ok_or_else(|| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                strings.offset,
                Some(3),
                record_index,
            )
        })?;
    let offset =
        usize::try_from(read_u32(bytes, descriptor, Some(3), record_index)?).map_err(|_| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                descriptor,
                Some(3),
                record_index,
            )
        })?;
    let length =
        usize::try_from(read_u32(bytes, descriptor + 4, Some(3), record_index)?).map_err(|_| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                descriptor + 4,
                Some(3),
                record_index,
            )
        })?;
    Ok((offset, length))
}

fn string_value(
    bytes: &[u8],
    strings: Section,
    count: usize,
    index: usize,
) -> Result<&str, FwirDecodeError> {
    if index >= count {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "string_index",
            },
            strings.offset,
            Some(3),
            u32::try_from(index).ok(),
        ));
    }
    let (relative, length) = string_descriptor(bytes, strings, index)?;
    let descriptors_length = count.checked_mul(8).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            strings.offset,
            Some(3),
            None,
        )
    })?;
    let area = strings
        .offset
        .checked_add(4)
        .and_then(|value| value.checked_add(descriptors_length))
        .ok_or_else(|| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                strings.offset,
                Some(3),
                None,
            )
        })?;
    let begin = area.checked_add(relative).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            strings.offset,
            Some(3),
            u32::try_from(index).ok(),
        )
    })?;
    let end = begin.checked_add(length).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            begin,
            Some(3),
            u32::try_from(index).ok(),
        )
    })?;
    let value = bytes.get(begin..end).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: usize_as_u64(end),
            },
            begin,
            Some(3),
            u32::try_from(index).ok(),
        )
    })?;
    std::str::from_utf8(value).map_err(|invalid| {
        error(
            FwirDecodeErrorKind::InvalidUtf8,
            begin + invalid.valid_up_to(),
            Some(3),
            u32::try_from(index).ok(),
        )
    })
}

fn validate_strings(
    bytes: &[u8],
    strings: Section,
    limits: &FwirDecodeLimits,
) -> Result<usize, FwirDecodeError> {
    let count = usize::try_from(read_u32(bytes, strings.offset, Some(3), None)?).map_err(|_| {
        resource_limit(
            FwirDecodeLimit::RecordsPerSection,
            u64::MAX,
            u64::from(limits.max_records_per_section),
            strings.offset,
            Some(3),
        )
    })?;
    if count == 0 {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "empty_string_section",
            },
            strings.offset,
            Some(3),
            None,
        ));
    }
    if usize_as_u64(count) > u64::from(limits.max_records_per_section) {
        return Err(resource_limit(
            FwirDecodeLimit::RecordsPerSection,
            usize_as_u64(count),
            u64::from(limits.max_records_per_section),
            strings.offset,
            Some(3),
        ));
    }
    let descriptor_bytes = count.checked_mul(8).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            strings.offset,
            Some(3),
            None,
        )
    })?;
    let area = 4_usize.checked_add(descriptor_bytes).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            strings.offset,
            Some(3),
            None,
        )
    })?;
    if area > strings.length {
        return Err(error(
            FwirDecodeErrorKind::InvalidSectionLength,
            strings.offset,
            Some(3),
            None,
        ));
    }
    let byte_count = strings.length - area;
    if byte_count > limits.max_string_bytes {
        return Err(resource_limit(
            FwirDecodeLimit::StringBytes,
            usize_as_u64(byte_count),
            usize_as_u64(limits.max_string_bytes),
            strings.offset + area,
            Some(3),
        ));
    }
    let mut expected_offset = 0;
    let mut previous: Option<&[u8]> = None;
    for index in 0..count {
        let (relative, length) = string_descriptor(bytes, strings, index)?;
        if relative != expected_offset {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "string_extent",
                },
                strings.offset + 4 + index * 8,
                Some(3),
                u32::try_from(index).ok(),
            ));
        }
        expected_offset = relative.checked_add(length).ok_or_else(|| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                strings.offset + 4 + index * 8,
                Some(3),
                u32::try_from(index).ok(),
            )
        })?;
        if expected_offset > byte_count {
            return Err(error(
                FwirDecodeErrorKind::InvalidSectionLength,
                strings.offset + 4 + index * 8,
                Some(3),
                u32::try_from(index).ok(),
            ));
        }
        let value = string_value(bytes, strings, count, index)?;
        if previous.is_some_and(|prior| prior >= value.as_bytes()) {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "string_order",
                },
                strings.offset + 4 + index * 8,
                Some(3),
                u32::try_from(index).ok(),
            ));
        }
        previous = Some(value.as_bytes());
    }
    if expected_offset != byte_count {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "string_extent",
            },
            strings.end,
            Some(3),
            None,
        ));
    }
    Ok(count)
}

fn validate_producer(bytes: &[u8], producer: Section) -> Result<(), FwirDecodeError> {
    let mut cursor = producer.offset;
    let name_length = usize::try_from(read_u32(bytes, cursor, Some(PRODUCER_SECTION_ID), None)?)
        .map_err(|_| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                cursor,
                Some(PRODUCER_SECTION_ID),
                None,
            )
        })?;
    cursor += 4;
    let name_end = cursor.checked_add(name_length).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        )
    })?;
    let name = bytes.get(cursor..name_end).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: usize_as_u64(name_end),
            },
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        )
    })?;
    if name != b"faraweave" {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "producer_name",
            },
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        ));
    }
    cursor = name_end;
    let version_length = usize::try_from(read_u32(bytes, cursor, Some(PRODUCER_SECTION_ID), None)?)
        .map_err(|_| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                cursor,
                Some(PRODUCER_SECTION_ID),
                None,
            )
        })?;
    cursor += 4;
    let version_end = cursor.checked_add(version_length).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::InvalidSectionLength,
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        )
    })?;
    let version = bytes.get(cursor..version_end).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: usize_as_u64(version_end),
            },
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        )
    })?;
    if version.is_empty() || !version.is_ascii() || version.first() == Some(&b'v') {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "producer_version",
            },
            cursor,
            Some(PRODUCER_SECTION_ID),
            None,
        ));
    }
    cursor = version_end;
    let algorithm = read_u16(bytes, cursor, Some(PRODUCER_SECTION_ID), None)?;
    let digest_length = usize::from(read_u16(
        bytes,
        cursor + 2,
        Some(PRODUCER_SECTION_ID),
        None,
    )?);
    cursor += 4;
    let expected_digest = match algorithm {
        0 => 0,
        1 => 32,
        _ => {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "digest_algorithm",
                },
                cursor - 4,
                Some(PRODUCER_SECTION_ID),
                None,
            ));
        }
    };
    if digest_length != expected_digest || cursor.checked_add(digest_length) != Some(producer.end) {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalRecord {
                field: "digest_length",
            },
            cursor - 2,
            Some(PRODUCER_SECTION_ID),
            None,
        ));
    }
    Ok(())
}

fn preflight(bytes: &[u8], limits: &FwirDecodeLimits) -> Result<DecodePlan, FwirDecodeError> {
    if bytes.len() > limits.max_artifact_bytes {
        return Err(error(
            FwirDecodeErrorKind::ArtifactTooLarge {
                actual: bytes.len(),
                limit: limits.max_artifact_bytes,
            },
            0,
            None,
            None,
        ));
    }
    if bytes.len() < HEADER_SIZE {
        return Err(error(
            FwirDecodeErrorKind::Truncated { needed_end: 32 },
            bytes.len(),
            None,
            None,
        ));
    }
    if bytes.get(..8) != Some(MAGIC) {
        return Err(error(
            FwirDecodeErrorKind::InvalidHeader { field: "magic" },
            0,
            None,
            None,
        ));
    }
    let major = read_u16(bytes, 8, None, None)?;
    let minor = read_u16(bytes, 10, None, None)?;
    if major != 1 {
        return Err(error(
            FwirDecodeErrorKind::UnsupportedFormatVersion { major, minor },
            8,
            None,
            None,
        ));
    }
    if read_u32(bytes, 12, None, None)? != 32 {
        return Err(error(
            FwirDecodeErrorKind::InvalidHeader {
                field: "header_size",
            },
            12,
            None,
            None,
        ));
    }
    if read_u16(bytes, 16, None, None)? != 24 {
        return Err(error(
            FwirDecodeErrorKind::InvalidHeader {
                field: "directory_entry_size",
            },
            16,
            None,
            None,
        ));
    }
    if read_u16(bytes, 18, None, None)? != 0 {
        return Err(error(
            FwirDecodeErrorKind::InvalidHeader { field: "reserved" },
            18,
            None,
            None,
        ));
    }
    let section_count = read_u32(bytes, 20, None, None)?;
    if section_count > limits.max_sections {
        return Err(resource_limit(
            FwirDecodeLimit::Sections,
            u64::from(section_count),
            u64::from(limits.max_sections),
            20,
            None,
        ));
    }
    if read_u64(bytes, 24, None, None)? != 32 {
        return Err(error(
            FwirDecodeErrorKind::InvalidHeader {
                field: "directory_offset",
            },
            24,
            None,
            None,
        ));
    }
    let directory_bytes = usize::try_from(section_count)
        .ok()
        .and_then(|count| count.checked_mul(DIRECTORY_ENTRY_SIZE))
        .ok_or_else(|| {
            error(
                FwirDecodeErrorKind::Truncated {
                    needed_end: u64::MAX,
                },
                20,
                None,
                None,
            )
        })?;
    let directory_end = HEADER_SIZE.checked_add(directory_bytes).ok_or_else(|| {
        error(
            FwirDecodeErrorKind::Truncated {
                needed_end: u64::MAX,
            },
            HEADER_SIZE,
            None,
            None,
        )
    })?;
    if directory_end > bytes.len() {
        return Err(error(
            FwirDecodeErrorKind::Truncated {
                needed_end: usize_as_u64(directory_end),
            },
            bytes.len(),
            None,
            None,
        ));
    }

    let mut sections = [None; KNOWN_SECTION_SLOTS];
    let mut previous_id = 0_u16;
    let mut expected_payload = directory_end;
    let mut total_records = 0_u64;
    for index in 0..section_count {
        let index_usize = usize::try_from(index).map_err(|_| {
            resource_limit(
                FwirDecodeLimit::Sections,
                u64::from(index),
                u64::from(limits.max_sections),
                20,
                None,
            )
        })?;
        let entry = index_usize
            .checked_mul(DIRECTORY_ENTRY_SIZE)
            .and_then(|relative| HEADER_SIZE.checked_add(relative))
            .ok_or_else(|| {
                error(
                    FwirDecodeErrorKind::Truncated {
                        needed_end: u64::MAX,
                    },
                    HEADER_SIZE,
                    None,
                    Some(index),
                )
            })?;
        let id = read_u16(bytes, entry, None, Some(index))?;
        let flags = read_u16(bytes, entry + 2, Some(id), Some(index))?;
        let record_size = usize::try_from(read_u32(bytes, entry + 4, Some(id), Some(index))?)
            .map_err(|_| {
                error(
                    FwirDecodeErrorKind::InvalidSectionLength,
                    entry + 4,
                    Some(id),
                    Some(index),
                )
            })?;
        let payload_offset = checked_usize(
            read_u64(bytes, entry + 8, Some(id), Some(index))?,
            entry + 8,
            Some(id),
            "offset",
        )?;
        let payload_length = checked_usize(
            read_u64(bytes, entry + 16, Some(id), Some(index))?,
            entry + 16,
            Some(id),
            "length",
        )?;
        if id <= previous_id {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalDirectory { field: "order" },
                entry,
                Some(id),
                Some(index),
            ));
        }
        previous_id = id;
        if flags & !3 != 0 {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalDirectory { field: "flags" },
                entry + 2,
                Some(id),
                Some(index),
            ));
        }
        if payload_offset != expected_payload {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalDirectory {
                    field: "contiguous_payload",
                },
                entry + 8,
                Some(id),
                Some(index),
            ));
        }
        let payload_end = payload_offset.checked_add(payload_length).ok_or_else(|| {
            error(
                FwirDecodeErrorKind::InvalidSectionLength,
                entry + 16,
                Some(id),
                Some(index),
            )
        })?;
        if payload_end > bytes.len() {
            return Err(error(
                FwirDecodeErrorKind::Truncated {
                    needed_end: usize_as_u64(payload_end),
                },
                bytes.len(),
                Some(id),
                Some(index),
            ));
        }
        expected_payload = payload_end;

        if let Some((slot, expected_flags, expected_record_size)) = known_section(id) {
            if flags != expected_flags {
                return Err(error(
                    FwirDecodeErrorKind::NonCanonicalDirectory { field: "flags" },
                    entry + 2,
                    Some(id),
                    Some(index),
                ));
            }
            if record_size != expected_record_size {
                return Err(error(
                    FwirDecodeErrorKind::NonCanonicalDirectory {
                        field: "record_size",
                    },
                    entry + 4,
                    Some(id),
                    Some(index),
                ));
            }
            if (id == 1 && payload_length != 8)
                || (id != 1 && id != PRODUCER_SECTION_ID && payload_length == 0)
                || (record_size != 0 && payload_length % record_size != 0)
            {
                return Err(error(
                    FwirDecodeErrorKind::InvalidSectionLength,
                    entry + 16,
                    Some(id),
                    Some(index),
                ));
            }
            let count = payload_length
                .checked_div(record_size)
                .map_or(0, |value| value);
            if usize_as_u64(count) > u64::from(limits.max_records_per_section) {
                return Err(resource_limit(
                    FwirDecodeLimit::RecordsPerSection,
                    usize_as_u64(count),
                    u64::from(limits.max_records_per_section),
                    entry + 16,
                    Some(id),
                ));
            }
            total_records = total_records
                .checked_add(usize_as_u64(count))
                .ok_or_else(|| {
                    resource_limit(
                        FwirDecodeLimit::TotalRecords,
                        u64::MAX,
                        limits.max_total_records,
                        entry + 16,
                        Some(id),
                    )
                })?;
            sections[slot] = Some(Section {
                id,
                offset: payload_offset,
                end: payload_end,
                length: payload_length,
                record_size,
            });
        } else {
            if flags & 1 != 0 {
                return Err(error(
                    FwirDecodeErrorKind::UnknownMandatoryExtension { id },
                    entry,
                    Some(id),
                    Some(index),
                ));
            }
            if minor == 0 || flags & 2 != 0 {
                return Err(error(
                    FwirDecodeErrorKind::NonCanonicalDirectory {
                        field: "unknown_extension",
                    },
                    entry,
                    Some(id),
                    Some(index),
                ));
            }
            if flags != 0 {
                return Err(error(
                    FwirDecodeErrorKind::NonCanonicalDirectory { field: "flags" },
                    entry + 2,
                    Some(id),
                    Some(index),
                ));
            }
        }
    }
    if expected_payload != bytes.len() {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalDirectory {
                field: "trailing_bytes",
            },
            expected_payload,
            None,
            None,
        ));
    }
    if sections[0].is_none() {
        return Err(error(
            FwirDecodeErrorKind::NonCanonicalDirectory {
                field: "missing_module",
            },
            HEADER_SIZE,
            Some(1),
            None,
        ));
    }

    let mut string_count = 0;
    if let Some(strings) = sections[2] {
        string_count = validate_strings(bytes, strings, limits)?;
        total_records = total_records
            .checked_add(usize_as_u64(string_count))
            .ok_or_else(|| {
                resource_limit(
                    FwirDecodeLimit::TotalRecords,
                    u64::MAX,
                    limits.max_total_records,
                    strings.offset,
                    Some(3),
                )
            })?;
    }
    if total_records > limits.max_total_records {
        return Err(resource_limit(
            FwirDecodeLimit::TotalRecords,
            total_records,
            limits.max_total_records,
            20,
            None,
        ));
    }
    if let Some(producer) = sections[18] {
        validate_producer(bytes, producer)?;
    }
    Ok(DecodePlan {
        sections,
        string_count,
        format_minor: minor,
    })
}

fn record_u8(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative: usize,
) -> Result<u8, FwirDecodeError> {
    read_u8(
        bytes,
        record_offset(section, index) + relative,
        Some(section.id),
        u32::try_from(index).ok(),
    )
}

fn record_u16(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative: usize,
) -> Result<u16, FwirDecodeError> {
    read_u16(
        bytes,
        record_offset(section, index) + relative,
        Some(section.id),
        u32::try_from(index).ok(),
    )
}

fn record_u32(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative: usize,
) -> Result<u32, FwirDecodeError> {
    read_u32(
        bytes,
        record_offset(section, index) + relative,
        Some(section.id),
        u32::try_from(index).ok(),
    )
}

fn record_u64(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative: usize,
) -> Result<u64, FwirDecodeError> {
    read_u64(
        bytes,
        record_offset(section, index) + relative,
        Some(section.id),
        u32::try_from(index).ok(),
    )
}

fn record_usize(
    bytes: &[u8],
    section: Section,
    index: usize,
    relative: usize,
    field: &'static str,
) -> Result<usize, FwirDecodeError> {
    usize::try_from(record_u32(bytes, section, index, relative)?)
        .map_err(|_| record_error(section, index, relative, field))
}

fn validate_features(bytes: &[u8], plan: &DecodePlan) -> Result<(), FwirDecodeError> {
    let Some(features) = section(plan, 2) else {
        return Ok(());
    };
    let mut previous = 0_u16;
    for index in 0..features.record_count() {
        let id = record_u16(bytes, features, index, 0)?;
        let class = record_u8(bytes, features, index, 2)?;
        let reserved = record_u8(bytes, features, index, 3)?;
        if id == 0 || id <= previous {
            return Err(record_error(features, index, 0, "feature_order"));
        }
        previous = id;
        if reserved != 0 {
            return Err(record_error(features, index, 3, "reserved"));
        }
        if id <= Feature::BackendNativeMathV1.numeric() {
            if class != 0 {
                return Err(record_error(features, index, 2, "feature_class"));
            }
            if matches!(id, 5 | 6) && plan.format_minor == 0 {
                return Err(record_error(features, index, 0, "feature_format_minor"));
            }
        } else if class == 0 {
            return Err(error(
                FwirDecodeErrorKind::UnknownMandatoryExtension { id },
                record_offset(features, index),
                Some(2),
                u32::try_from(index).ok(),
            ));
        } else if class != 1 {
            return Err(record_error(features, index, 2, "feature_class"));
        }
    }
    Ok(())
}

fn has_application_plans_feature(bytes: &[u8], plan: &DecodePlan) -> Result<bool, FwirDecodeError> {
    let Some(features) = section(plan, 2) else {
        return Ok(false);
    };
    for index in 0..features.record_count() {
        if record_u16(bytes, features, index, 0)? == 5 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn validate_record_canonicality(bytes: &[u8], plan: &DecodePlan) -> Result<(), FwirDecodeError> {
    validate_features(bytes, plan)?;
    let explicit_application_plans = has_application_plans_feature(bytes, plan)?;

    if let Some(sources) = section(plan, 4) {
        for index in 0..sources.record_count() {
            let string = record_usize(bytes, sources, index, 0, "diagnostic_name")?;
            if section(plan, 3).is_none() || string >= plan.string_count {
                return Err(record_error(sources, index, 0, "diagnostic_name"));
            }
        }
    }
    if let Some(parameters) = section(plan, 5) {
        for index in 0..parameters.record_count() {
            let string = record_usize(bytes, parameters, index, 4, "name")?;
            if section(plan, 3).is_none() || string >= plan.string_count {
                return Err(record_error(parameters, index, 4, "name"));
            }
            if scalar_type_from_tag(record_u8(bytes, parameters, index, 8)?).is_none() {
                return Err(record_error(parameters, index, 8, "scalar_type"));
            }
            zero_bytes(bytes, parameters, index, 9, 3, "reserved")?;
        }
    }
    if let Some(types) = section(plan, 6) {
        for index in 0..types.record_count() {
            let kind = record_u8(bytes, types, index, 0)?;
            let scalar = record_u8(bytes, types, index, 1)?;
            let start = record_u32(bytes, types, index, 4)?;
            let count = record_u32(bytes, types, index, 8)?;
            zero_bytes(bytes, types, index, 2, 2, "reserved")?;
            match kind {
                1 | 2 if scalar_type_from_tag(scalar).is_some() && start == 0 && count == 0 => {}
                3 if scalar == 0 => {}
                1 | 2 => return Err(record_error(types, index, 1, "unused_type_range")),
                3 => return Err(record_error(types, index, 1, "tuple_scalar_type")),
                _ => return Err(record_error(types, index, 0, "kind")),
            }
        }
    }
    if let Some(constants) = section(plan, 8) {
        for index in 0..constants.record_count() {
            let kind = record_u8(bytes, constants, index, 0)?;
            let scalar = record_u8(bytes, constants, index, 1)?;
            let start = record_u32(bytes, constants, index, 4)?;
            let count = record_u32(bytes, constants, index, 8)?;
            let payload = record_u64(bytes, constants, index, 12)?;
            zero_bytes(bytes, constants, index, 2, 2, "reserved")?;
            match kind {
                1 if scalar_constant_from_parts(scalar, payload).is_some()
                    && start == 0
                    && count == 0 => {}
                2 if scalar_type_from_tag(scalar).is_some() && payload == 0 => {}
                1 => return Err(record_error(constants, index, 1, "scalar_payload")),
                2 => return Err(record_error(constants, index, 12, "vector_payload")),
                _ => return Err(record_error(constants, index, 0, "kind")),
            }
        }
    }
    if let Some(elements) = section(plan, 9) {
        for index in 0..elements.record_count() {
            let scalar = record_u8(bytes, elements, index, 0)?;
            let payload = record_u64(bytes, elements, index, 4)?;
            if scalar_constant_from_parts(scalar, payload).is_none() {
                return Err(record_error(elements, index, 0, "scalar_payload"));
            }
            zero_bytes(bytes, elements, index, 1, 3, "reserved")?;
        }
    }
    if let Some(edges) = section(plan, 11) {
        for index in 0..edges.record_count() {
            let access = record_u8(bytes, edges, index, 8)?;
            let cardinality = record_u8(bytes, edges, index, 9)?;
            let conversion = record_u8(bytes, edges, index, 10)?;
            let ownership = record_u8(bytes, edges, index, 11)?;
            let access_index = record_u32(bytes, edges, index, 12)?;
            let cardinality_length = record_u32(bytes, edges, index, 16)?;
            if !matches!((access, access_index), (1 | 3, 0) | (2, _)) {
                return Err(record_error(edges, index, 8, "access"));
            }
            if cardinality_from_parts(cardinality, cardinality_length).is_none() {
                return Err(record_error(edges, index, 9, "cardinality"));
            }
            if !matches!(conversion, 1 | 2) {
                return Err(record_error(edges, index, 10, "conversion"));
            }
            if !matches!(ownership, 1..=3) {
                return Err(record_error(edges, index, 11, "ownership"));
            }
        }
    }
    let mut selected_nodes = 0_usize;
    if let Some(nodes) = section(plan, 14) {
        for index in 0..nodes.record_count() {
            let kind = record_u8(bytes, nodes, index, 0)?;
            let cardinality = record_u8(bytes, nodes, index, 1)?;
            let lift = record_u8(bytes, nodes, index, 2)?;
            let result_element = record_u8(bytes, nodes, index, 3)?;
            let cardinality_length = record_u32(bytes, nodes, index, 8)?;
            if cardinality_from_parts(cardinality, cardinality_length).is_none() {
                return Err(record_error(nodes, index, 1, "cardinality"));
            }
            let mut arguments = [0_u32; 8];
            for (argument, value) in arguments.iter_mut().enumerate() {
                *value = record_u32(bytes, nodes, index, 24 + argument * 4)?;
            }
            match kind {
                1 | 2 => {
                    if lift != 0 || result_element != 0 || arguments[1..].iter().any(|v| *v != 0) {
                        return Err(record_error(nodes, index, 2, "unused_variant"));
                    }
                }
                3 | 5 => {
                    if lift != 0 || result_element != 0 || arguments.iter().any(|v| *v != 0) {
                        return Err(record_error(nodes, index, 2, "unused_variant"));
                    }
                }
                4 => {
                    selected_nodes = selected_nodes
                        .checked_add(1)
                        .ok_or_else(|| record_error(nodes, index, 0, "application_plan_count"))?;
                    if !(matches!(lift, 1..=3)
                        || (explicit_application_plans && matches!(lift, 4 | 5)))
                        || scalar_type_from_tag(result_element).is_none()
                        || arguments[0..3]
                            .iter()
                            .any(|value| *value > u32::from(u16::MAX))
                        || arguments[7] != 0
                    {
                        return Err(record_error(nodes, index, 2, "selected_apply"));
                    }
                    let primitive = u16::try_from(arguments[0])
                        .map_err(|_| record_error(nodes, index, 24, "semantic_id"))?;
                    let signature = u16::try_from(arguments[1])
                        .map_err(|_| record_error(nodes, index, 28, "semantic_id"))?;
                    let implementation = u16::try_from(arguments[2])
                        .map_err(|_| record_error(nodes, index, 32, "semantic_id"))?;
                    let signature_descriptor =
                        crate::semantic_registry::signature_from_numeric(signature);
                    let implementation_descriptor =
                        crate::semantic_registry::implementation_from_numeric(implementation);
                    let identities_match = match (signature_descriptor, implementation_descriptor) {
                        (Ok(signature_value), Ok(implementation_value)) => {
                            signature_value.primitive_id.numeric() == primitive
                                && implementation_value.primitive_id.numeric() == primitive
                                && signature_value.signature_id.numeric()
                                    == implementation_value.signature_id.numeric()
                        }
                        _ => false,
                    };
                    if crate::semantic_registry::primitive_from_numeric(primitive).is_err()
                        || !identities_match
                    {
                        return Err(record_error(nodes, index, 24, "semantic_id"));
                    }
                }
                6 => {
                    if lift != 0
                        || result_element != 0
                        || arguments[3..].iter().any(|value| *value != 0)
                    {
                        return Err(record_error(nodes, index, 2, "unused_variant"));
                    }
                }
                _ => return Err(record_error(nodes, index, 0, "kind")),
            }
        }
    }
    match (
        explicit_application_plans,
        section(plan, 17),
        selected_nodes,
    ) {
        (false, Some(plans), _) => {
            return Err(record_error(plans, 0, 0, "application_plans_feature"));
        }
        (true, None, count) if count != 0 => {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "missing_application_plans",
                },
                section(plan, 14).map_or(0, |nodes| nodes.offset),
                Some(17),
                None,
            ));
        }
        (_, Some(plans), count) => {
            if plans.record_count() != count {
                return Err(record_error(plans, 0, 0, "application_plan_count"));
            }
            let Some(nodes) = section(plan, 14) else {
                return Err(record_error(plans, 0, 0, "application_plan_node"));
            };
            let mut plan_index = 0;
            for node_index in 0..nodes.record_count() {
                if record_u8(bytes, nodes, node_index, 0)? != 4 {
                    continue;
                }
                if record_u32(bytes, plans, plan_index, 0)?
                    != u32::try_from(node_index).unwrap_or(u32::MAX)
                {
                    return Err(record_error(plans, plan_index, 0, "application_plan_node"));
                }
                let plan_id = record_u16(bytes, plans, plan_index, 4)?;
                zero_bytes(bytes, plans, plan_index, 6, 2, "reserved")?;
                let implementation = u16::try_from(record_u32(bytes, nodes, node_index, 32)?)
                    .map_err(|_| record_error(nodes, node_index, 32, "semantic_id"))?;
                let expected =
                    crate::semantic_registry::implementation_from_numeric(implementation)
                        .map(|descriptor| descriptor.application_plan.id.numeric())
                        .map_err(|_| record_error(plans, plan_index, 4, "application_plan_id"))?;
                if plan_id == 0
                    || plan_id != expected
                    || crate::semantic_registry::application_plan_from_numeric(plan_id).is_err()
                {
                    return Err(record_error(plans, plan_index, 4, "application_plan_id"));
                }
                plan_index += 1;
            }
        }
        _ => {}
    }
    if let Some(ownership) = section(plan, 15) {
        for index in 0..ownership.record_count() {
            if !matches!(record_u8(bytes, ownership, index, 4)?, 1 | 2) {
                return Err(record_error(ownership, index, 4, "release_kind"));
            }
            zero_bytes(bytes, ownership, index, 5, 3, "reserved")?;
        }
    }
    if let Some(references) = section(plan, 18) {
        for index in 0..references.record_count() {
            zero_bytes(bytes, references, index, 6, 2, "reserved")?;
            zero_bytes(bytes, references, index, 12, 4, "reserved")?;
            let primitive = record_u16(bytes, references, index, 0)?;
            let signature = record_u16(bytes, references, index, 2)?;
            let implementation = record_u16(bytes, references, index, 4)?;
            let descriptor = crate::semantic_registry::implementation_from_numeric(implementation);
            let valid = descriptor.is_ok_and(|descriptor| {
                descriptor.primitive_id.numeric() == primitive
                    && descriptor.signature_id.numeric() == signature
                    && descriptor.behavior
                        == crate::semantic_registry::StructuralBehavior::Elementwise
            });
            if !valid {
                return Err(record_error(references, index, 0, "semantic_id"));
            }
        }
    }
    Ok(())
}

fn validate_string_use(
    bytes: &[u8],
    plan: &DecodePlan,
    injection: FwirDecodeAllocationFailureInjection,
) -> Result<(), FwirDecodeError> {
    if plan.string_count == 0 {
        return Ok(());
    }
    let strings = match section(plan, 3) {
        Some(value) => value,
        None => {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "missing_string_section",
                },
                0,
                Some(3),
                None,
            ));
        }
    };
    let mut used = Vec::new();
    reserve_exact(
        &mut used,
        plan.string_count,
        FwirDecodeAllocationSite::StringUse,
        injection,
        strings.offset,
        Some(3),
    )?;
    used.resize(plan.string_count, false);
    for (section_id, relative) in [(4_u16, 0_usize), (5, 4)] {
        if let Some(records) = section(plan, section_id) {
            for index in 0..records.record_count() {
                let string = record_usize(bytes, records, index, relative, "string_index")?;
                if let Some(value) = used.get_mut(string) {
                    *value = true;
                }
            }
        }
    }
    if let Some(unused) = used.iter().position(|value| !value) {
        return Err(record_error(strings, unused, 0, "unused_string"));
    }
    Ok(())
}

fn whole_range(count: usize, field: &'static str) -> Result<IndexRange, FwirDecodeError> {
    Ok(IndexRange {
        start: 0,
        count: u32::try_from(count).map_err(|_| {
            error(
                FwirDecodeErrorKind::NonCanonicalRecord { field },
                0,
                None,
                None,
            )
        })?,
    })
}

fn section_count(plan: &DecodePlan, id: u16) -> usize {
    section(plan, id).map_or(0, Section::record_count)
}

fn reserve_arena<T>(
    values: &mut Vec<T>,
    count: usize,
    site: FwirDecodeAllocationSite,
    injection: FwirDecodeAllocationFailureInjection,
    section_value: Option<Section>,
) -> Result<(), FwirDecodeError> {
    if count == 0 {
        return Ok(());
    }
    reserve_exact(
        values,
        count,
        site,
        injection,
        section_value.map_or(0, |value| value.offset),
        section_value.map(|value| value.id),
    )
}

fn copy_name(
    value: &str,
    site: FwirDecodeAllocationSite,
    injection: FwirDecodeAllocationFailureInjection,
    offset: usize,
    section_id: u16,
    record_index: usize,
) -> Result<String, FwirDecodeError> {
    if injection.fail_at == Some(site) {
        return Err(error(
            FwirDecodeErrorKind::AllocationUnavailable { site },
            offset,
            Some(section_id),
            u32::try_from(record_index).ok(),
        ));
    }
    let mut output = String::new();
    output
        .try_reserve_exact(value.len())
        .map_err(|_: TryReserveError| {
            error(
                FwirDecodeErrorKind::AllocationUnavailable { site },
                offset,
                Some(section_id),
                u32::try_from(record_index).ok(),
            )
        })?;
    output.push_str(value);
    Ok(output)
}

fn reconstruct_program(
    bytes: &[u8],
    plan: &DecodePlan,
    injection: FwirDecodeAllocationFailureInjection,
) -> Result<RawProgram, FwirDecodeError> {
    let module_section = match section(plan, 1) {
        Some(value) => value,
        None => {
            return Err(error(
                FwirDecodeErrorKind::NonCanonicalDirectory {
                    field: "missing_module",
                },
                HEADER_SIZE,
                Some(1),
                None,
            ));
        }
    };
    let explicit_application_plans = has_application_plans_feature(bytes, plan)?;
    let strings = section(plan, 3);
    let mut features = Vec::new();
    let mut source_units = Vec::new();
    let mut parameters = Vec::new();
    let mut types = Vec::new();
    let mut type_elements = Vec::new();
    let mut constants = Vec::new();
    let mut constant_elements = Vec::new();
    let mut origins = Vec::new();
    let mut operation_references = Vec::new();
    let mut edges = Vec::new();
    let mut shape_checks = Vec::new();
    let mut branches = Vec::new();
    let mut nodes = Vec::new();
    let mut ownership = Vec::new();
    let mut roots = Vec::new();

    reserve_arena(
        &mut features,
        section_count(plan, 2),
        FwirDecodeAllocationSite::Features,
        injection,
        section(plan, 2),
    )?;
    reserve_arena(
        &mut source_units,
        section_count(plan, 4),
        FwirDecodeAllocationSite::SourceUnits,
        injection,
        section(plan, 4),
    )?;
    reserve_arena(
        &mut parameters,
        section_count(plan, 5),
        FwirDecodeAllocationSite::Parameters,
        injection,
        section(plan, 5),
    )?;
    reserve_arena(
        &mut types,
        section_count(plan, 6),
        FwirDecodeAllocationSite::Types,
        injection,
        section(plan, 6),
    )?;
    reserve_arena(
        &mut type_elements,
        section_count(plan, 7),
        FwirDecodeAllocationSite::TypeElements,
        injection,
        section(plan, 7),
    )?;
    reserve_arena(
        &mut constants,
        section_count(plan, 8),
        FwirDecodeAllocationSite::Constants,
        injection,
        section(plan, 8),
    )?;
    reserve_arena(
        &mut constant_elements,
        section_count(plan, 9),
        FwirDecodeAllocationSite::ConstantElements,
        injection,
        section(plan, 9),
    )?;
    reserve_arena(
        &mut origins,
        section_count(plan, 10),
        FwirDecodeAllocationSite::Origins,
        injection,
        section(plan, 10),
    )?;
    reserve_arena(
        &mut operation_references,
        section_count(plan, 18),
        FwirDecodeAllocationSite::OperationReferences,
        injection,
        section(plan, 18),
    )?;
    reserve_arena(
        &mut edges,
        section_count(plan, 11),
        FwirDecodeAllocationSite::Edges,
        injection,
        section(plan, 11),
    )?;
    reserve_arena(
        &mut shape_checks,
        section_count(plan, 12),
        FwirDecodeAllocationSite::ShapeChecks,
        injection,
        section(plan, 12),
    )?;
    reserve_arena(
        &mut branches,
        section_count(plan, 13),
        FwirDecodeAllocationSite::Branches,
        injection,
        section(plan, 13),
    )?;
    reserve_arena(
        &mut nodes,
        section_count(plan, 14),
        FwirDecodeAllocationSite::Nodes,
        injection,
        section(plan, 14),
    )?;
    reserve_arena(
        &mut ownership,
        section_count(plan, 15),
        FwirDecodeAllocationSite::Ownership,
        injection,
        section(plan, 15),
    )?;
    reserve_arena(
        &mut roots,
        section_count(plan, 16),
        FwirDecodeAllocationSite::Roots,
        injection,
        section(plan, 16),
    )?;

    if let Some(records) = section(plan, 2) {
        for index in 0..records.record_count() {
            let id = record_u16(bytes, records, index, 0)?;
            if id <= Feature::BackendNativeMathV1.numeric() {
                features.push(id);
            }
        }
    }
    if let Some(records) = section(plan, 4) {
        let string_section = match strings {
            Some(value) => value,
            None => return Err(record_error(records, 0, 0, "diagnostic_name")),
        };
        for index in 0..records.record_count() {
            let string_index = record_usize(bytes, records, index, 0, "diagnostic_name")?;
            let value = string_value(bytes, string_section, plan.string_count, string_index)?;
            source_units.push(SourceUnit {
                diagnostic_name: copy_name(
                    value,
                    FwirDecodeAllocationSite::SourceName,
                    injection,
                    record_offset(records, index),
                    4,
                    index,
                )?,
                byte_length: record_u32(bytes, records, index, 4)?,
            });
        }
    }
    if let Some(records) = section(plan, 5) {
        let string_section = match strings {
            Some(value) => value,
            None => return Err(record_error(records, 0, 4, "name")),
        };
        for index in 0..records.record_count() {
            let string_index = record_usize(bytes, records, index, 4, "name")?;
            let scalar_tag = record_u8(bytes, records, index, 8)?;
            let scalar_type = scalar_type_from_tag(scalar_tag)
                .ok_or_else(|| record_error(records, index, 8, "scalar_type"))?;
            let value = string_value(bytes, string_section, plan.string_count, string_index)?;
            parameters.push(Parameter {
                slot: record_u32(bytes, records, index, 0)?,
                name: copy_name(
                    value,
                    FwirDecodeAllocationSite::ParameterName,
                    injection,
                    record_offset(records, index) + 4,
                    5,
                    index,
                )?,
                scalar_type,
                declaration_origin: OriginIndex(record_u32(bytes, records, index, 12)?),
                name_origin: OriginIndex(record_u32(bytes, records, index, 16)?),
            });
        }
    }
    if let Some(records) = section(plan, 6) {
        for index in 0..records.record_count() {
            let kind = record_u8(bytes, records, index, 0)?;
            let scalar = scalar_type_from_tag(record_u8(bytes, records, index, 1)?);
            let elements = IndexRange {
                start: record_u32(bytes, records, index, 4)?,
                count: record_u32(bytes, records, index, 8)?,
            };
            let value = match (kind, scalar) {
                (1, Some(value)) => TypeRecord::Scalar(value),
                (2, Some(value)) => TypeRecord::Vector(value),
                (3, None) => TypeRecord::Tuple { elements },
                _ => return Err(record_error(records, index, 0, "kind")),
            };
            types.push(value);
        }
    }
    if let Some(records) = section(plan, 7) {
        for index in 0..records.record_count() {
            type_elements.push(TypeIndex(record_u32(bytes, records, index, 0)?));
        }
    }
    if let Some(records) = section(plan, 8) {
        for index in 0..records.record_count() {
            let kind = record_u8(bytes, records, index, 0)?;
            let scalar_tag = record_u8(bytes, records, index, 1)?;
            let payload = record_u64(bytes, records, index, 12)?;
            let value = match kind {
                1 => ConstantRecord::Scalar(
                    scalar_constant_from_parts(scalar_tag, payload)
                        .ok_or_else(|| record_error(records, index, 1, "scalar_payload"))?,
                ),
                2 => ConstantRecord::Vector {
                    element_type: scalar_type_from_tag(scalar_tag)
                        .ok_or_else(|| record_error(records, index, 1, "scalar_type"))?,
                    elements: IndexRange {
                        start: record_u32(bytes, records, index, 4)?,
                        count: record_u32(bytes, records, index, 8)?,
                    },
                },
                _ => return Err(record_error(records, index, 0, "kind")),
            };
            constants.push(value);
        }
    }
    if let Some(records) = section(plan, 9) {
        for index in 0..records.record_count() {
            constant_elements.push(
                scalar_constant_from_parts(
                    record_u8(bytes, records, index, 0)?,
                    record_u64(bytes, records, index, 4)?,
                )
                .ok_or_else(|| record_error(records, index, 0, "scalar_payload"))?,
            );
        }
    }
    if let Some(records) = section(plan, 10) {
        for index in 0..records.record_count() {
            origins.push(Origin {
                source_unit: SourceUnitIndex(record_u32(bytes, records, index, 0)?),
                span: OriginSpan {
                    begin: OriginPosition {
                        offset: record_u32(bytes, records, index, 4)?,
                        line: record_u32(bytes, records, index, 8)?,
                        column: record_u32(bytes, records, index, 12)?,
                    },
                    end: OriginPosition {
                        offset: record_u32(bytes, records, index, 16)?,
                        line: record_u32(bytes, records, index, 20)?,
                        column: record_u32(bytes, records, index, 24)?,
                    },
                },
            });
        }
    }
    if let Some(records) = section(plan, 11) {
        for index in 0..records.record_count() {
            let access = match record_u8(bytes, records, index, 8)? {
                1 => ValueAccess::WholeValue,
                2 => ValueAccess::TupleElement(record_u32(bytes, records, index, 12)?),
                3 => ValueAccess::FanOutOperandBorrow,
                _ => return Err(record_error(records, index, 8, "access")),
            };
            let cardinality = cardinality_from_parts(
                record_u8(bytes, records, index, 9)?,
                record_u32(bytes, records, index, 16)?,
            )
            .ok_or_else(|| record_error(records, index, 9, "cardinality"))?;
            let conversion = match record_u8(bytes, records, index, 10)? {
                1 => Conversion::Identity,
                2 => Conversion::PromoteIntToDouble,
                _ => return Err(record_error(records, index, 10, "conversion")),
            };
            let ownership_mode = match record_u8(bytes, records, index, 11)? {
                1 => OwnershipMode::OwnedInput,
                2 => OwnershipMode::ImmutableBorrow,
                3 => OwnershipMode::InfallibleTransfer,
                _ => return Err(record_error(records, index, 11, "ownership")),
            };
            edges.push(Edge {
                producer: NodeIndex(record_u32(bytes, records, index, 0)?),
                argument_position: record_u32(bytes, records, index, 4)?,
                access,
                cardinality,
                conversion,
                ownership: ownership_mode,
                origin: OriginIndex(record_u32(bytes, records, index, 20)?),
            });
        }
    }
    if let Some(records) = section(plan, 12) {
        for index in 0..records.record_count() {
            shape_checks.push(record_u32(bytes, records, index, 0)?);
        }
    }
    if let Some(records) = section(plan, 13) {
        for index in 0..records.record_count() {
            branches.push(FanOutBranch {
                nodes: IndexRange {
                    start: record_u32(bytes, records, index, 0)?,
                    count: record_u32(bytes, records, index, 4)?,
                },
                root: NodeIndex(record_u32(bytes, records, index, 8)?),
                placeholder_origin: OriginIndex(record_u32(bytes, records, index, 12)?),
                origin: OriginIndex(record_u32(bytes, records, index, 16)?),
            });
        }
    }
    let mut application_plan_record = 0_usize;
    if let Some(records) = section(plan, 14) {
        for index in 0..records.record_count() {
            let kind_tag = record_u8(bytes, records, index, 0)?;
            let kind = match kind_tag {
                1 => NodeKind::Constant {
                    constant: ConstantIndex(record_u32(bytes, records, index, 24)?),
                },
                2 => NodeKind::ParameterBorrow {
                    parameter: ParameterIndex(record_u32(bytes, records, index, 24)?),
                },
                3 => NodeKind::TupleConstruct,
                4 => {
                    let lift = match record_u8(bytes, records, index, 2)? {
                        1 => LiftMode::Scalar,
                        2 => LiftMode::Vector,
                        3 => LiftMode::DynamicVector,
                        4 => LiftMode::ContainerScalar,
                        5 => LiftMode::ContainerVector,
                        _ => return Err(record_error(records, index, 2, "lift")),
                    };
                    let anchor = record_u32(bytes, records, index, 40)?;
                    let implementation_id =
                        u16::try_from(record_u32(bytes, records, index, 32)?)
                            .map_err(|_| record_error(records, index, 32, "semantic_id"))?;
                    let application_plan_id = if explicit_application_plans {
                        let plans = section(plan, 17).ok_or_else(|| {
                            error(
                                FwirDecodeErrorKind::NonCanonicalRecord {
                                    field: "missing_application_plans",
                                },
                                records.offset,
                                Some(17),
                                None,
                            )
                        })?;
                        let value = record_u16(bytes, plans, application_plan_record, 4)?;
                        application_plan_record =
                            application_plan_record.checked_add(1).ok_or_else(|| {
                                record_error(
                                    plans,
                                    application_plan_record,
                                    4,
                                    "application_plan_count",
                                )
                            })?;
                        value
                    } else {
                        crate::semantic_registry::implementation_from_numeric(implementation_id)
                            .map(|descriptor| descriptor.application_plan.id.numeric())
                            .map_err(|_| record_error(records, index, 32, "application_plan_id"))?
                    };
                    NodeKind::SelectedApply {
                        primitive_id: u16::try_from(record_u32(bytes, records, index, 24)?)
                            .map_err(|_| record_error(records, index, 24, "semantic_id"))?,
                        signature_id: u16::try_from(record_u32(bytes, records, index, 28)?)
                            .map_err(|_| record_error(records, index, 28, "semantic_id"))?,
                        implementation_id,
                        application_plan_id,
                        primitive_origin: OriginIndex(record_u32(bytes, records, index, 36)?),
                        lift,
                        result_element_type: scalar_type_from_tag(record_u8(
                            bytes, records, index, 3,
                        )?)
                        .ok_or_else(|| record_error(records, index, 3, "result_element_type"))?,
                        shape: ShapePlan {
                            static_anchor: if anchor == NONE { None } else { Some(anchor) },
                            dynamic_checks: IndexRange {
                                start: record_u32(bytes, records, index, 44)?,
                                count: record_u32(bytes, records, index, 48)?,
                            },
                        },
                    }
                }
                5 => NodeKind::PrefixSpreadPrepare,
                6 => NodeKind::FanOut {
                    branches: IndexRange {
                        start: record_u32(bytes, records, index, 24)?,
                        count: record_u32(bytes, records, index, 28)?,
                    },
                    keyword_origin: OriginIndex(record_u32(bytes, records, index, 32)?),
                },
                _ => return Err(record_error(records, index, 0, "kind")),
            };
            nodes.push(Node {
                kind,
                result_type: TypeIndex(record_u32(bytes, records, index, 4)?),
                cardinality: cardinality_from_parts(
                    record_u8(bytes, records, index, 1)?,
                    record_u32(bytes, records, index, 8)?,
                )
                .ok_or_else(|| record_error(records, index, 1, "cardinality"))?,
                edges: IndexRange {
                    start: record_u32(bytes, records, index, 12)?,
                    count: record_u32(bytes, records, index, 16)?,
                },
                origin: OriginIndex(record_u32(bytes, records, index, 20)?),
            });
        }
    }
    if let Some(records) = section(plan, 15) {
        for index in 0..records.record_count() {
            let release_index = record_u32(bytes, records, index, 8)?;
            let release_after = match record_u8(bytes, records, index, 4)? {
                1 => ReleaseAfter::Node(NodeIndex(release_index)),
                2 => ReleaseAfter::Root(RootIndex(release_index)),
                _ => return Err(record_error(records, index, 4, "release_kind")),
            };
            ownership.push(Ownership {
                owner: NodeIndex(record_u32(bytes, records, index, 0)?),
                release_after,
            });
        }
    }
    if let Some(records) = section(plan, 16) {
        for index in 0..records.record_count() {
            roots.push(Root {
                node: NodeIndex(record_u32(bytes, records, index, 0)?),
                origin: OriginIndex(record_u32(bytes, records, index, 4)?),
            });
        }
    }
    if let Some(records) = section(plan, 18) {
        for index in 0..records.record_count() {
            operation_references.push(OperationReference {
                primitive_id: record_u16(bytes, records, index, 0)?,
                signature_id: record_u16(bytes, records, index, 2)?,
                implementation_id: record_u16(bytes, records, index, 4)?,
                origin: OriginIndex(record_u32(bytes, records, index, 8)?),
            });
        }
    }

    let parameter_header = read_u32(bytes, module_section.offset + 4, Some(1), Some(0))?;
    let ranges = ProgramRanges {
        features: whole_range(features.len(), "features")?,
        source_units: whole_range(source_units.len(), "source_units")?,
        parameters: whole_range(parameters.len(), "parameters")?,
        types: whole_range(types.len(), "types")?,
        type_elements: whole_range(type_elements.len(), "type_elements")?,
        constants: whole_range(constants.len(), "constants")?,
        constant_elements: whole_range(constant_elements.len(), "constant_elements")?,
        nodes: whole_range(nodes.len(), "nodes")?,
        edges: whole_range(edges.len(), "edges")?,
        shape_checks: whole_range(shape_checks.len(), "shape_checks")?,
        origins: whole_range(origins.len(), "origins")?,
        operation_references: whole_range(operation_references.len(), "operation_references")?,
        branches: whole_range(branches.len(), "branches")?,
        ownership: whole_range(ownership.len(), "ownership")?,
        roots: whole_range(roots.len(), "roots")?,
    };
    Ok(RawProgram {
        module: ModuleMetadata {
            semantic_major: read_u16(bytes, module_section.offset, Some(1), Some(0))?,
            semantic_minor: read_u16(bytes, module_section.offset + 2, Some(1), Some(0))?,
            parameter_header_origin: if parameter_header == NONE {
                None
            } else {
                Some(OriginIndex(parameter_header))
            },
            ranges,
        },
        features,
        source_units,
        parameters,
        types,
        type_elements,
        constants,
        constant_elements,
        nodes,
        edges,
        shape_checks,
        origins,
        operation_references,
        branches,
        ownership,
        roots,
    })
}

pub fn decode_fwir(
    bytes: &[u8],
    limits: &FwirDecodeLimits,
) -> Result<VerifiedProgram, FwirDecodeError> {
    decode_fwir_with_allocation_failure(bytes, limits, FwirDecodeAllocationFailureInjection::none())
}

#[doc(hidden)]
pub fn decode_fwir_with_allocation_failure(
    bytes: &[u8],
    limits: &FwirDecodeLimits,
    injection: FwirDecodeAllocationFailureInjection,
) -> Result<VerifiedProgram, FwirDecodeError> {
    let plan = preflight(bytes, limits)?;
    validate_record_canonicality(bytes, &plan)?;
    validate_string_use(bytes, &plan, injection)?;
    let raw = reconstruct_program(bytes, &plan, injection)?;
    let verify_injection = match injection.fail_at {
        Some(FwirDecodeAllocationSite::Verifier(site)) => {
            VerifyAllocationFailureInjection::at(site)
        }
        _ => VerifyAllocationFailureInjection::none(),
    };
    raw.verify_with_allocation_failure(verify_injection)
        .map_err(|verify_error| {
            let kind = match verify_error {
                VerifyError::AllocationUnavailable { site } => {
                    FwirDecodeErrorKind::AllocationUnavailable {
                        site: FwirDecodeAllocationSite::Verifier(site),
                    }
                }
                error @ VerifyError::MalformedProgram(_) => {
                    FwirDecodeErrorKind::MalformedProgram(error)
                }
            };
            error(kind, 0, None, None)
        })
}

fn reserve_exact<T>(
    values: &mut Vec<T>,
    count: usize,
    site: FwirDecodeAllocationSite,
    injection: FwirDecodeAllocationFailureInjection,
    offset: usize,
    section_id: Option<u16>,
) -> Result<(), FwirDecodeError> {
    if injection.fail_at == Some(site) {
        return Err(error(
            FwirDecodeErrorKind::AllocationUnavailable { site },
            offset,
            section_id,
            None,
        ));
    }
    values
        .try_reserve_exact(count)
        .map_err(|_: TryReserveError| {
            error(
                FwirDecodeErrorKind::AllocationUnavailable { site },
                offset,
                section_id,
                None,
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Feature, FwirEncodeOptions, RawProgramBuilder, encode_fwir};

    fn must<T, E: std::fmt::Debug>(result: Result<T, E>) -> T {
        match result {
            Ok(value) => value,
            Err(error) => panic!("fixture construction failed: {error:?}"),
        }
    }

    fn example_bytes(name: &str) -> Vec<u8> {
        let text = match name {
            "empty" => include_str!("../spec/examples/fwir-v1-empty.hex"),
            "scalar-true" => include_str!("../spec/examples/fwir-v1-scalar-true.hex"),
            "complete" => include_str!("../spec/examples/fwir-v1-complete.hex"),
            _ => panic!("unknown example"),
        };
        match text
            .split_ascii_whitespace()
            .map(|byte| u8::from_str_radix(byte, 16))
            .collect()
        {
            Ok(bytes) => bytes,
            Err(error) => panic!("invalid checked-in example: {error}"),
        }
    }

    fn put_u16_at(bytes: &mut [u8], offset: usize, value: u16) {
        bytes[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32_at(bytes: &mut [u8], offset: usize, value: u32) {
        bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u64_at(bytes: &mut [u8], offset: usize, value: u64) {
        bytes[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
    }

    fn test_u16(bytes: &[u8], offset: usize) -> u16 {
        u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
    }

    fn test_u32(bytes: &[u8], offset: usize) -> u32 {
        u32::from_le_bytes([
            bytes[offset],
            bytes[offset + 1],
            bytes[offset + 2],
            bytes[offset + 3],
        ])
    }

    fn test_u64(bytes: &[u8], offset: usize) -> u64 {
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

    fn test_section(bytes: &[u8], wanted: u16) -> (usize, usize, usize) {
        let count = test_u32(bytes, 20) as usize;
        for index in 0..count {
            let entry = 32 + index * 24;
            if test_u16(bytes, entry) == wanted {
                return (
                    entry,
                    test_u64(bytes, entry + 8) as usize,
                    test_u64(bytes, entry + 16) as usize,
                );
            }
        }
        panic!("section {wanted} missing")
    }

    fn replace_test_section(bytes: &[u8], wanted: u16, replacement: Option<&[u8]>) -> Vec<u8> {
        let count = test_u32(bytes, 20) as usize;
        let mut sections = Vec::new();
        let mut found = false;
        for index in 0..count {
            let entry_offset = 32 + index * 24;
            let entry = bytes[entry_offset..entry_offset + 24].to_vec();
            let id = test_u16(&entry, 0);
            let payload_offset = test_u64(&entry, 8) as usize;
            let payload_length = test_u64(&entry, 16) as usize;
            let payload = if id == wanted {
                found = true;
                match replacement {
                    Some(value) => value.to_vec(),
                    None => continue,
                }
            } else {
                bytes[payload_offset..payload_offset + payload_length].to_vec()
            };
            sections.push((entry, payload));
        }
        assert!(found, "section {wanted} missing from test artifact");

        let mut rebuilt = bytes[..32].to_vec();
        put_u32_at(&mut rebuilt, 20, sections.len() as u32);
        let mut payload_offset = 32 + sections.len() * 24;
        for (entry, payload) in &mut sections {
            put_u64_at(entry, 8, payload_offset as u64);
            put_u64_at(entry, 16, payload.len() as u64);
            rebuilt.extend_from_slice(entry);
            payload_offset += payload.len();
        }
        for (_, payload) in sections {
            rebuilt.extend_from_slice(&payload);
        }
        rebuilt
    }

    fn assert_noncanonical_record(
        bytes: &[u8],
        field: &'static str,
        offset: usize,
        section_id: u16,
        record_index: Option<u32>,
    ) {
        assert_eq!(
            decode_fwir(bytes, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord { field },
                offset: offset as u64,
                section_id: Some(section_id),
                record_index,
            })
        );
    }

    fn empty_with_extension(minor: u16, flags: u16) -> Vec<u8> {
        let empty = example_bytes("empty");
        let mut bytes = empty[..32].to_vec();
        put_u16_at(&mut bytes, 10, minor);
        put_u32_at(&mut bytes, 20, 2);
        let mut module_entry = empty[32..56].to_vec();
        put_u64_at(&mut module_entry, 8, 80);
        bytes.extend_from_slice(&module_entry);
        bytes.extend_from_slice(&100_u16.to_le_bytes());
        bytes.extend_from_slice(&flags.to_le_bytes());
        bytes.extend_from_slice(&0_u32.to_le_bytes());
        bytes.extend_from_slice(&88_u64.to_le_bytes());
        bytes.extend_from_slice(&1_u64.to_le_bytes());
        bytes.extend_from_slice(&empty[56..64]);
        bytes.push(0xa5);
        bytes
    }

    fn empty_with_feature(id: u16, class: u8) -> Vec<u8> {
        let empty = example_bytes("empty");
        let mut bytes = empty[..32].to_vec();
        put_u32_at(&mut bytes, 20, 2);
        let mut module_entry = empty[32..56].to_vec();
        put_u64_at(&mut module_entry, 8, 80);
        bytes.extend_from_slice(&module_entry);
        bytes.extend_from_slice(&2_u16.to_le_bytes());
        bytes.extend_from_slice(&3_u16.to_le_bytes());
        bytes.extend_from_slice(&4_u32.to_le_bytes());
        bytes.extend_from_slice(&88_u64.to_le_bytes());
        bytes.extend_from_slice(&4_u64.to_le_bytes());
        bytes.extend_from_slice(&empty[56..64]);
        bytes.extend_from_slice(&id.to_le_bytes());
        bytes.push(class);
        bytes.push(0);
        if id == Feature::BackendNativeMathV1.numeric() {
            put_u16_at(&mut bytes, 82, 1);
        }
        bytes
    }

    fn operation_reference_bytes() -> Vec<u8> {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::OperationReferences.numeric()));
        let source = must(builder.push_source_unit(SourceUnit {
            diagnostic_name: "reference.fw".to_owned(),
            byte_length: 4,
        }));
        let origin = must(builder.push_origin(Origin {
            source_unit: source,
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
        must(builder.push_operation_reference(OperationReference {
            primitive_id: 5,
            signature_id: 9,
            implementation_id: 9,
            origin,
        }));
        let program = must(must(builder.finish()).verify());
        must(encode_fwir(&program, &FwirEncodeOptions::default()))
    }

    #[test]
    fn preflight_accepts_canonical_examples_and_rejects_every_truncation() {
        let limits = FwirDecodeLimits::default();
        for name in ["empty", "scalar-true", "complete"] {
            let bytes = example_bytes(name);
            assert!(preflight(&bytes, &limits).is_ok(), "{name}");
            for length in 0..bytes.len() {
                assert!(
                    preflight(&bytes[..length], &limits).is_err(),
                    "{name} {length}"
                );
            }
        }
    }

    #[test]
    fn preflight_limits_and_directory_failures_have_stable_precedence() {
        let bytes = example_bytes("empty");
        let limits = FwirDecodeLimits {
            max_artifact_bytes: bytes.len() - 1,
            ..FwirDecodeLimits::default()
        };
        assert!(matches!(
            preflight(&bytes, &limits),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::ArtifactTooLarge { .. },
                offset: 0,
                ..
            })
        ));

        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            preflight(&trailing, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalDirectory {
                    field: "trailing_bytes"
                },
                ..
            })
        ));
    }

    #[test]
    fn canonical_examples_decode_verify_and_reencode_byte_identically() {
        for name in ["empty", "scalar-true", "complete"] {
            let bytes = example_bytes(name);
            let program = match decode_fwir(&bytes, &FwirDecodeLimits::default()) {
                Ok(value) => value,
                Err(error) => panic!("{name} failed to decode: {error:?}"),
            };
            let encoded = match crate::encode_fwir(&program, &crate::FwirEncodeOptions::default()) {
                Ok(value) => value,
                Err(error) => panic!("{name} failed to reencode: {error:?}"),
            };
            assert_eq!(encoded, bytes, "{name}");
        }
    }

    #[test]
    fn directory_extensions_and_feature_compatibility_are_explicit() {
        let optional = empty_with_extension(1, 0);
        let decoded = match decode_fwir(&optional, &FwirDecodeLimits::default()) {
            Ok(value) => value,
            Err(error) => panic!("optional extension failed: {error:?}"),
        };
        assert_eq!(
            encode_fwir(&decoded, &FwirEncodeOptions::default()),
            Ok(example_bytes("empty"))
        );
        assert!(matches!(
            decode_fwir(&empty_with_extension(1, 1), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::UnknownMandatoryExtension { id: 100 },
                ..
            })
        ));
        assert!(matches!(
            decode_fwir(&empty_with_extension(0, 0), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalDirectory {
                    field: "unknown_extension"
                },
                ..
            })
        ));
        assert!(matches!(
            decode_fwir(&empty_with_extension(1, 2), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalDirectory {
                    field: "unknown_extension"
                },
                ..
            })
        ));

        let advisory = empty_with_feature(100, 1);
        let decoded = match decode_fwir(&advisory, &FwirDecodeLimits::default()) {
            Ok(value) => value,
            Err(error) => panic!("advisory feature failed: {error:?}"),
        };
        assert_eq!(
            encode_fwir(&decoded, &FwirEncodeOptions::default()),
            Ok(example_bytes("empty"))
        );
        let math = empty_with_feature(Feature::BackendNativeMathV1.numeric(), 0);
        let decoded = match decode_fwir(&math, &FwirDecodeLimits::default()) {
            Ok(value) => value,
            Err(error) => panic!("backend-native math feature failed: {error:?}"),
        };
        assert_eq!(
            encode_fwir(&decoded, &FwirEncodeOptions::default()),
            Ok(math.clone())
        );
        let mut wrong_semantic_version = math;
        put_u16_at(&mut wrong_semantic_version, 82, 0);
        assert!(matches!(
            decode_fwir(&wrong_semantic_version, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(crate::VerifyError::MalformedProgram(
                    crate::MalformedProgram {
                        invariant: crate::Invariant::UnsupportedVersion,
                        field: "semantic_version",
                        ..
                    }
                )),
                ..
            })
        ));
        assert!(matches!(
            decode_fwir(&empty_with_feature(1, 1), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "feature_class"
                },
                ..
            })
        ));
        assert!(matches!(
            decode_fwir(&empty_with_feature(100, 0), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::UnknownMandatoryExtension { id: 100 },
                ..
            })
        ));

        assert!(matches!(
            decode_fwir(&empty_with_feature(5, 0), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "feature_format_minor"
                },
                ..
            })
        ));
        assert!(matches!(
            decode_fwir(&empty_with_feature(6, 0), &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "feature_format_minor"
                },
                ..
            })
        ));
        let mut supported_reference_feature = empty_with_feature(6, 0);
        put_u16_at(&mut supported_reference_feature, 10, 1);
        let (_, module, _) = test_section(&supported_reference_feature, 1);
        put_u16_at(&mut supported_reference_feature, module + 2, 1);
        assert!(decode_fwir(&supported_reference_feature, &FwirDecodeLimits::default()).is_ok());
    }

    #[test]
    fn operation_reference_records_reject_reserved_bytes_and_invalid_identity() {
        let canonical = operation_reference_bytes();
        let (_, references, length) = test_section(&canonical, 18);
        assert_eq!(length, 16);

        for offset in [references + 6, references + 12] {
            let mut reserved = canonical.clone();
            reserved[offset] = 1;
            assert!(matches!(
                decode_fwir(&reserved, &FwirDecodeLimits::default()),
                Err(FwirDecodeError {
                    kind: FwirDecodeErrorKind::NonCanonicalRecord { field: "reserved" },
                    section_id: Some(18),
                    ..
                })
            ));
        }

        for (relative, value) in [(0, 6_u16), (2, 10), (4, 35)] {
            let mut identity = canonical.clone();
            put_u16_at(&mut identity, references + relative, value);
            assert!(matches!(
                decode_fwir(&identity, &FwirDecodeLimits::default()),
                Err(FwirDecodeError {
                    kind: FwirDecodeErrorKind::NonCanonicalRecord {
                        field: "semantic_id"
                    },
                    section_id: Some(18),
                    ..
                })
            ));
        }
        let mut structural = canonical;
        put_u16_at(&mut structural, references, 19);
        put_u16_at(&mut structural, references + 2, 34);
        put_u16_at(&mut structural, references + 4, 34);
        assert!(matches!(
            decode_fwir_with_allocation_failure(
                &structural,
                &FwirDecodeLimits::default(),
                FwirDecodeAllocationFailureInjection::at(
                    FwirDecodeAllocationSite::OperationReferences
                )
            ),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "semantic_id"
                },
                section_id: Some(18),
                ..
            })
        ));
    }

    #[test]
    fn malformed_directory_claims_are_rejected_before_record_decoding() {
        let empty = example_bytes("empty");
        let cases = [
            (34, 4_u64, "flags"),
            (36, 4, "record_size"),
            (40, 57, "contiguous_payload"),
            (48, u64::MAX, "length"),
        ];
        for (offset, value, expected) in cases {
            let mut bytes = empty.clone();
            if offset == 34 || offset == 36 {
                put_u16_at(&mut bytes, offset, value as u16);
            } else {
                put_u64_at(&mut bytes, offset, value);
            }
            let result = decode_fwir(&bytes, &FwirDecodeLimits::default());
            assert!(
                matches!(
                    result,
                    Err(FwirDecodeError {
                        kind: FwirDecodeErrorKind::NonCanonicalDirectory { field },
                        ..
                    }) if field == expected
                ) || matches!(
                    result,
                    Err(FwirDecodeError {
                        kind: FwirDecodeErrorKind::InvalidSectionLength,
                        ..
                    }) if expected == "length"
                ),
                "{offset}: {result:?}"
            );
        }
    }

    #[test]
    fn record_mutations_cover_reserved_tags_utf8_strings_ids_and_graphs() {
        let canonical = example_bytes("complete");
        let (_, strings, string_length) = test_section(&canonical, 3);
        let string_count = test_u32(&canonical, strings) as usize;
        let string_area = strings + 4 + string_count * 8;
        assert!(string_area < strings + string_length);
        let (_, parameters, _) = test_section(&canonical, 5);
        let (_, constants, _) = test_section(&canonical, 8);
        let (_, nodes, _) = test_section(&canonical, 14);
        let (_, roots, _) = test_section(&canonical, 16);

        let mutations = [
            (parameters + 9, 1_u8, "reserved"),
            (constants, 0, "kind"),
            (nodes, 0, "kind"),
        ];
        for (offset, value, expected_field) in mutations {
            let mut bytes = canonical.clone();
            bytes[offset] = value;
            assert!(matches!(
                decode_fwir(&bytes, &FwirDecodeLimits::default()),
                Err(FwirDecodeError {
                    kind: FwirDecodeErrorKind::NonCanonicalRecord { field },
                    ..
                }) if field == expected_field
            ));
        }

        let mut invalid_utf8 = canonical.clone();
        invalid_utf8[string_area] = 0xff;
        assert!(matches!(
            decode_fwir(&invalid_utf8, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::InvalidUtf8,
                ..
            })
        ));

        let mut invalid_id = canonical.clone();
        let selected_apply = invalid_id[nodes..]
            .chunks_exact(56)
            .position(|record| record[0] == 4)
            .map(|index| nodes + index * 56)
            .unwrap_or_else(|| panic!("complete fixture has no selected apply"));
        put_u32_at(&mut invalid_id, selected_apply + 24, u32::MAX);
        assert!(matches!(
            decode_fwir(&invalid_id, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord { .. },
                ..
            })
        ));

        let mut graph = canonical;
        put_u32_at(&mut graph, roots, u32::MAX);
        assert!(matches!(
            decode_fwir(&graph, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(_)),
                ..
            })
        ));
    }

    #[test]
    fn explicit_limits_refuse_claims_before_arena_allocation() {
        let complete = example_bytes("complete");
        for limits in [
            FwirDecodeLimits {
                max_sections: 1,
                ..FwirDecodeLimits::default()
            },
            FwirDecodeLimits {
                max_records_per_section: 1,
                ..FwirDecodeLimits::default()
            },
            FwirDecodeLimits {
                max_total_records: 1,
                ..FwirDecodeLimits::default()
            },
            FwirDecodeLimits {
                max_string_bytes: 1,
                ..FwirDecodeLimits::default()
            },
        ] {
            assert!(matches!(
                decode_fwir(&complete, &limits),
                Err(FwirDecodeError {
                    kind: FwirDecodeErrorKind::ResourceLimit { .. },
                    ..
                })
            ));
        }
    }

    #[test]
    fn every_decoder_allocation_site_has_an_explicit_refusal() {
        let bytes = example_bytes("complete");
        for site in [
            FwirDecodeAllocationSite::Features,
            FwirDecodeAllocationSite::SourceUnits,
            FwirDecodeAllocationSite::SourceName,
            FwirDecodeAllocationSite::Parameters,
            FwirDecodeAllocationSite::ParameterName,
            FwirDecodeAllocationSite::Types,
            FwirDecodeAllocationSite::TypeElements,
            FwirDecodeAllocationSite::Constants,
            FwirDecodeAllocationSite::ConstantElements,
            FwirDecodeAllocationSite::Origins,
            FwirDecodeAllocationSite::Edges,
            FwirDecodeAllocationSite::ShapeChecks,
            FwirDecodeAllocationSite::Branches,
            FwirDecodeAllocationSite::Nodes,
            FwirDecodeAllocationSite::Ownership,
            FwirDecodeAllocationSite::Roots,
            FwirDecodeAllocationSite::StringUse,
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::DynamicShapeScratch),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::ReachabilityBits),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::ReachabilityWorklist),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::FanOutBorrowContext),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::OwnershipSinks),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::OwnershipLastUse),
            FwirDecodeAllocationSite::Verifier(VerifyAllocationSite::OwnershipRootOwner),
        ] {
            assert!(
                matches!(
                    decode_fwir_with_allocation_failure(
                        &bytes,
                        &FwirDecodeLimits::default(),
                        FwirDecodeAllocationFailureInjection::at(site)
                    ),
                    Err(FwirDecodeError {
                        kind: FwirDecodeErrorKind::AllocationUnavailable { site: actual },
                        ..
                    }) if actual == site
                ),
                "{site:?}"
            );
        }
        let reference_bytes = operation_reference_bytes();
        let site = FwirDecodeAllocationSite::OperationReferences;
        assert!(matches!(
            decode_fwir_with_allocation_failure(
                &reference_bytes,
                &FwirDecodeLimits::default(),
                FwirDecodeAllocationFailureInjection::at(site)
            ),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::AllocationUnavailable { site: actual },
                ..
            }) if actual == site
        ));
    }

    fn planned_program(depth: u32, explicit_application_plans: bool) -> VerifiedProgram {
        planned_program_with_features(depth, explicit_application_plans, false, false)
    }

    fn length_program() -> VerifiedProgram {
        must(crate::lowering::compile_source_with_name(
            "length[(1 2 3)]\n",
            "length.faraweave",
        ))
    }

    fn sort_program() -> VerifiedProgram {
        must(crate::lowering::compile_source_with_name(
            "sort[(3 1 2)]\n",
            "sort.faraweave",
        ))
    }

    fn sum_program() -> VerifiedProgram {
        must(crate::lowering::compile_source_with_name(
            "sum[(1 2 3)]\n",
            "sum.faraweave",
        ))
    }

    fn planned_program_with_features(
        depth: u32,
        explicit_application_plans: bool,
        operation_references: bool,
        backend_native_math: bool,
    ) -> VerifiedProgram {
        let mut builder = RawProgramBuilder::new();
        must(builder.push_feature(Feature::StableSemanticIds.numeric()));
        if explicit_application_plans {
            must(builder.push_feature(Feature::ApplicationPlans.numeric()));
        }
        if operation_references {
            must(builder.push_feature(Feature::OperationReferences.numeric()));
        }
        if backend_native_math {
            must(builder.push_feature(Feature::BackendNativeMathV1.numeric()));
        }
        let source = must(builder.push_source_unit(SourceUnit {
            diagnostic_name: "deep.fw".to_owned(),
            byte_length: 1,
        }));
        let origin = must(builder.push_origin(Origin {
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
        }));
        if operation_references {
            must(builder.push_operation_reference(OperationReference {
                primitive_id: 5,
                signature_id: 9,
                implementation_id: 9,
                origin,
            }));
        }
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
                    application_plan_id: 1,
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
        let mut raw = must(builder.finish());
        if explicit_application_plans || operation_references || backend_native_math {
            raw.module.semantic_minor = 1;
        }
        must(raw.verify())
    }

    fn deep_program(depth: u32) -> VerifiedProgram {
        planned_program(depth, false)
    }

    #[test]
    fn application_plan_minor_extension_roundtrips_and_preserves_v1_0_bytes() {
        let legacy = must(encode_fwir(
            &planned_program(1, false),
            &FwirEncodeOptions::default(),
        ));
        assert_eq!(read_u16(&legacy, 10, None, None), Ok(0));
        let (_, legacy_nodes, legacy_nodes_length) = test_section(&legacy, 14);
        let legacy_apply = legacy[legacy_nodes..legacy_nodes + legacy_nodes_length]
            .chunks_exact(56)
            .position(|record| record[0] == 4)
            .map(|index| legacy_nodes + index * 56)
            .unwrap_or(legacy_nodes);
        assert_eq!(
            read_u32(&legacy, legacy_apply + 52, Some(14), Some(1)),
            Ok(0)
        );
        let decoded_legacy = must(decode_fwir(&legacy, &FwirDecodeLimits::default()));
        let legacy_plan = decoded_legacy
            .as_raw()
            .nodes
            .iter()
            .find_map(|node| match node.kind {
                NodeKind::SelectedApply {
                    application_plan_id,
                    ..
                } => Some(application_plan_id),
                _ => None,
            });
        assert_eq!(legacy_plan, Some(1));
        assert_eq!(
            must(encode_fwir(&decoded_legacy, &FwirEncodeOptions::default())),
            legacy
        );

        let explicit = must(encode_fwir(
            &planned_program(1, true),
            &FwirEncodeOptions::default(),
        ));
        assert_eq!(read_u16(&explicit, 10, None, None), Ok(1));
        let (_, explicit_nodes, explicit_nodes_length) = test_section(&explicit, 14);
        let explicit_apply = explicit[explicit_nodes..explicit_nodes + explicit_nodes_length]
            .chunks_exact(56)
            .position(|record| record[0] == 4)
            .map(|index| explicit_nodes + index * 56)
            .unwrap_or(explicit_nodes);
        assert_eq!(
            read_u32(&explicit, explicit_apply + 52, Some(14), Some(1)),
            Ok(0)
        );
        let (_, explicit_plans, explicit_plans_length) = test_section(&explicit, 17);
        assert_eq!(explicit_plans_length, 8);
        assert_eq!(
            read_u32(&explicit, explicit_plans, Some(17), Some(0)),
            Ok(1)
        );
        assert_eq!(
            read_u16(&explicit, explicit_plans + 4, Some(17), Some(0)),
            Ok(1)
        );
        let decoded_explicit = must(decode_fwir(&explicit, &FwirDecodeLimits::default()));
        assert_eq!(
            must(encode_fwir(
                &decoded_explicit,
                &FwirEncodeOptions::default()
            )),
            explicit
        );
        assert!(matches!(
            decode_fwir_with_allocation_failure(
                &explicit,
                &FwirDecodeLimits::default(),
                FwirDecodeAllocationFailureInjection::at(FwirDecodeAllocationSite::Nodes)
            ),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::AllocationUnavailable {
                    site: FwirDecodeAllocationSite::Nodes
                },
                ..
            })
        ));

        let mut unknown_plan = explicit;
        put_u16_at(&mut unknown_plan, explicit_plans + 4, 99);
        assert_eq!(
            decode_fwir(&unknown_plan, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::NonCanonicalRecord {
                    field: "application_plan_id"
                },
                offset: (explicit_plans + 4) as u64,
                section_id: Some(17),
                record_index: Some(0),
            })
        );
    }

    #[test]
    fn length_plan_roundtrips_and_malformed_physical_records_are_rejected() {
        let encoded = must(encode_fwir(
            &length_program(),
            &FwirEncodeOptions::default(),
        ));
        assert_eq!(read_u16(&encoded, 10, None, None), Ok(1));
        let (_, plans, plans_length) = test_section(&encoded, 17);
        assert_eq!(plans_length, 8);
        assert_eq!(read_u16(&encoded, plans + 4, Some(17), Some(0)), Ok(3));
        let decoded = must(decode_fwir(&encoded, &FwirDecodeLimits::default()));
        assert_eq!(
            must(encode_fwir(&decoded, &FwirEncodeOptions::default())),
            encoded
        );

        let mut wrong_plan = encoded.clone();
        put_u16_at(&mut wrong_plan, plans + 4, 1);
        assert_noncanonical_record(&wrong_plan, "application_plan_id", plans + 4, 17, Some(0));

        let (_, nodes, nodes_length) = test_section(&encoded, 14);
        let length_node = encoded[nodes..nodes + nodes_length]
            .chunks_exact(56)
            .position(|record| record[0] == 4 && test_u32(record, 24) == 21)
            .map(|index| nodes + index * 56)
            .unwrap_or(nodes);
        let mut scalar_lift = encoded;
        scalar_lift[length_node + 2] = 1;
        assert!(matches!(
            decode_fwir(&scalar_lift, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                    crate::MalformedProgram {
                        invariant: crate::Invariant::InconsistentResultMetadata,
                        record: crate::RecordKind::Node,
                        field: "result",
                        ..
                    }
                )),
                ..
            })
        ));
    }

    #[test]
    fn sort_plan_roundtrips_and_malformed_physical_records_are_rejected() {
        let encoded = must(encode_fwir(&sort_program(), &FwirEncodeOptions::default()));
        assert_eq!(read_u16(&encoded, 10, None, None), Ok(1));
        let (_, plans, plans_length) = test_section(&encoded, 17);
        assert_eq!(plans_length, 8);
        assert_eq!(read_u16(&encoded, plans + 4, Some(17), Some(0)), Ok(4));
        let decoded = must(decode_fwir(&encoded, &FwirDecodeLimits::default()));
        assert_eq!(
            must(encode_fwir(&decoded, &FwirEncodeOptions::default())),
            encoded
        );

        let mut wrong_plan = encoded.clone();
        put_u16_at(&mut wrong_plan, plans + 4, 3);
        assert_noncanonical_record(&wrong_plan, "application_plan_id", plans + 4, 17, Some(0));

        let (_, nodes, nodes_length) = test_section(&encoded, 14);
        let sort_node = encoded[nodes..nodes + nodes_length]
            .chunks_exact(56)
            .position(|record| record[0] == 4 && test_u32(record, 24) == 22)
            .map(|index| nodes + index * 56)
            .unwrap_or(nodes);
        let mut scalar_lift = encoded;
        scalar_lift[sort_node + 2] = 4;
        assert!(matches!(
            decode_fwir(&scalar_lift, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                    crate::MalformedProgram {
                        invariant: crate::Invariant::InconsistentResultMetadata,
                        record: crate::RecordKind::Node,
                        field: "result",
                        ..
                    }
                )),
                ..
            })
        ));
    }

    #[test]
    fn sum_plan_roundtrips_and_malformed_physical_records_are_rejected() {
        let encoded = must(encode_fwir(&sum_program(), &FwirEncodeOptions::default()));
        assert_eq!(read_u16(&encoded, 10, None, None), Ok(1));
        let (_, plans, plans_length) = test_section(&encoded, 17);
        assert_eq!(plans_length, 8);
        assert_eq!(read_u16(&encoded, plans + 4, Some(17), Some(0)), Ok(5));
        let decoded = must(decode_fwir(&encoded, &FwirDecodeLimits::default()));
        assert_eq!(
            must(encode_fwir(&decoded, &FwirEncodeOptions::default())),
            encoded
        );

        let mut wrong_plan = encoded.clone();
        put_u16_at(&mut wrong_plan, plans + 4, 3);
        assert_noncanonical_record(&wrong_plan, "application_plan_id", plans + 4, 17, Some(0));

        let (_, nodes, nodes_length) = test_section(&encoded, 14);
        let sum_node = encoded[nodes..nodes + nodes_length]
            .chunks_exact(56)
            .position(|record| record[0] == 4 && test_u32(record, 24) == 23)
            .map(|index| nodes + index * 56)
            .unwrap_or(nodes);
        let mut scalar_lift = encoded;
        scalar_lift[sum_node + 2] = 1;
        assert!(matches!(
            decode_fwir(&scalar_lift, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                    crate::MalformedProgram {
                        invariant: crate::Invariant::InconsistentResultMetadata,
                        record: crate::RecordKind::Node,
                        field: "result",
                        ..
                    }
                )),
                ..
            })
        ));
    }

    #[test]
    fn application_plan_section_presence_count_and_node_mapping_errors_are_exact() {
        let explicit_one = must(encode_fwir(
            &planned_program(1, true),
            &FwirEncodeOptions::default(),
        ));
        let (_, features, features_length) = test_section(&explicit_one, 2);
        let mut retained_features = Vec::new();
        for record in explicit_one[features..features + features_length].chunks_exact(4) {
            if test_u16(record, 0) != Feature::ApplicationPlans.numeric() {
                retained_features.extend_from_slice(record);
            }
        }
        let section_without_feature =
            replace_test_section(&explicit_one, 2, Some(&retained_features));
        let (_, plans_without_feature, _) = test_section(&section_without_feature, 17);
        assert_noncanonical_record(
            &section_without_feature,
            "application_plans_feature",
            plans_without_feature,
            17,
            Some(0),
        );

        let missing_section = replace_test_section(&explicit_one, 17, None);
        let (_, missing_section_nodes, _) = test_section(&missing_section, 14);
        assert_noncanonical_record(
            &missing_section,
            "missing_application_plans",
            missing_section_nodes,
            17,
            None,
        );

        let explicit_two = must(encode_fwir(
            &planned_program(2, true),
            &FwirEncodeOptions::default(),
        ));
        let (_, plans, plans_length) = test_section(&explicit_two, 17);
        assert_eq!(plans_length, 16);
        let count_mismatch =
            replace_test_section(&explicit_two, 17, Some(&explicit_two[plans..plans + 8]));
        let (_, mismatched_plans, _) = test_section(&count_mismatch, 17);
        assert_noncanonical_record(
            &count_mismatch,
            "application_plan_count",
            mismatched_plans,
            17,
            Some(0),
        );

        let plan_records = &explicit_two[plans..plans + plans_length];
        let first_node = test_u32(plan_records, 0);
        let second_node = test_u32(plan_records, 8);
        assert!(first_node < second_node);

        let mut out_of_order_records = plan_records.to_vec();
        put_u32_at(&mut out_of_order_records, 0, second_node);
        put_u32_at(&mut out_of_order_records, 8, first_node);
        let out_of_order = replace_test_section(&explicit_two, 17, Some(&out_of_order_records));
        let (_, out_of_order_plans, _) = test_section(&out_of_order, 17);
        assert_noncanonical_record(
            &out_of_order,
            "application_plan_node",
            out_of_order_plans,
            17,
            Some(0),
        );

        let mut duplicate_records = plan_records.to_vec();
        put_u32_at(&mut duplicate_records, 8, first_node);
        let duplicate = replace_test_section(&explicit_two, 17, Some(&duplicate_records));
        let (_, duplicate_plans, _) = test_section(&duplicate, 17);
        assert_noncanonical_record(
            &duplicate,
            "application_plan_node",
            duplicate_plans + 8,
            17,
            Some(1),
        );
    }

    #[test]
    fn application_plan_record_identity_and_version_errors_are_exact() {
        let explicit = must(encode_fwir(
            &planned_program(1, true),
            &FwirEncodeOptions::default(),
        ));
        let (_, plans, _) = test_section(&explicit, 17);

        let mut reserved = explicit.clone();
        reserved[plans + 6] = 1;
        assert_noncanonical_record(&reserved, "reserved", plans + 6, 17, Some(0));

        let mut known_mismatch = explicit.clone();
        put_u16_at(&mut known_mismatch, plans + 4, 2);
        assert_noncanonical_record(
            &known_mismatch,
            "application_plan_id",
            plans + 4,
            17,
            Some(0),
        );

        let mut physical_minor_zero = explicit.clone();
        put_u16_at(&mut physical_minor_zero, 10, 0);
        let (_, features, features_length) = test_section(&physical_minor_zero, 2);
        let application_plan_feature = physical_minor_zero[features..features + features_length]
            .chunks_exact(4)
            .position(|record| test_u16(record, 0) == Feature::ApplicationPlans.numeric())
            .unwrap_or_else(|| panic!("application-plans feature missing"));
        assert_noncanonical_record(
            &physical_minor_zero,
            "feature_format_minor",
            features + application_plan_feature * 4,
            2,
            u32::try_from(application_plan_feature).ok(),
        );

        let mut semantic_minor_zero = explicit;
        let (_, module, _) = test_section(&semantic_minor_zero, 1);
        put_u16_at(&mut semantic_minor_zero, module + 2, 0);
        assert_eq!(
            decode_fwir(&semantic_minor_zero, &FwirDecodeLimits::default()),
            Err(FwirDecodeError {
                kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
                    crate::MalformedProgram {
                        invariant: crate::Invariant::UnsupportedVersion,
                        record: crate::RecordKind::Module,
                        index: None,
                        field: "application_plans",
                    }
                )),
                offset: 0,
                section_id: None,
                record_index: None,
            })
        );
    }

    #[test]
    fn application_plans_operation_references_and_backend_math_coexist_canonically() {
        let encoded = must(encode_fwir(
            &planned_program_with_features(1, true, true, true),
            &FwirEncodeOptions::default(),
        ));
        assert_eq!(read_u16(&encoded, 10, None, None), Ok(1));
        let decoded = must(decode_fwir(&encoded, &FwirDecodeLimits::default()));
        assert_eq!(
            decoded.as_raw().features,
            vec![
                Feature::StableSemanticIds.numeric(),
                Feature::ApplicationPlans.numeric(),
                Feature::OperationReferences.numeric(),
                Feature::BackendNativeMathV1.numeric(),
            ]
        );
        assert_eq!(test_section(&encoded, 17).2, 8);
        assert_eq!(test_section(&encoded, 18).2, 16);
        assert_eq!(
            must(encode_fwir(&decoded, &FwirEncodeOptions::default())),
            encoded
        );
    }

    #[test]
    fn deep_valid_and_invalid_graphs_decode_on_a_reduced_stack() {
        let valid = match encode_fwir(&deep_program(2_000), &FwirEncodeOptions::default()) {
            Ok(value) => value,
            Err(error) => panic!("deep fixture encoding failed: {error:?}"),
        };
        let mut invalid = valid.clone();
        let (_, roots, _) = test_section(&invalid, 16);
        put_u32_at(&mut invalid, roots, u32::MAX);
        let thread = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                assert!(decode_fwir(&valid, &FwirDecodeLimits::default()).is_ok());
                assert!(matches!(
                    decode_fwir(&invalid, &FwirDecodeLimits::default()),
                    Err(FwirDecodeError {
                        kind: FwirDecodeErrorKind::MalformedProgram(_),
                        ..
                    })
                ));
            });
        match thread {
            Ok(handle) => assert!(handle.join().is_ok()),
            Err(error) => panic!("unable to create reduced-stack thread: {error}"),
        }
    }
}
