use faraweave::{
    AllocationFailureInjection, DomainErrorReason, EvaluationConfiguration, ExecutionProfile,
    Feature, FwirDecodeLimits, FwirEncodeOptions, LiftMode, NodeKind, ResourceErrorReason,
    ResourceLimits, RootPresentation, Value, compile_source_to_fwir,
    compile_source_to_fwir_with_name, compile_source_to_verified_program, decode_fwir, encode_fwir,
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
static EVENT_TEST_LOCK: Mutex<()> = Mutex::new(());

fn lock_event_test() -> std::sync::MutexGuard<'static, ()> {
    EVENT_TEST_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

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

fn section(encoded: &[u8], wanted: u16) -> (usize, usize, usize) {
    let count = u32::from_le_bytes(encoded[20..24].try_into().expect("section count")) as usize;
    for index in 0..count {
        let entry = 32 + index * 24;
        let id = u16::from_le_bytes(encoded[entry..entry + 2].try_into().expect("section id"));
        if id == wanted {
            let record_size = u32::from_le_bytes(
                encoded[entry + 4..entry + 8]
                    .try_into()
                    .expect("record size"),
            ) as usize;
            let offset =
                u64::from_le_bytes(encoded[entry + 8..entry + 16].try_into().expect("offset"))
                    as usize;
            return (entry, offset, record_size);
        }
    }
    panic!("section {wanted} missing");
}

#[test]
fn value_formatting_is_semantic_and_physical_1_5_and_roundtrips_root_presentation() {
    let source =
        "format[\"{}|{}|{{}}\" \"é\\0\" [1 \"x\"]]\nprintf[\"raw={}\\0\" (nan -0.0 inf)]\n";
    let direct =
        compile_source_to_verified_program(source, "formatting.faraweave").expect("compile");
    let encoded = encode_fwir(&direct, &FwirEncodeOptions::default()).expect("encode");
    assert_eq!(u16::from_le_bytes([encoded[10], encoded[11]]), 5);
    assert_eq!(direct.module().semantic_minor, 5);
    assert!(
        direct
            .as_raw()
            .features
            .contains(&Feature::ValueFormatting.numeric())
    );
    assert_eq!(
        direct
            .as_raw()
            .roots
            .iter()
            .map(|root| root.presentation)
            .collect::<Vec<_>>(),
        [
            RootPresentation::CanonicalValue,
            RootPresentation::RawString
        ]
    );
    let (_, nodes, node_record_size) = section(&encoded, 14);
    assert_eq!(node_record_size, 56);
    let format_node = (0..direct.as_raw().nodes.len())
        .find(|index| encoded[nodes + index * node_record_size] == 11)
        .expect("format node");
    let (_, roots, root_record_size) = section(&encoded, 16);
    assert_eq!(root_record_size, 12);
    assert_eq!(encoded[roots + 8], 1);
    assert_eq!(encoded[roots + root_record_size + 8], 2);

    let decoded = decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode"),
        encoded
    );
    let direct_result =
        evaluate_verified_program_with_arguments(&direct, &[], EvaluationConfiguration::default())
            .expect("direct execute");
    let decoded_result =
        evaluate_verified_program_with_arguments(&decoded, &[], EvaluationConfiguration::default())
            .expect("decoded execute");
    assert_eq!(direct_result, decoded_result);
    assert_eq!(
        decoded_result.formatted,
        ["\"é\\0|[1 \\\"x\\\"]|{}\"", "raw=(nan -0.0 inf)\0"]
    );
    assert_eq!(
        decoded_result.presentations,
        [
            RootPresentation::CanonicalValue,
            RootPresentation::RawString
        ]
    );

    for relative in [8usize, 9] {
        let mut hostile = encoded.clone();
        hostile[roots + relative] = if relative == 8 { 0 } else { 1 };
        assert!(decode_fwir(&hostile, &FwirDecodeLimits::default()).is_err());
    }
    let format_record = nodes + format_node * node_record_size;
    for relative in [24usize, 28, 32] {
        let mut hostile = encoded.clone();
        hostile[format_record + relative..format_record + relative + 4]
            .copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(decode_fwir(&hostile, &FwirDecodeLimits::default()).is_err());
    }
    let edge_start = u32::from_le_bytes(
        encoded[format_record + 12..format_record + 16]
            .try_into()
            .expect("edge start"),
    ) as usize;
    let (_, edges, edge_record_size) = section(&encoded, 11);
    let mut wrong_conversion = encoded.clone();
    wrong_conversion[edges + edge_start * edge_record_size + 10] = 2;
    assert!(decode_fwir(&wrong_conversion, &FwirDecodeLimits::default()).is_err());

    let mut old_physical = encoded.clone();
    old_physical[10..12].copy_from_slice(&4_u16.to_le_bytes());
    assert!(decode_fwir(&old_physical, &FwirDecodeLimits::default()).is_err());
    let (_, module, _) = section(&encoded, 1);
    let mut old_semantic = encoded.clone();
    old_semantic[module + 2..module + 4].copy_from_slice(&4_u16.to_le_bytes());
    assert!(decode_fwir(&old_semantic, &FwirDecodeLimits::default()).is_err());

    let feature = encoded
        .windows(4)
        .position(|record| record == [11, 0, 0, 0])
        .expect("feature 11");
    let mut missing = encoded;
    missing[feature..feature + 2].copy_from_slice(&100_u16.to_le_bytes());
    missing[feature + 2] = 1;
    assert!(decode_fwir(&missing, &FwirDecodeLimits::default()).is_err());
}

