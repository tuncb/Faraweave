use crate::{
    Error, ErrorKind, ParameterErrorContext, ParameterErrorReason, ScalarType, SourceLocation,
    SourceSpan, Value,
};

#[derive(Clone, Debug)]
pub(crate) struct Program {
    pub parameter_header: Option<SourceSpan>,
    pub parameters: Vec<Parameter>,
    pub roots: Vec<Expr>,
}

#[derive(Clone, Debug)]
pub(crate) struct Parameter {
    pub name: String,
    pub scalar_type: ScalarType,
    pub span: SourceSpan,
    pub name_span: SourceSpan,
}

#[derive(Clone, Debug)]
pub(crate) struct Expr {
    pub kind: ExprKind,
    pub span: SourceSpan,
}

#[derive(Clone, Debug)]
pub(crate) struct UnaryStep {
    pub name: String,
    pub name_span: SourceSpan,
    pub span: SourceSpan,
}

#[derive(Clone, Debug)]
pub(crate) enum ExprKind {
    Literal(Value),
    Vector(ScalarType, Vec<Value>),
    Tuple(Vec<Expr>),
    DeepTuple {
        depth: usize,
        leaf: Value,
    },
    UnaryChain {
        leaf: Value,
        leaf_span: SourceSpan,
        steps: Vec<UnaryStep>,
    },
    Parameter(usize),
    Call {
        name: String,
        syntax: CallSyntax,
        arguments: Vec<Expr>,
        name_span: SourceSpan,
    },
    UnresolvedName {
        name: String,
        name_span: SourceSpan,
    },
    Placeholder,
    Fanout {
        operand: Box<Expr>,
        branches: Vec<Expr>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum CallSyntax {
    Direct,
    Prefix,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TokenKind {
    Name,
    Bool,
    Int,
    Double,
    BoolType,
    IntType,
    DoubleType,
    LeftBracket,
    RightBracket,
    LeftParenthesis,
    RightParenthesis,
    LeftBrace,
    RightBrace,
    Placeholder,
    Space,
    Comment,
    Newline,
    MalformedLiteral,
    RangeLiteral,
    Invalid,
}

#[derive(Clone, Debug)]
struct Token {
    kind: TokenKind,
    span: SourceSpan,
    spelling: String,
    value: Option<Value>,
}

pub(crate) fn parse(source: &str) -> Result<Program, Error> {
    let tokens = tokenize(source);
    if let Some(result) = parse_deep_singleton_tuple(&tokens) {
        return result;
    }
    if let Some(result) = parse_deep_unary_chain(&tokens) {
        return result;
    }
    Parser::new(tokens).parse_program()
}

fn parse_deep_unary_chain(tokens: &[Token]) -> Option<Result<Program, Error>> {
    const COMPACT_DEPTH: usize = 128;
    let mut first = 0usize;
    while tokens.get(first).is_some_and(|token| is_trivia(token.kind)) {
        first += 1;
    }

    let prefix = parse_prefix_chain(tokens, first, COMPACT_DEPTH);
    if prefix.is_some() {
        return prefix;
    }
    parse_bracket_chain(tokens, first, COMPACT_DEPTH)
}

fn parse_prefix_chain(
    tokens: &[Token],
    first: usize,
    minimum_depth: usize,
) -> Option<Result<Program, Error>> {
    let mut index = first;
    let mut names = Vec::new();
    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Name)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::Space)
    {
        names.push(tokens[index].clone());
        index += 2;
    }
    if names.len() < minimum_depth {
        return None;
    }
    let leaf_token = tokens.get(index)?;
    let leaf = leaf_token.value.clone()?;
    index += 1;
    while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
        index += 1;
    }
    if index != tokens.len() {
        return None;
    }
    let mut steps = Vec::new();
    if steps.try_reserve_exact(names.len()).is_err() {
        return Some(Err(parser_allocation_error(leaf_token.span.begin)));
    }
    for name in names.into_iter().rev() {
        steps.push(UnaryStep {
            name: name.spelling,
            name_span: name.span,
            span: SourceSpan {
                begin: name.span.begin,
                end: leaf_token.span.end,
            },
        });
    }
    let span = SourceSpan {
        begin: tokens[first].span.begin,
        end: leaf_token.span.end,
    };
    Some(Ok(single_root_unary_program(
        leaf,
        leaf_token.span,
        steps,
        span,
    )))
}

fn parse_bracket_chain(
    tokens: &[Token],
    first: usize,
    minimum_depth: usize,
) -> Option<Result<Program, Error>> {
    let mut index = first;
    let mut names = Vec::new();
    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::Name)
        && tokens.get(index + 1).map(|token| token.kind) == Some(TokenKind::LeftBracket)
    {
        names.push(tokens[index].clone());
        index += 2;
        while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
            index += 1;
        }
    }
    if names.len() < minimum_depth {
        return None;
    }
    let Some(leaf_token) = tokens.get(index) else {
        return Some(Err(Error::at_span(
            ErrorKind::SyntaxError,
            insertion_span(tokens),
            "expected an expression",
        )));
    };
    let leaf = leaf_token.value.clone()?;
    index += 1;
    let mut steps = Vec::new();
    if steps.try_reserve_exact(names.len()).is_err() {
        return Some(Err(parser_allocation_error(leaf_token.span.begin)));
    }
    for name in names.into_iter().rev() {
        while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
            index += 1;
        }
        let Some(close) = tokens.get(index) else {
            return Some(Err(Error::at_span(
                ErrorKind::SyntaxError,
                insertion_span(tokens),
                "missing closing delimiter",
            )));
        };
        if close.kind != TokenKind::RightBracket {
            if matches!(
                close.kind,
                TokenKind::RightParenthesis | TokenKind::RightBrace
            ) {
                return Some(Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    close.span,
                    "mismatched closing delimiter",
                )));
            }
            return None;
        }
        steps.push(UnaryStep {
            name: name.spelling,
            name_span: name.span,
            span: SourceSpan {
                begin: name.span.begin,
                end: close.span.end,
            },
        });
        index += 1;
    }
    while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
        index += 1;
    }
    if index != tokens.len() {
        return None;
    }
    let span = SourceSpan {
        begin: tokens[first].span.begin,
        end: steps
            .last()
            .map_or(leaf_token.span.end, |step| step.span.end),
    };
    Some(Ok(single_root_unary_program(
        leaf,
        leaf_token.span,
        steps,
        span,
    )))
}

fn insertion_span(tokens: &[Token]) -> SourceSpan {
    let location = tokens
        .last()
        .map_or(SourceLocation::start(), |token| token.span.end);
    SourceSpan {
        begin: location,
        end: location,
    }
}

