use crate::{
    Error, ErrorKind, EvaluationConfiguration, FwirEncodeError, FwirEncodeOptions, ProgramResult,
    RunnerEvaluationResult, SourceLocation, VerifiedProgram, encode_fwir,
    evaluate_verified_program, format_value,
};
use std::fmt::Write as _;

#[derive(Debug)]
#[allow(clippy::large_enum_variant)]
pub enum CompileFwirError {
    Compile(Error),
    Encode(FwirEncodeError),
}

impl std::fmt::Display for CompileFwirError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Compile(error) => error.fmt(formatter),
            Self::Encode(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CompileFwirError {}

#[derive(Debug)]
pub enum FwirInspectError {
    Encode(FwirEncodeError),
    SizeOverflow,
    AllocationUnavailable,
}

impl std::fmt::Display for FwirInspectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "FWIR inspection failed: {self:?}")
    }
}

impl std::error::Error for FwirInspectError {}

/// Compiles source through the sole typed lowerer into a verified program.
///
/// `diagnostic_name` is retained as provenance and must be nonempty. No
/// backend receives a partial program when parsing, analysis, building, or
/// verification fails.
pub fn compile_source_to_verified_program(
    source: &str,
    diagnostic_name: &str,
) -> Result<VerifiedProgram, Error> {
    if diagnostic_name.is_empty() {
        return Err(Error::new(
            ErrorKind::TypeError,
            SourceLocation::start(),
            "logical source name must not be empty",
        ));
    }
    crate::lowering::compile_source_with_name(source, diagnostic_name)
        .map_err(crate::lowering::CompileError::into_evaluation_error)
}

/// Compiles source and returns canonical FWIR v1.0 bytes.
///
/// The default logical source name is `<source>`; use
/// [`compile_source_to_fwir_with_name`] when diagnostics need a stable name.
pub fn compile_source_to_fwir(
    source: &str,
    options: &FwirEncodeOptions,
) -> Result<Vec<u8>, CompileFwirError> {
    compile_source_to_fwir_with_name(source, "<source>", options)
}

/// Compiles source while retaining a caller-selected logical diagnostic name.
///
/// The name and provenance positions participate in canonical program
/// identity and can be visible to artifact recipients.
pub fn compile_source_to_fwir_with_name(
    source: &str,
    diagnostic_name: &str,
    options: &FwirEncodeOptions,
) -> Result<Vec<u8>, CompileFwirError> {
    let program = compile_source_to_verified_program(source, diagnostic_name)
        .map_err(CompileFwirError::Compile)?;
    encode_fwir(&program, options).map_err(CompileFwirError::Encode)
}

/// Decodes textual arguments only after accepting an already verified program.
///
/// Argument count and complete argument decoding precede execution. Programs
/// obtained from bytes must first come from [`crate::decode_fwir`].
pub fn evaluate_verified_program_with_arguments(
    program: &VerifiedProgram,
    arguments: &[&str],
    configuration: EvaluationConfiguration,
) -> Result<RunnerEvaluationResult, Error> {
    let decoded = crate::interpreter::decode_verified_arguments(program, arguments)?;
    let ProgramResult { values, usage } =
        evaluate_verified_program(program, &decoded, configuration)?;
    let mut formatted = Vec::new();
    formatted.try_reserve_exact(values.len()).map_err(|_| {
        Error::new(
            ErrorKind::FormattingError,
            SourceLocation::start(),
            "unable to allocate formatted output",
        )
    })?;
    for value in &values {
        formatted.push(format_value(value)?);
    }
    Ok(RunnerEvaluationResult {
        values,
        formatted,
        usage,
    })
}

/// Produces deterministic, non-executable text and exact canonical bits.
///
/// Inspection can reveal all artifact content and is not a confidentiality
/// boundary.
pub fn inspect_fwir(program: &VerifiedProgram) -> Result<String, FwirInspectError> {
    let canonical =
        encode_fwir(program, &FwirEncodeOptions::default()).map_err(FwirInspectError::Encode)?;
    let raw = program.as_raw();
    let capacity = canonical
        .len()
        .checked_mul(2)
        .and_then(|value| value.checked_add(4096))
        .ok_or(FwirInspectError::SizeOverflow)?;
    let mut output = FallibleText::with_capacity(capacity)?;
    append_format(&mut output, format_args!("FWIR inspection v1\n"))?;
    append_format(
        &mut output,
        format_args!(
            "semantic {}.{}",
            raw.module.semantic_major, raw.module.semantic_minor
        ),
    )?;
    append_format(&mut output, format_args!("\n"))?;
    append_format(
        &mut output,
        format_args!(
            "counts features={} sources={} parameters={} types={} constants={} operation_references={} nodes={} roots={}",
            raw.features.len(),
            raw.source_units.len(),
            raw.parameters.len(),
            raw.types.len(),
            raw.constants.len(),
            raw.operation_references.len(),
            raw.nodes.len(),
            raw.roots.len()
        ),
    )?;
    append_format(&mut output, format_args!("\n"))?;
    for (index, source) in raw.source_units.iter().enumerate() {
        append_format(
            &mut output,
            format_args!(
                "source[{index}] name={:?} byte_length={}",
                source.diagnostic_name, source.byte_length
            ),
        )?;
        append_format(&mut output, format_args!("\n"))?;
    }
    for (index, parameter) in raw.parameters.iter().enumerate() {
        append_format(
            &mut output,
            format_args!("parameter[{index}] {parameter:?}\n"),
        )?;
    }
    for (index, constant) in raw.constants.iter().enumerate() {
        append_format(
            &mut output,
            format_args!("constant[{index}] {constant:?}\n"),
        )?;
    }
    for (index, reference) in raw.operation_references.iter().enumerate() {
        append_format(
            &mut output,
            format_args!("operation_reference[{index}] {reference:?}\n"),
        )?;
    }
    output.push_str("canonical-hex ")?;
    for byte in canonical {
        append_format(&mut output, format_args!("{byte:02x}"))?;
    }
    output.push_str("\n")?;
    Ok(output.finish())
}

struct FallibleText {
    text: String,
    allocation_failed: bool,
}

impl FallibleText {
    fn with_capacity(capacity: usize) -> Result<Self, FwirInspectError> {
        let mut text = String::new();
        text.try_reserve_exact(capacity)
            .map_err(|_| FwirInspectError::AllocationUnavailable)?;
        Ok(Self {
            text,
            allocation_failed: false,
        })
    }

    fn push_str(&mut self, value: &str) -> Result<(), FwirInspectError> {
        self.text
            .try_reserve_exact(value.len())
            .map_err(|_| FwirInspectError::AllocationUnavailable)?;
        self.text.push_str(value);
        Ok(())
    }

    fn finish(self) -> String {
        self.text
    }
}

impl std::fmt::Write for FallibleText {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        if self.text.try_reserve_exact(value.len()).is_err() {
            self.allocation_failed = true;
            return Err(std::fmt::Error);
        }
        self.text.push_str(value);
        Ok(())
    }
}

fn append_format(
    output: &mut FallibleText,
    arguments: std::fmt::Arguments<'_>,
) -> Result<(), FwirInspectError> {
    output.write_fmt(arguments).map_err(|_| {
        if output.allocation_failed {
            FwirInspectError::AllocationUnavailable
        } else {
            FwirInspectError::SizeOverflow
        }
    })
}
