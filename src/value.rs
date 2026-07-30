use crate::{Error, ErrorKind, SourceLocation};
use std::cell::Cell;
use std::fmt::Write;
use std::ops::Deref;

thread_local! {
    static FORMAT_FAIL_AT: Cell<Option<usize>> = const { Cell::new(None) };
    static FORMAT_ALLOCATION_ORDINAL: Cell<usize> = const { Cell::new(0) };
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScalarType {
    Bool,
    Int,
    Double,
    String,
}

impl ScalarType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "Bool",
            Self::Int => "Int",
            Self::Double => "Double",
            Self::String => "String",
        }
    }

    pub(crate) const fn byte_width(self) -> usize {
        match self {
            Self::Bool => 1,
            Self::Int | Self::Double => 8,
            Self::String => 16,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Type {
    Scalar(ScalarType),
    Vector(ScalarType),
    Tuple(Vec<Type>),
    #[doc(hidden)]
    RepeatedTuple {
        depth: usize,
        leaf: ScalarType,
    },
}

#[derive(Clone, Debug, PartialEq)]
pub struct TupleValues {
    values: Vec<Value>,
}

impl TupleValues {
    pub fn new(values: Vec<Value>) -> Self {
        Self { values }
    }
}

impl From<Vec<Value>> for TupleValues {
    fn from(values: Vec<Value>) -> Self {
        Self::new(values)
    }
}

impl Deref for TupleValues {
    type Target = [Value];

    fn deref(&self) -> &Self::Target {
        &self.values
    }
}

impl Drop for TupleValues {
    fn drop(&mut self) {
        let mut pending = std::mem::take(&mut self.values);
        let mut current = None;
        loop {
            let Some(mut value) = current.take().or_else(|| pending.pop()) else {
                break;
            };
            if let Value::Tuple(tuple) = &mut value {
                let mut children = std::mem::take(&mut tuple.values);
                if let Some(child) = children.pop() {
                    if !pending.is_empty() {
                        children.push(Value::Tuple(TupleValues { values: pending }));
                    }
                    pending = children;
                    current = Some(child);
                }
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Bool(bool),
    Int(i64),
    Double(f64),
    String(String),
    BoolVector(Vec<bool>),
    IntVector(Vec<i64>),
    DoubleVector(Vec<f64>),
    StringVector(Vec<String>),
    Tuple(TupleValues),
}

impl Value {
    pub(crate) fn try_clone(&self) -> Result<Self, ()> {
        enum CloneTask<'a> {
            Value(&'a Value),
            FinishTuple(usize),
        }

        fn push_value(values: &mut Vec<Value>, value: Value) -> Result<(), ()> {
            values.try_reserve(1).map_err(|_| ())?;
            values.push(value);
            Ok(())
        }

        let mut pending = Vec::new();
        pending.try_reserve(1).map_err(|_| ())?;
        pending.push(CloneTask::Value(self));
        let mut cloned = Vec::new();
        while let Some(task) = pending.pop() {
            match task {
                CloneTask::Value(value) => match value {
                    Self::Bool(value) => push_value(&mut cloned, Self::Bool(*value))?,
                    Self::Int(value) => push_value(&mut cloned, Self::Int(*value))?,
                    Self::Double(value) => push_value(&mut cloned, Self::Double(*value))?,
                    Self::String(value) => {
                        let mut copy = String::new();
                        copy.try_reserve_exact(value.len()).map_err(|_| ())?;
                        copy.push_str(value);
                        push_value(&mut cloned, Self::String(copy))?;
                    }
                    Self::BoolVector(values) => {
                        let mut copy = Vec::new();
                        copy.try_reserve_exact(values.len()).map_err(|_| ())?;
                        copy.extend_from_slice(values);
                        push_value(&mut cloned, Self::BoolVector(copy))?;
                    }
                    Self::IntVector(values) => {
                        let mut copy = Vec::new();
                        copy.try_reserve_exact(values.len()).map_err(|_| ())?;
                        copy.extend_from_slice(values);
                        push_value(&mut cloned, Self::IntVector(copy))?;
                    }
                    Self::DoubleVector(values) => {
                        let mut copy = Vec::new();
                        copy.try_reserve_exact(values.len()).map_err(|_| ())?;
                        copy.extend_from_slice(values);
                        push_value(&mut cloned, Self::DoubleVector(copy))?;
                    }
                    Self::StringVector(values) => {
                        let mut copy = Vec::new();
                        copy.try_reserve_exact(values.len()).map_err(|_| ())?;
                        for value in values {
                            let mut string = String::new();
                            string.try_reserve_exact(value.len()).map_err(|_| ())?;
                            string.push_str(value);
                            copy.push(string);
                        }
                        push_value(&mut cloned, Self::StringVector(copy))?;
                    }
                    Self::Tuple(values) => {
                        pending.try_reserve(values.len() + 1).map_err(|_| ())?;
                        pending.push(CloneTask::FinishTuple(values.len()));
                        for value in values.iter().rev() {
                            pending.push(CloneTask::Value(value));
                        }
                    }
                },
                CloneTask::FinishTuple(length) => {
                    let start = cloned.len().checked_sub(length).ok_or(())?;
                    let mut values = Vec::new();
                    values.try_reserve_exact(length).map_err(|_| ())?;
                    values.extend(cloned.drain(start..));
                    push_value(&mut cloned, Self::Tuple(values.into()))?;
                }
            }
        }
        if cloned.len() == 1 {
            cloned.pop().ok_or(())
        } else {
            Err(())
        }
    }

    pub const fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Bool(_) | Self::BoolVector(_) => Some(ScalarType::Bool),
            Self::Int(_) | Self::IntVector(_) => Some(ScalarType::Int),
            Self::Double(_) | Self::DoubleVector(_) => Some(ScalarType::Double),
            Self::String(_) | Self::StringVector(_) => Some(ScalarType::String),
            Self::Tuple(_) => None,
        }
    }

    pub const fn is_scalar(&self) -> bool {
        matches!(
            self,
            Self::Bool(_) | Self::Int(_) | Self::Double(_) | Self::String(_)
        )
    }

    pub const fn is_vector(&self) -> bool {
        matches!(
            self,
            Self::BoolVector(_)
                | Self::IntVector(_)
                | Self::DoubleVector(_)
                | Self::StringVector(_)
        )
    }

    pub fn len(&self) -> usize {
        match self {
            Self::BoolVector(values) => values.len(),
            Self::IntVector(values) => values.len(),
            Self::DoubleVector(values) => values.len(),
            Self::StringVector(values) => values.len(),
            Self::Tuple(values) => values.len(),
            Self::Bool(_) | Self::Int(_) | Self::Double(_) | Self::String(_) => 1,
        }
    }

    pub const fn is_empty(&self) -> bool {
        match self {
            Self::BoolVector(values) => values.is_empty(),
            Self::IntVector(values) => values.is_empty(),
            Self::DoubleVector(values) => values.is_empty(),
            Self::StringVector(values) => values.is_empty(),
            Self::Tuple(values) => values.values.is_empty(),
            Self::Bool(_) | Self::Int(_) | Self::Double(_) | Self::String(_) => false,
        }
    }

    pub fn value_type(&self) -> Type {
        match self {
            Self::Bool(_) => Type::Scalar(ScalarType::Bool),
            Self::Int(_) => Type::Scalar(ScalarType::Int),
            Self::Double(_) => Type::Scalar(ScalarType::Double),
            Self::String(_) => Type::Scalar(ScalarType::String),
            Self::BoolVector(_) => Type::Vector(ScalarType::Bool),
            Self::IntVector(_) => Type::Vector(ScalarType::Int),
            Self::DoubleVector(_) => Type::Vector(ScalarType::Double),
            Self::StringVector(_) => Type::Vector(ScalarType::String),
            Self::Tuple(values) => Type::Tuple(values.iter().map(Self::value_type).collect()),
        }
    }

    pub(crate) fn into_canonical_bytes(self) -> Result<usize, Error> {
        let mut total = 0usize;
        let mut pending = Vec::new();
        let mut current = Some(self);
        loop {
            let Some(mut value) = current.take().or_else(|| pending.pop()) else {
                return Ok(total);
            };
            let charge = match &mut value {
                Self::Bool(_) | Self::Int(_) | Self::Double(_) => 0,
                Self::String(value) => value.len(),
                Self::BoolVector(values) => values.len(),
                Self::IntVector(values) => values.len().checked_mul(8).ok_or_else(sizing_error)?,
                Self::DoubleVector(values) => {
                    values.len().checked_mul(8).ok_or_else(sizing_error)?
                }
                Self::StringVector(values) => {
                    let descriptors = values.len().checked_mul(16).ok_or_else(sizing_error)?;
                    values.iter().try_fold(descriptors, |bytes, value| {
                        bytes.checked_add(value.len()).ok_or_else(sizing_error)
                    })?
                }
                Self::Tuple(tuple) => {
                    let charge = tuple
                        .values
                        .len()
                        .checked_mul(16)
                        .ok_or_else(sizing_error)?;
                    let mut children = std::mem::take(&mut tuple.values);
                    if let Some(child) = children.pop() {
                        if !pending.is_empty() {
                            children.push(Self::Tuple(TupleValues { values: pending }));
                        }
                        pending = children;
                        current = Some(child);
                    }
                    charge
                }
            };
            total = total.checked_add(charge).ok_or_else(sizing_error)?;
        }
    }
}

fn sizing_error() -> Error {
    Error::new(
        ErrorKind::ResourceError,
        SourceLocation::start(),
        "resource sizing failed: size_overflow",
    )
}

enum TypeFormatTask<'a> {
    Value(&'a Type),
    Text(&'static str),
}

pub fn format_type(value_type: &Type) -> String {
    let mut output = String::new();
    let mut pending = vec![TypeFormatTask::Value(value_type)];
    while let Some(task) = pending.pop() {
        match task {
            TypeFormatTask::Text(text) => output.push_str(text),
            TypeFormatTask::Value(Type::Scalar(scalar)) => output.push_str(scalar.name()),
            TypeFormatTask::Value(Type::Vector(scalar)) => {
                output.push_str("Vector<");
                output.push_str(scalar.name());
                output.push('>');
            }
            TypeFormatTask::Value(Type::Tuple(elements)) => {
                output.push_str("Tuple<");
                pending.push(TypeFormatTask::Text(">"));
                for (index, element) in elements.iter().enumerate().rev() {
                    pending.push(TypeFormatTask::Value(element));
                    if index != 0 {
                        pending.push(TypeFormatTask::Text(", "));
                    }
                }
            }
            TypeFormatTask::Value(Type::RepeatedTuple { depth, leaf }) => {
                for _ in 0..*depth {
                    output.push_str("Tuple<");
                }
                output.push_str(leaf.name());
                for _ in 0..*depth {
                    output.push('>');
                }
            }
        }
    }
    output
}

enum ValueFormatTask<'a> {
    Value(&'a Value),
    Text(&'static str),
}

pub fn format_value(value: &Value) -> Result<String, Error> {
    let required = formatted_value_length(value)?;
    let mut output = String::new();
    format_allocation_attempt()?;
    output
        .try_reserve_exact(required)
        .map_err(|_| formatting_allocation_error())?;
    write_value(value, &mut output)?;
    Ok(output)
}

fn formatted_value_length(value: &Value) -> Result<usize, Error> {
    let mut output = LengthWriter { length: 0 };
    write_value(value, &mut output)?;
    Ok(output.length)
}

pub(crate) fn format_template_placeholder_count(template: &str) -> Result<usize, Error> {
    let bytes = template.as_bytes();
    let mut index = 0usize;
    let mut placeholders = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'{' if bytes.get(index + 1) == Some(&b'{') => index += 2,
            b'}' if bytes.get(index + 1) == Some(&b'}') => index += 2,
            b'{' if bytes.get(index + 1) == Some(&b'}') => {
                placeholders = placeholders
                    .checked_add(1)
                    .ok_or_else(formatting_size_error)?;
                index += 2;
            }
            b'{' | b'}' => {
                return Err(Error::new(
                    ErrorKind::FormattingError,
                    SourceLocation::start(),
                    "malformed format template brace",
                ));
            }
            _ => index += 1,
        }
    }
    Ok(placeholders)
}

pub(crate) fn measure_interpolation(template: &str, arguments: &[&Value]) -> Result<usize, Error> {
    let mut output = LengthWriter { length: 0 };
    write_interpolation(template, arguments, &mut output)?;
    Ok(output.length)
}

pub(crate) fn render_interpolation(
    template: &str,
    arguments: &[&Value],
    output: &mut String,
) -> Result<(), Error> {
    write_interpolation(template, arguments, output)
}

fn write_interpolation(
    template: &str,
    arguments: &[&Value],
    output: &mut impl Write,
) -> Result<(), Error> {
    let bytes = template.as_bytes();
    let mut index = 0usize;
    let mut literal_start = 0usize;
    let mut argument = 0usize;
    while index < bytes.len() {
        let replacement = match (bytes[index], bytes.get(index + 1).copied()) {
            (b'{', Some(b'{')) => Some('{'),
            (b'}', Some(b'}')) => Some('}'),
            (b'{', Some(b'}')) => {
                output
                    .write_str(&template[literal_start..index])
                    .map_err(formatting_error)?;
                let value = arguments.get(argument).ok_or_else(|| {
                    Error::new(
                        ErrorKind::FormattingError,
                        SourceLocation::start(),
                        "format placeholder count does not match interpolation arguments",
                    )
                })?;
                match value {
                    Value::String(value) => {
                        output.write_str(value).map_err(formatting_error)?;
                    }
                    value => write_value(value, output)?,
                }
                argument = argument.checked_add(1).ok_or_else(formatting_size_error)?;
                index += 2;
                literal_start = index;
                continue;
            }
            (b'{' | b'}', _) => {
                return Err(Error::new(
                    ErrorKind::FormattingError,
                    SourceLocation::start(),
                    "malformed format template brace",
                ));
            }
            _ => None,
        };
        if let Some(character) = replacement {
            output
                .write_str(&template[literal_start..index])
                .map_err(formatting_error)?;
            output.write_char(character).map_err(formatting_error)?;
            index += 2;
            literal_start = index;
        } else {
            index += 1;
        }
    }
    output
        .write_str(&template[literal_start..])
        .map_err(formatting_error)?;
    if argument != arguments.len() {
        return Err(Error::new(
            ErrorKind::FormattingError,
            SourceLocation::start(),
            "format placeholder count does not match interpolation arguments",
        ));
    }
    Ok(())
}

fn write_value(output_value: &Value, output: &mut impl Write) -> Result<(), Error> {
    let mut pending = Vec::new();
    reserve_format_tasks(&mut pending, 1)?;
    pending.push(ValueFormatTask::Value(output_value));
    while let Some(task) = pending.pop() {
        match task {
            ValueFormatTask::Text(text) => output.write_str(text).map_err(formatting_error)?,
            ValueFormatTask::Value(Value::Bool(value)) => {
                output
                    .write_str(if *value { "true" } else { "false" })
                    .map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::Int(value)) => {
                write!(output, "{value}").map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::Double(value)) => append_double(output, *value)?,
            ValueFormatTask::Value(Value::String(value)) => append_string(output, value)?,
            ValueFormatTask::Value(Value::BoolVector(values)) => {
                output.write_char('(').map_err(formatting_error)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.write_char(' ').map_err(formatting_error)?;
                    }
                    output
                        .write_str(if *value { "true" } else { "false" })
                        .map_err(formatting_error)?;
                }
                output.write_char(')').map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::IntVector(values)) => {
                output.write_char('(').map_err(formatting_error)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.write_char(' ').map_err(formatting_error)?;
                    }
                    write!(output, "{value}").map_err(formatting_error)?;
                }
                output.write_char(')').map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::DoubleVector(values)) => {
                output.write_char('(').map_err(formatting_error)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.write_char(' ').map_err(formatting_error)?;
                    }
                    append_double(output, *value)?;
                }
                output.write_char(')').map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::StringVector(values)) => {
                output.write_char('(').map_err(formatting_error)?;
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.write_char(' ').map_err(formatting_error)?;
                    }
                    append_string(output, value)?;
                }
                output.write_char(')').map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::Tuple(values)) => {
                output.write_char('[').map_err(formatting_error)?;
                let task_count = values
                    .len()
                    .checked_mul(2)
                    .and_then(|count| count.checked_add(1))
                    .ok_or_else(formatting_size_error)?;
                reserve_format_tasks(&mut pending, task_count)?;
                pending.push(ValueFormatTask::Text("]"));
                for (index, value) in values.iter().enumerate().rev() {
                    pending.push(ValueFormatTask::Value(value));
                    if index != 0 {
                        pending.push(ValueFormatTask::Text(" "));
                    }
                }
            }
        }
    }
    Ok(())
}

