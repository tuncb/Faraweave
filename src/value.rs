use crate::{Error, ErrorKind, SourceLocation};
use std::fmt::Write;
use std::ops::Deref;

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
        while let Some(mut value) = pending.pop() {
            if let Value::Tuple(tuple) = &mut value {
                pending.append(&mut tuple.values);
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

    pub(crate) fn canonical_bytes(&self) -> Result<usize, Error> {
        let mut total = 0usize;
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            let charge = match value {
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
                Self::Tuple(values) => {
                    pending.try_reserve(values.len()).map_err(|_| {
                        Error::new(
                            ErrorKind::ResourceError,
                            SourceLocation::start(),
                            "resource sizing failed: allocation_unavailable",
                        )
                    })?;
                    pending.extend(values.iter());
                    values.len().checked_mul(16).ok_or_else(sizing_error)?
                }
            };
            total = total.checked_add(charge).ok_or_else(sizing_error)?;
        }
        Ok(total)
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
    let mut output = String::new();
    let mut pending = vec![ValueFormatTask::Value(value)];
    while let Some(task) = pending.pop() {
        match task {
            ValueFormatTask::Text(text) => output.push_str(text),
            ValueFormatTask::Value(Value::Bool(value)) => {
                output.push_str(if *value { "true" } else { "false" });
            }
            ValueFormatTask::Value(Value::Int(value)) => {
                write!(output, "{value}").map_err(formatting_error)?;
            }
            ValueFormatTask::Value(Value::Double(value)) => append_double(&mut output, *value),
            ValueFormatTask::Value(Value::String(value)) => append_string(&mut output, value)?,
            ValueFormatTask::Value(Value::BoolVector(values)) => {
                output.push('(');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(' ');
                    }
                    output.push_str(if *value { "true" } else { "false" });
                }
                output.push(')');
            }
            ValueFormatTask::Value(Value::IntVector(values)) => {
                output.push('(');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(' ');
                    }
                    write!(output, "{value}").map_err(formatting_error)?;
                }
                output.push(')');
            }
            ValueFormatTask::Value(Value::DoubleVector(values)) => {
                output.push('(');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(' ');
                    }
                    append_double(&mut output, *value);
                }
                output.push(')');
            }
            ValueFormatTask::Value(Value::StringVector(values)) => {
                let required = values.iter().try_fold(2usize, |total, value| {
                    let value_length = formatted_string_length(value)?;
                    total
                        .checked_add(value_length)
                        .and_then(|value| value.checked_add(usize::from(total != 2)))
                        .ok_or_else(formatting_size_error)
                })?;
                output
                    .try_reserve_exact(required)
                    .map_err(|_| formatting_allocation_error())?;
                output.push('(');
                for (index, value) in values.iter().enumerate() {
                    if index != 0 {
                        output.push(' ');
                    }
                    append_string_unchecked(&mut output, value);
                }
                output.push(')');
            }
            ValueFormatTask::Value(Value::Tuple(values)) => {
                output.push('[');
                let task_count = values.len().checked_mul(2).ok_or_else(|| {
                    Error::new(
                        ErrorKind::FormattingError,
                        SourceLocation::start(),
                        "formatting traversal size overflow",
                    )
                })?;
                pending.try_reserve(task_count).map_err(|_| {
                    Error::new(
                        ErrorKind::FormattingError,
                        SourceLocation::start(),
                        "unable to allocate formatting traversal",
                    )
                })?;
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
    Ok(output)
}

fn formatted_string_length(value: &str) -> Result<usize, Error> {
    value.chars().try_fold(2usize, |length, character| {
        let width = match character {
            '"' | '\\' | '\n' | '\r' | '\t' | '\0' => 2,
            character if character.is_control() => {
                let mut scalar = character as u32;
                let mut digits = 1usize;
                while scalar >= 16 {
                    scalar /= 16;
                    digits += 1;
                }
                digits.checked_add(4).ok_or_else(formatting_size_error)?
            }
            character => character.len_utf8(),
        };
        length.checked_add(width).ok_or_else(formatting_size_error)
    })
}

fn append_string(output: &mut String, value: &str) -> Result<(), Error> {
    let required = formatted_string_length(value)?;
    output
        .try_reserve_exact(required)
        .map_err(|_| formatting_allocation_error())?;
    append_string_unchecked(output, value);
    Ok(())
}

fn append_string_unchecked(output: &mut String, value: &str) {
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            '\0' => output.push_str("\\0"),
            character if character.is_control() => {
                let _ = write!(output, "\\u{{{:x}}}", character as u32);
            }
            character => output.push(character),
        }
    }
    output.push('"');
}

fn formatting_size_error() -> Error {
    Error::new(
        ErrorKind::FormattingError,
        SourceLocation::start(),
        "formatted String size overflow",
    )
}

fn formatting_allocation_error() -> Error {
    Error::new(
        ErrorKind::FormattingError,
        SourceLocation::start(),
        "unable to allocate formatted String",
    )
}

fn append_double(output: &mut String, value: f64) {
    if value.is_nan() {
        output.push_str("nan");
    } else if value == f64::INFINITY {
        output.push_str("inf");
    } else if value == f64::NEG_INFINITY {
        output.push_str("-inf");
    } else {
        let magnitude = value.abs();
        let text = if magnitude >= 1.0e6 || (magnitude != 0.0 && magnitude < 1.0e-4) {
            format!("{value:e}")
        } else {
            value.to_string()
        };
        output.push_str(&text);
        if !text.contains('.') && !text.contains('e') && !text.contains('E') {
            output.push_str(".0");
        }
    }
}

fn formatting_error(_: std::fmt::Error) -> Error {
    Error::new(
        ErrorKind::FormattingError,
        SourceLocation::start(),
        "unable to format value",
    )
}
