use std::ffi::OsString;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_faraweave"))
}

fn unique(name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("faraweave-{name}-{nonce}"))
}

#[test]
fn cli_help_version_and_unknown_contracts() {
    let absent = Command::new(binary()).output().expect("no arguments");
    assert!(!absent.status.success());
    assert!(absent.stdout.is_empty());
    assert_eq!(absent.stderr, b"error: expected a subcommand or --help\n");

    let version = Command::new(binary())
        .arg("--version")
        .output()
        .expect("version");
    assert!(version.status.success());
    assert_eq!(version.stdout, b"faraweave 0.1.0\n");
    assert!(version.stderr.is_empty());

    let help = Command::new(binary()).arg("--help").output().expect("help");
    assert!(help.status.success());
    let help = String::from_utf8(help.stdout).expect("utf8");
    assert!(help.contains("Usage: faraweave"));
    assert!(help.contains("interactive Faraweave session"));
    for command in ["compile-ir", "inspect-ir", "run-ir"] {
        assert!(help.contains(command), "{command}");
    }
    for removed in ["emit-c", "emit-c-ir", "build", "build-ir"] {
        assert!(!help.contains(removed), "{removed}");
        let output = Command::new(binary())
            .arg(removed)
            .output()
            .expect("removed command");
        assert!(!output.status.success());
        assert!(output.stdout.is_empty());
        assert_eq!(
            output.stderr,
            format!("error: unknown subcommand '{removed}'\n").as_bytes()
        );
    }

    let unknown = Command::new(binary())
        .arg("unknown")
        .output()
        .expect("unknown");
    assert!(!unknown.status.success());
    assert!(unknown.stdout.is_empty());
    assert_eq!(unknown.stderr, b"error: unknown subcommand 'unknown'\n");

    let option = Command::new(binary())
        .arg("--unknown")
        .output()
        .expect("unknown option");
    assert!(!option.status.success());
    assert_eq!(option.stderr, b"error: unknown option '--unknown'\n");
}