fn parser_allocation_error(location: SourceLocation) -> Error {
    Error::new(
        ErrorKind::ResourceError,
        location,
        "parser failed: allocation_unavailable",
    )
}

fn single_root_unary_program(
    leaf: Value,
    leaf_span: SourceSpan,
    steps: Vec<UnaryStep>,
    span: SourceSpan,
) -> Program {
    Program {
        parameter_header: None,
        parameters: Vec::new(),
        roots: vec![Expr {
            kind: ExprKind::UnaryChain {
                leaf,
                leaf_span,
                steps,
            },
            span,
        }],
    }
}

fn parse_deep_singleton_tuple(tokens: &[Token]) -> Option<Result<Program, Error>> {
    const COMPACT_DEPTH: usize = 128;
    let mut index = 0usize;
    while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
        index += 1;
    }
    let first = index;
    let mut depth = 0usize;
    while tokens.get(index).map(|token| token.kind) == Some(TokenKind::LeftBracket) {
        depth += 1;
        index += 1;
        while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
            index += 1;
        }
    }
    if depth < COMPACT_DEPTH {
        return None;
    }
    let Some(leaf_token) = tokens.get(index) else {
        let insertion = tokens
            .last()
            .map_or(SourceLocation::start(), |token| token.span.end);
        return Some(Err(Error::at_span(
            ErrorKind::SyntaxError,
            SourceSpan {
                begin: insertion,
                end: insertion,
            },
            "expected an expression",
        )));
    };
    let leaf = leaf_token.value.clone()?;
    index += 1;
    let mut closed = 0usize;
    let mut closing_end = leaf_token.span.end;
    while closed < depth {
        while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
            index += 1;
        }
        match tokens.get(index) {
            Some(token) if token.kind == TokenKind::RightBracket => {
                closed += 1;
                closing_end = token.span.end;
                index += 1;
            }
            Some(token)
                if matches!(
                    token.kind,
                    TokenKind::RightParenthesis | TokenKind::RightBrace
                ) =>
            {
                return Some(Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    token.span,
                    "mismatched closing delimiter",
                )));
            }
            None => {
                let insertion = tokens
                    .last()
                    .map_or(SourceLocation::start(), |token| token.span.end);
                return Some(Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    SourceSpan {
                        begin: insertion,
                        end: insertion,
                    },
                    "missing closing delimiter",
                )));
            }
            Some(_) => return None,
        }
    }
    while tokens.get(index).is_some_and(|token| is_trivia(token.kind)) {
        index += 1;
    }
    if index != tokens.len() {
        return None;
    }
    let span = SourceSpan {
        begin: tokens[first].span.begin,
        end: closing_end,
    };
    Some(Ok(Program {
        parameter_header: None,
        parameters: Vec::new(),
        roots: vec![Expr {
            kind: ExprKind::DeepTuple { depth, leaf },
            span,
        }],
    }))
}

pub(crate) fn program_contains_tuple(program: &Program) -> bool {
    program.roots.iter().any(expression_contains_tuple)
}

pub(crate) fn first_tuple_location(program: &Program) -> Option<SourceLocation> {
    program.roots.iter().find_map(first_tuple_in_expression)
}

fn first_tuple_in_expression(expression: &Expr) -> Option<SourceLocation> {
    match &expression.kind {
        ExprKind::Tuple(elements) => elements
            .iter()
            .find_map(first_tuple_in_expression)
            .or(Some(expression.span.begin)),
        ExprKind::DeepTuple { .. } => Some(expression.span.begin),
        ExprKind::UnaryChain { .. } => None,
        ExprKind::Fanout { operand, branches } => first_tuple_in_expression(operand)
            .or_else(|| branches.iter().find_map(first_tuple_in_expression))
            .or(Some(expression.span.begin)),
        ExprKind::Call { arguments, .. } => arguments.iter().find_map(first_tuple_in_expression),
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::Parameter(_)
        | ExprKind::UnresolvedName { .. }
        | ExprKind::Placeholder => None,
    }
}

fn expression_contains_tuple(expression: &Expr) -> bool {
    match &expression.kind {
        ExprKind::Tuple(_) | ExprKind::DeepTuple { .. } | ExprKind::Fanout { .. } => true,
        ExprKind::UnaryChain { .. } => false,
        ExprKind::Call { arguments, .. } => arguments.iter().any(expression_contains_tuple),
        ExprKind::Literal(_)
        | ExprKind::Vector(_, _)
        | ExprKind::Parameter(_)
        | ExprKind::UnresolvedName { .. }
        | ExprKind::Placeholder => false,
    }
}