fn reserve_format_tasks(
    pending: &mut Vec<ValueFormatTask<'_>>,
    additional: usize,
) -> Result<(), Error> {
    format_allocation_attempt()?;
    pending
        .try_reserve(additional)
        .map_err(|_| formatting_allocation_error())
}

fn format_allocation_attempt() -> Result<(), Error> {
    if cfg!(test) {
        let ordinal = FORMAT_ALLOCATION_ORDINAL.with(|value| {
            let ordinal = value.get();
            value.set(ordinal.saturating_add(1));
            ordinal
        });
        let refused = FORMAT_FAIL_AT.with(|value| value.get() == Some(ordinal));
        if refused {
            return Err(formatting_allocation_error());
        }
    }
    Ok(())
}

fn append_string(output: &mut impl Write, value: &str) -> Result<(), Error> {
    output.write_char('"').map_err(formatting_error)?;
    for character in value.chars() {
        match character {
            '"' => output.write_str("\\\"").map_err(formatting_error)?,
            '\\' => output.write_str("\\\\").map_err(formatting_error)?,
            '\n' => output.write_str("\\n").map_err(formatting_error)?,
            '\r' => output.write_str("\\r").map_err(formatting_error)?,
            '\t' => output.write_str("\\t").map_err(formatting_error)?,
            '\0' => output.write_str("\\0").map_err(formatting_error)?,
            character if character.is_control() => {
                write!(output, "\\u{{{:x}}}", character as u32).map_err(formatting_error)?;
            }
            character => output.write_char(character).map_err(formatting_error)?,
        }
    }
    output.write_char('"').map_err(formatting_error)
}

