use crate::{ResourceUsage, ScalarType, Type, Value};
use std::fmt;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceLocation {
    pub offset: usize,
    pub line: usize,
    pub column: usize,
}

impl SourceLocation {
    pub const fn start() -> Self {
        Self {
            offset: 1,
            line: 1,
            column: 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct SourceSpan {
    pub begin: SourceLocation,
    pub end: SourceLocation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrorKind {
    InvalidByte,
    MalformedLiteral,
    LiteralRangeError,
    SyntaxError,
    UnknownPrimitive,
    ParameterError,
    TypeError,
    EmptyExpression,
    ArityError,
    ArgumentError,
    ShapeMismatch,
    InvalidExecutionProfile,
    ProfileError,
    ValueError,
    ResourceError,
    DomainError,
    FormattingError,
    OutputError,
    FileError,
}

impl ErrorKind {
    pub const fn diagnostic_name(self) -> &'static str {
        match self {
            Self::InvalidByte => "InvalidByte",
            Self::MalformedLiteral => "MalformedLiteral",
            Self::LiteralRangeError => "LiteralRangeError",
            Self::SyntaxError => "SyntaxError",
            Self::UnknownPrimitive => "UnknownPrimitive",
            Self::ParameterError => "ParameterError",
            Self::TypeError => "TypeError",
            Self::EmptyExpression => "EmptyExpression",
            Self::ArityError => "ArityError",
            Self::ArgumentError => "ArgumentError",
            Self::ShapeMismatch => "ShapeMismatch",
            Self::InvalidExecutionProfile => "InvalidExecutionProfile",
            Self::ProfileError => "ProfileError",
            Self::ValueError => "ValueError",
            Self::ResourceError => "ResourceError",
            Self::DomainError => "DomainError",
            Self::FormattingError => "FormattingError",
            Self::OutputError => "OutputError",
            Self::FileError => "file error",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceErrorReason {
    SizeOverflow,
    ProfileLimit,
    AllocationUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceErrorContext {
    pub reason: ResourceErrorReason,
    pub requested_elements: Option<usize>,
    pub requested_bytes: Option<usize>,
    pub profile: &'static str,
    pub limit_kind: Option<&'static str>,
    pub configured_limit: Option<usize>,
    pub usage_before: Option<usize>,
    pub refused_charge: Option<usize>,
    pub allocation_ordinal: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DomainErrorReason {
    IntegerOverflow,
    DivisionByZero,
}

#[derive(Clone, Debug, PartialEq)]
pub struct DomainErrorContext {
    pub reason: DomainErrorReason,
    pub parameter_types: Vec<ScalarType>,
    pub result_type: ScalarType,
    pub operands: Vec<Value>,
    pub element_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArgumentErrorReason {
    Missing,
    Extra,
    InvalidLiteral,
    OutOfRange,
    InvalidTypedValue,
    ContainerMismatch,
    TypeMismatch,
}

impl ArgumentErrorReason {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Extra => "extra",
            Self::InvalidLiteral => "invalid_literal",
            Self::OutOfRange => "out_of_range",
            Self::InvalidTypedValue => "invalid_typed_value",
            Self::ContainerMismatch => "container_mismatch",
            Self::TypeMismatch => "type_mismatch",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArgumentErrorContext {
    pub reason: ArgumentErrorReason,
    pub required_count: usize,
    pub supplied_count: usize,
    pub position: usize,
    pub parameter_name: Option<String>,
    pub expected_type: Option<ScalarType>,
    pub declaration_span: Option<SourceSpan>,
    pub actual_container: Option<&'static str>,
    pub actual_type: Option<ScalarType>,
    pub invalid_value_invariant: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ParameterErrorReason {
    SecondParameterHeader,
    ParameterHeaderAfterRoot,
    ExpectedHeaderOpen,
    ExpectedParameterName,
    ExpectedParameterType,
    MissingHeaderClose,
    UnexpectedHeaderToken,
    TrailingHeaderBytes,
    DuplicateParameterName,
    ReservedParameterName,
    ProgramOnlyParameterHeader,
    UnsupportedParameterizedSurface,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ParameterErrorContext {
    pub reason: ParameterErrorReason,
    pub primary_span: SourceSpan,
    pub context_span: SourceSpan,
    pub related_span: Option<SourceSpan>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Error {
    pub kind: ErrorKind,
    pub location: SourceLocation,
    pub span: Option<SourceSpan>,
    pub message: String,
    pub primitive: Option<String>,
    pub argument_position: Option<usize>,
    pub expected_arity: Vec<usize>,
    pub actual_arity: Option<usize>,
    pub expected_types: Vec<Vec<ScalarType>>,
    pub actual_types: Vec<Type>,
    pub expected_shape: Option<Vec<usize>>,
    pub actual_shape: Option<Vec<usize>>,
    pub resource: Option<ResourceErrorContext>,
    pub domain: Option<DomainErrorContext>,
    pub argument: Option<ArgumentErrorContext>,
    pub parameter: Option<ParameterErrorContext>,
    pub usage: Option<ResourceUsage>,
}

impl Error {
    pub fn new(kind: ErrorKind, location: SourceLocation, message: impl Into<String>) -> Self {
        Self {
            kind,
            location,
            span: None,
            message: message.into(),
            primitive: None,
            argument_position: None,
            expected_arity: Vec::new(),
            actual_arity: None,
            expected_types: Vec::new(),
            actual_types: Vec::new(),
            expected_shape: None,
            actual_shape: None,
            resource: None,
            domain: None,
            argument: None,
            parameter: None,
            usage: None,
        }
    }

    pub fn at_span(kind: ErrorKind, span: SourceSpan, message: impl Into<String>) -> Self {
        let mut error = Self::new(kind, span.begin, message);
        error.span = Some(span);
        error
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

#[derive(Clone, Debug, PartialEq)]
pub struct LocatedError {
    pub source_name: String,
    pub error: Error,
}

impl fmt::Display for LocatedError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "{}:{}:{}: {}: {}",
            self.source_name,
            self.error.location.line,
            self.error.location.column,
            self.error.kind.diagnostic_name(),
            self.error.message
        )
    }
}

impl std::error::Error for LocatedError {}