fn tokenize(source: &str) -> Vec<Token> {
    let bytes = source.as_bytes();
    let mut tokens = Vec::new();
    let mut index = 0;
    let mut location = SourceLocation::start();
    while index < bytes.len() {
        let begin = location;
        let byte = bytes[index];
        if matches!(byte, b' ' | b'\t') {
            while index < bytes.len() && matches!(bytes[index], b' ' | b'\t') {
                advance_ascii(&mut location);
                index += 1;
            }
            tokens.push(token(
                TokenKind::Space,
                begin,
                location,
                &source[begin.offset - 1..location.offset - 1],
                None,
            ));
            continue;
        }
        if byte == b'#' {
            advance_ascii(&mut location);
            index += 1;
            while index < bytes.len()
                && bytes[index] != b'\n'
                && !(bytes[index] == b'\r' && bytes.get(index + 1) == Some(&b'\n'))
            {
                advance_ascii(&mut location);
                index += 1;
            }
            tokens.push(token(TokenKind::Comment, begin, location, "", None));
            continue;
        }
        if byte == b'\n' || (byte == b'\r' && bytes.get(index + 1) == Some(&b'\n')) {
            if byte == b'\r' {
                index += 2;
                location.offset += 2;
            } else {
                index += 1;
                location.offset += 1;
            }
            location.line += 1;
            location.column = 1;
            tokens.push(token(TokenKind::Newline, begin, location, "", None));
            continue;
        }
        if byte.is_ascii_lowercase() {
            while index < bytes.len()
                && (bytes[index].is_ascii_lowercase()
                    || bytes[index].is_ascii_digit()
                    || bytes[index] == b'_')
            {
                advance_ascii(&mut location);
                index += 1;
            }
            let spelling = &source[begin.offset - 1..location.offset - 1];
            let (kind, value) = match spelling {
                "true" => (TokenKind::Bool, Some(Value::Bool(true))),
                "false" => (TokenKind::Bool, Some(Value::Bool(false))),
                "inf" => (TokenKind::Double, Some(Value::Double(f64::INFINITY))),
                "nan" => (TokenKind::Double, Some(Value::Double(f64::NAN))),
                _ => (TokenKind::Name, None),
            };
            tokens.push(token(kind, begin, location, spelling, value));
            continue;
        }
        if byte == b'_' {
            advance_ascii(&mut location);
            index += 1;
            tokens.push(token(TokenKind::Placeholder, begin, location, "_", None));
            continue;
        }
        if byte.is_ascii_uppercase() {
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric() || bytes[index] == b'_')
            {
                advance_ascii(&mut location);
                index += 1;
            }
            let spelling = &source[begin.offset - 1..location.offset - 1];
            let kind = match spelling {
                "Bool" => TokenKind::BoolType,
                "Int" => TokenKind::IntType,
                "Double" => TokenKind::DoubleType,
                _ => TokenKind::Invalid,
            };
            tokens.push(token(kind, begin, location, spelling, None));
            continue;
        }
        if byte.is_ascii_digit() || matches!(byte, b'-' | b'+' | b'.') {
            while index < bytes.len()
                && (bytes[index].is_ascii_alphanumeric()
                    || matches!(bytes[index], b'_' | b'.' | b'+' | b'-'))
            {
                advance_ascii(&mut location);
                index += 1;
            }
            let spelling = &source[begin.offset - 1..location.offset - 1];
            let (kind, value) = parse_numeric(spelling);
            tokens.push(token(kind, begin, location, spelling, value));
            continue;
        }
        if !byte.is_ascii() {
            advance_ascii(&mut location);
            index += 1;
            tokens.push(token(TokenKind::Invalid, begin, location, "", None));
            continue;
        }
        let kind = match byte {
            b'[' => TokenKind::LeftBracket,
            b']' => TokenKind::RightBracket,
            b'(' => TokenKind::LeftParenthesis,
            b')' => TokenKind::RightParenthesis,
            b'{' => TokenKind::LeftBrace,
            b'}' => TokenKind::RightBrace,
            _ => TokenKind::Invalid,
        };
        advance_ascii(&mut location);
        index += 1;
        let spelling = &source[begin.offset - 1..location.offset - 1];
        tokens.push(token(kind, begin, location, spelling, None));
    }
    tokens
}

fn advance_ascii(location: &mut SourceLocation) {
    location.offset += 1;
    location.column += 1;
}

fn token(
    kind: TokenKind,
    begin: SourceLocation,
    end: SourceLocation,
    spelling: &str,
    value: Option<Value>,
) -> Token {
    Token {
        kind,
        span: SourceSpan { begin, end },
        spelling: spelling.to_owned(),
        value,
    }
}

fn parse_numeric(spelling: &str) -> (TokenKind, Option<Value>) {
    if spelling == "-inf" {
        return (TokenKind::Double, Some(Value::Double(f64::NEG_INFINITY)));
    }
    if canonical_integer(spelling) {
        return match spelling.parse::<i64>() {
            Ok(value) => (TokenKind::Int, Some(Value::Int(value))),
            Err(_) => (TokenKind::RangeLiteral, None),
        };
    }
    if !canonical_double(spelling) {
        return (TokenKind::MalformedLiteral, None);
    }
    match spelling.parse::<f64>() {
        Ok(value) if value.is_finite() => (TokenKind::Double, Some(Value::Double(value))),
        Ok(value) if value == 0.0 => (TokenKind::Double, Some(Value::Double(value))),
        _ if decimal_underflows_to_zero(spelling) => (
            TokenKind::Double,
            Some(Value::Double(if spelling.starts_with('-') {
                -0.0
            } else {
                0.0
            })),
        ),
        _ => (TokenKind::RangeLiteral, None),
    }
}

fn canonical_integer(spelling: &str) -> bool {
    let digits = spelling.strip_prefix('-').unwrap_or(spelling);
    if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return false;
    }
    if digits.starts_with('0') {
        return digits == "0" && !spelling.starts_with('-');
    }
    true
}

fn canonical_double(spelling: &str) -> bool {
    let text = spelling.strip_prefix('-').unwrap_or(spelling);
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(index) => (&text[..index], Some(&text[index + 1..])),
        None => (text, None),
    };
    if let Some(exponent) = exponent {
        let digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return false;
        }
    }
    let has_exponent = exponent.is_some();
    let mut parts = mantissa.split('.');
    let integer = parts.next().unwrap_or_default();
    let fraction = parts.next();
    if parts.next().is_some()
        || integer.is_empty()
        || !integer.bytes().all(|byte| byte.is_ascii_digit())
        || (integer.starts_with('0') && integer.len() != 1)
    {
        return false;
    }
    let has_fraction = fraction.is_some();
    if let Some(fraction) = fraction
        && (fraction.is_empty() || !fraction.bytes().all(|byte| byte.is_ascii_digit()))
    {
        return false;
    }
    has_fraction || has_exponent
}

