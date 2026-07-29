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
    for command in [
        "compile-ir",
        "inspect-ir",
        "run-ir",
        "emit-c-ir",
        "build-ir",
    ] {
        assert!(help.contains(command), "{command}");
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
    let c_output = directory.join("program.c");
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

    let emitted = Command::new(binary())
        .arg("emit-c-ir")
        .arg(&artifact)
        .args(["-o"])
        .arg(&c_output)
        .output()
        .expect("emit C from IR");
    assert!(emitted.status.success(), "{:?}", emitted.stderr);
    assert!(
        fs::read_to_string(&c_output)
            .expect("C")
            .contains("/* VerifiedProgram-driven definitions. */")
    );

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
fn cli_fwir_outputs_reject_aliases_and_preserve_destinations_on_every_failure() {
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

    let artifact_before = fs::read(&artifact).expect("artifact bytes");
    let ir_alias = Command::new(binary())
        .arg("emit-c-ir")
        .arg(&artifact)
        .args(["-o"])
        .arg(&artifact)
        .output()
        .expect("artifact alias");
    assert!(!ir_alias.status.success());
    assert_eq!(
        fs::read(&artifact).expect("artifact preserved"),
        artifact_before
    );

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

    let malformed = directory.join("malformed.fwir");
    let c_destination = directory.join("existing.c");
    fs::write(&malformed, b"bad").expect("malformed");
    fs::write(&c_destination, b"keep-c").expect("sentinel");
    let invalid_emit = Command::new(binary())
        .arg("emit-c-ir")
        .arg(&malformed)
        .args(["-o"])
        .arg(&c_destination)
        .output()
        .expect("invalid emit");
    assert!(!invalid_emit.status.success());
    assert_eq!(fs::read(&c_destination).expect("C destination"), b"keep-c");

    let native_destination = directory.join(if cfg!(windows) {
        "existing.exe"
    } else {
        "existing"
    });
    fs::write(&native_destination, b"keep-native").expect("native sentinel");
    let failed_build = Command::new(binary())
        .arg("build-ir")
        .arg(&artifact)
        .args(["-o"])
        .arg(&native_destination)
        .args(["--cc", "faraweave-compiler-that-does-not-exist"])
        .env_remove("CC")
        .output()
        .expect("compiler failure");
    assert!(!failed_build.status.success());
    assert_eq!(
        fs::read(&native_destination).expect("native destination"),
        b"keep-native"
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
        37
    );
    assert_eq!(
        stdout
            .lines()
            .filter(|line| line.starts_with("  signature "))
            .count(),
        62
    );
    assert!(stdout.ends_with("kernel=ceil_double\n> 6\n> "));
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
    assert!(stdout.ends_with("kernel=ceil_double\n> 1\t.internal\n2\t.history\n> "));
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
fn cli_parameters_and_diagnostics_contract() {
    let directory = unique("parameters");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("args.faraweave");
    fs::write(
        &source,
        "parameters[n Int scale Double enabled Bool]\nn\nscale\nenabled\n",
    )
    .expect("source");
    let success = Command::new(binary())
        .args(["run"])
        .arg(&source)
        .args(["--", "-5", "2.5", "true"])
        .output()
        .expect("run");
    assert!(success.status.success());
    assert_eq!(success.stdout, b"-5\n2.5\ntrue\n");
    let missing = Command::new(binary())
        .args(["run"])
        .arg(&source)
        .args(["--", "-5"])
        .output()
        .expect("missing");
    assert!(!missing.status.success());
    assert!(missing.stdout.is_empty());
    assert!(
        missing
            .stderr
            .starts_with(b"faraweave_argument_error reason=missing")
    );
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_emit_c_is_deterministic_and_alias_safe() {
    let directory = unique("emit");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("input.faraweave");
    let left = directory.join("left.c");
    let right = directory.join("right.c");
    fs::write(&source, "add[1 iota[3]]\n").expect("source");
    for output in [&left, &right] {
        let result = Command::new(binary())
            .arg("emit-c")
            .arg(&source)
            .arg("-o")
            .arg(output)
            .output()
            .expect("emit");
        assert!(result.status.success(), "{:?}", result.stderr);
    }
    assert_eq!(
        fs::read(&left).expect("left"),
        fs::read(&right).expect("right")
    );
    assert!(
        fs::read_to_string(&left)
            .expect("emitted source")
            .contains("/* VerifiedProgram-driven definitions. */")
    );
    let original = fs::read(&source).expect("original");
    let alias = Command::new(binary())
        .arg("emit-c")
        .arg(&source)
        .arg("-o")
        .arg(&source)
        .output()
        .expect("alias");
    assert!(!alias.status.success());
    assert_eq!(fs::read(&source).expect("preserved"), original);
    fs::remove_dir_all(directory).expect("cleanup");
}

#[test]
fn cli_deep_tuple_journeys_do_not_depend_on_host_recursion() {
    let directory = unique("deep-tuple");
    fs::create_dir_all(&directory).expect("mkdir");
    let source_path = directory.join("deep.faraweave");
    let emitted_path = directory.join("deep.c");
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

    let emitted = Command::new(binary())
        .arg("emit-c")
        .arg(&source_path)
        .arg("-o")
        .arg(&emitted_path)
        .output()
        .expect("deep emitter process");
    assert!(emitted.status.success(), "{:?}", emitted.stderr);
    assert!(emitted.stdout.is_empty());
    assert!(emitted.stderr.is_empty());
    let c_source = fs::read_to_string(&emitted_path).expect("emitted C");
    assert_eq!(
        c_source
            .matches("fw_make_tuple(out, 1U, \"tuple_literal\"")
            .count(),
        depth
    );
    assert!(!c_source.contains(&"[".repeat(depth)));

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
fn cli_unicode_space_paths_lexical_aliases_and_failure_cleanup() {
    let directory = unique("unicode-space").join("nested path ü");
    fs::create_dir_all(&directory).expect("mkdir");
    let source = directory.join("program source.faraweave");
    let emitted = directory.join("generated output.c");
    fs::write(&source, "add[1 2]\n").expect("source");

    let run = Command::new(binary())
        .arg("run")
        .arg(&source)
        .output()
        .expect("unicode run");
    assert!(run.status.success(), "{:?}", run.stderr);
    assert_eq!(run.stdout, b"3\n");

    let emit = Command::new(binary())
        .arg("emit-c")
        .arg(&source)
        .arg("-o")
        .arg(&emitted)
        .output()
        .expect("unicode emit");
    assert!(emit.status.success(), "{:?}", emit.stderr);
    assert!(emitted.exists());

    let lexical_alias = directory
        .join("child")
        .join("..")
        .join("program source.faraweave");
    let original = fs::read(&source).expect("original source");
    let alias = Command::new(binary())
        .arg("emit-c")
        .arg(&source)
        .arg("-o")
        .arg(&lexical_alias)
        .output()
        .expect("lexical alias");
    assert!(!alias.status.success());
    assert_eq!(fs::read(&source).expect("preserved source"), original);

    let native = directory.join(if cfg!(windows) {
        "existing native.exe"
    } else {
        "existing native"
    });
    fs::write(&native, b"preserve-me").expect("native sentinel");
    let failed = Command::new(binary())
        .arg("build")
        .arg(&source)
        .arg("-o")
        .arg(&native)
        .arg("--cc")
        .arg("faraweave-compiler-that-does-not-exist")
        .env_remove("CC")
        .output()
        .expect("failed native build");
    assert!(!failed.status.success());
    assert!(failed.stdout.is_empty());
    assert_eq!(fs::read(&native).expect("preserved native"), b"preserve-me");
    let leftovers: Vec<_> = fs::read_dir(&directory)
        .expect("directory listing")
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".existing native")
        })
        .collect();
    assert!(
        leftovers.is_empty(),
        "temporary files escaped: {leftovers:?}"
    );

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
        .arg("emit-c")
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
    let emitted = directory.join("program.c");
    let emit_ir = Command::new(binary())
        .arg("emit-c-ir")
        .arg(&artifact)
        .args(["-o"])
        .arg(&emitted)
        .output()
        .expect("long-path emit IR");
    assert!(emit_ir.status.success(), "{:?}", emit_ir.stderr);
    assert!(emitted.exists());
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