#[test]
fn cli_explicit_fwir_lifecycle_is_deterministic_and_phase_separated() {
    let directory = unique("fwir-lifecycle");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("logical source.anything");
    let artifact = directory.join("program.data");
    fs::write(
        &source,
        "parameters[n Int]\n-0.0\ninc[n]\nfanout[iota[n] {inc[_]} {add[_ 10]}]\n",
    )
    .expect("source");

    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("compile IR");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);
    assert!(compiled.stdout.is_empty());
    assert!(compiled.stderr.is_empty());

    let first = Command::new(binary())
        .arg("inspect-ir")
        .arg(&artifact)
        .output()
        .expect("inspect");
    let second = Command::new(binary())
        .arg("inspect-ir")
        .arg(&artifact)
        .output()
        .expect("inspect again");
    assert!(first.status.success(), "{:?}", first.stderr);
    assert_eq!(first.stdout, second.stdout);
    assert!(
        first
            .stdout
            .windows(31)
            .any(|window| { window == b"DoubleBits(9223372036854775808)" })
    );

    let run = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .args(["--", "3"])
        .output()
        .expect("run IR");
    assert!(run.status.success(), "{:?}", run.stderr);
    assert_eq!(run.stdout, b"-0.0\n4\n[(2 3 4) (11 12 13)]\n");

    let overflow = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .args(["--", "9223372036854775807"])
        .output()
        .expect("runtime error");
    assert!(!overflow.status.success());
    assert!(overflow.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&overflow.stderr)
            .starts_with(&source.to_string_lossy().to_string()),
        "{:?}",
        overflow.stderr
    );

    let malformed = directory.join("malformed.not-fwir");
    fs::write(&malformed, b"not FWIR").expect("malformed");
    let malformed_with_argument = Command::new(binary())
        .arg("run-ir")
        .arg(&malformed)
        .args(["--", "not-an-int"])
        .output()
        .expect("malformed before argument");
    assert!(!malformed_with_argument.status.success());
    assert!(malformed_with_argument.stdout.is_empty());
    assert!(
        String::from_utf8_lossy(&malformed_with_argument.stderr).contains("artifact error"),
        "{:?}",
        malformed_with_argument.stderr
    );
    assert!(
        !malformed_with_argument
            .stderr
            .starts_with(b"faraweave_argument_error")
    );

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_compile_ir_rejects_aliases_and_preserves_destinations_on_failure() {
    let directory = unique("fwir-transactions");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("source.faraweave");
    let artifact = directory.join("program.fwir");
    fs::write(&source, "inc[41]\n").expect("source");
    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("compile");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);

    let source_before = fs::read(&source).expect("source bytes");
    let lexical_alias = directory.join("child").join("..").join("source.faraweave");
    let lexical = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&lexical_alias)
        .output()
        .expect("lexical alias");
    assert!(!lexical.status.success());
    assert_eq!(fs::read(&source).expect("preserved source"), source_before);

    #[cfg(unix)]
    {
        use std::os::unix::fs::symlink;
        let canonical_alias = directory.join("canonical-source-alias");
        symlink(&source, &canonical_alias).expect("source symlink");
        let canonical = Command::new(binary())
            .arg("compile-ir")
            .arg(&source)
            .args(["-o"])
            .arg(&canonical_alias)
            .output()
            .expect("canonical alias");
        assert!(!canonical.status.success());
        assert_eq!(
            fs::read(&source).expect("canonical source preserved"),
            source_before
        );
    }

    let invalid_source = directory.join("invalid.faraweave");
    let compile_destination = directory.join("existing.fwir");
    fs::write(&invalid_source, "inc[").expect("invalid source");
    fs::write(&compile_destination, b"keep-compile").expect("sentinel");
    let invalid_compile = Command::new(binary())
        .arg("compile-ir")
        .arg(&invalid_source)
        .args(["-o"])
        .arg(&compile_destination)
        .output()
        .expect("invalid compilation");
    assert!(!invalid_compile.status.success());
    assert_eq!(
        fs::read(&compile_destination).expect("compile destination"),
        b"keep-compile"
    );

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_repl_transcript_recovers_resets_and_rejects_program_headers() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut input = child.stdin.take().expect("REPL stdin");
    input
        .write_all(b"\ninc 5\nadd[1]\ninc 6\nparameters[x Int]\n")
        .expect("REPL transcript");
    drop(input);
    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > 6\n> > 7\n> > ");
    let stderr = String::from_utf8(result.stderr).expect("REPL stderr UTF-8");
    assert!(stderr.contains("<repl>:1:1: ArityError:"));
    assert!(stderr.contains("invalid parameter header"));
}

fn repl_output(transcript: &[u8]) -> std::process::Output {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let Some(mut input) = child.stdin.take() else {
        panic!("REPL stdin was not piped");
    };
    input.write_all(transcript).expect("REPL transcript");
    drop(input);
    child.wait_with_output().expect("REPL output")
}

#[test]
fn cli_repl_history_preserves_inclusion_text_numbering_and_crlf() {
    let result = repl_output(b"\n \t\r\n# comment\r\nadd[1]\r\n.history\r\n");
    assert!(result.status.success());
    assert_eq!(
        result.stdout,
        b"> > > > > 1\t \t\n2\t# comment\n3\tadd[1]\n4\t.history\n> "
    );
    let stderr = String::from_utf8(result.stderr).expect("REPL stderr UTF-8");
    assert!(stderr.contains("<repl>:1:1: ArityError:"));

    let utf8 = repl_output("🦀\n.history\n".as_bytes());
    assert!(utf8.status.success());
    assert_eq!(utf8.stdout, "> > 1\t🦀\n2\t.history\n> ".as_bytes());
    assert!(
        String::from_utf8(utf8.stderr)
            .expect("UTF-8 diagnostic")
            .contains("InvalidByte")
    );
}

#[test]
fn cli_repl_eof_lone_cr_is_preserved_and_diagnosed() {
    let result = repl_output(b"1\r");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > ");
    assert_eq!(
        result.stderr,
        b"<repl>:1:2: InvalidByte: invalid source byte\n"
    );
}