fn decimal_underflows_to_zero(spelling: &str) -> bool {
    let lower = spelling.to_ascii_lowercase();
    let exponent = lower
        .split_once('e')
        .and_then(|(_, exponent)| exponent.parse::<i32>().ok())
        .unwrap_or(0);
    exponent < -324
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    parameters: Vec<Parameter>,
    fanout_depth: usize,
    branch_depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self {
            tokens,
            index: 0,
            parameters: Vec::new(),
            fanout_depth: 0,
            branch_depth: 0,
        }
    }

    fn parse_program(mut self) -> Result<Program, Error> {
        self.skip_newlines_and_spaces();
        let mut parameter_header = None;
        if self.is_name("parameters") {
            parameter_header = Some(self.parse_parameter_header()?);
        }
        let mut roots: Vec<Expr> = Vec::new();
        loop {
            self.skip_newlines_and_spaces();
            if self.at_end() {
                break;
            }
            if self.is_name("parameters") {
                let keyword = self
                    .peek()
                    .cloned()
                    .ok_or_else(|| self.eof_error("expected an expression"))?;
                let (reason, related) = if let Some(header) = parameter_header {
                    (ParameterErrorReason::SecondParameterHeader, header)
                } else {
                    (
                        ParameterErrorReason::ParameterHeaderAfterRoot,
                        roots.first().map_or(keyword.span, |root| root.span),
                    )
                };
                return Err(parameter_syntax_error(
                    reason,
                    keyword.span,
                    keyword.span,
                    Some(related),
                ));
            }
            let root = self.parse_expr(false)?;
            roots.push(root);
            self.skip_spaces();
            if self.at_end() {
                break;
            }
            if !self.take_if(TokenKind::Newline) {
                if let Some(token) = self.peek() {
                    if token.kind == TokenKind::Invalid {
                        return Err(Error::at_span(
                            ErrorKind::InvalidByte,
                            token.span,
                            "invalid source byte",
                        ));
                    }
                    if matches!(
                        token.kind,
                        TokenKind::RightParenthesis
                            | TokenKind::RightBracket
                            | TokenKind::RightBrace
                    ) {
                        return Err(Error::at_span(
                            ErrorKind::SyntaxError,
                            token.span,
                            "mismatched closing delimiter",
                        ));
                    }
                }
                return Err(self.syntax_here("root expression has trailing input"));
            }
        }
        Ok(Program {
            parameter_header,
            parameters: self.parameters,
            roots,
        })
    }

    fn parse_parameter_header(&mut self) -> Result<SourceSpan, Error> {
        let keyword = self
            .take()
            .ok_or_else(|| self.eof_error("expected parameters"))?;
        let Some(open) = self.take_kind(TokenKind::LeftBracket) else {
            let primary = self
                .peek()
                .map_or_else(|| self.insertion_span(), |token| token.span);
            return Err(parameter_syntax_error(
                ParameterErrorReason::ExpectedHeaderOpen,
                primary,
                keyword.span,
                Some(keyword.span),
            ));
        };
        loop {
            self.skip_inside();
            if let Some(close) = self.take_kind(TokenKind::RightBracket) {
                let header = SourceSpan {
                    begin: keyword.span.begin,
                    end: close.span.end,
                };
                self.skip_spaces();
                if self.at_end() {
                    return Ok(header);
                }
                if self.take_if(TokenKind::Newline) {
                    return Ok(header);
                }
                let primary = self
                    .peek()
                    .map_or_else(|| self.insertion_span(), |token| token.span);
                return Err(parameter_syntax_error(
                    ParameterErrorReason::TrailingHeaderBytes,
                    primary,
                    header,
                    Some(header),
                ));
            }
            let Some(name) = self.take() else {
                let primary = self.insertion_span();
                return Err(parameter_syntax_error(
                    ParameterErrorReason::MissingHeaderClose,
                    primary,
                    SourceSpan {
                        begin: keyword.span.begin,
                        end: primary.end,
                    },
                    Some(open.span),
                ));
            };
            if !syntactic_parameter_name(&name.spelling) {
                let reason = if token_scalar_type(name.kind).is_some() {
                    ParameterErrorReason::ExpectedParameterName
                } else {
                    ParameterErrorReason::UnexpectedHeaderToken
                };
                return Err(parameter_syntax_error(
                    reason,
                    name.span,
                    SourceSpan {
                        begin: keyword.span.begin,
                        end: name.span.end,
                    },
                    Some(open.span),
                ));
            }
            if !self.has_separator() {
                let primary = match self.peek() {
                    None => self.insertion_span(),
                    Some(token) if token.kind == TokenKind::RightBracket => SourceSpan {
                        begin: token.span.begin,
                        end: token.span.begin,
                    },
                    Some(token) => token.span,
                };
                let reason = if self.at_end() || self.peek_kind() == Some(TokenKind::RightBracket) {
                    ParameterErrorReason::ExpectedParameterType
                } else {
                    ParameterErrorReason::UnexpectedHeaderToken
                };
                return Err(parameter_syntax_error(
                    reason,
                    primary,
                    name.span,
                    Some(name.span),
                ));
            }
            self.skip_inside();
            let Some(type_token) = self.take() else {
                return Err(parameter_syntax_error(
                    ParameterErrorReason::ExpectedParameterType,
                    self.insertion_span(),
                    name.span,
                    Some(name.span),
                ));
            };
            let Some(scalar_type) = token_scalar_type(type_token.kind) else {
                let (reason, primary) = if type_token.kind == TokenKind::RightBracket {
                    (
                        ParameterErrorReason::ExpectedParameterType,
                        SourceSpan {
                            begin: type_token.span.begin,
                            end: type_token.span.begin,
                        },
                    )
                } else {
                    (ParameterErrorReason::UnexpectedHeaderToken, type_token.span)
                };
                return Err(parameter_syntax_error(
                    reason,
                    primary,
                    name.span,
                    Some(name.span),
                ));
            };
            let span = SourceSpan {
                begin: name.span.begin,
                end: type_token.span.end,
            };
            self.parameters.push(Parameter {
                name: name.spelling,
                scalar_type,
                span,
                name_span: name.span,
            });
            if self.at_end() {
                let primary = self.insertion_span();
                return Err(parameter_syntax_error(
                    ParameterErrorReason::MissingHeaderClose,
                    primary,
                    SourceSpan {
                        begin: keyword.span.begin,
                        end: primary.end,
                    },
                    Some(open.span),
                ));
            }
            if !self.has_separator() && self.peek_kind() != Some(TokenKind::RightBracket) {
                let primary = self
                    .peek()
                    .map_or_else(|| self.insertion_span(), |token| token.span);
                return Err(parameter_syntax_error(
                    ParameterErrorReason::UnexpectedHeaderToken,
                    primary,
                    SourceSpan {
                        begin: keyword.span.begin,
                        end: primary.end,
                    },
                    Some(type_token.span),
                ));
            }
        }
    }

    fn parse_expr(&mut self, allow_newline: bool) -> Result<Expr, Error> {
        if allow_newline {
            self.skip_inside();
        }
        let token = self
            .peek()
            .cloned()
            .ok_or_else(|| self.eof_error("expected an expression"))?;
        match token.kind {
            TokenKind::Bool | TokenKind::Int | TokenKind::Double => {
                self.index += 1;
                Ok(Expr {
                    kind: ExprKind::Literal(
                        token
                            .value
                            .ok_or_else(|| self.syntax_here("invalid literal value"))?,
                    ),
                    span: token.span,
                })
            }
            TokenKind::MalformedLiteral => {
                self.index += 1;
                Err(Error::at_span(
                    ErrorKind::MalformedLiteral,
                    token.span,
                    "malformed scalar literal",
                ))
            }
            TokenKind::RangeLiteral => {
                self.index += 1;
                Err(Error::at_span(
                    ErrorKind::LiteralRangeError,
                    token.span,
                    "scalar literal is outside its accepted range",
                ))
            }
            TokenKind::BoolType | TokenKind::IntType | TokenKind::DoubleType => {
                self.parse_typed_empty()
            }
            TokenKind::LeftParenthesis => self.parse_vector(),
            TokenKind::LeftBracket => self.parse_tuple(),
            TokenKind::Name => self.parse_name_expr(),
            TokenKind::Placeholder if self.branch_depth != 0 => {
                self.index += 1;
                Ok(Expr {
                    kind: ExprKind::Placeholder,
                    span: token.span,
                })
            }
            TokenKind::Placeholder => Err(Error::at_span(
                ErrorKind::SyntaxError,
                token.span,
                "fanout token is not valid in this position",
            )),
            TokenKind::Invalid => Err(Error::at_span(
                ErrorKind::InvalidByte,
                token.span,
                "invalid source byte",
            )),
            TokenKind::RightParenthesis | TokenKind::RightBracket | TokenKind::RightBrace => {
                Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    token.span,
                    "mismatched closing delimiter",
                ))
            }
            _ => Err(Error::at_span(
                ErrorKind::SyntaxError,
                token.span,
                "expected an expression",
            )),
        }
    }

    fn parse_typed_empty(&mut self) -> Result<Expr, Error> {
        let type_token = self.take().ok_or_else(|| self.eof_error("expected type"))?;
        let scalar_type = token_scalar_type(type_token.kind)
            .ok_or_else(|| self.syntax_here("expected scalar type"))?;
        if self.peek_kind() != Some(TokenKind::LeftParenthesis) {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                type_token.span,
                "empty vector requires Bool(), Int(), or Double()",
            ));
        }
        self.index += 1;
        self.skip_comment_trivia();
        let close = self
            .take()
            .ok_or_else(|| self.eof_error("missing closing delimiter"))?;
        if close.kind != TokenKind::RightParenthesis {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                type_token.span,
                "vector elements must be scalar literals",
            ));
        }
        Ok(Expr {
            kind: ExprKind::Vector(scalar_type, Vec::new()),
            span: SourceSpan {
                begin: type_token.span.begin,
                end: close.span.end,
            },
        })
    }

    fn parse_vector(&mut self) -> Result<Expr, Error> {
        let open = self
            .take_kind(TokenKind::LeftParenthesis)
            .ok_or_else(|| self.eof_error("expected '('"))?;
        self.skip_inside();
        if self.peek_kind() == Some(TokenKind::RightParenthesis) {
            let close = self.take().ok_or_else(|| self.eof_error("missing ')'"))?;
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                SourceSpan {
                    begin: open.span.begin,
                    end: close.span.end,
                },
                "empty vector requires Bool(), Int(), or Double()",
            ));
        }
        let mut values = Vec::new();
        let mut scalar_type = None;
        loop {
            self.skip_inside();
            let token = self
                .take()
                .ok_or_else(|| self.eof_error("missing closing delimiter"))?;
            if matches!(token.kind, TokenKind::RightBracket | TokenKind::RightBrace) {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    token.span,
                    "mismatched closing delimiter",
                ));
            }
            if token.kind == TokenKind::RightParenthesis {
                let element_type =
                    scalar_type.ok_or_else(|| self.syntax_here("empty vector requires type"))?;
                return Ok(Expr {
                    kind: ExprKind::Vector(element_type, values),
                    span: SourceSpan {
                        begin: open.span.begin,
                        end: token.span.end,
                    },
                });
            }
            let value = token.value.ok_or_else(|| {
                Error::at_span(
                    ErrorKind::SyntaxError,
                    token.span,
                    "vector elements must be scalar literals",
                )
            })?;
            let element_type = value
                .scalar_type()
                .ok_or_else(|| self.syntax_here("invalid vector element"))?;
            if scalar_type.is_some_and(|current| current != element_type) {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    token.span,
                    "vector elements must have one scalar type",
                ));
            }
            scalar_type = Some(element_type);
            values.push(value);
            self.require_sibling_separator_or_close(TokenKind::RightParenthesis)?;
        }
    }

    fn parse_tuple(&mut self) -> Result<Expr, Error> {
        let open = self
            .take_kind(TokenKind::LeftBracket)
            .ok_or_else(|| self.eof_error("expected '['"))?;
        let mut elements = Vec::new();
        loop {
            self.skip_inside();
            if let Some(close) = self.take_kind(TokenKind::RightBracket) {
                return Ok(Expr {
                    kind: ExprKind::Tuple(elements),
                    span: SourceSpan {
                        begin: open.span.begin,
                        end: close.span.end,
                    },
                });
            }
            elements.push(self.parse_expr(true)?);
            self.require_sibling_separator_or_close(TokenKind::RightBracket)?;
        }
    }

    fn parse_name_expr(&mut self) -> Result<Expr, Error> {
        let name = self.take().ok_or_else(|| self.eof_error("expected name"))?;
        if name.spelling == "fanout" {
            return self.parse_fanout(name);
        }
        if let Some(position) = self
            .parameters
            .iter()
            .position(|parameter| parameter.name == name.spelling)
            && self.peek_kind() != Some(TokenKind::LeftBracket)
        {
            return Ok(Expr {
                kind: ExprKind::Parameter(position),
                span: name.span,
            });
        }
        if self.peek_kind() == Some(TokenKind::LeftBracket) {
            let _open = self.take().ok_or_else(|| self.eof_error("expected '['"))?;
            let mut arguments = Vec::new();
            loop {
                self.skip_inside();
                if let Some(close) = self.take_kind(TokenKind::RightBracket) {
                    return Ok(Expr {
                        kind: ExprKind::Call {
                            name: name.spelling,
                            syntax: CallSyntax::Direct,
                            arguments,
                            name_span: name.span,
                        },
                        span: SourceSpan {
                            begin: name.span.begin,
                            end: close.span.end,
                        },
                    });
                }
                arguments.push(self.parse_expr(true)?);
                self.require_sibling_separator_or_close(TokenKind::RightBracket)?;
            }
        }
        if self.peek_kind() == Some(TokenKind::Space) {
            self.skip_spaces();
            let argument = self.parse_expr(false)?;
            return Ok(Expr {
                span: SourceSpan {
                    begin: name.span.begin,
                    end: argument.span.end,
                },
                kind: ExprKind::Call {
                    name: name.spelling,
                    syntax: CallSyntax::Prefix,
                    arguments: vec![argument],
                    name_span: name.span,
                },
            });
        }
        if matches!(
            self.peek_kind(),
            None | Some(
                TokenKind::Comment
                    | TokenKind::Newline
                    | TokenKind::RightParenthesis
                    | TokenKind::RightBracket
                    | TokenKind::RightBrace
            )
        ) {
            return Ok(Expr {
                span: name.span,
                kind: ExprKind::UnresolvedName {
                    name: name.spelling,
                    name_span: name.span,
                },
            });
        }
        if !self.has_separator() {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                name.span,
                "primitive name requires bracketed or unary prefix application",
            ));
        }
        Err(Error::at_span(
            ErrorKind::SyntaxError,
            name.span,
            "primitive name requires bracketed or unary prefix application",
        ))
    }

    fn parse_fanout(&mut self, keyword: Token) -> Result<Expr, Error> {
        if self.fanout_depth != 0 {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                keyword.span,
                "nested fanout is not supported",
            ));
        }
        if self.peek_kind() != Some(TokenKind::LeftBracket) {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                self.peek()
                    .map_or_else(|| self.insertion_span(), |token| token.span),
                "expected adjacent '[' after fanout",
            ));
        }
        self.index += 1;
        self.fanout_depth += 1;
        self.skip_inside();
        if self.peek_kind() == Some(TokenKind::RightBracket) {
            return Err(self.syntax_here("expected fanout operand"));
        }
        let operand = self.parse_expr(true)?;
        if !self.has_separator() {
            return Err(self.syntax_here("expected at least one fanout branch"));
        }
        let mut branches = Vec::new();
        loop {
            self.skip_inside();
            if let Some(close) = self.take_kind(TokenKind::RightBracket) {
                if branches.is_empty() {
                    return Err(Error::at_span(
                        ErrorKind::SyntaxError,
                        close.span,
                        "expected at least one fanout branch",
                    ));
                }
                self.fanout_depth -= 1;
                let expression = Expr {
                    span: SourceSpan {
                        begin: keyword.span.begin,
                        end: close.span.end,
                    },
                    kind: ExprKind::Fanout {
                        operand: Box::new(operand),
                        branches,
                    },
                };
                return Ok(expression);
            }
            let open = self
                .take()
                .ok_or_else(|| self.eof_error("missing fanout close"))?;
            if open.kind != TokenKind::LeftBrace {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    open.span,
                    "expected fanout branch opening '{'",
                ));
            }
            self.skip_inside();
            if self.peek_kind() == Some(TokenKind::RightBrace) {
                return Err(self.syntax_here("expected fanout branch body"));
            }
            self.branch_depth += 1;
            let branch = self.parse_expr(true)?;
            self.branch_depth -= 1;
            self.skip_inside();
            let close = self
                .take()
                .ok_or_else(|| self.eof_error("missing branch close"))?;
            if close.kind != TokenKind::RightBrace {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    close.span,
                    "expected fanout branch closing '}'",
                ));
            }
            if !matches!(branch.kind, ExprKind::Call { .. }) {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    branch.span,
                    "fanout branch root must be a primitive call",
                ));
            }
            let (placeholders, first_placeholder, owned_position) =
                inspect_branch_placeholders(&branch);
            if placeholders != 1 {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    first_placeholder.unwrap_or(branch.span),
                    "fanout branch must contain exactly one placeholder",
                ));
            }
            if owned_position {
                return Err(Error::at_span(
                    ErrorKind::SyntaxError,
                    first_placeholder.unwrap_or(branch.span),
                    "fanout placeholder cannot appear in an owned aggregate",
                ));
            }
            branches.push(branch);
            self.require_sibling_separator_or_close(TokenKind::RightBracket)?;
        }
    }

    fn take(&mut self) -> Option<Token> {
        let token = self.tokens.get(self.index).cloned()?;
        self.index += 1;
        Some(token)
    }

    fn take_kind(&mut self, kind: TokenKind) -> Option<Token> {
        if self.peek_kind() == Some(kind) {
            self.take()
        } else {
            None
        }
    }

    fn take_if(&mut self, kind: TokenKind) -> bool {
        self.take_kind(kind).is_some()
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.index)
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(|token| token.kind)
    }

    fn at_end(&self) -> bool {
        self.index == self.tokens.len()
    }

    fn is_name(&self, name: &str) -> bool {
        self.peek()
            .is_some_and(|token| token.kind == TokenKind::Name && token.spelling == name)
    }

    fn skip_spaces(&mut self) {
        while self.peek_kind().is_some_and(is_horizontal_trivia) {
            self.index += 1;
        }
    }

    fn skip_newlines_and_spaces(&mut self) {
        while self.peek_kind().is_some_and(is_trivia) {
            self.index += 1;
        }
    }

    fn skip_inside(&mut self) {
        self.skip_newlines_and_spaces();
    }

    fn skip_comment_trivia(&mut self) {
        let mut index = self.index;
        let mut saw_comment = false;
        while let Some(token) = self.tokens.get(index).filter(|token| is_trivia(token.kind)) {
            saw_comment |= token.kind == TokenKind::Comment;
            index += 1;
        }
        if saw_comment {
            self.index = index;
        }
    }

    fn has_separator(&self) -> bool {
        self.peek_kind().is_some_and(is_trivia)
    }

    fn require_sibling_separator_or_close(&self, close: TokenKind) -> Result<(), Error> {
        if self.has_separator() || self.peek_kind() == Some(close) {
            return Ok(());
        }
        let Some(token) = self.peek() else {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                self.insertion_span(),
                "missing closing delimiter",
            ));
        };
        if token.kind == TokenKind::Invalid {
            return Err(Error::at_span(
                ErrorKind::InvalidByte,
                token.span,
                "invalid source byte",
            ));
        }
        if matches!(
            token.kind,
            TokenKind::RightParenthesis | TokenKind::RightBracket | TokenKind::RightBrace
        ) {
            return Err(Error::at_span(
                ErrorKind::SyntaxError,
                token.span,
                "mismatched closing delimiter",
            ));
        }
        Err(Error::at_span(
            ErrorKind::SyntaxError,
            token.span,
            "sibling expressions require separating whitespace",
        ))
    }

    fn insertion_span(&self) -> SourceSpan {
        let location = self
            .tokens
            .last()
            .map_or(SourceLocation::start(), |token| token.span.end);
        SourceSpan {
            begin: location,
            end: location,
        }
    }

    fn syntax_here(&self, message: impl Into<String>) -> Error {
        let span = self
            .peek()
            .map_or_else(|| self.insertion_span(), |token| token.span);
        Error::at_span(ErrorKind::SyntaxError, span, message)
    }

    fn eof_error(&self, message: impl Into<String>) -> Error {
        self.syntax_here(message)
    }
}