#[test]
fn formatting_admits_one_complete_result_and_fault_cleanup_is_deterministic() {
    let _event_test = lock_event_test();
    let program = compile_source_to_verified_program(
        "format[\"{}|{}\" \"é\" [1 \"x\"]]",
        "format-resources.faraweave",
    )
    .expect("compile");
    let configuration = EvaluationConfiguration {
        profile: ExecutionProfile::BoundedV2,
        limits: ResourceLimits {
            max_vector_bytes: None,
            max_tuple_table_bytes: Some(1024),
            max_live_evaluation_bytes: Some(1024),
            max_work_units: Some(1024),
        },
        allocation_failure: AllocationFailureInjection::default(),
    };
    let _ = take_events();
    let result = evaluate_verified_program_with_observer(&program, &[], configuration, observe)
        .expect("run");
    assert_eq!(result.values, [Value::String("é|[1 \"x\"]".to_owned())]);
    let events = take_events();
    let format = events
        .iter()
        .find(|event| {
            event.kind == faraweave::ResourceEventKind::Admission && event.producer == "format"
        })
        .expect("format admission");
    assert_eq!(format.bytes, Some("é|[1 \"x\"]".len()));
    assert_eq!(format.work, "é|[1 \"x\"]".len());
    let format_ordinal = format.ordinal.expect("allocation ordinal");
    assert_eq!(
        events
            .iter()
            .filter(|event| event.producer == "format"
                && event.kind == faraweave::ResourceEventKind::Admission)
            .count(),
        1
    );

    let _ = take_events();
    let failure = evaluate_verified_program_with_observer(
        &program,
        &[],
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(format_ordinal),
            },
            ..configuration
        },
        observe,
    )
    .expect_err("format allocation refusal");
    assert_eq!(
        failure.resource.as_ref().map(|context| context.reason),
        Some(ResourceErrorReason::AllocationUnavailable)
    );
    assert_eq!(
        failure
            .resource
            .as_ref()
            .and_then(|context| context.allocation_ordinal),
        Some(format_ordinal)
    );
    assert_eq!(
        failure.usage.map(|usage| usage.live_evaluation_bytes),
        Some(0)
    );
    let failure_events = take_events();
    assert!(failure_events.iter().any(|event| {
        event.kind == faraweave::ResourceEventKind::Refusal
            && event.producer == "format"
            && event.ordinal == Some(format_ordinal)
    }));
    assert!(
        failure_events
            .iter()
            .any(|event| { event.kind == faraweave::ResourceEventKind::Release })
    );
}

#[test]
fn utf8_strings_are_semantic_and_physical_1_4_and_roundtrip_exactly() {
    let source = "parameters[name String]\n[\"nul\\0é\" name equals[name \"é\"] length[name] sort[(\"é\" \"a\" \"\")]]\n";
    let encoded =
        compile_source_to_fwir(source, &FwirEncodeOptions::default()).expect("compile strings");
    assert_eq!(u16::from_le_bytes([encoded[8], encoded[9]]), 1);
    assert_eq!(u16::from_le_bytes([encoded[10], encoded[11]]), 4);

    let decoded = decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode strings");
    assert_eq!(decoded.module().semantic_minor, 4);
    assert!(
        decoded
            .as_raw()
            .features
            .contains(&Feature::Strings.numeric())
    );
    assert!(
        decoded
            .as_raw()
            .string_values
            .iter()
            .any(|value| value.as_bytes() == b"nul\0\xc3\xa9")
    );
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("re-encode strings"),
        encoded
    );

    let result = evaluate_verified_program_with_arguments(
        &decoded,
        &["é"],
        EvaluationConfiguration::default(),
    )
    .expect("execute decoded strings");
    assert_eq!(
        result.formatted,
        vec!["[\"nul\\0é\" \"é\" true 1 (\"\" \"a\" \"é\")]"]
    );

    let feature_record = encoded
        .windows(4)
        .position(|record| record == [10, 0, 0, 0])
        .expect("feature 10 record");
    let mut missing = encoded.clone();
    missing[feature_record..feature_record + 2].copy_from_slice(&11_u16.to_le_bytes());
    missing[feature_record + 2] = 1;
    assert!(decode_fwir(&missing, &FwirDecodeLimits::default()).is_err());

    let mut old_physical = encoded.clone();
    old_physical[10..12].copy_from_slice(&3_u16.to_le_bytes());
    assert!(decode_fwir(&old_physical, &FwirDecodeLimits::default()).is_err());
}