#[test]
fn cli_repl_history_is_process_local_and_clear_remains_unsupported() {
    let first = repl_output(b"1\n.history\n");
    assert!(first.status.success());
    assert_eq!(first.stdout, b"> 1\n> 1\t1\n2\t.history\n> ");
    assert!(first.stderr.is_empty());

    let fresh = repl_output(b".history\n");
    assert!(fresh.status.success());
    assert_eq!(fresh.stdout, b"> 1\t.history\n> ");
    assert!(fresh.stderr.is_empty());

    let clear = repl_output(b".clear\n.history\n");
    assert!(clear.status.success());
    assert_eq!(clear.stdout, b"> > 1\t.clear\n2\t.history\n> ");
    assert!(
        String::from_utf8(clear.stderr)
            .expect("clear diagnostic")
            .contains("MalformedLiteral")
    );
}

#[test]
fn cli_repl_history_records_cls_before_meta_command_dispatch() {
    let result = repl_output(b".cls\r\n.history\n.exit\ninc 5\n");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > 1\t.cls\n2\t.history\n> ");
    assert!(result.stderr.is_empty());
}

#[test]
fn cli_repl_history_discards_oversized_input_and_recovers() {
    let mut transcript = vec![b'1'; 65_537];
    transcript.extend_from_slice(b"\n.history\n");
    let result = repl_output(&transcript);
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > 1\t.history\n> ");
    assert_eq!(
        result.stderr,
        b"error: REPL input exceeds 65536 retained bytes\n"
    );
}

#[test]
fn cli_repl_cls_is_crlf_exact_non_evaluating_and_redirect_safe() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut input = child.stdin.take().expect("REPL stdin");
    input
        .write_all(b".cls\r\ninc 5\n")
        .expect("REPL .cls transcript");
    drop(input);
    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > 6\n> ");
    assert!(!result.stdout.contains(&0x1b));
    assert!(result.stderr.is_empty());
}

#[test]
fn cli_repl_cls_and_exit_commands_compose_without_evaluation() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut input = child.stdin.take().expect("REPL stdin");
    input
        .write_all(b".cls\r\n.exit\ninc 5\n")
        .expect("REPL meta-command transcript");
    drop(input);
    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > ");
    assert!(result.stderr.is_empty());
}

#[test]
fn cli_repl_internal_reports_registry_without_evaluating_source() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut input = child.stdin.take().expect("REPL stdin");
    input
        .write_all(b".internal\r\ninc 5\r\n")
        .expect("REPL internal transcript");
    drop(input);

    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let stdout = String::from_utf8(result.stdout).expect("REPL stdout UTF-8");
    assert!(stdout.starts_with(
        "> Faraweave semantic registry (internal human-readable diagnostics; format is unstable)\n"
    ));
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("primitive "))
            .count(),
        39
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("  signature "))
            .count(),
        66
    );
    assert!(stdout.ends_with("kernel=filter_double\n> 6\n> "));
}

#[test]
fn cli_repl_history_records_internal_before_dispatch() {
    let result = repl_output(b".internal\n.history\n.exit\ninc 5\n");
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let stdout = String::from_utf8(result.stdout).expect("REPL stdout UTF-8");
    assert!(stdout.starts_with(
        "> Faraweave semantic registry (internal human-readable diagnostics; format is unstable)\n"
    ));
    assert!(stdout.ends_with("kernel=filter_double\n> 1\t.internal\n2\t.history\n> "));
}

#[test]
fn cli_repl_ignores_comment_only_lines() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut input = child.stdin.take().expect("REPL stdin");
    input
        .write_all("# comment-only\n \t# utf8 🦀\r\ninc 5\n".as_bytes())
        .expect("REPL transcript");
    drop(input);
    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > > 6\n> ");
    assert!(result.stderr.is_empty());
}

