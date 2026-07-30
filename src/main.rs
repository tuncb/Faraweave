mod repl_terminal;

use faraweave::{
    ArgumentErrorContext, CompileFwirError, Error, ErrorKind, FwirDecodeLimits, FwirEncodeOptions,
    RootPresentation, VERSION, compile_source_to_fwir_with_name, decode_fwir, evaluate_expression,
    evaluate_runner_source, evaluate_verified_program_with_arguments, format_value, inspect_fwir,
    write_internal_registry_diagnostics,
};
use std::env;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::{self, BufRead, Read, Write};
use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::time::{SystemTime, UNIX_EPOCH};

const HELP: &str = "Usage: faraweave <command> [arguments]\n\
                     \x20      faraweave --help\n\
                     \x20      faraweave --version\n\
                     \n\
                     Commands:\n\
                     \x20 repl    Start an interactive Faraweave session\n\
                     \x20 run <source> [-- <arguments...>]\n\
                     \x20         Run a Faraweave source file\n\
                     \x20 compile-ir <source> -o <artifact.fwir>\n\
                     \x20         Compile source to canonical FWIR v1\n\
                     \x20 inspect-ir <artifact.fwir>\n\
                     \x20         Inspect a verified FWIR artifact\n\
                     \x20 run-ir <artifact.fwir> [-- <arguments...>]\n\
                     \x20         Run a verified FWIR artifact\n";
const REPL_HISTORY_MAX_ENTRIES: usize = 100;
const REPL_HISTORY_MAX_BYTES: usize = 65_536;

