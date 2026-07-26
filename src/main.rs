use faraweave::{
    ArgumentErrorContext, Error, ErrorKind, NativeBuildRequest, VERSION, build_native,
    emit_c_source, evaluate_expression, evaluate_runner_source, format_value,
    publish_file_atomically,
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
                     \x20 emit-c  Emit C source for a Faraweave source file\n\
                     \x20 build   Build a Faraweave source file\n";

fn main() -> ExitCode {
    match run_cli(env::args_os().collect()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(()) => ExitCode::FAILURE,
    }
}

fn run_cli(arguments: Vec<OsString>) -> Result<(), ()> {
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
        "run" => run_command(&arguments),
        "emit-c" => emit_c_command(&arguments),
        "build" => build_command(&arguments),
        unknown => {
            eprintln!("error: unknown subcommand '{unknown}'");
            Err(())
        }
    }
}

fn run_command(arguments: &[OsString]) -> Result<(), ()> {
    if arguments.len() == 2 {
        eprintln!("error: expected one source path after 'run'");
        return Err(());
    }
    if arguments.len() > 3 && arguments.get(3).and_then(|value| value.to_str()) != Some("--") {
        eprintln!("error: expected 'run <source> [-- <arguments...>]'");
        return Err(());
    }
    let path = Path::new(&arguments[2]);
    let source = read_source(path)?;
    let argument_strings: Vec<&str> = arguments
        .get(4..)
        .unwrap_or_default()
        .iter()
        .map(|argument| {
            argument.to_str().ok_or_else(|| {
                eprintln!("error: unable to decode Unicode command line");
            })
        })
        .collect::<Result<_, _>>()?;
    match evaluate_runner_source(&source, &argument_strings) {
        Ok(result) => {
            let mut output = String::new();
            for formatted in result.formatted {
                output.push_str(&formatted);
                output.push('\n');
            }
            publish_runner_stdout(output.as_bytes())
        }
        Err(error) => {
            report_error(&path.to_string_lossy(), &error);
            Err(())
        }
    }
}

fn emit_c_command(arguments: &[OsString]) -> Result<(), ()> {
    if arguments.len() == 2 {
        eprintln!("error: expected a source path after 'emit-c'");
        return Err(());
    }
    if arguments.len() != 5 || arguments.get(3).and_then(|value| value.to_str()) != Some("-o") {
        eprintln!("error: expected 'emit-c <source> -o <output>'");
        return Err(());
    }
    let source_path = Path::new(&arguments[2]);
    let output_path = Path::new(&arguments[4]);
    reject_alias(source_path, output_path)?;
    let source = read_source(source_path)?;
    let emitted = emit_c_source(&source).map_err(|error| {
        report_error(&source_path.to_string_lossy(), &error);
    })?;
    write_atomically(output_path, emitted.source.as_bytes()).map_err(|message| {
        eprintln!("{}:1:1: file error: {message}", output_path.display());
    })
}

fn build_command(arguments: &[OsString]) -> Result<(), ()> {
    if arguments.len() == 2 {
        eprintln!("error: expected a source path after 'build'");
        return Err(());
    }
    if !matches!(arguments.len(), 5 | 7)
        || arguments.get(3).and_then(|value| value.to_str()) != Some("-o")
        || (arguments.len() == 7
            && arguments.get(5).and_then(|value| value.to_str()) != Some("--cc"))
    {
        eprintln!("error: expected 'build <source> -o <output> [--cc <compiler>]'");
        return Err(());
    }
    let source_path = Path::new(&arguments[2]);
    let output_path = Path::new(&arguments[4]);
    reject_alias(source_path, output_path)?;
    let source = read_source(source_path)?;
    let emitted = emit_c_source(&source).map_err(|error| {
        report_error(&source_path.to_string_lossy(), &error);
    })?;
    let explicit = arguments.get(6).and_then(|value| value.to_str());
    if arguments.len() == 7 && explicit.is_none_or(str::is_empty) {
        eprintln!("error: --cc requires a nonempty compiler");
        return Err(());
    }
    let environment = env::var("CC").ok();
    build_native(&NativeBuildRequest {
        c_source: &emitted.source,
        output_path,
        explicit_compiler: explicit,
        environment_compiler: environment.as_deref(),
    })
    .map(|_| ())
    .map_err(|error| {
        eprintln!("error: native build: {}", error.message);
    })
}

fn repl() -> Result<(), ()> {
    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();
    loop {
        publish_stdout(b"> ")?;
        line.clear();
        match input.read_line(&mut line) {
            Ok(0) => return Ok(()),
            Ok(_) => {}
            Err(_) => {
                eprintln!("error: unable to read stdin");
                return Err(());
            }
        }
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        if line.bytes().all(|byte| matches!(byte, b' ' | b'\t')) {
            continue;
        }
        match evaluate_expression(&line) {
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
    use super::{OutputFailureReason, OutputPublicationFailure, publish_to};
    use std::io::{self, Write};

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
}