#[test]
fn cli_repl_exit_accepts_exact_whitespace_crlf_and_preserves_eof_success() {
    for input in [
        b".exit\n".as_slice(),
        b" \t.exit\t \r\n".as_slice(),
        b".exit\r\ninc 5\r\n".as_slice(),
        b"".as_slice(),
    ] {
        let mut child = Command::new(binary())
            .arg("repl")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn REPL");
        let mut stdin = child.stdin.take().expect("REPL stdin");
        stdin.write_all(input).expect("REPL transcript");
        drop(stdin);

        let result = child.wait_with_output().expect("REPL output");
        assert!(result.status.success());
        assert_eq!(result.stdout, b"> ");
        assert!(result.stderr.is_empty());
    }
}

#[test]
fn cli_repl_exit_rejects_case_arguments_prefixes_and_source_comments() {
    let mut child = Command::new(binary())
        .arg("repl")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn REPL");
    let mut stdin = child.stdin.take().expect("REPL stdin");
    stdin
        .write_all(
            b"# .exit is comment text\n.EXIT\n.Exit\n.exit-now\n.exit argument\n\
              .exit # trailing comment\n.exit#adjacent-comment\ninc 5\n.exit\ninc 6\n",
        )
        .expect("REPL transcript");
    drop(stdin);

    let result = child.wait_with_output().expect("REPL output");
    assert!(result.status.success());
    assert_eq!(result.stdout, b"> > > > > > > > 6\n> ");
    assert_eq!(
        result.stderr,
        b"<repl>:1:1: MalformedLiteral: malformed scalar literal\n\
          <repl>:1:1: MalformedLiteral: malformed scalar literal\n\
          <repl>:1:1: MalformedLiteral: malformed scalar literal\n\
          <repl>:1:1: MalformedLiteral: malformed scalar literal\n\
          <repl>:1:1: MalformedLiteral: malformed scalar literal\n\
          <repl>:1:1: MalformedLiteral: malformed scalar literal\n"
    );
}