fn main() -> ExitCode {
    match run_cli(env::args_os().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn run_cli(arguments: Vec<OsString>) -> Result<(), ()> {
    run_cli_with_runner_support(
        arguments,
        RunnerFailureInjection::none(),
        publish_runner_stdout,
        |failure| {
            if let Some(diagnostic) = failure.diagnostic() {
                eprintln!("{diagnostic}");
            }
        },
    )
}

fn run_cli_with_runner_support(
    arguments: Vec<OsString>,
    runner_injection: RunnerFailureInjection,
    publish_runner_output: impl FnOnce(&[u8]) -> Result<(), ()>,
    report_runner_failure: impl FnOnce(RunnerCommandFailure),
) -> Result<(), ()> {
    let Some(command) = arguments.get(1).and_then(|value| value.to_str()) else {
        eprintln!("error: expected a subcommand or --help");
        return Err(());
    };
    match command {
        "--help" if arguments.len() == 2 => publish_stdout(HELP.as_bytes()),
        "--version" if arguments.len() == 2 => {
            publish_stdout(format!("faraweave {VERSION}\n").as_bytes())
        }
        option if option.starts_with('-') => {
            eprintln!("error: unknown option '{option}'");
            Err(())
        }
        "repl" => {
            if arguments.len() != 2 {
                eprintln!("error: 'repl' does not accept arguments");
                Err(())
            } else {
                repl()
            }
        }
        "run" => {
            run_command(&arguments, runner_injection, publish_runner_output).map_err(|failure| {
                report_runner_failure(failure);
            })
        }
        "compile-ir" => compile_ir_command(&arguments),
        "inspect-ir" => inspect_ir_command(&arguments),
        "run-ir" => {
            run_ir_command(&arguments, runner_injection, publish_runner_output).map_err(|failure| {
                report_runner_failure(failure);
            })
        }
        unknown => {
            eprintln!("error: unknown subcommand '{unknown}'");
            Err(())
        }
    }
}

fn compile_ir_command(arguments: &[OsString]) -> Result<(), ()> {
    if arguments.len() != 5 || arguments.get(3).and_then(|value| value.to_str()) != Some("-o") {
        eprintln!("error: expected 'compile-ir <source> -o <artifact.fwir>'");
        return Err(());
    }
    let source_path = Path::new(&arguments[2]);
    let output_path = Path::new(&arguments[4]);
    reject_alias(source_path, output_path)?;
    let source = read_source(source_path)?;
    let logical_name = source_path.to_string_lossy();
    let bytes =
        compile_source_to_fwir_with_name(&source, &logical_name, &FwirEncodeOptions::default())
            .map_err(|error| {
                report_compile_fwir_error(&logical_name, &error);
            })?;
    write_atomically(output_path, &bytes).map_err(|message| {
        eprintln!("{}:1:1: file error: {message}", output_path.display());
    })
}

fn inspect_ir_command(arguments: &[OsString]) -> Result<(), ()> {
    if arguments.len() != 3 {
        eprintln!("error: expected 'inspect-ir <artifact.fwir>'");
        return Err(());
    }
    let artifact_path = Path::new(&arguments[2]);
    let program = read_verified_artifact(artifact_path)?;
    let inspection = inspect_fwir(&program).map_err(|error| {
        eprintln!("{}:1:1: inspection error: {error}", artifact_path.display());
    })?;
    publish_stdout(inspection.as_bytes())
}

fn run_ir_command(
    arguments: &[OsString],
    injection: RunnerFailureInjection,
    publish_runner_output: impl FnOnce(&[u8]) -> Result<(), ()>,
) -> Result<(), RunnerCommandFailure> {
    if arguments.len() == 2 {
        eprintln!("error: expected one artifact path after 'run-ir'");
        return Err(RunnerCommandFailure::Reported);
    }
    if arguments.len() > 3 && arguments.get(3).and_then(|value| value.to_str()) != Some("--") {
        eprintln!("error: expected 'run-ir <artifact.fwir> [-- <arguments...>]'");
        return Err(RunnerCommandFailure::Reported);
    }
    let artifact_path = Path::new(&arguments[2]);
    let program =
        read_verified_artifact(artifact_path).map_err(|()| RunnerCommandFailure::Reported)?;
    let raw_arguments = arguments.get(4..).unwrap_or_default();
    let argument_strings = collect_command_line_arguments(raw_arguments, injection.arguments)
        .map_err(RunnerCommandFailure::CommandLineArguments)?;
    match evaluate_verified_program_with_arguments(
        &program,
        &argument_strings,
        faraweave::EvaluationConfiguration::default(),
    ) {
        Ok(result) => publish_formatted_runner_output(
            &result.formatted,
            &result.presentations,
            injection.formatted_output,
            publish_runner_output,
        ),
        Err(error) => {
            report_error(logical_source_name(&program), &error);
            Err(RunnerCommandFailure::Reported)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CommandLineArgumentFailure {
    AllocationUnavailable,
    InvalidUnicode,
}

impl CommandLineArgumentFailure {
    const fn diagnostic(self) -> &'static str {
        match self {
            Self::AllocationUnavailable => "error: unable to allocate command-line arguments",
            Self::InvalidUnicode => "error: unable to decode Unicode command line",
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct CommandLineArgumentFailureInjection {
    refuse_reservation: bool,
}

impl CommandLineArgumentFailureInjection {
    const fn none() -> Self {
        Self {
            refuse_reservation: false,
        }
    }

    #[cfg(test)]
    const fn refuse_reservation() -> Self {
        Self {
            refuse_reservation: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FormattedOutputFailure {
    SizeOverflow,
    AllocationUnavailable,
}

impl FormattedOutputFailure {
    const fn diagnostic(self) -> &'static str {
        "error: unable to allocate formatted output"
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct FormattedOutputFailureInjection {
    refuse_reservation: bool,
}

impl FormattedOutputFailureInjection {
    const fn none() -> Self {
        Self {
            refuse_reservation: false,
        }
    }

    #[cfg(test)]
    const fn refuse_reservation() -> Self {
        Self {
            refuse_reservation: true,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum RunnerCommandFailure {
    Reported,
    CommandLineArguments(CommandLineArgumentFailure),
    FormattedOutput(FormattedOutputFailure),
}

impl RunnerCommandFailure {
    const fn diagnostic(self) -> Option<&'static str> {
        match self {
            Self::Reported => None,
            Self::CommandLineArguments(failure) => Some(failure.diagnostic()),
            Self::FormattedOutput(failure) => Some(failure.diagnostic()),
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct RunnerFailureInjection {
    arguments: CommandLineArgumentFailureInjection,
    formatted_output: FormattedOutputFailureInjection,
}

impl RunnerFailureInjection {
    const fn none() -> Self {
        Self {
            arguments: CommandLineArgumentFailureInjection::none(),
            formatted_output: FormattedOutputFailureInjection::none(),
        }
    }

    #[cfg(test)]
    const fn refuse_argument_reservation() -> Self {
        Self {
            arguments: CommandLineArgumentFailureInjection::refuse_reservation(),
            formatted_output: FormattedOutputFailureInjection::none(),
        }
    }

    #[cfg(test)]
    const fn refuse_formatted_output_reservation() -> Self {
        Self {
            arguments: CommandLineArgumentFailureInjection::none(),
            formatted_output: FormattedOutputFailureInjection::refuse_reservation(),
        }
    }
}

fn collect_command_line_arguments(
    arguments: &[OsString],
    injection: CommandLineArgumentFailureInjection,
) -> Result<Vec<&str>, CommandLineArgumentFailure> {
    let mut decoded = Vec::new();
    if injection.refuse_reservation || decoded.try_reserve_exact(arguments.len()).is_err() {
        return Err(CommandLineArgumentFailure::AllocationUnavailable);
    }
    for argument in arguments {
        decoded.push(
            argument
                .to_str()
                .ok_or(CommandLineArgumentFailure::InvalidUnicode)?,
        );
    }
    Ok(decoded)
}

#[cfg(test)]
fn checked_formatted_output_length(
    formatted_lengths: impl IntoIterator<Item = usize>,
) -> Result<usize, FormattedOutputFailure> {
    formatted_lengths
        .into_iter()
        .try_fold(0usize, |total, length| {
            total
                .checked_add(length)
                .and_then(|value| value.checked_add(1))
                .ok_or(FormattedOutputFailure::SizeOverflow)
        })
}

fn build_formatted_runner_output(
    formatted: &[String],
    presentations: &[RootPresentation],
    injection: FormattedOutputFailureInjection,
) -> Result<String, FormattedOutputFailure> {
    if formatted.len() != presentations.len() {
        return Err(FormattedOutputFailure::SizeOverflow);
    }
    let output_length =
        formatted
            .iter()
            .zip(presentations)
            .try_fold(0usize, |total, (value, presentation)| {
                total
                    .checked_add(value.len())
                    .and_then(|length| {
                        if *presentation == RootPresentation::CanonicalValue {
                            length.checked_add(1)
                        } else {
                            Some(length)
                        }
                    })
                    .ok_or(FormattedOutputFailure::SizeOverflow)
            })?;
    let mut output = String::new();
    if injection.refuse_reservation || output.try_reserve_exact(output_length).is_err() {
        return Err(FormattedOutputFailure::AllocationUnavailable);
    }
    for (value, presentation) in formatted.iter().zip(presentations) {
        output.push_str(value);
        if *presentation == RootPresentation::CanonicalValue {
            output.push('\n');
        }
    }
    Ok(output)
}

fn publish_formatted_runner_output(
    formatted: &[String],
    presentations: &[RootPresentation],
    injection: FormattedOutputFailureInjection,
    publish_runner_output: impl FnOnce(&[u8]) -> Result<(), ()>,
) -> Result<(), RunnerCommandFailure> {
    let output = build_formatted_runner_output(formatted, presentations, injection)
        .map_err(RunnerCommandFailure::FormattedOutput)?;
    publish_runner_output(output.as_bytes()).map_err(|()| RunnerCommandFailure::Reported)
}

fn run_command(
    arguments: &[OsString],
    injection: RunnerFailureInjection,
    publish_runner_output: impl FnOnce(&[u8]) -> Result<(), ()>,
) -> Result<(), RunnerCommandFailure> {
    if arguments.len() == 2 {
        eprintln!("error: expected one source path after 'run'");
        return Err(RunnerCommandFailure::Reported);
    }
    if arguments.len() > 3 && arguments.get(3).and_then(|value| value.to_str()) != Some("--") {
        eprintln!("error: expected 'run <source> [-- <arguments...>]'");
        return Err(RunnerCommandFailure::Reported);
    }
    let path = Path::new(&arguments[2]);
    let source = read_source(path).map_err(|()| RunnerCommandFailure::Reported)?;
    let raw_arguments = arguments.get(4..).unwrap_or_default();
    let argument_strings = collect_command_line_arguments(raw_arguments, injection.arguments)
        .map_err(RunnerCommandFailure::CommandLineArguments)?;
    match evaluate_runner_source(&source, &argument_strings) {
        Ok(result) => publish_formatted_runner_output(
            &result.formatted,
            &result.presentations,
            injection.formatted_output,
            publish_runner_output,
        ),
        Err(error) => {
            report_error(&path.to_string_lossy(), &error);
            Err(RunnerCommandFailure::Reported)
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReplHistoryLimits {
    max_entries: usize,
    max_bytes: usize,
}

impl Default for ReplHistoryLimits {
    fn default() -> Self {
        Self {
            max_entries: REPL_HISTORY_MAX_ENTRIES,
            max_bytes: REPL_HISTORY_MAX_BYTES,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct ReplAllocationFailureInjection {
    fail_at_attempt: Option<usize>,
    attempts: usize,
}

impl ReplAllocationFailureInjection {
    const fn none() -> Self {
        Self {
            fail_at_attempt: None,
            attempts: 0,
        }
    }

    #[cfg(test)]
    const fn at(attempt: usize) -> Self {
        Self {
            fail_at_attempt: Some(attempt),
            attempts: 0,
        }
    }

    fn refuse(&mut self) -> bool {
        let attempt = self.attempts;
        self.attempts = self.attempts.saturating_add(1);
        self.fail_at_attempt == Some(attempt)
    }
}

#[derive(Debug, Eq, PartialEq)]
struct ReplHistoryEntry {
    number: u64,
    text: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplHistoryRecordFailureReason {
    AllocationUnavailable,
    EntryNumberOverflow,
    EntryTooLarge,
}

#[derive(Debug, Eq, PartialEq)]
struct ReplHistoryRecordFailure {
    reason: ReplHistoryRecordFailureReason,
    line: String,
}

struct ReplHistory {
    entries: Vec<ReplHistoryEntry>,
    retained_bytes: usize,
    next_number: u64,
    limits: ReplHistoryLimits,
    allocation_failure: ReplAllocationFailureInjection,
}

impl ReplHistory {
    fn new() -> Self {
        Self::with_limits_and_failure(
            ReplHistoryLimits::default(),
            ReplAllocationFailureInjection::none(),
        )
    }

    fn with_limits_and_failure(
        limits: ReplHistoryLimits,
        allocation_failure: ReplAllocationFailureInjection,
    ) -> Self {
        Self {
            entries: Vec::new(),
            retained_bytes: 0,
            next_number: 1,
            limits,
            allocation_failure,
        }
    }

    fn record(&mut self, line: String) -> Result<(), ReplHistoryRecordFailure> {
        if self.limits.max_entries == 0 || line.len() > self.limits.max_bytes {
            return Err(ReplHistoryRecordFailure {
                reason: ReplHistoryRecordFailureReason::EntryTooLarge,
                line,
            });
        }
        let Some(following_number) = self.next_number.checked_add(1) else {
            return Err(ReplHistoryRecordFailure {
                reason: ReplHistoryRecordFailureReason::EntryNumberOverflow,
                line,
            });
        };
        let Some(mut retained_after_insert) = self.retained_bytes.checked_add(line.len()) else {
            return Err(ReplHistoryRecordFailure {
                reason: ReplHistoryRecordFailureReason::EntryTooLarge,
                line,
            });
        };
        let Some(mut count_after_insert) = self.entries.len().checked_add(1) else {
            return Err(ReplHistoryRecordFailure {
                reason: ReplHistoryRecordFailureReason::EntryTooLarge,
                line,
            });
        };
        let mut evicted = 0usize;
        while count_after_insert > self.limits.max_entries
            || retained_after_insert > self.limits.max_bytes
        {
            let Some(entry) = self.entries.get(evicted) else {
                return Err(ReplHistoryRecordFailure {
                    reason: ReplHistoryRecordFailureReason::EntryTooLarge,
                    line,
                });
            };
            retained_after_insert -= entry.text.len();
            count_after_insert -= 1;
            evicted += 1;
        }
        if self.allocation_failure.refuse()
            || (evicted == 0 && self.entries.try_reserve(1).is_err())
        {
            return Err(ReplHistoryRecordFailure {
                reason: ReplHistoryRecordFailureReason::AllocationUnavailable,
                line,
            });
        }
        if evicted != 0 {
            self.entries.drain(..evicted);
        }
        self.entries.push(ReplHistoryEntry {
            number: self.next_number,
            text: line,
        });
        self.retained_bytes = retained_after_insert;
        self.next_number = following_number;
        Ok(())
    }

    fn latest_text(&self) -> Option<&str> {
        self.entries.last().map(|entry| entry.text.as_str())
    }

    fn write_to(&self, output: &mut impl Write) -> io::Result<()> {
        for entry in &self.entries {
            writeln!(output, "{}\t{}", entry.number, entry.text)?;
        }
        output.flush()
    }
}

#[derive(Debug, Eq, PartialEq)]
enum ReplLineRead {
    Eof,
    Line(String),
    Oversized,
    AllocationUnavailable,
}

fn read_repl_line(
    input: &mut impl BufRead,
    maximum_bytes: usize,
    allocation_failure: &mut ReplAllocationFailureInjection,
) -> io::Result<ReplLineRead> {
    let raw_limit = maximum_bytes.checked_add(1).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "REPL input limit cannot represent a CR terminator",
        )
    })?;
    let mut bytes = Vec::new();
    let mut rejection = None;
    let mut consumed_any = false;
    let mut consumed_lf = false;
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            break;
        }
        consumed_any = true;
        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_length = newline.unwrap_or(available.len());
        if rejection.is_none() {
            let projected = bytes.len().checked_add(content_length);
            if projected.is_none_or(|length| length > raw_limit) {
                rejection = Some(ReplLineRead::Oversized);
            } else if content_length != 0 {
                if allocation_failure.refuse() || bytes.try_reserve_exact(content_length).is_err() {
                    rejection = Some(ReplLineRead::AllocationUnavailable);
                } else {
                    bytes.extend_from_slice(&available[..content_length]);
                }
            }
        }
        let consumed = newline.map_or(available.len(), |position| position + 1);
        input.consume(consumed);
        if newline.is_some() {
            consumed_lf = true;
            break;
        }
    }
    if let Some(rejection) = rejection {
        return Ok(rejection);
    }
    if !consumed_any {
        return Ok(ReplLineRead::Eof);
    }
    if consumed_lf && bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    if bytes.len() > maximum_bytes {
        return Ok(ReplLineRead::Oversized);
    }
    String::from_utf8(bytes)
        .map(ReplLineRead::Line)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "REPL input is not UTF-8"))
}

fn publish_history_stdout(history: &ReplHistory) -> Result<(), ()> {
    let mut stdout = io::stdout().lock();
    history.write_to(&mut stdout).map_err(|_| {
        eprintln!("error: unable to write stdout");
    })
}

fn report_history_failure(reason: ReplHistoryRecordFailureReason) {
    match reason {
        ReplHistoryRecordFailureReason::AllocationUnavailable => {
            eprintln!("error: unable to retain REPL history entry");
        }
        ReplHistoryRecordFailureReason::EntryNumberOverflow => {
            eprintln!("error: REPL history entry number overflow");
        }
        ReplHistoryRecordFailureReason::EntryTooLarge => {
            eprintln!("error: REPL input exceeds {REPL_HISTORY_MAX_BYTES} retained bytes");
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ReplCommand {
    Exit,
}

fn repl_command(line: &str) -> Option<ReplCommand> {
    match line.trim_matches([' ', '\t']) {
        ".exit" => Some(ReplCommand::Exit),
        _ => None,
    }
}

fn repl() -> Result<(), ()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut history = ReplHistory::new();
    let mut input_allocation_failure = ReplAllocationFailureInjection::none();
    loop {
        publish_stdout(b"> ")?;
        let line = match read_repl_line(
            &mut input,
            REPL_HISTORY_MAX_BYTES,
            &mut input_allocation_failure,
        ) {
            Ok(ReplLineRead::Eof) => return Ok(()),
            Ok(ReplLineRead::Line(line)) => line,
            Ok(ReplLineRead::Oversized) => {
                eprintln!("error: REPL input exceeds {REPL_HISTORY_MAX_BYTES} retained bytes");
                continue;
            }
            Ok(ReplLineRead::AllocationUnavailable) => {
                eprintln!("error: unable to allocate REPL input");
                continue;
            }
            Err(_) => {
                eprintln!("error: unable to read stdin");
                return Err(());
            }
        };
        if line.is_empty() {
            continue;
        }
        let mut unrecorded = None;
        if let Err(failure) = history.record(line) {
            report_history_failure(failure.reason);
            unrecorded = Some(failure.line);
        }
        let Some(line) = unrecorded.as_deref().or_else(|| history.latest_text()) else {
            eprintln!("error: unable to access REPL input");
            return Err(());
        };
        match repl_terminal::classify_repl_input(line) {
            repl_terminal::ReplInputKind::Blank => continue,
            repl_terminal::ReplInputKind::Clear => {
                clear_repl_terminal();
                continue;
            }
            repl_terminal::ReplInputKind::Evaluate => {
                if line.trim_start_matches([' ', '\t']).starts_with('#') {
                    continue;
                }
            }
        }
        if repl_command(line) == Some(ReplCommand::Exit) {
            return Ok(());
        }
        if line == ".internal" {
            let mut stdout = io::stdout().lock();
            write_internal_registry_diagnostics(&mut stdout).map_err(|error| {
                eprintln!("error: {}", error.diagnostic());
            })?;
            continue;
        }
        if line.trim_matches([' ', '\t']) == ".history" {
            publish_history_stdout(&history)?;
            continue;
        }
        match evaluate_expression(line) {
            Ok(result) => {
                let formatted = format_value(&result.value).map_err(|error| {
                    report_error("<repl>", &error);
                })?;
                publish_stdout(format!("{formatted}\n").as_bytes())?;
            }
            Err(error) => report_error("<repl>", &error),
        }
    }
}

fn clear_repl_terminal() {
    let capability = repl_terminal::stdout_capability();
    let mut stdout = io::stdout().lock();
    let result = repl_terminal::clear_terminal(
        capability,
        &mut stdout,
        repl_terminal::clear_windows_console,
    );
    drop(stdout);
    if let Err(failure) = result {
        let mut stderr = io::stderr().lock();
        let _ = repl_terminal::report_clear_failure(failure, &mut stderr);
    }
}

fn read_source(path: &Path) -> Result<String, ()> {
    let mut file = fs::File::open(path).map_err(|_| {
        eprintln!("{}:1:1: file error: unable to read source", path.display());
    })?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).map_err(|_| {
        eprintln!("{}:1:1: file error: unable to read source", path.display());
    })?;
    String::from_utf8(bytes).map_err(|_| {
        eprintln!(
            "{}:1:1: file error: source is not valid UTF-8",
            path.display()
        );
    })
}

fn read_verified_artifact(path: &Path) -> Result<faraweave::VerifiedProgram, ()> {
    let limits = FwirDecodeLimits::default();
    let mut file = fs::File::open(path).map_err(|_| {
        eprintln!(
            "{}:1:1: file error: unable to read FWIR artifact",
            path.display()
        );
    })?;
    let mut bytes = Vec::new();
    let initial = file
        .metadata()
        .ok()
        .and_then(|metadata| usize::try_from(metadata.len()).ok())
        .unwrap_or(0)
        .min(limits.max_artifact_bytes.saturating_add(1));
    bytes.try_reserve_exact(initial).map_err(|_| {
        eprintln!(
            "{}:1:1: artifact error: unable to allocate artifact input",
            path.display()
        );
    })?;
    let maximum_input = limits.max_artifact_bytes.saturating_add(1);
    let mut buffer = [0_u8; 8192];
    loop {
        let remaining = maximum_input.saturating_sub(bytes.len());
        if remaining == 0 {
            break;
        }
        let read_count = remaining.min(buffer.len());
        let count = file.read(&mut buffer[..read_count]).map_err(|_| {
            eprintln!(
                "{}:1:1: file error: unable to read FWIR artifact",
                path.display()
            );
        })?;
        if count == 0 {
            break;
        }
        bytes.try_reserve_exact(count).map_err(|_| {
            eprintln!(
                "{}:1:1: artifact error: unable to allocate artifact input",
                path.display()
            );
        })?;
        bytes.extend_from_slice(&buffer[..count]);
    }
    decode_fwir(&bytes, &limits).map_err(|error| {
        eprintln!("{}:1:1: artifact error: {error}", path.display());
    })
}

fn logical_source_name(program: &faraweave::VerifiedProgram) -> &str {
    program
        .as_raw()
        .source_units
        .first()
        .map_or("<artifact>", |source| source.diagnostic_name.as_str())
}

fn report_compile_fwir_error(source_name: &str, error: &CompileFwirError) {
    match error {
        CompileFwirError::Compile(error) => report_error(source_name, error),
        CompileFwirError::Encode(error) => {
            eprintln!("{source_name}:1:1: artifact error: {error}");
        }
    }
}

fn reject_alias(source: &Path, output: &Path) -> Result<(), ()> {
    let source_absolute = absolute_normalized(source).map_err(|()| {
        eprintln!("error: unable to determine source/output path identity");
    })?;
    let output_absolute = absolute_normalized(output).map_err(|()| {
        eprintln!("error: unable to determine source/output path identity");
    })?;
    let aliases = source_absolute == output_absolute
        || (output.exists()
            && source
                .canonicalize()
                .ok()
                .zip(output.canonicalize().ok())
                .is_some_and(|(left, right)| left == right));
    if aliases {
        eprintln!("error: source/output alias: output path refers to input source");
        return Err(());
    }
    Ok(())
}

fn absolute_normalized(path: &Path) -> Result<PathBuf, ()> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|current| current.join(path))
            .map_err(|_| ())?
    };
    let mut normalized = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(component.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !normalized.pop() {
                    return Err(());
                }
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    Ok(normalized)
}

fn write_atomically(path: &Path, contents: &[u8]) -> Result<(), &'static str> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("faraweave-output");
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| "unable to create output")?
        .as_nanos();
    let temporary = parent.join(format!(".{name}.{nonce}.tmp"));
    let result = (|| {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary)
            .map_err(|_| "unable to write output")?;
        file.write_all(contents)
            .and_then(|()| file.sync_all())
            .map_err(|_| "unable to write output")?;
        drop(file);
        publish_file_atomically(&temporary, path).map_err(|_| "unable to replace output")
    })();
    let _ = fs::remove_file(temporary);
    result
}

fn publish_file_atomically(temporary: &Path, output: &Path) -> io::Result<()> {
    if !output.exists() {
        return fs::rename(temporary, output);
    }
    replace_existing_file(temporary, output)
}

#[cfg(not(windows))]
fn replace_existing_file(temporary: &Path, output: &Path) -> io::Result<()> {
    fs::rename(temporary, output)
}

#[cfg(windows)]
#[allow(unsafe_code)]
fn replace_existing_file(temporary: &Path, output: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;
    use std::ptr;

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn ReplaceFileW(
            replaced_file_name: *const u16,
            replacement_file_name: *const u16,
            backup_file_name: *const u16,
            replace_flags: u32,
            exclude: *mut core::ffi::c_void,
            reserved: *mut core::ffi::c_void,
        ) -> i32;
    }

    let mut output_wide: Vec<u16> = output.as_os_str().encode_wide().collect();
    let mut temporary_wide: Vec<u16> = temporary.as_os_str().encode_wide().collect();
    output_wide.push(0);
    temporary_wide.push(0);
    // SAFETY: both path buffers are NUL-terminated and live for the call; the
    // optional backup, exclusion, and reserved pointers are permitted to be
    // null by ReplaceFileW. Both files were resolved by the caller.
    let replaced = unsafe {
        ReplaceFileW(
            output_wide.as_ptr(),
            temporary_wide.as_ptr(),
            ptr::null(),
            1,
            ptr::null_mut(),
            ptr::null_mut(),
        )
    };
    if replaced == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

fn publish_stdout(bytes: &[u8]) -> Result<(), ()> {
    let mut stdout = io::stdout().lock();
    publish_to(&mut stdout, bytes).map_err(|_| {
        eprintln!("error: unable to write stdout");
    })
}

fn publish_runner_stdout(bytes: &[u8]) -> Result<(), ()> {
    let mut stdout = io::stdout().lock();
    publish_to(&mut stdout, bytes).map_err(|failure| {
        eprintln!(
            "faraweave_output_error reason={} pending_byte_count={} accepted_byte_count={} output_position={}",
            failure.reason.name(),
            failure.pending_byte_count,
            failure.accepted_byte_count,
            failure.output_position
        );
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum OutputFailureReason {
    WriteFailed,
    FlushFailed,
}

impl OutputFailureReason {
    const fn name(self) -> &'static str {
        match self {
            Self::WriteFailed => "write_failed",
            Self::FlushFailed => "flush_failed",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OutputPublicationFailure {
    reason: OutputFailureReason,
    pending_byte_count: usize,
    accepted_byte_count: usize,
    output_position: usize,
}

fn publish_to(output: &mut impl Write, bytes: &[u8]) -> Result<(), OutputPublicationFailure> {
    let mut accepted = 0usize;
    while accepted < bytes.len() {
        match output.write(&bytes[accepted..]) {
            Ok(0) | Err(_) => {
                return Err(OutputPublicationFailure {
                    reason: OutputFailureReason::WriteFailed,
                    pending_byte_count: bytes.len(),
                    accepted_byte_count: accepted,
                    output_position: accepted,
                });
            }
            Ok(count) => accepted = accepted.saturating_add(count),
        }
    }
    output.flush().map_err(|_| OutputPublicationFailure {
        reason: OutputFailureReason::FlushFailed,
        pending_byte_count: bytes.len(),
        accepted_byte_count: accepted,
        output_position: accepted,
    })
}

fn report_error(source_name: &str, error: &Error) {
    if error.kind == ErrorKind::ArgumentError
        && let Some(argument) = &error.argument
    {
        report_argument_error(argument);
        return;
    }
    eprintln!(
        "{}:{}:{}: {}: {}",
        source_name,
        error.location.line,
        error.location.column,
        error.kind.diagnostic_name(),
        error.message
    );
}

fn report_argument_error(argument: &ArgumentErrorContext) {
    let name = argument.parameter_name.as_deref().unwrap_or("-");
    let expected = argument.expected_type.map_or("-", |value| value.name());
    let declaration = argument.declaration_span.map_or_else(
        || "-".to_owned(),
        |span| {
            format!(
                "{}:{}:{}-{}:{}:{}",
                span.begin.offset,
                span.begin.line,
                span.begin.column,
                span.end.offset,
                span.end.line,
                span.end.column
            )
        },
    );
    let actual_type = argument.actual_type.map_or("-", |value| value.name());
    eprintln!(
        "faraweave_argument_error reason={} required_count={} supplied_count={} position={} parameter_name={} expected_type={} declaration_span={} actual_container={} actual_type={} invalid_value_invariant={}",
        argument.reason.name(),
        argument.required_count,
        argument.supplied_count,
        argument.position,
        name,
        expected,
        declaration,
        argument.actual_container.unwrap_or("-"),
        actual_type,
        argument.invalid_value_invariant.unwrap_or("-")
    );
}

#[cfg(test)]
mod output_tests {
    use super::{
        CommandLineArgumentFailure, CommandLineArgumentFailureInjection, FormattedOutputFailure,
        FormattedOutputFailureInjection, OutputFailureReason, OutputPublicationFailure,
        ReplCommand, RunnerCommandFailure, RunnerFailureInjection, build_formatted_runner_output,
        checked_formatted_output_length, collect_command_line_arguments,
        compile_source_to_fwir_with_name, publish_to, repl_command, run_cli_with_runner_support,
    };
    use faraweave::{FwirEncodeOptions, RootPresentation};
    use std::cell::Cell;
    use std::ffi::OsString;
    use std::fs;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_RUNNER_FIXTURE: AtomicU64 = AtomicU64::new(0);

    struct RunnerFixture {
        directory: PathBuf,
        source: PathBuf,
        artifact: PathBuf,
    }

    impl RunnerFixture {
        fn new() -> Self {
            let sequence = NEXT_RUNNER_FIXTURE.fetch_add(1, Ordering::Relaxed);
            let directory = std::env::temp_dir().join(format!(
                "faraweave-runner-seams-{}-{sequence}",
                std::process::id()
            ));
            fs::create_dir_all(&directory).expect("create runner fixture directory");
            let source = directory.join("program.faraweave");
            let artifact = directory.join("program.fwir");
            let source_text = "parameters[n Int]\ninc[n]\n";
            fs::write(&source, source_text).expect("write runner source fixture");
            let bytes = compile_source_to_fwir_with_name(
                source_text,
                &source.to_string_lossy(),
                &FwirEncodeOptions::default(),
            )
            .expect("compile runner artifact fixture");
            fs::write(&artifact, bytes).expect("write runner artifact fixture");
            Self {
                directory,
                source,
                artifact,
            }
        }

        fn arguments(&self, command: &str) -> Vec<OsString> {
            let input = match command {
                "run" => &self.source,
                "run-ir" => &self.artifact,
                _ => panic!("unsupported runner fixture command"),
            };
            vec![
                OsString::from("faraweave"),
                OsString::from(command),
                input.as_os_str().to_owned(),
                OsString::from("--"),
                OsString::from("3"),
            ]
        }
    }

    impl Drop for RunnerFixture {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.directory).expect("remove runner fixture directory");
        }
    }

    struct ShortWriter {
        accepted: usize,
        flush_ok: bool,
    }

    impl Write for ShortWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.accepted == 0 {
                return Ok(0);
            }
            let count = self.accepted.min(bytes.len());
            self.accepted -= count;
            Ok(count)
        }

        fn flush(&mut self) -> io::Result<()> {
            self.flush_ok
                .then_some(())
                .ok_or_else(|| io::Error::other("injected flush failure"))
        }
    }

    #[test]
    fn publication_retains_exact_write_and_flush_positions() {
        let write = publish_to(
            &mut ShortWriter {
                accepted: 6,
                flush_ok: true,
            },
            b"result\n",
        )
        .expect_err("short write");
        assert_eq!(
            write,
            OutputPublicationFailure {
                reason: OutputFailureReason::WriteFailed,
                pending_byte_count: 7,
                accepted_byte_count: 6,
                output_position: 6,
            }
        );

        let flush = publish_to(
            &mut ShortWriter {
                accepted: 7,
                flush_ok: false,
            },
            b"result\n",
        )
        .expect_err("flush failure");
        assert_eq!(
            flush,
            OutputPublicationFailure {
                reason: OutputFailureReason::FlushFailed,
                pending_byte_count: 7,
                accepted_byte_count: 7,
                output_position: 7,
            }
        );
    }

    #[test]
    fn command_line_argument_reservation_refusal_is_explicit_and_exact() {
        let arguments = [OsString::from("3")];
        let failure = collect_command_line_arguments(
            &arguments,
            CommandLineArgumentFailureInjection::refuse_reservation(),
        )
        .expect_err("injected reservation refusal");
        assert_eq!(failure, CommandLineArgumentFailure::AllocationUnavailable);
        assert_eq!(
            failure.diagnostic(),
            "error: unable to allocate command-line arguments"
        );
        assert_eq!(
            collect_command_line_arguments(&arguments, CommandLineArgumentFailureInjection::none()),
            Ok(vec!["3"])
        );
    }

    #[test]
    fn command_line_argument_unicode_failure_is_explicit_and_exact() {
        #[cfg(unix)]
        let invalid = {
            use std::os::unix::ffi::OsStringExt;
            OsString::from_vec(vec![0xff])
        };
        #[cfg(windows)]
        let invalid = {
            use std::os::windows::ffi::OsStringExt;
            OsString::from_wide(&[0xd800])
        };
        let failure =
            collect_command_line_arguments(&[invalid], CommandLineArgumentFailureInjection::none())
                .expect_err("invalid Unicode");
        assert_eq!(failure, CommandLineArgumentFailure::InvalidUnicode);
        assert_eq!(
            failure.diagnostic(),
            "error: unable to decode Unicode command line"
        );
    }

    #[test]
    fn formatted_output_size_is_checked_before_reservation() {
        assert_eq!(checked_formatted_output_length([3, 0, 4]), Ok(10));
        assert_eq!(
            checked_formatted_output_length([usize::MAX]),
            Err(FormattedOutputFailure::SizeOverflow)
        );
        assert_eq!(
            checked_formatted_output_length([usize::MAX - 1, 0]),
            Err(FormattedOutputFailure::SizeOverflow)
        );
    }

    #[test]
    fn formatted_output_reservation_refusal_is_explicit_and_transactional() {
        let formatted = vec!["first".to_owned(), "é".to_owned()];
        let presentations = vec![
            RootPresentation::CanonicalValue,
            RootPresentation::CanonicalValue,
        ];
        let failure = build_formatted_runner_output(
            &formatted,
            &presentations,
            FormattedOutputFailureInjection::refuse_reservation(),
        )
        .expect_err("injected formatted output reservation refusal");
        assert_eq!(failure, FormattedOutputFailure::AllocationUnavailable);
        assert_eq!(
            failure.diagnostic(),
            "error: unable to allocate formatted output"
        );
        assert_eq!(
            build_formatted_runner_output(
                &formatted,
                &presentations,
                FormattedOutputFailureInjection::none()
            ),
            Ok("first\né\n".to_owned())
        );
    }

    #[test]
    fn raw_and_canonical_roots_publish_in_source_order_without_implicit_raw_newline() {
        let formatted = vec!["1".to_owned(), "raw=é\0".to_owned(), "\"tail\"".to_owned()];
        let presentations = vec![
            RootPresentation::CanonicalValue,
            RootPresentation::RawString,
            RootPresentation::CanonicalValue,
        ];
        assert_eq!(
            build_formatted_runner_output(
                &formatted,
                &presentations,
                FormattedOutputFailureInjection::none(),
            ),
            Ok("1\nraw=é\0\"tail\"\n".to_owned())
        );
    }

    #[test]
    fn both_runner_entry_paths_use_fallible_argument_reservation() {
        let fixture = RunnerFixture::new();
        for command in ["run", "run-ir"] {
            let published_bytes = Cell::new(0usize);
            let observed_failure = Cell::new(None);
            run_cli_with_runner_support(
                fixture.arguments(command),
                RunnerFailureInjection::refuse_argument_reservation(),
                |bytes| {
                    published_bytes.set(published_bytes.get() + bytes.len());
                    Ok(())
                },
                |failure| observed_failure.set(Some(failure)),
            )
            .expect_err("injected runner argument reservation refusal");
            let failure = observed_failure
                .get()
                .expect("runner argument failure was reported");
            assert_eq!(
                failure,
                RunnerCommandFailure::CommandLineArguments(
                    CommandLineArgumentFailure::AllocationUnavailable
                ),
                "{command}"
            );
            assert_eq!(
                failure.diagnostic(),
                Some("error: unable to allocate command-line arguments"),
                "{command}"
            );
            assert_eq!(published_bytes.get(), 0, "{command}");
        }
    }

    #[test]
    fn both_runner_entry_paths_pre_reserve_complete_formatted_output() {
        let fixture = RunnerFixture::new();
        for command in ["run", "run-ir"] {
            let published_bytes = Cell::new(0usize);
            let observed_failure = Cell::new(None);
            run_cli_with_runner_support(
                fixture.arguments(command),
                RunnerFailureInjection::refuse_formatted_output_reservation(),
                |bytes| {
                    published_bytes.set(published_bytes.get() + bytes.len());
                    Ok(())
                },
                |failure| observed_failure.set(Some(failure)),
            )
            .expect_err("injected runner formatted output reservation refusal");
            let failure = observed_failure
                .get()
                .expect("runner formatted output failure was reported");
            assert_eq!(
                failure,
                RunnerCommandFailure::FormattedOutput(
                    FormattedOutputFailure::AllocationUnavailable
                ),
                "{command}"
            );
            assert_eq!(
                failure.diagnostic(),
                Some("error: unable to allocate formatted output"),
                "{command}"
            );
            assert_eq!(published_bytes.get(), 0, "{command}");
        }
    }

    #[test]
    fn repl_command_dispatch_is_exact_and_case_sensitive() {
        for accepted in [".exit", " .exit", "\t.exit", " \t.exit\t "] {
            assert_eq!(repl_command(accepted), Some(ReplCommand::Exit));
        }
        for rejected in [
            "",
            " ",
            ".EXIT",
            ".Exit",
            ".exit-now",
            ".exit argument",
            ".exit # trailing source comment",
            ".exit#trailing-source-comment",
            "\u{a0}.exit",
            ".exit\u{a0}",
        ] {
            assert_eq!(repl_command(rejected), None);
        }
    }
}

#[cfg(test)]
mod repl_tests {
    use super::{
        ReplAllocationFailureInjection, ReplHistory, ReplHistoryLimits,
        ReplHistoryRecordFailureReason, ReplLineRead, read_repl_line,
    };
    use std::io::{self, Cursor, Write};

    fn record(history: &mut ReplHistory, text: &str) {
        if let Err(error) = history.record(text.to_owned()) {
            panic!("history record failed: {:?}", error.reason);
        }
    }

    #[test]
    fn history_evicts_oldest_at_exact_count_and_utf8_byte_boundaries() {
        let mut history = ReplHistory::with_limits_and_failure(
            ReplHistoryLimits {
                max_entries: 3,
                max_bytes: 8,
            },
            ReplAllocationFailureInjection::none(),
        );
        record(&mut history, "a");
        record(&mut history, "éé");
        record(&mut history, "ccc");
        assert_eq!(history.retained_bytes, 8);
        assert_eq!(
            history
                .entries
                .iter()
                .map(|entry| (entry.number, entry.text.as_str()))
                .collect::<Vec<_>>(),
            [(1, "a"), (2, "éé"), (3, "ccc")]
        );

        record(&mut history, "d");
        assert_eq!(history.retained_bytes, 8);
        assert_eq!(
            history
                .entries
                .iter()
                .map(|entry| (entry.number, entry.text.as_str()))
                .collect::<Vec<_>>(),
            [(2, "éé"), (3, "ccc"), (4, "d")]
        );

        record(&mut history, "zzzz");
        assert_eq!(history.retained_bytes, 8);
        assert_eq!(
            history
                .entries
                .iter()
                .map(|entry| (entry.number, entry.text.as_str()))
                .collect::<Vec<_>>(),
            [(3, "ccc"), (4, "d"), (5, "zzzz")]
        );

        record(&mut history, "12345678");
        assert_eq!(history.retained_bytes, 8);
        assert_eq!(
            history
                .entries
                .iter()
                .map(|entry| (entry.number, entry.text.as_str()))
                .collect::<Vec<_>>(),
            [(6, "12345678")]
        );
    }

    #[test]
    fn default_history_limits_retain_only_the_latest_hundred_entries() {
        let mut history = ReplHistory::new();
        assert_eq!(
            history.limits,
            ReplHistoryLimits {
                max_entries: 100,
                max_bytes: 65_536,
            }
        );
        for _ in 0..101 {
            record(&mut history, "x");
        }
        assert_eq!(history.entries.len(), 100);
        assert_eq!(history.retained_bytes, 100);
        assert_eq!(history.entries.first().map(|entry| entry.number), Some(2));
        assert_eq!(history.entries.last().map(|entry| entry.number), Some(101));
    }

    #[test]
    fn history_refusal_and_oversize_leave_existing_entries_and_input_owned() {
        let mut refused = ReplHistory::with_limits_and_failure(
            ReplHistoryLimits {
                max_entries: 1,
                max_bytes: 8,
            },
            ReplAllocationFailureInjection::at(1),
        );
        record(&mut refused, "one");
        let failure = refused
            .record("two".to_owned())
            .expect_err("injected history reservation refusal");
        assert_eq!(
            failure.reason,
            ReplHistoryRecordFailureReason::AllocationUnavailable
        );
        assert_eq!(failure.line, "two");
        assert_eq!(refused.next_number, 2);
        assert_eq!(refused.retained_bytes, 3);
        assert_eq!(refused.entries[0].text, "one");

        let oversized = refused
            .record("123456789".to_owned())
            .expect_err("history entry over byte limit");
        assert_eq!(
            oversized.reason,
            ReplHistoryRecordFailureReason::EntryTooLarge
        );
        assert_eq!(oversized.line, "123456789");
        assert_eq!(refused.entries.len(), 1);
    }

    #[test]
    fn bounded_reader_removes_lf_and_crlf_and_recovers_after_rejections() {
        let mut input = Cursor::new("éé\r\nplain\nok\n");
        let mut injection = ReplAllocationFailureInjection::none();
        assert_eq!(
            read_repl_line(&mut input, 4, &mut injection).expect("UTF-8 CRLF line"),
            ReplLineRead::Line("éé".to_owned())
        );
        assert_eq!(
            read_repl_line(&mut input, 4, &mut injection).expect("oversized line"),
            ReplLineRead::Oversized
        );
        assert_eq!(
            read_repl_line(&mut input, 4, &mut injection).expect("line after oversize"),
            ReplLineRead::Line("ok".to_owned())
        );
        assert_eq!(
            read_repl_line(&mut input, 4, &mut injection).expect("end of input"),
            ReplLineRead::Eof
        );

        let mut input = Cursor::new("first\nok\n");
        let mut injection = ReplAllocationFailureInjection::at(0);
        assert_eq!(
            read_repl_line(&mut input, 8, &mut injection).expect("refused first line"),
            ReplLineRead::AllocationUnavailable
        );
        assert_eq!(
            read_repl_line(&mut input, 8, &mut injection).expect("line after refusal"),
            ReplLineRead::Line("ok".to_owned())
        );
    }

    #[test]
    fn bounded_reader_preserves_and_counts_lone_cr_at_eof() {
        let mut input = Cursor::new(b"1\r");
        let mut injection = ReplAllocationFailureInjection::none();
        assert_eq!(
            read_repl_line(&mut input, 2, &mut injection).expect("EOF line with lone CR"),
            ReplLineRead::Line("1\r".to_owned())
        );
        assert_eq!(
            read_repl_line(&mut input, 2, &mut injection).expect("end after lone CR"),
            ReplLineRead::Eof
        );

        let mut input = Cursor::new(b"1\r");
        assert_eq!(
            read_repl_line(&mut input, 1, &mut injection).expect("bounded EOF line with lone CR"),
            ReplLineRead::Oversized
        );
    }

    struct FailingWriter {
        remaining: usize,
        fail_flush: bool,
    }

    impl Write for FailingWriter {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected history output failure",
                ));
            }
            let accepted = self.remaining.min(bytes.len());
            self.remaining -= accepted;
            Ok(accepted)
        }

        fn flush(&mut self) -> io::Result<()> {
            if self.fail_flush {
                Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "injected history flush failure",
                ))
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn history_output_is_exact_and_write_or_flush_failure_is_recoverable() {
        let mut history = ReplHistory::with_limits_and_failure(
            ReplHistoryLimits {
                max_entries: 3,
                max_bytes: 32,
            },
            ReplAllocationFailureInjection::none(),
        );
        record(&mut history, " α ");
        record(&mut history, ".history");
        let mut output = Vec::new();
        history.write_to(&mut output).expect("history output");
        assert_eq!(output, "1\t α \n2\t.history\n".as_bytes());

        let write = history
            .write_to(&mut FailingWriter {
                remaining: 2,
                fail_flush: false,
            })
            .expect_err("injected write failure");
        assert_eq!(write.kind(), io::ErrorKind::BrokenPipe);

        let flush = history
            .write_to(&mut FailingWriter {
                remaining: usize::MAX,
                fail_flush: true,
            })
            .expect_err("injected flush failure");
        assert_eq!(flush.kind(), io::ErrorKind::BrokenPipe);
    }
}
