use faraweave::{
    AllocationFailureInjection, ArgumentErrorReason, DomainErrorReason, Error, ErrorKind,
    EvaluationConfiguration, ExecutionProfile, ParameterErrorReason, ResourceErrorReason,
    ResourceLimits, Value, emit_c_source_with_configuration, evaluate_expression,
    evaluate_expression_with_configuration, evaluate_source, evaluate_source_with_arguments,
    evaluate_source_with_configuration,
};
use std::sync::Mutex;

#[derive(Clone, Debug, Eq, PartialEq)]
struct ObservedResourceEvent {
    kind: faraweave::ResourceEventKind,
    producer: String,
    bytes: Option<usize>,
    work: usize,
    ordinal: Option<usize>,
    refusal: Option<ResourceErrorReason>,
    usage: faraweave::ResourceUsage,
}

static RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static LENGTH_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static SORT_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static SUM_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());

fn observe_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = RESOURCE_EVENTS.lock() {
        events.push(ObservedResourceEvent {
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

fn observe_length_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = LENGTH_RESOURCE_EVENTS.lock() {
        events.push(ObservedResourceEvent {
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

fn observe_sort_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = SORT_RESOURCE_EVENTS.lock() {
        events.push(ObservedResourceEvent {
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

fn observe_sum_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = SUM_RESOURCE_EVENTS.lock() {
        events.push(ObservedResourceEvent {
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

fn bounded(limits: ResourceLimits) -> EvaluationConfiguration {
    EvaluationConfiguration {
        profile: ExecutionProfile::BoundedV2,
        limits,
        allocation_failure: AllocationFailureInjection::default(),
    }
}

fn resource(error: &Error) -> &faraweave::ResourceErrorContext {
    error.resource.as_ref().expect("structured resource error")
}

#[test]
fn profile_configuration_precedes_source_and_backend_analysis() {
    let invalid = EvaluationConfiguration {
        profile: ExecutionProfile::TrustedLocalV2,
        limits: ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        },
        allocation_failure: AllocationFailureInjection::default(),
    };
    for error in [
        evaluate_expression_with_configuration("@", invalid).expect_err("expression profile"),
        evaluate_source_with_configuration("@", invalid).expect_err("program profile"),
        emit_c_source_with_configuration("@", invalid).expect_err("emitter profile"),
    ] {
        assert_eq!(error.kind, ErrorKind::InvalidExecutionProfile);
    }
}

#[test]
fn vector_tuple_and_work_limits_cover_zero_exact_and_one_past() {
    let exact_vector = evaluate_expression_with_configuration(
        "iota[2]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(16),
            ..ResourceLimits::default()
        }),
    )
    .expect("exact vector limit");
    assert_eq!(exact_vector.usage.live_evaluation_bytes, 16);
    assert_eq!(exact_vector.usage.work_units, 2);
    assert_eq!(exact_vector.usage.allocation_attempts, 1);

    let vector_refused = evaluate_expression_with_configuration(
        "iota[2]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(15),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("one byte past vector limit");
    let context = resource(&vector_refused);
    assert_eq!(context.reason, ResourceErrorReason::ProfileLimit);
    assert_eq!(context.limit_kind, Some("max_vector_bytes"));
    assert_eq!(context.configured_limit, Some(15));
    assert_eq!(context.usage_before, Some(0));
    assert_eq!(context.refused_charge, Some(16));
    assert_eq!(context.requested_elements, Some(2));
    assert_eq!(context.requested_bytes, Some(16));

    let exact_tuple = evaluate_expression_with_configuration(
        "[1 2]",
        bounded(ResourceLimits {
            max_tuple_table_bytes: Some(32),
            ..ResourceLimits::default()
        }),
    )
    .expect("exact tuple-table limit");
    assert_eq!(exact_tuple.usage.live_evaluation_bytes, 32);
    assert_eq!(exact_tuple.usage.work_units, 0);
    assert_eq!(exact_tuple.usage.allocation_attempts, 1);

    let tuple_refused = evaluate_expression_with_configuration(
        "[1 2]",
        bounded(ResourceLimits {
            max_tuple_table_bytes: Some(31),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("one byte past tuple-table limit");
    let context = resource(&tuple_refused);
    assert_eq!(context.limit_kind, Some("max_tuple_table_bytes"));
    assert_eq!(context.usage_before, Some(0));
    assert_eq!(context.refused_charge, Some(32));

    let zero_work = evaluate_expression_with_configuration(
        "iota[1]",
        bounded(ResourceLimits {
            max_work_units: Some(0),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("positive work at zero limit");
    let context = resource(&zero_work);
    assert_eq!(context.limit_kind, Some("max_work_units"));
    assert_eq!(context.usage_before, Some(0));
    assert_eq!(context.refused_charge, Some(1));
    assert_eq!(context.allocation_ordinal, None);

    let exact_work = evaluate_expression_with_configuration(
        "inc[1]",
        bounded(ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        }),
    )
    .expect("one scalar work unit");
    assert_eq!(exact_work.usage.work_units, 1);
    assert_eq!(exact_work.usage.allocation_attempts, 0);
}

#[test]
fn refusal_precedence_is_vector_then_live_then_work_then_allocation() {
    let vector = evaluate_expression_with_configuration(
        "iota[1]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(0),
            max_live_evaluation_bytes: Some(0),
            max_work_units: Some(0),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("simultaneous refusal");
    assert_eq!(resource(&vector).limit_kind, Some("max_vector_bytes"));

    let live = evaluate_expression_with_configuration(
        "iota[1]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(8),
            max_live_evaluation_bytes: Some(0),
            max_work_units: Some(0),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("live precedes work");
    assert_eq!(
        resource(&live).limit_kind,
        Some("max_live_evaluation_bytes")
    );

    let work = evaluate_expression_with_configuration(
        "iota[1]",
        EvaluationConfiguration {
            profile: ExecutionProfile::BoundedV2,
            limits: ResourceLimits {
                max_vector_bytes: Some(8),
                max_live_evaluation_bytes: Some(8),
                max_work_units: Some(0),
                ..ResourceLimits::default()
            },
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
        },
    )
    .expect_err("work precedes injected allocation");
    assert_eq!(resource(&work).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&work).allocation_ordinal, None);

    let allocation = evaluate_expression_with_configuration(
        "iota[1]",
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV2,
            limits: ResourceLimits::default(),
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
        },
    )
    .expect_err("first zero-based allocation ordinal");
    assert_eq!(
        resource(&allocation).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(resource(&allocation).allocation_ordinal, Some(0));
}

#[test]
fn tuple_allocation_ordinals_exclude_empty_tables_and_cleanup_failures() {
    let success = evaluate_expression("[[1] [2 3]]").expect("three tuple tables");
    assert_eq!(success.usage.allocation_attempts, 3);
    let empty = evaluate_expression("[[] [1]]").expect("empty table has no ordinal");
    assert_eq!(empty.usage.allocation_attempts, 2);

    for ordinal in 0..3 {
        let error = evaluate_expression_with_configuration(
            "[[1] [2 3]]",
            EvaluationConfiguration {
                profile: ExecutionProfile::TrustedLocalV2,
                limits: ResourceLimits::default(),
                allocation_failure: AllocationFailureInjection {
                    fail_at_ordinal: Some(ordinal),
                },
            },
        )
        .expect_err("injected tuple-table failure");
        assert_eq!(resource(&error).allocation_ordinal, Some(ordinal));
        assert_eq!(error.primitive.as_deref(), Some("tuple_literal"));
    }
}

#[test]
fn live_limit_observes_children_before_outer_tuple_admission() {
    let error = evaluate_expression_with_configuration(
        "[[1] [2]]",
        bounded(ResourceLimits {
            max_live_evaluation_bytes: Some(47),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("outer table sees both live children");
    let context = resource(&error);
    assert_eq!(context.limit_kind, Some("max_live_evaluation_bytes"));
    assert_eq!(context.usage_before, Some(32));
    assert_eq!(context.refused_charge, Some(32));
}

#[test]
fn v1_tuple_refusal_precedes_type_and_runtime_work() {
    let error = evaluate_expression_with_configuration(
        "add[[true] 1]",
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV1,
            limits: ResourceLimits::default(),
            allocation_failure: AllocationFailureInjection::default(),
        },
    )
    .expect_err("tuple profile");
    assert_eq!(error.kind, ErrorKind::ProfileError);
}

#[test]
fn typed_api_rejects_noncanonical_nan_without_normalizing_it() {
    let spelling = "parameters[x Double]\nx\n";
    let noncanonical = Value::Double(f64::from_bits(0x7ff0_0000_0000_0001));
    let error = evaluate_source_with_arguments(
        spelling,
        &[noncanonical],
        EvaluationConfiguration::default(),
    )
    .expect_err("stored signaling NaN");
    assert_eq!(error.kind, ErrorKind::ArgumentError);
    let context = error.argument.expect("argument context");
    assert_eq!(context.reason, ArgumentErrorReason::InvalidTypedValue);
    assert_eq!(context.actual_container, Some("scalar"));
    assert_eq!(context.actual_type, Some(faraweave::ScalarType::Double));
    assert_eq!(context.invalid_value_invariant, Some("noncanonical_nan"));

    let accepted = evaluate_source_with_arguments(
        spelling,
        &[Value::Double(f64::from_bits(0x7ff8_0000_0000_0000))],
        EvaluationConfiguration::default(),
    )
    .expect("canonical NaN");
    assert_eq!(accepted.values[0].as_double_bits(), 0x7ff8_0000_0000_0000);
}

#[test]
fn parameter_header_reason_and_span_contract_is_structured() {
    let fixtures = [
        (
            "parameters",
            ParameterErrorReason::ExpectedHeaderOpen,
            11,
            11,
        ),
        (
            "parameters[n]",
            ParameterErrorReason::ExpectedParameterType,
            13,
            13,
        ),
        (
            "parameters[n Int",
            ParameterErrorReason::MissingHeaderClose,
            17,
            17,
        ),
        (
            "parameters[]x",
            ParameterErrorReason::TrailingHeaderBytes,
            13,
            14,
        ),
        (
            "1\nparameters[]",
            ParameterErrorReason::ParameterHeaderAfterRoot,
            3,
            13,
        ),
    ];
    for (source, reason, begin, end) in fixtures {
        let error = evaluate_source(source).expect_err(source);
        assert_eq!(error.kind, ErrorKind::SyntaxError, "{source}");
        let context = error.parameter.expect("parameter context");
        assert_eq!(context.reason, reason, "{source}");
        assert_eq!(context.primary_span.begin.offset, begin, "{source}");
        assert_eq!(context.primary_span.end.offset, end, "{source}");
    }

    let duplicate =
        evaluate_source("parameters[n Int n Bool]\nn\n").expect_err("duplicate declaration");
    assert_eq!(duplicate.kind, ErrorKind::ParameterError);
    let context = duplicate.parameter.expect("duplicate context");
    assert_eq!(context.reason, ParameterErrorReason::DuplicateParameterName);
    assert_eq!(context.primary_span.begin.offset, 18);
    assert_eq!(
        context
            .related_span
            .expect("first declaration")
            .begin
            .offset,
        12
    );

    let expression =
        evaluate_expression("parameters[inc Int]\n1").expect_err("program-only header");
    assert_eq!(expression.kind, ErrorKind::SyntaxError);
    assert_eq!(
        expression.parameter.expect("surface context").reason,
        ParameterErrorReason::ProgramOnlyParameterHeader
    );
}

#[test]
fn generated_runtime_embeds_profile_and_verified_primitive_selection() {
    let configuration = EvaluationConfiguration {
        profile: ExecutionProfile::BoundedV2,
        limits: ResourceLimits {
            max_vector_bytes: Some(8),
            max_tuple_table_bytes: Some(16),
            max_live_evaluation_bytes: Some(24),
            max_work_units: Some(1),
        },
        allocation_failure: AllocationFailureInjection {
            fail_at_ordinal: Some(0),
        },
    };
    let emitted = emit_c_source_with_configuration("parameters[n Int]\ninc[n]\n", configuration)
        .expect("parameterized C");
    assert!(emitted.source.contains("const int fw_profile = 3;"));
    assert!(
        emitted
            .source
            .contains("const size_t fw_vector_limit = 8U;")
    );
    assert!(
        emitted
            .source
            .contains("const size_t fw_failure_ordinal = 0U;")
    );
    assert!(emitted.source.contains("static int fw_kernel_1("));
    assert!(emitted.source.contains("fw_impl_1(args, 1U"));
    assert!(!emitted.source.contains("static int fw_apply("));
    assert!(!emitted.source.contains("fw_apply_scalar"));
    assert!(emitted.source.contains("(void)fw_make_tuple;"));
    assert!(emitted.source.contains("setvbuf(stdout,NULL,_IONBF,0)"));
    assert!(!emitted.source.contains("strcmp(name"));
    assert!(!emitted.source.contains("fw_format(buffer"));
    assert!(!emitted.source.contains("fw_free(&value->items"));
}

#[test]
fn failure_usage_is_post_cleanup_and_work_remains_monotonic() {
    let error = evaluate_source(
        "iota[2]\n\
         inc[9223372036854775807]\n",
    )
    .expect_err("later root failure");
    assert_eq!(error.kind, ErrorKind::DomainError);
    let usage = error.usage.expect("failure usage snapshot");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 3);
    assert_eq!(usage.allocation_attempts, 1);

    let lifted =
        evaluate_expression("inc[(9223372036854775807)]").expect_err("lifted failure cleanup");
    let usage = lifted.usage.expect("lifted usage snapshot");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 1);
    assert_eq!(usage.allocation_attempts, 2);
}

#[test]
fn div_admission_precedes_domain_and_failure_cleanup_is_exact() {
    let scalar = evaluate_expression("div[1 0]").expect_err("scalar division domain");
    assert_eq!(scalar.kind, ErrorKind::DomainError);
    assert_eq!(
        scalar.domain.as_ref().map(|context| context.reason),
        Some(DomainErrorReason::DivisionByZero)
    );
    assert_eq!(
        scalar.usage,
        Some(faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 0,
            work_units: 1,
            allocation_attempts: 0,
        })
    );

    let lifted = evaluate_expression("div[(8 9) (2 0)]").expect_err("lifted division domain");
    assert_eq!(lifted.kind, ErrorKind::DomainError);
    assert_eq!(
        lifted
            .domain
            .as_ref()
            .and_then(|context| context.element_index),
        Some(1)
    );
    assert_eq!(
        lifted.usage,
        Some(faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 48,
            work_units: 2,
            allocation_attempts: 3,
        })
    );

    let work = evaluate_expression_with_configuration(
        "div[(8 9) (2 0)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(64),
            max_live_evaluation_bytes: Some(64),
            max_work_units: Some(1),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("work refusal precedes division domain");
    assert_eq!(work.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&work).limit_kind, Some("max_work_units"));
    assert!(work.domain.is_none());

    let allocation = evaluate_expression_with_configuration(
        "div[(8 9) (2 0)]",
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV2,
            limits: ResourceLimits::default(),
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(2),
            },
        },
    )
    .expect_err("result allocation refusal precedes division domain");
    assert_eq!(allocation.kind, ErrorKind::ResourceError);
    assert_eq!(
        resource(&allocation).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(resource(&allocation).allocation_ordinal, Some(2));
    assert!(allocation.domain.is_none());
}

#[test]
fn length_charges_constant_work_borrows_input_and_has_no_result_allocation() {
    LENGTH_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let result = faraweave::evaluate_expression_with_observer(
        "length[(1 2 3)]",
        EvaluationConfiguration::default(),
        observe_length_resource_event,
    )
    .expect("vector length");
    assert_eq!(result.value, Value::Int(3));
    assert_eq!(
        result.usage,
        faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 24,
            work_units: 1,
            allocation_attempts: 1,
        }
    );
    let events = LENGTH_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[0].bytes, Some(24));
    assert_eq!(events[0].ordinal, Some(0));
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].producer, "length");
    assert_eq!(events[1].bytes, None);
    assert_eq!(events[1].work, 1);
    assert_eq!(events[1].ordinal, None);
    assert_eq!(events[1].usage.live_evaluation_bytes, 24);
    assert_eq!(events[1].usage.work_units, 1);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 0);
    assert_eq!(events[2].usage.work_units, 1);

    let no_result_allocation = evaluate_expression_with_configuration(
        "length[(1 2 3)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("length performs no second allocation");
    assert_eq!(no_result_allocation.value, Value::Int(3));
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let input_refusal = evaluate_expression_with_configuration(
        "length[(1 2 3)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect_err("input allocation refusal");
    assert_eq!(input_refusal.kind, ErrorKind::ResourceError);
    assert_eq!(
        resource(&input_refusal).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(input_refusal.usage.expect("refusal usage").work_units, 0);

    let work_refusal = evaluate_expression_with_configuration(
        "length[(1 2 3)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(24),
            max_work_units: Some(0),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("constant work refusal");
    assert_eq!(work_refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&work_refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&work_refusal).refused_charge, Some(1));
    let usage = work_refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn sort_admits_owned_output_with_input_live_and_cleans_up_refused_output() {
    SORT_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let result = faraweave::evaluate_expression_with_observer(
        "sort[(4 1 3 2)]",
        EvaluationConfiguration::default(),
        observe_sort_resource_event,
    )
    .expect("vector sort");
    assert_eq!(result.value, Value::IntVector(vec![1, 2, 3, 4]));
    assert_eq!(
        result.usage,
        faraweave::ResourceUsage {
            live_evaluation_bytes: 32,
            peak_live_evaluation_bytes: 64,
            work_units: 4,
            allocation_attempts: 2,
        }
    );
    let events = SORT_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[0].bytes, Some(32));
    assert_eq!(events[0].ordinal, Some(0));
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].producer, "sort");
    assert_eq!(events[1].bytes, Some(32));
    assert_eq!(events[1].work, 4);
    assert_eq!(events[1].ordinal, Some(1));
    assert_eq!(events[1].usage.live_evaluation_bytes, 64);
    assert_eq!(events[1].usage.peak_live_evaluation_bytes, 64);
    assert_eq!(events[1].usage.work_units, 4);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 32);

    SORT_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let refusal = faraweave::evaluate_expression_with_observer(
        "sort[(4 1 3 2)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
        observe_sort_resource_event,
    )
    .expect_err("sort output allocation refusal");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(
        resource(&refusal).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(resource(&refusal).allocation_ordinal, Some(1));
    let events = SORT_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Refusal);
    assert_eq!(events[1].producer, "sort");
    assert_eq!(events[1].bytes, Some(32));
    assert_eq!(events[1].work, 4);
    assert_eq!(
        events[1].refusal,
        Some(ResourceErrorReason::AllocationUnavailable)
    );
    assert_eq!(events[1].usage.live_evaluation_bytes, 32);
    assert_eq!(events[1].usage.work_units, 0);
    assert_eq!(events[1].usage.allocation_attempts, 2);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 0);
    assert_eq!(refusal.usage.expect("post-cleanup usage"), events[2].usage);

    let empty = evaluate_expression_with_configuration(
        "sort[Int()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty input and output have no allocation ordinal");
    assert_eq!(empty.value, Value::IntVector(Vec::new()));
    assert_eq!(empty.usage.live_evaluation_bytes, 0);
    assert_eq!(empty.usage.peak_live_evaluation_bytes, 0);
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);
}

#[test]
fn sort_large_bounded_input_has_linear_semantic_work_and_exact_limit_seam() {
    let success = evaluate_expression_with_configuration(
        "sort iota 4096",
        bounded(ResourceLimits {
            max_vector_bytes: Some(32_768),
            max_live_evaluation_bytes: Some(65_536),
            max_work_units: Some(8_192),
            ..ResourceLimits::default()
        }),
    )
    .expect("bounded large sort");
    assert_eq!(success.value.len(), 4_096);
    assert_eq!(success.usage.peak_live_evaluation_bytes, 65_536);
    assert_eq!(success.usage.work_units, 8_192);
    assert_eq!(success.usage.allocation_attempts, 2);

    let refusal = evaluate_expression_with_configuration(
        "sort iota 4096",
        bounded(ResourceLimits {
            max_vector_bytes: Some(32_768),
            max_live_evaluation_bytes: Some(65_536),
            max_work_units: Some(8_191),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("one work unit below sort schedule");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(4_096));
    assert_eq!(resource(&refusal).refused_charge, Some(4_096));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 4_096);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn sum_charges_full_work_before_reduction_and_allocates_no_result() {
    SUM_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let result = faraweave::evaluate_expression_with_observer(
        "sum[(1 2 3)]",
        EvaluationConfiguration::default(),
        observe_sum_resource_event,
    )
    .expect("vector sum");
    assert_eq!(result.value, Value::Int(6));
    assert_eq!(
        result.usage,
        faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 24,
            work_units: 3,
            allocation_attempts: 1,
        }
    );
    let events = SUM_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[0].bytes, Some(24));
    assert_eq!(events[0].ordinal, Some(0));
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].producer, "sum");
    assert_eq!(events[1].bytes, None);
    assert_eq!(events[1].work, 3);
    assert_eq!(events[1].ordinal, None);
    assert_eq!(events[1].usage.live_evaluation_bytes, 24);
    assert_eq!(events[1].usage.work_units, 3);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 0);
    assert_eq!(events[2].usage.work_units, 3);

    let no_result_allocation = evaluate_expression_with_configuration(
        "sum[(1 2 3)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("sum performs no result allocation");
    assert_eq!(no_result_allocation.value, Value::Int(6));
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let empty = evaluate_expression_with_configuration(
        "sum[Double()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty sum has no allocation ordinal");
    assert_eq!(empty.value, Value::Double(0.0));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    let work_refusal = evaluate_expression_with_configuration(
        "sum[(1 2 3)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(24),
            max_work_units: Some(2),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("full sum work refusal");
    assert_eq!(work_refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&work_refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&work_refusal).refused_charge, Some(3));
    let usage = work_refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn sum_overflow_retains_admitted_work_and_large_dynamic_limits_are_exact() {
    let overflow = evaluate_expression_with_configuration(
        "sum[(9223372036854775807 1 -1)]",
        EvaluationConfiguration::default(),
    )
    .expect_err("sum overflow");
    assert_eq!(overflow.kind, ErrorKind::DomainError);
    assert_eq!(
        overflow.message,
        "sum failed: integer_overflow at result index 1"
    );
    let usage = overflow.usage.expect("overflow cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 3);
    assert_eq!(usage.allocation_attempts, 1);

    let success = evaluate_expression_with_configuration(
        "sum iota 4096",
        bounded(ResourceLimits {
            max_vector_bytes: Some(32_768),
            max_live_evaluation_bytes: Some(32_768),
            max_work_units: Some(8_192),
            ..ResourceLimits::default()
        }),
    )
    .expect("bounded large sum");
    assert_eq!(success.value, Value::Int(8_390_656));
    assert_eq!(success.usage.live_evaluation_bytes, 0);
    assert_eq!(success.usage.peak_live_evaluation_bytes, 32_768);
    assert_eq!(success.usage.work_units, 8_192);
    assert_eq!(success.usage.allocation_attempts, 1);

    let refusal = evaluate_expression_with_configuration(
        "sum iota 4096",
        bounded(ResourceLimits {
            max_vector_bytes: Some(32_768),
            max_live_evaluation_bytes: Some(32_768),
            max_work_units: Some(8_191),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("one work unit below sum schedule");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(4_096));
    assert_eq!(resource(&refusal).refused_charge, Some(4_096));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 4_096);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn resource_observer_reports_commit_refusal_and_cleanup_order() {
    RESOURCE_EVENTS.lock().expect("event lock").clear();
    let configuration = EvaluationConfiguration {
        allocation_failure: AllocationFailureInjection {
            fail_at_ordinal: Some(1),
        },
        ..EvaluationConfiguration::default()
    };
    let error = faraweave::evaluate_expression_with_observer(
        "inc[(1 2)]",
        configuration,
        observe_resource_event,
    )
    .expect_err("second allocation refusal");
    assert_eq!(error.kind, ErrorKind::ResourceError);

    let events = RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(
        events[0],
        ObservedResourceEvent {
            kind: faraweave::ResourceEventKind::Admission,
            producer: "vector_literal".to_owned(),
            bytes: Some(16),
            work: 0,
            ordinal: Some(0),
            refusal: None,
            usage: faraweave::ResourceUsage {
                live_evaluation_bytes: 16,
                peak_live_evaluation_bytes: 16,
                work_units: 0,
                allocation_attempts: 1,
            },
        }
    );
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Refusal);
    assert_eq!(events[1].producer, "inc");
    assert_eq!(events[1].ordinal, Some(1));
    assert_eq!(
        events[1].refusal,
        Some(ResourceErrorReason::AllocationUnavailable)
    );
    assert_eq!(events[1].usage.allocation_attempts, 2);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 0);
    assert_eq!(error.usage.expect("post-cleanup usage"), events[2].usage);
}

trait DoubleBits {
    fn as_double_bits(&self) -> u64;
}

impl DoubleBits for Value {
    fn as_double_bits(&self) -> u64 {
        match self {
            Self::Double(value) => value.to_bits(),
            other => panic!("expected Double, got {other:?}"),
        }
    }
}