#[test]
fn public_source_and_decoded_artifact_execution_and_resource_traces_match() {
    let _event_test = lock_event_test();
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
fn connected_completion_roundtrips_as_ordinary_existing_fwir() {
    let source = "parameters[n Int]\nadd[10] mul[2] n\n";
    let program =
        compile_source_to_verified_program(source, "connected.faraweave").expect("compile");
    assert_eq!(program.module().semantic_major, 1);
    assert_eq!(program.module().semantic_minor, 0);
    assert_eq!(
        program.as_raw().features,
        [Feature::StableSemanticIds.numeric()]
    );
    assert_eq!(
        program
            .as_raw()
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::SelectedApply { .. }))
            .count(),
        2
    );
    assert!(!program.as_raw().nodes.iter().any(|node| matches!(
        node.kind,
        NodeKind::TupleConstruct | NodeKind::PrefixSpreadPrepare
    )));

    let bytes = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode");
    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("re-encode"),
        bytes
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["20"],
        EvaluationConfiguration::default(),
    )
    .expect("direct");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["20"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded");
    assert_eq!(direct, loaded);
    assert_eq!(direct.formatted, ["50"]);

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

    let authored_tuple =
        compile_source_to_verified_program("add[] [10 20]\n", "tuple-bundle.faraweave")
            .expect("tuple bundle");
    assert!(
        !authored_tuple
            .as_raw()
            .features
            .contains(&Feature::Tuples.numeric())
    );
    assert_eq!(authored_tuple.module().semantic_minor, 0);
}

#[test]
fn connected_bindings_are_semantic_and_physical_1_2_and_roundtrip_exactly() {
    let _event_test = lock_event_test();
    let source = "parameters[x Int]\n\
                  add[10 _] x\n\
                  sub[_2 _1] [1 x]\n\
                  mul[_1 _1] iota[x]\n\
                  add[_] fanout[1 {add[_ 9]} {add[_ 19]}]\n";
    let program = compile_source_to_verified_program(source, "connected-bindings.faraweave")
        .expect("compile");
    assert_eq!(program.module().semantic_major, 1);
    assert_eq!(program.module().semantic_minor, 2);
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ConnectedApplicationBindings.numeric())
    );
    assert_eq!(
        program
            .as_raw()
            .nodes
            .iter()
            .filter(|node| matches!(node.kind, NodeKind::ConnectedBinding))
            .count(),
        4
    );

    let bytes = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode");
    assert_eq!(u16::from_le_bytes([bytes[8], bytes[9]]), 1);
    assert_eq!(u16::from_le_bytes([bytes[10], bytes[11]]), 2);
    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("re-encode"),
        bytes
    );
    let inspection = inspect_fwir(&decoded).expect("inspect bindings");
    assert!(inspection.contains("feature["));
    assert!(inspection.contains("id=8"));
    assert!(inspection.contains("ConnectedBinding"));
    assert!(inspection.contains("ConnectedBindingElement"));

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
    assert_eq!(direct.formatted, ["13", "2", "(1 4 9)", "30"]);

    take_events();
    let observed_direct = evaluate_verified_program_with_observer(
        &program,
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("observed direct");
    let direct_events = take_events();
    let observed_loaded = evaluate_verified_program_with_observer(
        &decoded,
        &[Value::Int(3)],
        EvaluationConfiguration::default(),
        observe,
    )
    .expect("observed decoded");
    let loaded_events = take_events();
    assert_eq!(observed_direct, observed_loaded);
    assert_eq!(direct_events, loaded_events);
}