fn token_scalar_type(kind: TokenKind) -> Option<ScalarType> {
    match kind {
        TokenKind::BoolType => Some(ScalarType::Bool),
        TokenKind::IntType => Some(ScalarType::Int),
        TokenKind::DoubleType => Some(ScalarType::Double),
        _ => None,
    }
}

fn is_horizontal_trivia(kind: TokenKind) -> bool {
    matches!(kind, TokenKind::Space | TokenKind::Comment)
}

fn is_trivia(kind: TokenKind) -> bool {
    is_horizontal_trivia(kind) || kind == TokenKind::Newline
}

fn parameter_syntax_error(
    reason: ParameterErrorReason,
    primary_span: SourceSpan,
    context_span: SourceSpan,
    related_span: Option<SourceSpan>,
) -> Error {
    let mut error = Error::at_span(
        ErrorKind::SyntaxError,
        primary_span,
        "invalid parameter header",
    );
    error.parameter = Some(ParameterErrorContext {
        reason,
        primary_span,
        context_span,
        related_span,
    });
    error
}

fn parameter_declaration_error(
    reason: ParameterErrorReason,
    primary_span: SourceSpan,
    context_span: SourceSpan,
    related_span: SourceSpan,
) -> Error {
    let mut error = Error::at_span(
        ErrorKind::ParameterError,
        primary_span,
        "invalid parameter declaration",
    );
    error.parameter = Some(ParameterErrorContext {
        reason,
        primary_span,
        context_span,
        related_span: Some(related_span),
    });
    error
}

