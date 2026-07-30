use faraweave::{
    AllocationFailureInjection, ArgumentErrorReason, DomainErrorReason, Error, ErrorKind,
    EvaluationConfiguration, ExecutionProfile, ParameterErrorReason, ResourceErrorReason,
    ResourceLimits, Value, compile_source_to_verified_program, evaluate_expression,
    evaluate_expression_with_configuration, evaluate_expression_with_observer, evaluate_source,
    evaluate_source_with_arguments, evaluate_source_with_configuration, evaluate_verified_program,
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
static ALL_OF_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static ANY_OF_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static NONE_OF_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static FOLDL_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static SCANL_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static SCANL_FAULT_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static FILTER_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static FILTER_REFUSAL_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static CONNECTED_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> = Mutex::new(Vec::new());
static CONNECTED_BINDING_RESOURCE_EVENTS: Mutex<Vec<ObservedResourceEvent>> =
    Mutex::new(Vec::new());

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

fn observe_all_of_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = ALL_OF_RESOURCE_EVENTS.lock() {
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

fn observe_any_of_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = ANY_OF_RESOURCE_EVENTS.lock() {
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

fn observe_none_of_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = NONE_OF_RESOURCE_EVENTS.lock() {
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

fn observe_foldl_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = FOLDL_RESOURCE_EVENTS.lock() {
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

fn observe_scanl_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = SCANL_RESOURCE_EVENTS.lock() {
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

fn observe_scanl_fault_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = SCANL_FAULT_RESOURCE_EVENTS.lock() {
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

fn observe_filter_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = FILTER_RESOURCE_EVENTS.lock() {
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

fn observe_filter_refusal_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = FILTER_REFUSAL_RESOURCE_EVENTS.lock() {
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

fn observe_connected_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = CONNECTED_RESOURCE_EVENTS.lock() {
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
fn profile_configuration_precedes_source_analysis_and_interpreter_execution() {
    let invalid = EvaluationConfiguration {
        profile: ExecutionProfile::TrustedLocalV2,
        limits: ResourceLimits {
            max_work_units: Some(1),
            ..ResourceLimits::default()
        },
        allocation_failure: AllocationFailureInjection::default(),
    };
    let program =
        compile_source_to_verified_program("inc[1]\n", "profile.faraweave").expect("valid program");
    for error in [
        evaluate_expression_with_configuration("@", invalid).expect_err("expression profile"),
        evaluate_source_with_configuration("@", invalid).expect_err("program profile"),
        evaluate_verified_program(&program, &[], invalid).expect_err("interpreter profile"),
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
fn connected_completion_preserves_template_first_operand_once_resource_order() {
    let configuration = EvaluationConfiguration::default();
    let sources = [
        "add[(1 2) (3 4)]",
        "add[(1 2)] (3 4)",
        "add[] [(1 2) (3 4)]",
    ];
    let mut outcomes = Vec::new();
    for source in sources {
        CONNECTED_RESOURCE_EVENTS
            .lock()
            .expect("event lock")
            .clear();
        let result = evaluate_expression_with_observer(
            source,
            configuration,
            observe_connected_resource_event,
        )
        .expect(source);
        outcomes.push((
            result,
            CONNECTED_RESOURCE_EVENTS
                .lock()
                .expect("event lock")
                .clone(),
        ));
    }
    assert_eq!(outcomes[0], outcomes[1]);
    assert_eq!(outcomes[0], outcomes[2]);
    assert_eq!(outcomes[0].0.value, Value::IntVector(vec![4, 6]));
    assert_eq!(outcomes[0].0.usage.allocation_attempts, 3);
    assert_eq!(outcomes[0].0.usage.work_units, 2);

    let template_failure =
        evaluate_expression("add[div[1 0]] iota[3]").expect_err("template fails first");
    assert_eq!(template_failure.kind, ErrorKind::DomainError);
    assert_eq!(
        template_failure
            .domain
            .as_ref()
            .map(|context| context.reason),
        Some(DomainErrorReason::DivisionByZero)
    );
    assert_eq!(
        template_failure.usage,
        Some(faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 0,
            work_units: 1,
            allocation_attempts: 0,
        })
    );

    let v1 = evaluate_expression_with_configuration(
        "add[] [10 20]",
        EvaluationConfiguration {
            profile: ExecutionProfile::TrustedLocalV1,
            ..EvaluationConfiguration::default()
        },
    )
    .expect("erased authored tuple requires no tuple profile");
    assert_eq!(v1.value, Value::Int(30));
}

fn observe_connected_binding_resource_event(event: &faraweave::ResourceEvent<'_>) {
    if let Ok(mut events) = CONNECTED_BINDING_RESOURCE_EVENTS.lock() {
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

#[test]
fn connected_binding_is_template_first_operand_once_and_cleanup_exact() {
    CONNECTED_BINDING_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clear();
    let repeated = evaluate_expression_with_observer(
        "mul[_1 _1] (2 3)",
        EvaluationConfiguration::default(),
        observe_connected_binding_resource_event,
    )
    .expect("repeated binding");
    assert_eq!(repeated.value, Value::IntVector(vec![4, 9]));
    assert_eq!(repeated.usage.allocation_attempts, 2);
    assert_eq!(repeated.usage.work_units, 2);
    let events = CONNECTED_BINDING_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clone();
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind == faraweave::ResourceEventKind::Admission
                    && event.producer == "vector_literal"
            })
            .count(),
        1
    );

    let template_failure =
        evaluate_expression("add[div[1 0] _] iota[3]").expect_err("template fails first");
    assert_eq!(template_failure.kind, ErrorKind::DomainError);
    assert_eq!(
        template_failure.usage,
        Some(faraweave::ResourceUsage {
            live_evaluation_bytes: 0,
            peak_live_evaluation_bytes: 0,
            work_units: 1,
            allocation_attempts: 0,
        })
    );

    CONNECTED_BINDING_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clear();
    let refusal = evaluate_expression_with_observer(
        "mul[_1 _1] (2 3)",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
        observe_connected_binding_resource_event,
    )
    .expect_err("result allocation refusal");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.allocation_attempts, 2);
    let events = CONNECTED_BINDING_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clone();
    assert_eq!(
        events
            .iter()
            .filter(|event| {
                event.kind == faraweave::ResourceEventKind::Admission
                    && event.producer == "vector_literal"
            })
            .count(),
        1
    );
    assert_eq!(
        events
            .iter()
            .filter(|event| event.kind == faraweave::ResourceEventKind::Release)
            .count(),
        1
    );
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
fn all_of_work_and_observer_trace_are_independent_of_the_decisive_position() {
    let mut reference_events = None;
    for source in [
        "all_of[(true true true true)]",
        "all_of[(false true true true)]",
        "all_of[(true false true true)]",
        "all_of[(true true false true)]",
        "all_of[(true true true false)]",
    ] {
        ALL_OF_RESOURCE_EVENTS.lock().expect("event lock").clear();
        let result = faraweave::evaluate_expression_with_observer(
            source,
            EvaluationConfiguration::default(),
            observe_all_of_resource_event,
        )
        .expect("all_of reduction");
        assert_eq!(result.usage.work_units, 4, "{source}");
        assert_eq!(result.usage.allocation_attempts, 1, "{source}");
        let events = ALL_OF_RESOURCE_EVENTS.lock().expect("event lock").clone();
        assert_eq!(events.len(), 3, "{source}");
        assert_eq!(events[0].producer, "vector_literal", "{source}");
        assert_eq!(events[1].producer, "all_of", "{source}");
        assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
        assert_eq!(events[1].bytes, None);
        assert_eq!(events[1].work, 4);
        assert_eq!(events[1].ordinal, None);
        assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
        if let Some(reference) = &reference_events {
            assert_eq!(&events, reference, "{source}");
        } else {
            reference_events = Some(events);
        }
    }
}

#[test]
fn all_of_empty_allocation_and_work_refusal_precedence_are_exact() {
    let no_result_allocation = evaluate_expression_with_configuration(
        "all_of[(false true true)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("all_of performs no result allocation");
    assert_eq!(no_result_allocation.value, Value::Bool(false));
    assert_eq!(no_result_allocation.usage.work_units, 3);
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let empty = evaluate_expression_with_configuration(
        "all_of[Bool()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty all_of has no allocation ordinal");
    assert_eq!(empty.value, Value::Bool(true));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    let refusal = evaluate_expression_with_configuration(
        "all_of[(false true true)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(3),
            max_work_units: Some(2),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("full all_of work refusal");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(0));
    assert_eq!(resource(&refusal).refused_charge, Some(3));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn any_of_work_and_observer_trace_are_independent_of_the_decisive_position() {
    let mut reference_events = None;
    for source in [
        "any_of[(false false false false)]",
        "any_of[(true false false false)]",
        "any_of[(false true false false)]",
        "any_of[(false false true false)]",
        "any_of[(false false false true)]",
    ] {
        ANY_OF_RESOURCE_EVENTS.lock().expect("event lock").clear();
        let result = faraweave::evaluate_expression_with_observer(
            source,
            EvaluationConfiguration::default(),
            observe_any_of_resource_event,
        )
        .expect("any_of reduction");
        assert_eq!(result.usage.work_units, 4, "{source}");
        assert_eq!(result.usage.allocation_attempts, 1, "{source}");
        let events = ANY_OF_RESOURCE_EVENTS.lock().expect("event lock").clone();
        assert_eq!(events.len(), 3, "{source}");
        assert_eq!(events[0].producer, "vector_literal", "{source}");
        assert_eq!(events[1].producer, "any_of", "{source}");
        assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
        assert_eq!(events[1].bytes, None);
        assert_eq!(events[1].work, 4);
        assert_eq!(events[1].ordinal, None);
        assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
        if let Some(reference) = &reference_events {
            assert_eq!(&events, reference, "{source}");
        } else {
            reference_events = Some(events);
        }
    }
}

#[test]
fn any_of_empty_allocation_and_work_refusal_precedence_are_exact() {
    let no_result_allocation = evaluate_expression_with_configuration(
        "any_of[(true false false)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("any_of performs no result allocation");
    assert_eq!(no_result_allocation.value, Value::Bool(true));
    assert_eq!(no_result_allocation.usage.work_units, 3);
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let empty = evaluate_expression_with_configuration(
        "any_of[Bool()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty any_of has no allocation ordinal");
    assert_eq!(empty.value, Value::Bool(false));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    let refusal = evaluate_expression_with_configuration(
        "any_of[(true false false)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(3),
            max_work_units: Some(2),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("full any_of work refusal");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(0));
    assert_eq!(resource(&refusal).refused_charge, Some(3));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn none_of_work_and_observer_trace_use_its_identity_at_every_decisive_position() {
    let mut reference_events = None;
    for source in [
        "none_of[(false false false false)]",
        "none_of[(true false false false)]",
        "none_of[(false true false false)]",
        "none_of[(false false true false)]",
        "none_of[(false false false true)]",
    ] {
        NONE_OF_RESOURCE_EVENTS.lock().expect("event lock").clear();
        let result = faraweave::evaluate_expression_with_observer(
            source,
            EvaluationConfiguration::default(),
            observe_none_of_resource_event,
        )
        .expect("none_of reduction");
        assert_eq!(result.usage.work_units, 4, "{source}");
        assert_eq!(result.usage.allocation_attempts, 1, "{source}");
        let events = NONE_OF_RESOURCE_EVENTS.lock().expect("event lock").clone();
        assert_eq!(events.len(), 3, "{source}");
        assert_eq!(events[0].producer, "vector_literal", "{source}");
        assert_eq!(events[1].producer, "none_of", "{source}");
        assert_ne!(events[1].producer, "any_of", "{source}");
        assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
        assert_eq!(events[1].bytes, None);
        assert_eq!(events[1].work, 4);
        assert_eq!(events[1].ordinal, None);
        assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
        if let Some(reference) = &reference_events {
            assert_eq!(&events, reference, "{source}");
        } else {
            reference_events = Some(events);
        }
    }
}

#[test]
fn none_of_empty_allocation_and_work_refusal_precedence_are_exact() {
    let no_result_allocation = evaluate_expression_with_configuration(
        "none_of[(true false false)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("none_of performs no result allocation");
    assert_eq!(no_result_allocation.value, Value::Bool(false));
    assert_eq!(no_result_allocation.usage.work_units, 3);
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let empty = evaluate_expression_with_configuration(
        "none_of[Bool()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty none_of has no allocation ordinal");
    assert_eq!(empty.value, Value::Bool(true));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    let refusal = evaluate_expression_with_configuration(
        "none_of[(true false false)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(3),
            max_work_units: Some(2),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("full none_of work refusal");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(refusal.primitive.as_deref(), Some("none_of"));
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(0));
    assert_eq!(resource(&refusal).refused_charge, Some(3));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn filter_splits_work_and_exact_result_admission_with_input_live() {
    FILTER_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let result = faraweave::evaluate_expression_with_observer(
        "filter[@odd (1 2 3 4 5)]",
        EvaluationConfiguration::default(),
        observe_filter_resource_event,
    )
    .expect("mixed filter");
    assert_eq!(result.value, Value::IntVector(vec![1, 3, 5]));
    assert_eq!(result.usage.live_evaluation_bytes, 24);
    assert_eq!(result.usage.peak_live_evaluation_bytes, 64);
    assert_eq!(result.usage.work_units, 5);
    assert_eq!(result.usage.allocation_attempts, 2);
    let events = FILTER_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[0].bytes, Some(40));
    assert_eq!(events[0].ordinal, Some(0));
    assert_eq!(events[1].producer, "filter");
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].bytes, None);
    assert_eq!(events[1].work, 5);
    assert_eq!(events[1].ordinal, None);
    assert_eq!(events[1].usage.live_evaluation_bytes, 40);
    assert_eq!(events[1].usage.work_units, 5);
    assert_eq!(events[2].producer, "filter");
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[2].bytes, Some(24));
    assert_eq!(events[2].work, 0);
    assert_eq!(events[2].ordinal, Some(1));
    assert_eq!(events[2].usage.live_evaluation_bytes, 64);
    assert_eq!(events[3].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[3].bytes, Some(40));
    assert_eq!(events[3].usage.live_evaluation_bytes, 24);

    FILTER_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let empty = faraweave::evaluate_expression_with_observer(
        "filter[@odd Int()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
        observe_filter_resource_event,
    )
    .expect("empty filter has no allocation attempt");
    assert_eq!(empty.value, Value::IntVector(Vec::new()));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);
    let empty_events = FILTER_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(empty_events.len(), 3);
    assert!(empty_events.iter().all(|event| event.ordinal.is_none()));
    assert_eq!(
        empty_events
            .iter()
            .map(|event| (event.producer.as_str(), event.kind, event.bytes, event.work))
            .collect::<Vec<_>>(),
        [
            (
                "vector_literal",
                faraweave::ResourceEventKind::Admission,
                Some(0),
                0,
            ),
            ("filter", faraweave::ResourceEventKind::Admission, None, 0,),
            (
                "filter",
                faraweave::ResourceEventKind::Admission,
                Some(0),
                0,
            ),
        ]
    );

    let none_kept = evaluate_expression_with_configuration(
        "filter[@odd (2 4)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("zero-byte result has no output allocation ordinal");
    assert_eq!(none_kept.value, Value::IntVector(Vec::new()));
    assert_eq!(none_kept.usage.live_evaluation_bytes, 0);
    assert_eq!(none_kept.usage.peak_live_evaluation_bytes, 16);
    assert_eq!(none_kept.usage.work_units, 2);
    assert_eq!(none_kept.usage.allocation_attempts, 1);
}

#[test]
fn filter_refusals_preserve_phase_order_committed_work_and_cleanup() {
    let work = evaluate_expression_with_configuration(
        "filter[@odd (1 2 3 4 5)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(40),
            max_work_units: Some(4),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("work refusal precedes predicate inspection");
    assert_eq!(work.kind, ErrorKind::ResourceError);
    assert_eq!(work.primitive.as_deref(), Some("filter"));
    assert_eq!(resource(&work).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&work).refused_charge, Some(5));
    let work_usage = work.usage.expect("work refusal cleanup");
    assert_eq!(work_usage.live_evaluation_bytes, 0);
    assert_eq!(work_usage.work_units, 0);
    assert_eq!(work_usage.allocation_attempts, 1);

    FILTER_REFUSAL_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clear();
    let live = faraweave::evaluate_expression_with_observer(
        "filter[@odd (1 2 3 4 5)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(40),
            max_live_evaluation_bytes: Some(63),
            max_work_units: Some(5),
            ..ResourceLimits::default()
        }),
        observe_filter_refusal_resource_event,
    )
    .expect_err("exact result is refused only after discovery");
    assert_eq!(live.kind, ErrorKind::ResourceError);
    assert_eq!(live.primitive.as_deref(), Some("filter"));
    assert_eq!(
        resource(&live).limit_kind,
        Some("max_live_evaluation_bytes")
    );
    assert_eq!(resource(&live).requested_elements, Some(3));
    assert_eq!(resource(&live).requested_bytes, Some(24));
    assert_eq!(resource(&live).usage_before, Some(40));
    assert_eq!(resource(&live).refused_charge, Some(24));
    let live_usage = live.usage.expect("live refusal cleanup");
    assert_eq!(live_usage.live_evaluation_bytes, 0);
    assert_eq!(live_usage.peak_live_evaluation_bytes, 40);
    assert_eq!(live_usage.work_units, 5);
    assert_eq!(live_usage.allocation_attempts, 1);
    let live_events = FILTER_REFUSAL_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clone();
    assert_eq!(live_events.len(), 4);
    assert_eq!(live_events[1].producer, "filter");
    assert_eq!(live_events[1].work, 5);
    assert_eq!(live_events[2].kind, faraweave::ResourceEventKind::Refusal);
    assert_eq!(
        live_events[2].refusal,
        Some(ResourceErrorReason::ProfileLimit)
    );
    assert_eq!(live_events[2].bytes, Some(24));
    assert_eq!(live_events[2].work, 0);
    assert_eq!(live_events[3].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(live_events[3].bytes, Some(40));

    let allocation = evaluate_expression_with_configuration(
        "filter[@odd (1 2 3 4 5)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect_err("filter output allocation refusal");
    assert_eq!(allocation.kind, ErrorKind::ResourceError);
    assert_eq!(allocation.primitive.as_deref(), Some("filter"));
    assert_eq!(
        resource(&allocation).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(resource(&allocation).allocation_ordinal, Some(1));
    assert_eq!(resource(&allocation).requested_elements, Some(3));
    assert_eq!(resource(&allocation).requested_bytes, Some(24));
    let allocation_usage = allocation.usage.expect("allocation refusal cleanup");
    assert_eq!(allocation_usage.live_evaluation_bytes, 0);
    assert_eq!(allocation_usage.work_units, 5);
    assert_eq!(allocation_usage.allocation_attempts, 2);

    let exact = evaluate_expression_with_configuration(
        "filter[@odd (1 2 3 4 5)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(40),
            max_live_evaluation_bytes: Some(64),
            max_work_units: Some(5),
            ..ResourceLimits::default()
        }),
    )
    .expect("exact split limits");
    assert_eq!(exact.value, Value::IntVector(vec![1, 3, 5]));
    assert_eq!(exact.usage.peak_live_evaluation_bytes, 64);
}

#[test]
fn foldl_charges_full_work_before_reducer_steps_and_cleans_up_faults_exactly() {
    for source in [
        "foldl[@sub 20 (3 4 5)]",
        "foldl[@add 0 (3 4 5)]",
        "foldl[@div 60 (3 4 5)]",
    ] {
        FOLDL_RESOURCE_EVENTS.lock().expect("event lock").clear();
        let result = faraweave::evaluate_expression_with_observer(
            source,
            EvaluationConfiguration::default(),
            observe_foldl_resource_event,
        )
        .expect("foldl reduction");
        assert_eq!(result.usage.work_units, 3, "{source}");
        assert_eq!(result.usage.allocation_attempts, 1, "{source}");
        let events = FOLDL_RESOURCE_EVENTS.lock().expect("event lock").clone();
        assert_eq!(events.len(), 3, "{source}");
        assert_eq!(events[0].producer, "vector_literal", "{source}");
        assert_eq!(events[0].bytes, Some(24), "{source}");
        assert_eq!(events[1].producer, "foldl", "{source}");
        assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
        assert_eq!(events[1].bytes, None, "{source}");
        assert_eq!(events[1].work, 3, "{source}");
        assert_eq!(events[1].ordinal, None, "{source}");
        assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    }

    FOLDL_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let fault = faraweave::evaluate_expression_with_observer(
        "foldl[@div 10 (0 2)]",
        EvaluationConfiguration::default(),
        observe_foldl_resource_event,
    )
    .expect_err("first reducer step faults");
    assert_eq!(fault.kind, ErrorKind::DomainError);
    let usage = fault.usage.expect("post-cleanup foldl usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 2);
    assert_eq!(usage.allocation_attempts, 1);
    let events = FOLDL_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].producer, "foldl");
    assert_eq!(events[1].work, 2);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].usage.live_evaluation_bytes, 0);
}

#[test]
fn foldl_empty_allocation_and_work_refusal_precedence_are_exact() {
    let no_result_allocation = evaluate_expression_with_configuration(
        "foldl[@add 0 (1 2 3)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("foldl performs no result allocation");
    assert_eq!(no_result_allocation.value, Value::Int(6));
    assert_eq!(no_result_allocation.usage.work_units, 3);
    assert_eq!(no_result_allocation.usage.allocation_attempts, 1);

    let empty = evaluate_expression_with_configuration(
        "foldl[@div 7 Int()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(0),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty foldl has no allocation ordinal");
    assert_eq!(empty.value, Value::Int(7));
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 0);

    let refusal = evaluate_expression_with_configuration(
        "foldl[@div 10 (0)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(8),
            max_work_units: Some(0),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("foldl work refusal precedes reducer fault");
    assert_eq!(refusal.kind, ErrorKind::ResourceError);
    assert_eq!(refusal.primitive.as_deref(), Some("foldl"));
    assert_eq!(resource(&refusal).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&refusal).usage_before, Some(0));
    assert_eq!(resource(&refusal).refused_charge, Some(1));
    let usage = refusal.usage.expect("post-cleanup usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 1);
}

#[test]
fn scanl_admits_n_plus_one_output_before_population_with_input_live() {
    SCANL_RESOURCE_EVENTS.lock().expect("event lock").clear();
    let result = faraweave::evaluate_expression_with_observer(
        "scanl[@sub 20 (3 4 5)]",
        EvaluationConfiguration::default(),
        observe_scanl_resource_event,
    )
    .expect("scanl reduction");
    assert_eq!(result.value, Value::IntVector(vec![20, 17, 13, 8]));
    assert_eq!(result.usage.live_evaluation_bytes, 32);
    assert_eq!(result.usage.peak_live_evaluation_bytes, 56);
    assert_eq!(result.usage.work_units, 3);
    assert_eq!(result.usage.allocation_attempts, 2);
    let events = SCANL_RESOURCE_EVENTS.lock().expect("event lock").clone();
    assert_eq!(events.len(), 3);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[0].bytes, Some(24));
    assert_eq!(events[0].ordinal, Some(0));
    assert_eq!(events[1].producer, "scanl");
    assert_eq!(events[1].kind, faraweave::ResourceEventKind::Admission);
    assert_eq!(events[1].bytes, Some(32));
    assert_eq!(events[1].work, 3);
    assert_eq!(events[1].ordinal, Some(1));
    assert_eq!(events[1].usage.live_evaluation_bytes, 56);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].bytes, Some(24));
    assert_eq!(events[2].usage.live_evaluation_bytes, 32);

    let empty = evaluate_expression_with_configuration(
        "scanl[@add 7 Int()]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect("empty scan allocates only its one-element output");
    assert_eq!(empty.value, Value::IntVector(vec![7]));
    assert_eq!(empty.usage.live_evaluation_bytes, 8);
    assert_eq!(empty.usage.work_units, 0);
    assert_eq!(empty.usage.allocation_attempts, 1);
}

#[test]
fn scanl_fault_releases_output_before_input_and_retains_full_work() {
    SCANL_FAULT_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clear();
    let fault = faraweave::evaluate_expression_with_observer(
        "scanl[@div 10 (0 2)]",
        EvaluationConfiguration::default(),
        observe_scanl_fault_resource_event,
    )
    .expect_err("first reducer step faults after output allocation");
    assert_eq!(fault.kind, ErrorKind::DomainError);
    assert_eq!(
        fault
            .domain
            .as_ref()
            .and_then(|domain| domain.element_index),
        Some(0)
    );
    let usage = fault.usage.expect("post-cleanup scanl usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.peak_live_evaluation_bytes, 40);
    assert_eq!(usage.work_units, 2);
    assert_eq!(usage.allocation_attempts, 2);
    let events = SCANL_FAULT_RESOURCE_EVENTS
        .lock()
        .expect("event lock")
        .clone();
    assert_eq!(events.len(), 4);
    assert_eq!(events[0].producer, "vector_literal");
    assert_eq!(events[1].producer, "scanl");
    assert_eq!(events[1].bytes, Some(24));
    assert_eq!(events[1].work, 2);
    assert_eq!(events[2].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[2].bytes, Some(24));
    assert_eq!(events[2].usage.live_evaluation_bytes, 16);
    assert_eq!(events[3].kind, faraweave::ResourceEventKind::Release);
    assert_eq!(events[3].bytes, Some(16));
    assert_eq!(events[3].usage.live_evaluation_bytes, 0);
}

#[test]
fn scanl_output_allocation_and_work_refusals_precede_reducer_steps() {
    let allocation = evaluate_expression_with_configuration(
        "scanl[@div 10 (0 2)]",
        EvaluationConfiguration {
            allocation_failure: AllocationFailureInjection {
                fail_at_ordinal: Some(1),
            },
            ..EvaluationConfiguration::default()
        },
    )
    .expect_err("scan output allocation refusal");
    assert_eq!(allocation.kind, ErrorKind::ResourceError);
    assert_eq!(allocation.primitive.as_deref(), Some("scanl"));
    assert_eq!(
        resource(&allocation).reason,
        ResourceErrorReason::AllocationUnavailable
    );
    assert_eq!(resource(&allocation).requested_elements, Some(3));
    assert_eq!(resource(&allocation).requested_bytes, Some(24));
    let usage = allocation.usage.expect("post-cleanup allocation usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
    assert_eq!(usage.allocation_attempts, 2);

    let work = evaluate_expression_with_configuration(
        "scanl[@div 10 (0 2 3)]",
        bounded(ResourceLimits {
            max_vector_bytes: Some(32),
            max_work_units: Some(2),
            ..ResourceLimits::default()
        }),
    )
    .expect_err("full scan work refusal");
    assert_eq!(work.kind, ErrorKind::ResourceError);
    assert_eq!(work.primitive.as_deref(), Some("scanl"));
    assert_eq!(resource(&work).limit_kind, Some("max_work_units"));
    assert_eq!(resource(&work).usage_before, Some(0));
    assert_eq!(resource(&work).refused_charge, Some(3));
    let usage = work.usage.expect("post-cleanup work usage");
    assert_eq!(usage.live_evaluation_bytes, 0);
    assert_eq!(usage.work_units, 0);
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