fn formatting_allocation_error() -> Error {
    Error::new(
        ErrorKind::FormattingError,
        SourceLocation::start(),
        "unable to allocate formatted String",
    )
}

fn append_double(output: &mut impl Write, value: f64) -> Result<(), Error> {
    if value.is_nan() {
        output.write_str("nan").map_err(formatting_error)?;
    } else if value == f64::INFINITY {
        output.write_str("inf").map_err(formatting_error)?;
    } else if value == f64::NEG_INFINITY {
        output.write_str("-inf").map_err(formatting_error)?;
    } else {
        let magnitude = value.abs();
        let scientific = magnitude >= 1.0e6 || (magnitude != 0.0 && magnitude < 1.0e-4);
        if scientific {
            write!(output, "{value:e}").map_err(formatting_error)?;
        } else {
            write!(output, "{value}").map_err(formatting_error)?;
        }
        if !scientific && value.fract() == 0.0 {
            output.write_str(".0").map_err(formatting_error)?;
        }
    }
    Ok(())
}

struct LengthWriter {
    length: usize,
}

impl Write for LengthWriter {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        self.length = self.length.checked_add(text.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}

fn formatting_error(_: std::fmt::Error) -> Error {
    formatting_size_error()
}

fn formatting_size_error() -> Error {
    Error::new(
        ErrorKind::FormattingError,
        SourceLocation::start(),
        "formatted String size overflow",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn format_with_failure(value: &Value, fail_at: Option<usize>) -> Result<String, Error> {
        FORMAT_ALLOCATION_ORDINAL.with(|ordinal| ordinal.set(0));
        FORMAT_FAIL_AT.with(|failure| failure.set(fail_at));
        let result = format_value(value);
        FORMAT_FAIL_AT.with(|failure| failure.set(None));
        FORMAT_ALLOCATION_ORDINAL.with(|ordinal| ordinal.set(0));
        result
    }

    #[test]
    fn formatting_reports_initial_and_later_allocation_refusals() {
        let value = Value::Tuple(
            vec![
                Value::String("héllo\n".to_owned()),
                Value::Tuple(vec![Value::Double(2.0)].into()),
            ]
            .into(),
        );
        for ordinal in 0..=4 {
            let error = format_with_failure(&value, Some(ordinal)).expect_err("allocation refusal");
            assert_eq!(error.kind, ErrorKind::FormattingError);
            assert_eq!(error.message, "unable to allocate formatted String");
        }
        assert_eq!(
            format_with_failure(&value, None).expect("formatting succeeds"),
            "[\"héllo\\n\" [2.0]]"
        );
    }

    #[test]
    fn formatting_preserves_double_boundaries_string_escapes_and_nested_values() {
        let doubles = Value::DoubleVector(vec![
            f64::NAN,
            f64::INFINITY,
            f64::NEG_INFINITY,
            0.0,
            -0.0,
            999_999.0,
            1_000_000.0,
            0.0001,
            0.00001,
        ]);
        assert_eq!(
            format_value(&doubles).expect("Double formatting"),
            "(nan inf -inf 0.0 -0.0 999999.0 1e6 0.0001 1e-5)"
        );

        let strings = Value::StringVector(vec![
            "héllo 世界 🦀".to_owned(),
            "\"\\\n\r\t\0\u{1}".to_owned(),
        ]);
        assert_eq!(
            format_value(&strings).expect("String formatting"),
            "(\"héllo 世界 🦀\" \"\\\"\\\\\\n\\r\\t\\0\\u{1}\")"
        );

        let nested = Value::Tuple(
            vec![
                Value::Bool(true),
                Value::IntVector(vec![-1, 0, 2]),
                Value::Tuple(vec![Value::String("終".to_owned()), doubles].into()),
            ]
            .into(),
        );
        assert_eq!(
            format_value(&nested).expect("nested formatting"),
            "[true (-1 0 2) [\"終\" (nan inf -inf 0.0 -0.0 999999.0 1e6 0.0001 1e-5)]]"
        );
    }

    #[test]
    fn destructive_sizing_and_drop_handle_wide_and_deep_string_tuples_iteratively() {
        let thread = std::thread::Builder::new()
            .stack_size(64 * 1024)
            .spawn(|| {
                let width = 8_192usize;
                let wide = Value::Tuple(
                    (0..width)
                        .map(|_| Value::String("é".to_owned()))
                        .collect::<Vec<_>>()
                        .into(),
                );
                assert_eq!(
                    wide.into_canonical_bytes().expect("wide tuple sizing"),
                    width * (16 + "é".len())
                );

                let depth = 16_384usize;
                let mut deep = Value::String("終".to_owned());
                for _ in 0..depth {
                    deep = Value::Tuple(vec![deep].into());
                }
                assert_eq!(
                    deep.into_canonical_bytes().expect("deep tuple sizing"),
                    depth * 16 + "終".len()
                );

                let mut dropped = Value::String("🦀".to_owned());
                for _ in 0..depth {
                    dropped = Value::Tuple(vec![dropped].into());
                }
                drop(dropped);
            })
            .expect("spawn reduced-stack test");
        thread.join().expect("wide/deep tuple test");
    }

    #[test]
    fn interpolation_measure_and_render_are_identical_for_deep_values_and_raw_strings() {
        let raw = Value::String("Málaga\0🦀".to_owned());
        let raw_arguments = [&raw];
        let raw_length = measure_interpolation("{{{}}}", &raw_arguments).expect("measure raw");
        let mut raw_output = String::new();
        raw_output
            .try_reserve_exact(raw_length)
            .expect("reserve raw");
        render_interpolation("{{{}}}", &raw_arguments, &mut raw_output).expect("render raw");
        assert_eq!(raw_output.as_bytes(), b"{M\xc3\xa1laga\0\xf0\x9f\xa6\x80}");
        assert_eq!(raw_output.len(), raw_length);

        let mut deep = raw;
        for _ in 0..10_000 {
            deep = Value::Tuple(vec![deep].into());
        }
        let arguments = [&deep];
        let required = measure_interpolation("{}", &arguments).expect("measure deep");
        let mut output = String::new();
        output.try_reserve_exact(required).expect("reserve deep");
        render_interpolation("{}", &arguments, &mut output).expect("render deep");
        assert_eq!(output.len(), required);
        assert_eq!(output, format_value(&deep).expect("canonical deep"));
    }
}