pub(crate) fn validate_parameter_declarations(program: &Program) -> Result<(), Error> {
    let Some(header) = program.parameter_header else {
        return Ok(());
    };
    for (index, parameter) in program.parameters.iter().enumerate() {
        if let Some(earlier) = program.parameters[..index]
            .iter()
            .find(|earlier| earlier.name == parameter.name)
        {
            return Err(parameter_declaration_error(
                ParameterErrorReason::DuplicateParameterName,
                parameter.name_span,
                header,
                earlier.name_span,
            ));
        }
        if reserved_parameter_name(&parameter.name) {
            return Err(parameter_declaration_error(
                ParameterErrorReason::ReservedParameterName,
                parameter.name_span,
                header,
                parameter.name_span,
            ));
        }
    }
    Ok(())
}

fn syntactic_parameter_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    bytes.next().is_some_and(|byte| byte.is_ascii_lowercase())
        && bytes.all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn reserved_parameter_name(name: &str) -> bool {
    matches!(
        name,
        "true"
            | "false"
            | "inf"
            | "nan"
            | "parameters"
            | "fanout"
            | "inc"
            | "dec"
            | "neg"
            | "abs"
            | "add"
            | "sub"
            | "mul"
            | "equals"
            | "not_equals"
            | "not"
            | "and"
            | "or"
            | "odd"
            | "even"
            | "is_positive"
            | "is_negative"
            | "less_than"
            | "greater_than"
            | "iota"
    )
}

