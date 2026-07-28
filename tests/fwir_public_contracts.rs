use faraweave::{
    DomainErrorReason, EvaluationConfiguration, FwirDecodeLimits, FwirEncodeOptions,
    ResourceErrorReason, Value, compile_source_to_fwir, compile_source_to_fwir_with_name,
    compile_source_to_verified_program, decode_fwir, emit_c_from_verified_program, encode_fwir,
    evaluate_verified_program_with_arguments, evaluate_verified_program_with_observer,
    inspect_fwir,
};
use std::sync::Mutex;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedEvent {
    kind: faraweave::ResourceEventKind,
    producer: String,
    bytes: Option<usize>,
    work: usize,
    ordinal: Option<usize>,
    refusal: Option<ResourceErrorReason>,
    usage: faraweave::ResourceUsage,
}

static EVENTS: Mutex<Vec<ObservedEvent>> = Mutex::new(Vec::new());

fn observe(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = EVENTS.lock() {
        events.push(ObservedEvent {
            kind: event.kind,
            producer: event.producer.to_owned(),
            bytes: event.requested_bytes,
            work: event.requested_work_units,
            ordinal: event.allocation_ordinal,
            refusal: event.refusal_reason,
            usage: event.usage,
        });
    }
}

fn take_events() -> Vec<ObservedEvent> {
    match EVENTS.lock() {
        Ok(mut events) => std::mem::take(&mut *events),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    }
}

#[test]
fn public_source_artifact_execution_c_and_resource_traces_are_differential() {
    let source = "parameters[n Int]\nfanout[iota[n] {inc[_]} {add[_ 10]}]\n";
    let program =
        compile_source_to_verified_program(source, "logical/input.faraweave").expect("compile");
    let bytes = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode");
    let decoded =
        decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode and verification");

    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["3"],
        EvaluationConfiguration::default(),
    )
    .expect("direct");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["3"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded");
    assert_eq!(direct, loaded);
    assert_eq!(direct.formatted, ["[(2 3 4) (11 12 13)]"]);

    let arguments = [Value::Int(3)];
    let _ = take_events();
    let direct_observed = evaluate_verified_program_with_observer(
        &program,
        &arguments,
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("direct observed");
    let direct_events = take_events();
    let loaded_observed = evaluate_verified_program_with_observer(
        &decoded,
        &arguments,
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("decoded observed");
    let loaded_events = take_events();
    assert_eq!(direct_observed, loaded_observed);
    assert_eq!(direct_events, loaded_events);

    let direct_c =
        emit_c_from_verified_program(&program, EvaluationConfiguration::default()).expect("C");
    let loaded_c = emit_c_from_verified_program(&decoded, EvaluationConfiguration::default())
        .expect("decoded C");
    assert_eq!(direct_c, loaded_c);

    let direct_error = evaluate_verified_program_with_arguments(
        &program,
        &["9223372036854775807"],
        EvaluationConfiguration::default(),
    )
    .expect_err("direct overflow");
    let loaded_error = evaluate_verified_program_with_arguments(
        &decoded,
        &["9223372036854775807"],
        EvaluationConfiguration::default(),
    )
    .expect_err("decoded overflow");
    assert_eq!(direct_error, loaded_error);
}

#[test]
fn public_compile_errors_and_argument_errors_preserve_phase_and_provenance() {
    let invalid =
        compile_source_to_fwir("inc[", &FwirEncodeOptions::default()).expect_err("invalid source");
    assert!(matches!(invalid, faraweave::CompileFwirError::Compile(_)));

    let bytes = compile_source_to_fwir_with_name(
        "parameters[n Int]\ninc[n]\n",
        "retained logical name.faraweave",
        &FwirEncodeOptions::default(),
    )
    .expect("compile");
    let program = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode");
    assert_eq!(
        program.as_raw().source_units[0].diagnostic_name,
        "retained logical name.faraweave"
    );
    let error = evaluate_verified_program_with_arguments(
        &program,
        &["not-an-int"],
        EvaluationConfiguration::default(),
    )
    .expect_err("argument");
    assert_eq!(error.kind, faraweave::ErrorKind::ArgumentError);
}

#[test]
fn div_identity_domain_and_c_emission_survive_fwir_roundtrip() {
    let source = "div[(8 9 10) (2 0 5)]\n";
    let program =
        compile_source_to_verified_program(source, "division.faraweave").expect("compile div");
    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode div");
    let decoded = decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified div");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode div"),
        encoded
    );

    let direct =
        evaluate_verified_program_with_arguments(&program, &[], EvaluationConfiguration::default())
            .expect_err("direct division domain");
    let loaded =
        evaluate_verified_program_with_arguments(&decoded, &[], EvaluationConfiguration::default())
            .expect_err("decoded division domain");
    assert_eq!(loaded, direct);
    assert_eq!(
        direct.domain.as_ref().map(|context| context.reason),
        Some(DomainErrorReason::DivisionByZero)
    );
    assert_eq!(
        direct
            .domain
            .as_ref()
            .and_then(|context| context.element_index),
        Some(1)
    );

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    assert!(emitted.source.contains("static int fw_kernel_35("));
    assert!(emitted.source.contains("fw_selected_division_by_zero"));
    assert!(!emitted.source.contains("strcmp(name"));
}

#[test]
fn inspection_is_deterministic_and_carries_exact_binary64_bits() {
    let program = compile_source_to_verified_program("-0.0\nnan\n", "bits.faraweave")
        .expect("compile exact doubles");
    let first = inspect_fwir(&program).expect("first inspection");
    let second = inspect_fwir(&program).expect("second inspection");
    assert_eq!(first, second);
    assert!(first.contains("DoubleBits(9223372036854775808)"));
    assert!(first.contains("DoubleBits(9221120237041090560)"));
    assert!(first.contains("source[0] name=\"bits.faraweave\""));
    assert!(first.contains("canonical-hex 465749520d0a1a0a"));
}