#[test]
fn line_comments_preserve_source_length_and_interpreter_result() {
    let source = "# prologue 🦀\r\ninc[# argument\n1]# eof";
    let program =
        compile_source_to_verified_program(source, "comments.faraweave").expect("compile comments");
    assert_eq!(
        program.as_raw().source_units[0].byte_length,
        u32::try_from(source.len()).expect("fixture length")
    );
    let result =
        evaluate_verified_program_with_arguments(&program, &[], EvaluationConfiguration::default())
            .expect("interpret comments");
    assert_eq!(result.formatted, ["2"]);
}

#[test]
fn typed_empty_trivia_survives_verified_fwir_roundtrip() {
    let source = "Bool( \t)\nInt(\n)\nDouble(\t# empty\r\n )\n";
    let program =
        compile_source_to_verified_program(source, "typed-empty.faraweave").expect("compile");
    let bytes = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode");
    let decoded = decode_fwir(&bytes, &FwirDecodeLimits::default()).expect("decode");
    let result =
        evaluate_verified_program_with_arguments(&decoded, &[], EvaluationConfiguration::default())
            .expect("interpret");

    assert_eq!(result.formatted, ["()", "()", "()"]);
    assert_eq!(
        decoded
            .as_raw()
            .nodes
            .iter()
            .map(|node| node.cardinality)
            .collect::<Vec<_>>(),
        [
            Some(faraweave::Cardinality::StaticVector(0)),
            Some(faraweave::Cardinality::StaticVector(0)),
            Some(faraweave::Cardinality::StaticVector(0)),
        ]
    );
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
fn div_identity_and_domain_error_survive_fwir_roundtrip() {
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
}

#[test]
fn none_of_container_plan_roundtrips_and_dispatches_by_its_own_verified_identity() {
    let source = "parameters[n Int]\n\
                  none_of[Bool()]\n\
                  none_of[(false false false)]\n\
                  none_of equals[iota n iota n]\n";
    let program =
        compile_source_to_verified_program(source, "none-of.faraweave").expect("compile none_of");
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
                primitive_id: 26,
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
            (47, 47, 8, LiftMode::ContainerScalar),
            (47, 47, 8, LiftMode::ContainerScalar),
            (47, 47, 8, LiftMode::ContainerScalar),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode none_of");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified none_of");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode none_of"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct none_of");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded none_of");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["true", "true", "false"]);
}

#[test]
fn filter_roundtrips_predicate_links_dynamic_subset_metadata_and_direct_dispatch() {
    let source = "parameters[n Int]\n\
                  filter[@not Bool()]\n\
                  filter[@odd (1 2 3 4 5)]\n\
                  filter[@odd iota[n]]\n\
                  filter[@is_positive (-2.0 -0.0 0.0 3.0 inf nan)]\n";
    let program =
        compile_source_to_verified_program(source, "filter.faraweave").expect("compile filter");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::OperationReferences.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 39,
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference,
                lift,
                ..
            } => Some((
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference.map(|index| index.0),
                lift,
                node.cardinality,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (
                64,
                64,
                11,
                Some(0),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::DynamicVector),
            ),
            (
                65,
                65,
                11,
                Some(1),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::DynamicVector),
            ),
            (
                65,
                65,
                11,
                Some(2),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::DynamicVector),
            ),
            (
                66,
                66,
                11,
                Some(3),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::DynamicVector),
            ),
        ]
    );
    assert_eq!(
        program
            .as_raw()
            .operation_references
            .iter()
            .map(|reference| (
                reference.primitive_id,
                reference.signature_id,
                reference.implementation_id,
            ))
            .collect::<Vec<_>>(),
        [(10, 21, 21), (13, 24, 24), (13, 24, 24), (15, 27, 27)]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode filter");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified filter");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode filter"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["5"],
        EvaluationConfiguration::default(),
    )
    .expect("direct filter");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["5"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded filter");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["()", "(1 3 5)", "(1 3 5)", "(3.0 inf)"]);
}