fn inspect_branch_placeholders(expression: &Expr) -> (usize, Option<SourceSpan>, bool) {
    let mut pending = vec![(expression, false)];
    let mut count = 0usize;
    let mut first = None;
    let mut owned_position = false;
    while let Some((current, owned)) = pending.pop() {
        match &current.kind {
            ExprKind::Placeholder => {
                count = count.saturating_add(1);
                first.get_or_insert(current.span);
                owned_position |= owned;
            }
            ExprKind::Tuple(elements) => {
                pending.extend(elements.iter().rev().map(|element| (element, true)));
            }
            ExprKind::Call { arguments, .. } => {
                pending.extend(arguments.iter().rev().map(|argument| (argument, owned)));
            }
            ExprKind::Fanout { operand, branches } => {
                pending.push((operand, owned));
                pending.extend(branches.iter().rev().map(|branch| (branch, owned)));
            }
            ExprKind::Literal(_)
            | ExprKind::Vector(_, _)
            | ExprKind::DeepTuple { .. }
            | ExprKind::UnaryChain { .. }
            | ExprKind::Parameter(_)
            | ExprKind::UnresolvedName { .. } => {}
        }
    }
    (count, first, owned_position)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quoted_fields(line: &str) -> Vec<String> {
        let mut fields = Vec::new();
        let bytes = line.as_bytes();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != b'"' {
                index += 1;
                continue;
            }
            index += 1;
            let mut current = Vec::new();
            while index < bytes.len() && bytes[index] != b'"' {
                if bytes[index] != b'\\' {
                    current.push(bytes[index]);
                    index += 1;
                    continue;
                }
                index += 1;
                assert!(index < bytes.len(), "unterminated fixture escape");
                match bytes[index] {
                    b'n' => current.push(b'\n'),
                    b'r' => current.push(b'\r'),
                    b't' => current.push(b'\t'),
                    b'x' => {
                        assert!(index + 2 < bytes.len(), "short fixture hex escape");
                        let digits = std::str::from_utf8(&bytes[index + 1..index + 3])
                            .expect("fixture hex utf8");
                        current.push(u8::from_str_radix(digits, 16).expect("fixture hex byte"));
                        index += 2;
                    }
                    other => current.push(other),
                }
                index += 1;
            }
            if index < bytes.len() {
                fields.push(String::from_utf8(current).expect("fixture string utf8"));
                index += 1;
            } else {
                panic!("unterminated fixture string");
            }
        }
        fields
    }

    #[test]
    fn parses_literals_calls_tuples_parameters_and_fanout() {
        let source = "parameters[n Int factor Double]\n\
                      add[n factor]\n\
                      add [1 2]\n\
                      fanout[iota[n] {inc[_]} {add[_ 2]}]\n";
        let program = parse(source).expect("valid program");
        assert_eq!(program.parameters.len(), 2);
        assert_eq!(program.roots.len(), 3);
    }

    #[test]
    fn arbitrary_file_extension_is_outside_the_syntax() {
        assert!(parse("inc[1]\n").is_ok());
    }

    #[test]
    fn authored_normative_parser_fixture_corpus() {
        let corpus = include_str!("../tests/fixtures/rewrite_conformance_fixture.inc");
        let (valid, invalid) = corpus
            .split_once("const RewriteInvalidFixture rewrite_invalid_fixtures[] = {")
            .expect("fixture sections");
        let mut valid_count = 0;
        for line in valid.lines().map(str::trim) {
            if !line.starts_with("{\"") {
                continue;
            }
            let fields = quoted_fields(line);
            assert!(fields.len() >= 2, "{line}");
            parse(&fields[1])
                .unwrap_or_else(|error| panic!("valid fixture '{}' failed: {error:?}", fields[0]));
            valid_count += 1;
        }
        let mut invalid_count = 0;
        for line in invalid.lines().map(str::trim) {
            if !line.starts_with("{\"") {
                continue;
            }
            let fields = quoted_fields(line);
            assert!(fields.len() >= 2, "{line}");
            let error = parse(&fields[1]).expect_err(&fields[0]);
            let expected_kind = if line.contains("RewriteParseError::invalid_byte") {
                ErrorKind::InvalidByte
            } else if line.contains("RewriteParseError::malformed_literal") {
                ErrorKind::MalformedLiteral
            } else if line.contains("RewriteParseError::literal_range") {
                ErrorKind::LiteralRangeError
            } else {
                ErrorKind::SyntaxError
            };
            assert_eq!(error.kind, expected_kind, "{}", fields[0]);
            let span_text = line.split("{{").nth(1).expect("primary span");
            let offsets: Vec<usize> = span_text
                .split(|character: char| !character.is_ascii_digit())
                .filter(|part| !part.is_empty())
                .take(4)
                .map(|part| part.parse().expect("fixture offset"))
                .collect();
            assert!(offsets.len() >= 4, "{line}");
            let span = error.span.expect("parser errors carry a primary span");
            assert_eq!(span.begin.offset, offsets[0], "{}", fields[0]);
            assert_eq!(span.end.offset, offsets[3], "{}", fields[0]);
            invalid_count += 1;
        }
        assert_eq!(valid_count + invalid_count, 117);
    }

    #[test]
    fn rejects_noncanonical_numbers() {
        for source in ["01", "-0", "+1", "1.", ".5"] {
            assert!(parse(source).is_err(), "{source}");
        }
    }

    #[test]
    fn tokenizes_utf8_comments_without_retaining_their_text() {
        let source = "1# café\r\n2#終";
        let tokens = tokenize(source);
        assert_eq!(
            tokens.iter().map(|token| token.kind).collect::<Vec<_>>(),
            vec![
                TokenKind::Int,
                TokenKind::Comment,
                TokenKind::Newline,
                TokenKind::Int,
                TokenKind::Comment,
            ]
        );
        let comments = tokens
            .iter()
            .filter(|token| token.kind == TokenKind::Comment)
            .collect::<Vec<_>>();
        assert!(comments.iter().all(|token| token.spelling.is_empty()));
        assert_eq!(
            comments[0].span.begin.offset,
            source.find('#').unwrap_or(0) + 1
        );
        assert_eq!(
            comments[0].span.end.offset,
            source.find('\r').unwrap_or(0) + 1
        );
        assert_eq!(comments[1].span.end.offset, source.len() + 1);
        assert_eq!(tokens[2].span.begin.line, 1);
        assert_eq!(tokens[3].span.begin.line, 2);
    }

    #[test]
    fn comments_are_trivia_in_headers_delimiters_fanout_and_at_eof() {
        let source = "# prologue\r\n\
                      parameters[# header\n\
                      n# name\n\
                       Int # type\n\
                      ]# header tail\n\
                      add[# arguments\n\
                      n# adjacent\n\
                       1]# root tail\n\
                      fanout[# operand\n\
                      n # branch separator\n\
                       {inc[# branch body\n\
                      _]} # fanout tail\n\
                      ]\n\
                      # comentário final";
        let program = parse(source).expect("comments are accepted as trivia");
        assert_eq!(program.parameters.len(), 1);
        assert_eq!(program.roots.len(), 2);
    }

    #[test]
    fn typed_empty_vectors_accept_comment_trivia_and_preserve_close_diagnostics() {
        for (source, expected_type) in [
            ("Bool(# LF\n)", ScalarType::Bool),
            ("Int( # CRLF\r\n )", ScalarType::Int),
            ("Double(\n# UTF-8 🦀\n)", ScalarType::Double),
        ] {
            let program = parse(source).expect("commented typed empty");
            let root = program.roots.first().expect("typed empty root");
            match &root.kind {
                ExprKind::Vector(actual_type, values) => {
                    assert_eq!(*actual_type, expected_type);
                    assert!(values.is_empty());
                }
                _ => panic!("expected typed empty vector"),
            }
            assert_eq!(root.span.begin.offset, 1);
            assert_eq!(root.span.end.offset, source.len() + 1);
        }

        let eof_source = "Int(# no close";
        let eof_error = parse(eof_source).expect_err("commented typed empty missing close");
        assert_eq!(eof_error.kind, ErrorKind::SyntaxError);
        assert_eq!(eof_error.message, "missing closing delimiter");
        let eof_span = eof_error.span.expect("missing close insertion span");
        assert_eq!(eof_span.begin.offset, eof_source.len() + 1);
        assert_eq!(eof_span.begin, eof_span.end);

        let invalid_close = parse("Bool(# comment\r\n]").expect_err("invalid typed empty close");
        assert_eq!(invalid_close.kind, ErrorKind::SyntaxError);
        assert_eq!(
            invalid_close.message,
            "vector elements must be scalar literals"
        );
        let invalid_span = invalid_close.span.expect("invalid close type span");
        assert_eq!(invalid_span.begin.offset, 1);
        assert_eq!(invalid_span.end.offset, 5);

        let whitespace_only = parse("Int( )").expect_err("whitespace-only typed empty");
        assert_eq!(whitespace_only.kind, ErrorKind::SyntaxError);
        assert_eq!(
            whitespace_only.message,
            "vector elements must be scalar literals"
        );
        let whitespace_span = whitespace_only.span.expect("whitespace type span");
        assert_eq!(whitespace_span.begin.offset, 1);
        assert_eq!(whitespace_span.end.offset, 4);
    }

    #[test]
    fn eof_comment_preserves_missing_delimiter_insertion_span() {
        let source = "inc[1# no close 🦀";
        let error = parse(source).expect_err("missing close");
        assert_eq!(error.kind, ErrorKind::SyntaxError);
        assert_eq!(error.message, "expected an expression");
        let span = error.span.expect("insertion span");
        assert_eq!(span.begin.offset, source.len() + 1);
        assert_eq!(span.begin, span.end);
    }

    #[test]
    fn no_comment_diagnostic_is_unchanged() {
        let error = parse("inc[1").expect_err("missing close");
        assert_eq!(error.kind, ErrorKind::SyntaxError);
        assert_eq!(error.message, "missing closing delimiter");
        let span = error.span.expect("insertion span");
        assert_eq!(span.begin.offset, 6);
        assert_eq!(span.begin, span.end);
    }
}
