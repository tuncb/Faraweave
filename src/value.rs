use crate::{Error, ErrorKind, SourceLocation};
use std::fmt::Write;
use std::ops::Deref;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum ScalarType {
    Bool,
    Int,
    Double,
}

impl ScalarType {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Bool => "Bool",
            Self::Int => "Int",
            Self::Double => "Double",
        }
    }

    pub(crate) const fn byte_width(self) -> usize {
        match self {
            Self::Bool => 1,
            Self::Int | Self::Double => 8,
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
    BoolVector(Vec<bool>),
    IntVector(Vec<i64>),
    DoubleVector(Vec<f64>),
    Tuple(TupleValues),
}

impl Value {
    pub const fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Bool(_) | Self::BoolVector(_) => Some(ScalarType::Bool),
            Self::Int(_) | Self::IntVector(_) => Some(ScalarType::Int),
            Self::Double(_) | Self::DoubleVector(_) => Some(ScalarType::Double),
            Self::Tuple(_) => None,
        }
    }

    pub const fn is_scalar(&self) -> bool {
        matches!(self, Self::Bool(_) | Self::Int(_) | Self::Double(_))
    }

    pub const fn is_vector(&self) -> bool {
        matches!(
            self,
            Self::BoolVector(_) | Self::IntVector(_) | Self::DoubleVector(_)
        )
    }

    pub fn len(&self) -> usize {
        match self {
            Self::BoolVector(values) => values.len(),
            Self::IntVector(values) => values.len(),
            Self::DoubleVector(values) => values.len(),
            Self::Tuple(values) => values.len(),
            Self::Bool(_) | Self::Int(_) | Self::Double(_) => 1,
        }
    }

    pub const fn is_empty(&self) -> bool {
        match self {
            Self::BoolVector(values) => values.is_empty(),
            Self::IntVector(values) => values.is_empty(),
            Self::DoubleVector(values) => values.is_empty(),
            Self::Tuple(values) => values.values.is_empty(),
            Self::Bool(_) | Self::Int(_) | Self::Double(_) => false,
        }
    }

    pub fn value_type(&self) -> Type {
        match self {
            Self::Bool(_) => Type::Scalar(ScalarType::Bool),
            Self::Int(_) => Type::Scalar(ScalarType::Int),
            Self::Double(_) => Type::Scalar(ScalarType::Double),
            Self::BoolVector(_) => Type::Vector(ScalarType::Bool),
            Self::IntVector(_) => Type::Vector(ScalarType::Int),
            Self::DoubleVector(_) => Type::Vector(ScalarType::Double),
            Self::Tuple(values) => Type::Tuple(values.iter().map(Self::value_type).collect()),
        }
    }

    pub(crate) fn canonical_bytes(&self) -> Result<usize, Error> {
        let mut total = 0usize;
        let mut pending = vec![self];
        while let Some(value) = pending.pop() {
            let charge = match value {
                Self::Bool(_) | Self::Int(_) | Self::Double(_) => 0,
                Self::BoolVector(values) => values.len(),
                Self::IntVector(values) => values.len().checked_mul(8).ok_or_else(sizing_error)?,
                Self::DoubleVector(values) => {
                    values.len().checked_mul(8).ok_or_else(sizing_error)?
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