#[test]
fn filter_randomized_direct_and_decoded_fwir_results_agree() {
    let mut state = 0x5eed_86f1_7e12_4a39_u64;
    for case in 0..128 {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        let length = usize::try_from(state % 33).expect("bounded length");
        let mut values = Vec::with_capacity(length);
        for _ in 0..length {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            values.push(i64::try_from(state % 2_001).expect("bounded value") - 1_000);
        }
        let source = if values.is_empty() {
            "filter[@odd Int()]\n".to_owned()
        } else {
            format!(
                "filter[@odd ({})]\n",
                values
                    .iter()
                    .map(i64::to_string)
                    .collect::<Vec<_>>()
                    .join(" ")
            )
        };
        let program = compile_source_to_verified_program(&source, "random-filter.faraweave")
            .expect("compile randomized filter");
        let encoded =
            encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode randomized filter");
        let decoded =
            decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode randomized filter");
        let direct = evaluate_verified_program_with_arguments(
            &program,
            &[],
            EvaluationConfiguration::default(),
        )
        .expect("direct randomized filter");
        let loaded = evaluate_verified_program_with_arguments(
            &decoded,
            &[],
            EvaluationConfiguration::default(),
        )
        .expect("decoded randomized filter");
        assert_eq!(loaded, direct, "case {case}");
        assert_eq!(
            direct.values,
            [Value::IntVector(
                values.into_iter().filter(|value| value % 2 != 0).collect()
            )],
            "case {case}"
        );
    }
}

#[test]
fn foldl_roundtrips_reducer_links_and_dispatches_only_verified_identities() {
    let source = "parameters[n Int]\n\
                  foldl[@sub 10 Int()]\n\
                  foldl[@sub 20 (3 4 5)]\n\
                  foldl[@add 0 iota[n]]\n\
                  foldl[@add 1 Double()]\n";
    let program =
        compile_source_to_verified_program(source, "foldl.faraweave").expect("compile foldl");
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::ApplicationPlans.numeric())
    );
    assert!(
        program
            .as_raw()
            .features
            .contains(&Feature::OperationReferences.numeric())
    );
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 27,
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference,
                lift,
                ..
            } => Some((
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference.map(|index| index.0),
                lift,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (49, 49, 9, Some(0), LiftMode::ContainerScalar),
            (49, 49, 9, Some(1), LiftMode::ContainerScalar),
            (49, 49, 9, Some(2), LiftMode::ContainerScalar),
            (50, 50, 9, Some(3), LiftMode::ContainerScalar),
        ]
    );
    assert_eq!(
        program
            .as_raw()
            .operation_references
            .iter()
            .map(|reference| (
                reference.primitive_id,
                reference.signature_id,
                reference.implementation_id,
            ))
            .collect::<Vec<_>>(),
        [(6, 11, 11), (6, 11, 11), (5, 9, 9), (5, 10, 10)]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode foldl");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified foldl");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode foldl"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct foldl");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded foldl");
    assert_eq!(loaded, direct);
    assert_eq!(direct.formatted, ["10", "8", "10", "1.0"]);
}

#[test]
fn scanl_roundtrips_reducer_links_plus_one_shape_and_direct_dispatch() {
    let source = "parameters[n Int]\n\
                  scanl[@sub 10 Int()]\n\
                  scanl[@sub 20 (3 4 5)]\n\
                  scanl[@add 0 iota[n]]\n\
                  scanl[@add 1 Double()]\n";
    let program =
        compile_source_to_verified_program(source, "scanl.faraweave").expect("compile scanl");
    let identities: Vec<_> = program
        .as_raw()
        .nodes
        .iter()
        .filter_map(|node| match node.kind {
            NodeKind::SelectedApply {
                primitive_id: 28,
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference,
                lift,
                ..
            } => Some((
                signature_id,
                implementation_id,
                application_plan_id,
                operation_reference.map(|index| index.0),
                lift,
                node.cardinality,
            )),
            _ => None,
        })
        .collect();
    assert_eq!(
        identities,
        [
            (
                52,
                52,
                10,
                Some(0),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::StaticVector(1)),
            ),
            (
                52,
                52,
                10,
                Some(1),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::StaticVector(4)),
            ),
            (
                52,
                52,
                10,
                Some(2),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::DynamicVector),
            ),
            (
                53,
                53,
                10,
                Some(3),
                LiftMode::ContainerVector,
                Some(faraweave::Cardinality::StaticVector(1)),
            ),
        ]
    );

    let encoded = encode_fwir(&program, &FwirEncodeOptions::default()).expect("encode scanl");
    let decoded =
        decode_fwir(&encoded, &FwirDecodeLimits::default()).expect("decode verified scanl");
    assert_eq!(
        encode_fwir(&decoded, &FwirEncodeOptions::default()).expect("reencode scanl"),
        encoded
    );
    let direct = evaluate_verified_program_with_arguments(
        &program,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("direct scanl");
    let loaded = evaluate_verified_program_with_arguments(
        &decoded,
        &["4"],
        EvaluationConfiguration::default(),
    )
    .expect("decoded scanl");
    assert_eq!(loaded, direct);
    assert_eq!(
        direct.formatted,
        ["(10)", "(20 17 13 8)", "(0 1 3 6 10)", "(1.0)"]
    );
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