#[test]
fn cli_run_is_extension_agnostic_and_transactional() {
    let directory = unique("run");
    fs::create_dir_all(&directory).expect("mkdir");
    for extension in ["faraweave", "bennu", "anything"] {
        let source = directory.join(format!("program.{extension}"));
        fs::write(&source, "1\ninc[2]\n").expect("source");
        let output = Command::new(binary())
            .arg("run")
            .arg(&source)
            .output()
            .expect("run");
        assert!(output.status.success());
        assert_eq!(output.stdout, b"1\n3\n");
    }
    let failure = directory.join("failure.faraweave");
    fs::write(&failure, "1\ninc[9223372036854775807]\n").expect("failure");
    let output = Command::new(binary())
        .arg("run")
        .arg(&failure)
        .output()
        .expect("run failure");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_source_and_verified_fwir_accept_typed_empty_trivia() {
    let directory = unique("typed-empty-trivia");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("typed-empty.faraweave");
    let artifact = directory.join("typed-empty.fwir");
    fs::write(
        &source,
        "Bool( \t)\nInt(\n)\nDouble(\t# mixed trivia\r\n )\n",
    )
    .expect("source");

    let run = Command::new(binary())
        .arg("run")
        .arg(&source)
        .output()
        .expect("run source");
    assert!(run.status.success(), "{:?}", run.stderr);
    assert_eq!(run.stdout, b"()\n()\n()\n");
    assert!(run.stderr.is_empty());

    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("compile IR");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);
    assert!(compiled.stdout.is_empty());
    assert!(compiled.stderr.is_empty());

    let run_ir = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .output()
        .expect("run IR");
    assert!(run_ir.status.success(), "{:?}", run_ir.stderr);
    assert_eq!(run_ir.stdout, b"()\n()\n()\n");
    assert!(run_ir.stderr.is_empty());

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_connected_completion_success_and_failure_are_transactional() {
    let directory = unique("connected-completion");
    fs::create_dir_all(&directory).expect("mkdir");
    let success = directory.join("success.faraweave");
    fs::write(
        &success,
        "add[10] 20\nadd[] [10 20]\nadd[10] (20 30)\nadd[10] mul[2] 20\n",
    )
    .expect("success source");
    let output = Command::new(binary())
        .arg("run")
        .arg(&success)
        .output()
        .expect("run connected success");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"30\n30\n(30 40)\n50\n");
    assert!(output.stderr.is_empty());
    let artifact = directory.join("success.fwir");
    let compile = Command::new(binary())
        .arg("compile-ir")
        .arg(&success)
        .arg("-o")
        .arg(&artifact)
        .output()
        .expect("compile connected artifact");
    assert!(compile.status.success());
    assert!(compile.stdout.is_empty());
    assert!(compile.stderr.is_empty());
    let loaded = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .output()
        .expect("run connected artifact");
    assert!(loaded.status.success());
    assert_eq!(loaded.stdout, output.stdout);
    assert!(loaded.stderr.is_empty());

    let failure = directory.join("failure.faraweave");
    fs::write(&failure, "1\nadd[] 1\n").expect("failure source");
    let output = Command::new(binary())
        .arg("run")
        .arg(&failure)
        .output()
        .expect("run connected failure");
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(
        output.stderr.ends_with(
            b":2:7: ArityError: add connected completion failed: missing_completion \
              (template_arity=0, supplied_width=1)\n"
        ),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_parameters_and_diagnostics_contract() {
    let directory = unique("parameters");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("args.faraweave");
    let artifact = directory.join("args.fwir");
    fs::write(
        &source,
        "parameters[n Int scale Double enabled Bool]\nn\nscale\nenabled\n",
    )
    .expect("source");
    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("compile IR");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);

    for (command, input) in [("run", &source), ("run-ir", &artifact)] {
        let success = Command::new(binary())
            .arg(command)
            .arg(input)
            .args(["--", "-5", "2.5", "true"])
            .output()
            .expect("runner");
        assert!(success.status.success(), "{command}: {:?}", success.stderr);
        assert_eq!(success.stdout, b"-5\n2.5\ntrue\n", "{command}");
        assert!(success.stderr.is_empty(), "{command}");

        let missing = Command::new(binary())
            .arg(command)
            .arg(input)
            .args(["--", "-5"])
            .output()
            .expect("missing argument");
        assert!(!missing.status.success(), "{command}");
        assert!(missing.stdout.is_empty(), "{command}");
        assert!(
            missing
                .stderr
                .starts_with(b"faraweave_argument_error reason=missing"),
            "{command}: {:?}",
            missing.stderr
        );
    }
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_run_and_run_ir_reject_invalid_unicode_arguments_identically() {
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

    let directory = unique("invalid-unicode-arguments");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("args.faraweave");
    let artifact = directory.join("args.fwir");
    fs::write(&source, "parameters[n Int]\nn\n").expect("source");
    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("compile IR");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);

    for (command, input) in [("run", &source), ("run-ir", &artifact)] {
        let failure = Command::new(binary())
            .arg(command)
            .arg(input)
            .arg("--")
            .arg(&invalid)
            .output()
            .expect("runner with invalid Unicode");
        assert!(!failure.status.success(), "{command}");
        assert!(failure.stdout.is_empty(), "{command}");
        assert_eq!(
            failure.stderr, b"error: unable to decode Unicode command line\n",
            "{command}"
        );
    }

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_deep_tuple_interpretation_does_not_depend_on_host_recursion() {
    let directory = unique("deep-tuple");
    fs::create_dir_all(&directory).expect("mkdir");
    let source_path = directory.join("deep.faraweave");
    let depth = 512;
    let source = format!("{}1{}\n", "[".repeat(depth), "]".repeat(depth));
    fs::write(&source_path, &source).expect("deep source");

    let evaluated = Command::new(binary())
        .arg("run")
        .arg(&source_path)
        .output()
        .expect("deep evaluator process");
    assert!(evaluated.status.success(), "{:?}", evaluated.stderr);
    assert_eq!(evaluated.stdout, source.as_bytes());
    assert!(evaluated.stderr.is_empty());

    fs::write(
        &source_path,
        format!("{}1{}", "[".repeat(depth), "]".repeat(depth - 1)),
    )
    .expect("invalid deep source");
    let invalid = Command::new(binary())
        .arg("run")
        .arg(&source_path)
        .output()
        .expect("deep invalid process");
    assert!(!invalid.status.success());
    assert!(invalid.stdout.is_empty());
    assert!(
        invalid
            .stderr
            .ends_with(b"SyntaxError: missing closing delimiter\n")
    );

    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_unicode_space_paths_roundtrip_through_verified_fwir() {
    let directory = unique("unicode-space").join("nested path ü");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("program source.faraweave");
    let artifact = directory.join("verified program.fwir");
    fs::write(&source, "add[1 2]\n").expect("source");

    let run = Command::new(binary())
        .arg("run")
        .arg(&source)
        .output()
        .expect("unicode run");
    assert!(run.status.success(), "{:?}", run.stderr);
    assert_eq!(run.stdout, b"3\n");

    let compile = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .arg("-o")
        .arg(&artifact)
        .output()
        .expect("unicode compile");
    assert!(compile.status.success(), "{:?}", compile.stderr);
    assert!(artifact.exists());
    let run_ir = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .output()
        .expect("unicode run IR");
    assert!(run_ir.status.success(), "{:?}", run_ir.stderr);
    assert_eq!(run_ir.stdout, b"3\n");

    let missing = Command::new(binary())
        .arg("run")
        .arg(directory.join("missing.faraweave"))
        .output()
        .expect("missing source");
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(
        missing
            .stderr
            .ends_with(b"file error: unable to read source\n")
    );

    let directory_output = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .arg("-o")
        .arg(&directory)
        .output()
        .expect("directory output");
    assert!(!directory_output.status.success());
    assert!(directory_output.stdout.is_empty());
    assert!(directory.is_dir());

    fs::remove_dir_all(directory.parent().expect("test parent")).expect("cleanup");
}

#[cfg(windows)]
#[test]
fn cli_windows_long_path_journey() {
    let base = unique("long-path");
    let mut directory = base.clone();
    while directory.as_os_str().len() < 300 {
        directory.push("segment-0123456789abcdef");
    }
    fs::create_dir_all(&directory).expect("long directory");
    let source = directory.join("program.faraweave");
    fs::write(&source, "inc[41]\n").expect("long source");
    let result = Command::new(binary())
        .arg("run")
        .arg(&source)
        .output()
        .expect("long-path process");
    assert!(result.status.success(), "{:?}", result.stderr);
    assert_eq!(result.stdout, b"42\n");
    let artifact = directory.join("program.fwir");
    let compiled = Command::new(binary())
        .arg("compile-ir")
        .arg(&source)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("long-path compile IR");
    assert!(compiled.status.success(), "{:?}", compiled.stderr);
    let run_ir = Command::new(binary())
        .arg("run-ir")
        .arg(&artifact)
        .output()
        .expect("long-path run IR");
    assert!(run_ir.status.success(), "{:?}", run_ir.stderr);
    assert_eq!(run_ir.stdout, b"42\n");
    fs::remove_dir_all(base).expect("long-path cleanup");
}

#[cfg(unix)]
#[test]
fn cli_unix_unreadable_source_contract() {
    use std::os::unix::fs::PermissionsExt;

    let directory = unique("unreadable");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("unreadable.faraweave");
    fs::write(&source, "1\n").expect("source");
    fs::set_permissions(&source, fs::Permissions::from_mode(0o0)).expect("permissions");
    let result = Command::new(binary())
        .arg("run")
        .arg(&source)
        .output()
        .expect("unreadable process");
    assert!(!result.status.success());
    assert!(result.stdout.is_empty());
    assert!(
        result
            .stderr
            .ends_with(b"file error: unable to read source\n")
    );
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).expect("restore");
    fs::remove_dir_all(directory).expect("cleanup");
}
