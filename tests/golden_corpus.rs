use faraweave::{ErrorKind, evaluate_expression, format_value};

const CORPUS: &str = include_str!("fixtures/rewrite_evaluator_conformance_fixture.inc");

fn quoted_fields(line: &str) -> Vec<String> {
    let mut fields = Vec::new();
    let mut current = String::new();
    let mut quoted = false;
    let mut escaped = false;
    for character in line.chars() {
        if !quoted {
            if character == '"' {
                quoted = true;
                current.clear();
            }
            continue;
        }
        if escaped {
            current.push(match character {
                'n' => '\n',
                'r' => '\r',
                't' => '\t',
                other => other,
            });
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            quoted = false;
            fields.push(current.clone());
        } else {
            current.push(character);
        }
    }
    fields
}

#[test]
fn authored_section_15_and_16_success_golden_corpus() {
    let section = CORPUS
        .split("rewrite_evaluator_golden_fixtures[] = {")
        .nth(1)
        .and_then(|tail| tail.split("\n};").next())
        .expect("checked-in success section");
    let mut executed = 0;
    for line in section.lines().map(str::trim) {
        if !line.starts_with("{\"") {
            continue;
        }
        let fields = quoted_fields(line);
        assert_eq!(fields.len(), 4, "{line}");
        let value = evaluate_expression(&fields[2])
            .unwrap_or_else(|error| panic!("{} ({}) failed: {error:?}", fields[0], fields[2]));
        assert_eq!(
            format_value(&value.value).expect("golden formatting"),
            fields[3],
            "{} ({})",
            fields[0],
            fields[1]
        );
        executed += 1;
    }
    assert_eq!(executed, 43);
}

#[test]
fn authored_section_15_and_16_failure_golden_corpus() {
    let section = CORPUS
        .split("rewrite_evaluator_error_fixtures[] = {")
        .nth(1)
        .and_then(|tail| tail.split("\n};").next())
        .expect("checked-in failure section");
    let mut executed = 0;
    for line in section.lines().map(str::trim) {
        if !line.starts_with("{\"") {
            continue;
        }
        let fields = quoted_fields(line);
        assert_eq!(fields.len(), 3, "{line}");
        let expected_kind = if line.contains("ErrorKind::shape_mismatch") {
            ErrorKind::ShapeMismatch
        } else if line.contains("ErrorKind::type_mismatch") {
            ErrorKind::TypeError
        } else if line.contains("ErrorKind::arity_error") {
            ErrorKind::ArityError
        } else if line.contains("ErrorKind::domain_error") {
            ErrorKind::DomainError
        } else {
            panic!("unknown golden error kind: {line}");
        };
        let error = evaluate_expression(&fields[2]).expect_err(&fields[0]);
        assert_eq!(error.kind, expected_kind, "{} ({})", fields[0], fields[1]);

        let suffix = line.split("ErrorKind::").nth(1).expect("error suffix");
        let columns: Vec<&str> = suffix.split(',').map(str::trim).collect();
        if let Some(position) = columns.get(1).and_then(|value| value.strip_suffix('U')) {
            assert_eq!(
                error.argument_position,
                Some(position.parse().expect("argument position")),
                "{}",
                fields[0]
            );
        }
        if let Some(index) = columns
            .get(2)
            .and_then(|value| value.trim_end_matches("},").strip_suffix('U'))
        {
            assert_eq!(
                error
                    .domain
                    .as_ref()
                    .and_then(|context| context.element_index),
                Some(index.parse().expect("element index")),
                "{}",
                fields[0]
            );
        }
        executed += 1;
    }
    assert_eq!(executed, 19);
}
