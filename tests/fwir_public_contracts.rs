use faraweave::{
    DomainErrorReason, EvaluationConfiguration, Feature, FwirDecodeLimits, FwirEncodeOptions,
    LiftMode, NodeKind, ResourceErrorReason, Value, compile_source_to_fwir,
    compile_source_to_fwir_with_name, compile_source_to_verified_program, decode_fwir,
    emit_c_from_verified_program, encode_fwir, evaluate_verified_program_with_arguments,
    evaluate_verified_program_with_observer, inspect_fwir,
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
fn line_comments_preserve_source_length_and_lower_to_c() {
    let source = "# prologue 🦀\r\ninc[# argument\n1]# eof";
    let program =
        compile_source_to_verified_program(source, "comments.faraweave").expect("compile comments");
    assert_eq!(
        program.as_raw().source_units[0].byte_length,
        u32::try_from(source.len()).expect("fixture length")
    );
    let emitted =
        emit_c_from_verified_program(&program, EvaluationConfiguration::default()).expect("emit C");
    assert!(emitted.source.contains("Strict C11"));
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
fn length_container_plan_roundtrips_and_dispatches_by_verified_identity() {
    let source = "parameters[n Int]\n\
                  length[(true false)]\n\
                  length[(1 2 3)]\n\
                  length[Double()]\n\
                  length iota n\n";
    let program =
        compile_source_to_verified_program(source, "length.faraweave").expect("compile length");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 21,
                signature_id,
                implementation_id,
                application_plan_id,
                lift,
                ..
            } => Some((signature_id, implementation_id, application_plan_id, lift)),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (37, 37, 3, LiftMode::ContainerScalar),
            (38, 38, 3, LiftMode::ContainerScalar),
            (39, 39, 3, LiftMode::ContainerScalar),
            (38, 38, 3, LiftMode::ContainerScalar),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode length");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified length");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode length"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct length");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded length");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["2", "3", "0", "4"]);

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    for implementation in 37..=39 {
        assert!(
            emitted
                .source
                .contains(&format!("static int fw_impl_{implementation}("))
        );
    }
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_length(")
            .count(),
        3
    );
    assert!(!emitted.source.contains("strcmp(name"));
}

#[test]
fn sort_container_plan_roundtrips_and_dispatches_by_verified_identity() {
    let source = "parameters[n Int]\n\
                  sort[(true false true)]\n\
                  sort[(3 1 2)]\n\
                  sort[(inf -0.0 0.0 -inf)]\n\
                  sort iota n\n";
    let program =
        compile_source_to_verified_program(source, "sort.faraweave").expect("compile sort");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 22,
                signature_id,
                implementation_id,
                application_plan_id,
                lift,
                ..
            } => Some((signature_id, implementation_id, application_plan_id, lift)),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (40, 40, 4, LiftMode::ContainerVector),
            (41, 41, 4, LiftMode::ContainerVector),
            (42, 42, 4, LiftMode::ContainerVector),
            (41, 41, 4, LiftMode::ContainerVector),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode sort");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified sort");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode sort"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct sort");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded sort");
    assert_eq!(loaded, direct);
    assert_eq!(
        direct.formatted,
        [
            "(false true true)",
            "(1 2 3)",
            "(-inf -0.0 0.0 inf)",
            "(1 2 3 4)",
        ]
    );

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    for implementation in 40..=42 {
        assert!(
            emitted
                .source
                .contains(&format!("static int fw_impl_{implementation}("))
        );
    }
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_sort(")
            .count(),
        3
    );
    assert!(emitted.source.contains("fw_double_order_key(values[left])"));
    assert!(!emitted.source.contains("qsort("));
    assert!(!emitted.source.contains("strcmp(name"));
}

#[test]
fn sum_container_plan_roundtrips_and_dispatches_by_verified_identity() {
    let source = "parameters[n Int]\n\
                  sum[(1 2 3)]\n\
                  sum[(1.5 -0.5 2.0)]\n\
                  sum[Int()]\n\
                  sum[Double()]\n\
                  sum iota n\n";
    let program = compile_source_to_verified_program(source, "sum.faraweave").expect("compile sum");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 23,
                signature_id,
                implementation_id,
                application_plan_id,
                lift,
                ..
            } => Some((signature_id, implementation_id, application_plan_id, lift)),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (43, 43, 5, LiftMode::ContainerScalar),
            (44, 44, 5, LiftMode::ContainerScalar),
            (43, 43, 5, LiftMode::ContainerScalar),
            (44, 44, 5, LiftMode::ContainerScalar),
            (43, 43, 5, LiftMode::ContainerScalar),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode sum");
    let decoded = decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified sum");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode sum"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct sum");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded sum");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["6", "3.0", "0", "0.0", "10"]);

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    for implementation in 43..=44 {
        assert!(
            emitted
                .source
                .contains(&format!("static int fw_impl_{implementation}("))
        );
    }
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_sum_int(")
            .count(),
        1
    );
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_sum_double(")
            .count(),
        1
    );
    assert!(
        emitted
            .source
            .contains("total=fw_double_arithmetic(total,values[index],FW_DOUBLE_ADD)")
    );
    assert!(!emitted.source.contains("strcmp(name"));
}

#[test]
fn all_of_container_plan_roundtrips_and_dispatches_by_verified_identity() {
    let source = "parameters[n Int]\n\
                  all_of[Bool()]\n\
                  all_of[(true false true)]\n\
                  all_of equals[iota n iota n]\n";
    let program =
        compile_source_to_verified_program(source, "all-of.faraweave").expect("compile all_of");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 24,
                signature_id,
                implementation_id,
                application_plan_id,
                lift,
                ..
            } => Some((signature_id, implementation_id, application_plan_id, lift)),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (45, 45, 6, LiftMode::ContainerScalar),
            (45, 45, 6, LiftMode::ContainerScalar),
            (45, 45, 6, LiftMode::ContainerScalar),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode all_of");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified all_of");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode all_of"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct all_of");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded all_of");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["true", "false", "true"]);

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    assert!(emitted.source.contains("static int fw_impl_45("));
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_all_of(")
            .count(),
        1
    );
    assert!(
        emitted
            .source
            .contains("if(!fw_charge_work(args[0].len,name,line,column))return 0;")
    );
    assert!(!emitted.source.contains("strcmp(name"));
}

#[test]
fn any_of_container_plan_roundtrips_and_dispatches_by_verified_identity() {
    let source = "parameters[n Int]\n\
                  any_of[Bool()]\n\
                  any_of[(false false false)]\n\
                  any_of equals[iota n iota n]\n";
    let program =
        compile_source_to_verified_program(source, "any-of.faraweave").expect("compile any_of");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 25,
                signature_id,
                implementation_id,
                application_plan_id,
                lift,
                ..
            } => Some((signature_id, implementation_id, application_plan_id, lift)),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (46, 46, 7, LiftMode::ContainerScalar),
            (46, 46, 7, LiftMode::ContainerScalar),
            (46, 46, 7, LiftMode::ContainerScalar),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode any_of");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified any_of");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode any_of"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct any_of");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded any_of");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["false", "false", "true"]);

    let emitted =
        emit_c_from_verified_program(&decoded, EvaluationConfiguration::default()).expect("C");
    assert!(emitted.source.contains("static int fw_impl_46("));
    assert_eq!(
        emitted
            .source
            .matches("return fw_apply_selected_any_of(")
            .count(),
        1
    );
    assert!(
        emitted
            .source
            .contains("if(!fw_charge_work(args[0].len,name,line,column))return 0;")
    );
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
