//! Faraweave's standalone Rust language runtime.
//!
//! Source APIs parse and lower exactly once to [`VerifiedProgram`]. Direct
//! execution then uses [`evaluate_verified_program`], while C and native
//! execution use [`emit_c_from_verified_program`] and
//! [`build_native_from_verified_program`]; none of those backends accepts a
//! parser AST or performs overload selection.
//!
//! [`decode_fwir`] treats bytes as untrusted, applies [`FwirDecodeLimits`], and
//! returns only after complete physical decoding and semantic verification.
//! FWIR v1 is deterministic but neither confidential nor a sandbox: artifacts
//! can reveal names, constants, graph structure, provenance, and producer
//! metadata, and generated/native code executes with the caller's authority.
#![allow(clippy::result_large_err)]

mod c_emitter;
mod error;
mod evaluator;
mod fwir_api;
mod fwir_decoder;
mod fwir_encoder;
mod interpreter;
mod lowering;
mod native_builder;
mod parser;
mod primitive;
mod resources;
mod semantic_registry;
mod strict_float;
mod typed_program;
mod value;

pub use c_emitter::{CEmissionResult, emit_c_source, emit_c_source_with_configuration};
pub use error::{
    ArgumentErrorContext, ArgumentErrorReason, DomainErrorContext, DomainErrorReason, Error,
    ErrorKind, LocatedError, ParameterErrorContext, ParameterErrorReason, ResourceErrorContext,
    ResourceErrorReason, SourceLocation, SourceSpan,
};
pub use evaluator::{
    EvaluationConfiguration, ProgramResult, RunnerEvaluationResult, ValueResult,
    evaluate_expression, evaluate_expression_with_configuration, evaluate_expression_with_observer,
    evaluate_runner_source, evaluate_source, evaluate_source_with_arguments,
    evaluate_source_with_arguments_and_observer, evaluate_source_with_configuration,
};
pub use fwir_api::{
    CompileFwirError, FwirInspectError, build_native_from_verified_program, compile_source_to_fwir,
    compile_source_to_fwir_with_name, compile_source_to_verified_program,
    emit_c_from_verified_program, evaluate_verified_program_with_arguments, inspect_fwir,
};
pub use fwir_decoder::{
    FwirDecodeAllocationFailureInjection, FwirDecodeAllocationSite, FwirDecodeError,
    FwirDecodeErrorKind, FwirDecodeLimit, FwirDecodeLimits, decode_fwir,
    decode_fwir_with_allocation_failure,
};
pub use fwir_encoder::{
    FwirEncodeAllocationFailureInjection, FwirEncodeAllocationSite, FwirEncodeError,
    FwirEncodeOptions, FwirOutputOperation, FwirProducerMetadata, encode_fwir,
    encode_fwir_with_allocation_failure, encode_fwir_with_atomic_publication, write_fwir,
};
pub use interpreter::{evaluate_verified_program, evaluate_verified_program_with_observer};
pub use native_builder::{
    CompilerConfiguration, CompilerSelection, NativeBuildRequest, NativeBuildResult,
    NativePlatform, build_native, make_c_compiler_arguments, native_platform,
    publish_file_atomically, select_c_compiler,
};
pub use resources::{
    AllocationFailureInjection, ExecutionProfile, ResourceEvent, ResourceEventKind, ResourceLimits,
    ResourceObserver, ResourceUsage,
};
#[doc(hidden)]
pub use semantic_registry::{InternalRegistryDiagnosticError, write_internal_registry_diagnostics};
pub use typed_program::{
    Arena, BuildError, Cardinality, ConstantIndex, ConstantRecord, Conversion, Edge, FanOutBranch,
    Feature, IndexRange, Invariant, LiftMode, MalformedProgram, ModuleMetadata, Node, NodeIndex,
    NodeKind, OperationReference, OperationReferenceIndex, Origin, OriginIndex, OriginPosition,
    OriginSpan, Ownership, OwnershipMode, Parameter, ParameterIndex, ProgramRanges, RawProgram,
    RawProgramBuilder, RecordKind, ReleaseAfter, Root, RootIndex, SUPPORTED_SEMANTIC_MAJOR,
    SUPPORTED_SEMANTIC_MINOR, ScalarConstant, ShapePlan, SourceUnit, SourceUnitIndex, TypeIndex,
    TypeRecord, ValueAccess, VerifiedProgram, VerifyAllocationFailureInjection,
    VerifyAllocationSite, VerifyError,
};
pub use value::{ScalarType, TupleValues, Type, Value, format_type, format_value};

/// Canonical product version, sourced at compile time from Cargo metadata.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
