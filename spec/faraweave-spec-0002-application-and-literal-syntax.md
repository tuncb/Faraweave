# FARAWEAVE-SPEC-0002: Rewrite Application and Literal Syntax

**Status:** Proposed

**Related issue:** [#24](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/24)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Structural tuples:** [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)

**Explicit sequential fan-out:** [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Target:** Faraweave language rewrite; scalar literals, rank-1 vector literals,
and primitive application

**Compatibility:** This syntax deliberately replaces, rather than extends,
the bootstrap Level 1 grammar.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

An implementation may organize its tokenizer and parser differently from the
productions below, but it must accept and reject the same source and produce the
same expression boundaries and source spans.

## 2. Outcome and scope

This specification defines one source grammar for the initial rewrite language:

- generic bracketed primitive application;
- unary prefix application, including `inc 5`;
- `Bool`, signed 64-bit `Int`, and IEEE 754 binary64 `Double` literals;
- homogeneous nonempty rank-1 vector literals;
- typed empty `Bool`, `Int`, and `Double` vector literals;
- whitespace, delimiter, nesting, line-ending, and trailing-input behavior; and
- deterministic source spans for parsing and structured diagnostics.

The initial primitive set remains `inc`, `add`, `equals`, `not`, and `iota`, as
defined by FARAWEAVE-SPEC-0001 and its implementation plan. The grammar is generic:
a syntactically valid primitive name is parsed without consulting that set.
Primitive lookup and arity validation happen after parsing.

This specification does not add comments, variables, user-defined functions,
partial application, pipelines, trains, infix operators, grouping parentheses,
nested vectors, boxed values, or multidimensional arrays.

FARAWEAVE-SPEC-0005 extends the complete-program grammar with one parameter header
and immutable scalar parameter-reference expressions. Those references are not
variables and do not add assignment or local scope.

FARAWEAVE-SPEC-0006 extends expressions with bracketed structural tuple literals
and amends prefix call preparation. Its compatibility section identifies every
statement below that it supersedes; all unrelated Level 2 syntax remains in
force.

FARAWEAVE-SPEC-0007 additionally extends expressions with the reserved adjacent
`fanout[...]` construct, brace-delimited primitive-call branch templates, and a
branch-local `_` placeholder. Its grammar and structural-error order supersede
this document only inside that construct.

## 3. Source bytes, lines, and positions

Faraweave syntax uses ASCII terminals. A source may be transported as UTF-8, but a
non-ASCII byte cannot form a token in this grammar and produces an invalid-byte
diagnostic.

The accepted line terminators are:

```text
LF     U+000A, encoded as 0A
CRLF   U+000D U+000A, encoded as 0D 0A
```

CRLF is one line terminator. A bare CR is invalid.

Every public source position contains one-based byte offset, line, and byte
column coordinates. The first source byte is offset 1, line 1, column 1. A tab
occupies one byte and advances the byte column by one; columns do not expand to
visual tab stops. LF and CRLF advance the line once and reset the column to 1.
The end-of-source insertion position has byte offset `source_size + 1`.

A source span is half-open:

```text
[begin, end)
```

Both positions use the coordinate rules above. A zero-length span has equal
begin and end positions and identifies an insertion point.

## 4. Lexical elements

### 4.1 Whitespace

Horizontal whitespace is ASCII space or tab:

```text
H = " " | "\t"
```

Inside `[...]` and `(...)`, a separator is one or more horizontal whitespace or
line terminators. Outside delimiters, a line terminator ends a root expression.
There is no comment syntax in this language level.

```text
separator = (H | line_terminator) { H | line_terminator }
optional_separator = { H | line_terminator }
line_terminator = LF | CRLF
```

### 4.2 Primitive names

A primitive name is:

```text
primitive_name = lowercase_letter { lowercase_letter | digit | "_" }
lowercase_letter = "a" ... "z"
digit = "0" ... "9"
```

Names are ASCII and case-sensitive. `true`, `false`, `inf`, and `nan` are
reserved literal words and cannot be primitive names. `Bool`, `Int`, and
`Double` are reserved type names and are valid only in the typed-empty forms in
section 9. FARAWEAVE-SPEC-0007 additionally reserves `fanout` as a construct keyword;
it cannot name a primitive even though its bytes match `primitive_name`.

An unknown but syntactically valid primitive name still forms a primitive-call
node. Later primitive lookup reports the unknown name at its primitive-name
span. The tokenizer must not assign one token kind per built-in primitive.

### 4.3 Token boundaries

Adjacent sibling expressions require a separator even when their token shapes
would otherwise be distinguishable:

```text
add[iota[3] 10]     valid
add[iota[3]10]      invalid: missing separator before 10
```

Whitespace is not part of an adjacent bracketed application:

```text
add[1 2]            valid
add [1 2]           not an adjacent bracketed application
```

At this specification level the second form is invalid. FARAWEAVE-SPEC-0006
deliberately supersedes that rejection: `[` in expression position begins a
tuple, so the form becomes prefix application to a tuple operand. It does not
become an alternate spelling of adjacent bracket application.

Whitespace is required between a primitive name and the operand of a unary
prefix application:

```text
inc 5               valid
inc5                 one primitive name, not an application
```

Commas, semicolons, braces, and operator punctuation do not separate ordinary
expressions. They are invalid bytes in this grammar. FARAWEAVE-SPEC-0007 introduces
braces only as explicit fan-out branch delimiters; it does not make them general
grouping or separator punctuation.

## 5. Programs and root expressions

A program contains zero or more root expressions. Empty input, whitespace-only
input, and blank lines are valid and contain zero roots.

At delimiter depth zero:

1. a nonblank logical record contains exactly one root expression;
2. after that expression, only horizontal whitespace followed by a line
   terminator or end of source is permitted;
3. a line terminator ends the logical record; and
4. one or more blank lines may occur between roots.

A missing final line terminator is valid. LF and CRLF programs have identical
parse structure and line/column results after each terminator.

A line terminator encountered while `[` or `(` remains open is separator
whitespace rather than a root terminator. Therefore a delimited expression may
span lines:

```text
add[
  1
  inc 2
]
```

Once the closing `]` returns delimiter depth to zero, only horizontal
whitespace may precede that record's line terminator or end of source.

Two roots cannot share one logical record:

```text
inc 1 inc 2
```

This parses `inc 1` and then reports trailing input at the second `inc`; it does
not create two roots.

## 6. Expression grammar

The expression forms are:

```text
expression = scalar_literal
           | vector_literal
           | typed_empty_vector
           | bracket_application
           | prefix_application
```

FARAWEAVE-SPEC-0005 adds parameter references on complete-program surfaces.
FARAWEAVE-SPEC-0006 additionally adds `tuple_literal`; its brackets are distinguished
from an adjacent application bracket by expression position.
FARAWEAVE-SPEC-0007 additionally adds `fanout_expression`; its reserved keyword,
outer brackets, branch braces, and `_` placeholder have no ordinary-call parse.

There are no infix expressions and no grouping expression. Parentheses always
denote a vector literal. Consequently Faraweave has no operator-precedence table.

Delimiter nesting is explicit. Bracketed applications may contain other
applications and vector literals. Vector literals contain scalar literals only,
so parentheses do not nest in this language level.

## 7. Primitive application

### 7.1 Bracketed application

The general application form is:

```text
bracket_application = primitive_name "[" call_interior "]"
call_interior = optional_separator
                [ expression { separator expression } ]
                optional_separator
```

Whitespace and line terminators may appear after `[` and before `]`. A
zero-argument call such as `add[]` has one valid parse and is rejected later by
ordinary arity validation. Arguments remain in source order.

Examples:

```text
inc[5]
add[1 2]
inc[iota[3]]
add[10 (1 2 3)]
equals[true (false true)]
```

### 7.2 Unary prefix application

Unary prefix application preserves the concise Level 1 and Anka-inspired form:

```text
prefix_application = primitive_name horizontal_separator expression
horizontal_separator = H { H }
```

It is syntax with exactly one operand:

```text
inc 5          == inc[5]
inc inc 5      == inc[inc[5]]
inc iota 3     == inc[iota[3]]
inc (1 2 3)    == inc[(1 2 3)]
```

Prefix application associates to the right because its operand is a complete
expression. The horizontal separator cannot contain a line terminator. The
operand itself may contain balanced delimiters that continue across later
lines.

At this specification level the operand supplies one semantic argument
regardless of the primitive's declared arity. Thus `add 1` parses as a
one-argument call and later produces an `ArityError`; `add 1 2` parses `add 1`
followed by invalid trailing input. FARAWEAVE-SPEC-0006 preserves the one-operand
grammar but supersedes the one-semantic-argument rule when that operand has a
static tuple type: its immediate elements become zero or more semantic
arguments before ordinary call validation.

### 7.3 No implicit application or precedence

A completed literal, vector, or bracketed call does not consume a following
expression implicitly. Faraweave does not infer application from a primitive's
registered arity. It does not provide prefix pipelines, composition, trains, or
infix precedence in this grammar.

## 8. Scalar literals

```text
scalar_literal = bool_literal
               | int_literal
               | finite_double
               | special_double
```

### 8.1 Bool

The only `Bool` literals are:

```text
bool_literal = "false" | "true"
```

They are lowercase and must end at a token boundary. Variants such as `False`,
`TRUE`, and `true0` are not Bool literals.

### 8.2 Int

An `Int` literal is canonical signed decimal:

```text
nonzero_digit = "1" ... "9"
unsigned_int = "0" | nonzero_digit { digit }
int_literal = unsigned_int | "-" nonzero_digit { digit }
```

Consequences:

- there is no leading plus sign;
- zero is spelled only `0`;
- `-0` is rejected because `Int` has no signed zero;
- a nonzero magnitude has no leading zeroes;
- decimal underscores and non-decimal bases are rejected; and
- the mathematical value must be in
  `-9223372036854775808` through `9223372036854775807`.

Grammar recognition and range validation must not depend on a host integer
parser accepting a prefix, selecting a base, or exposing a particular overflow
value. The complete literal spelling is consumed and checked against the
normative range.

### 8.3 Finite Double

A finite `Double` literal contains either a decimal point or an exponent:

```text
unsigned_mantissa = "0" | nonzero_digit { digit }
fraction = "." digit { digit }
exponent = ("e" | "E") [ "+" | "-" ] digit { digit }

finite_double = [ "-" ] unsigned_mantissa
                ( fraction [ exponent ] | exponent )
```

Examples:

```text
0.0
-0.0
2.5
1e3
1.25E-2
1.0e+10
```

At least one digit is required on both sides of a decimal point. Therefore
`.5` and `1.` are invalid. A leading plus sign, leading mantissa zeroes,
underscores, hexadecimal floating syntax, locale-specific separators, and
suffixes are invalid.

The mathematical decimal value is rounded once to the nearest IEEE 754
binary64 value using round-to-nearest, ties-to-even. A negative decimal that
rounds to zero retains negative zero. Underflow to signed zero is permitted.
A finite spelling whose magnitude rounds to infinity produces a
`LiteralRangeError`; infinity must use an explicit special spelling.

The conversion contract is independent of the host locale and host library.
An implementation may use a host conversion routine only if it proves complete
token consumption and the result required above on every supported platform.

### 8.4 Special Double

The special binary64 literals are exactly:

```text
inf
-inf
nan
```

`+inf`, `-nan`, capitalization variants, and NaN payload syntax are invalid.
`nan` constructs a NaN whose payload and sign are not language-observable, as
required by FARAWEAVE-SPEC-0001.

### 8.5 Malformed numeric candidates

The tokenizer must not accept a valid numeric prefix and silently reinterpret
the remaining bytes. Each of the following is one malformed-literal condition,
reported at the complete offending candidate rather than delegated to host
parser behavior:

| Source | Reason |
| --- | --- |
| `+1` | Leading plus is not accepted. |
| `-0` | Int zero has one spelling. |
| `00` | Leading zero. |
| `01.0` | Leading mantissa zero. |
| `.5` | Missing integral digits. |
| `1.` | Missing fractional digits. |
| `1e` | Missing exponent digits. |
| `1e+` | Missing exponent digits. |
| `1_000` | Underscore is not accepted. |
| `0x10` | Non-decimal base. |
| `1.0f` | Suffix is not accepted. |
| `9223372036854775808` | Int range overflow. |
| `1e9999` | Finite Double range overflow. |

## 9. Vector literals

### 9.1 Nonempty vectors

A nonempty vector literal is:

```text
vector_literal = "(" optional_separator scalar_literal
                 { separator scalar_literal } optional_separator ")"
```

The elements must be scalar literals; an application or another vector cannot
appear as an element. At least one separator is required between elements.
Commas are invalid.

The first element establishes the vector's element type. Every later element
must have exactly that same scalar type:

```text
(1 2 3)              Vector<Int>
(1.0 2.5 3.0)        Vector<Double>
(false true false)   Vector<Bool>
(10)                 singleton Vector<Int>, not grouping
```

Vector construction performs no implicit numeric promotion. `(1 2.0)` and
`(1.0 2)` are invalid heterogeneous literals. This keeps the implicit
`Int -> Double` conversion scoped to elementwise primitive signature selection,
as specified by FARAWEAVE-SPEC-0001. A homogeneous Double vector spells every
integral-valued element as a Double, for example `(1.0 2.5)`.

### 9.2 Typed empty vectors

Empty vectors retain their element type in source:

```text
typed_empty_vector = "Bool()" | "Int()" | "Double()"
```

These spellings contain no internal whitespace. They produce distinct
`Vector<Bool>`, `Vector<Int>`, and `Vector<Double>` values. Ordinary canonical
formatting still prints each value as `()`, as required by FARAWEAVE-SPEC-0001.

Bare `()` is recognized as an untyped-empty-vector error and never guesses an
element type. Semantic forms such as `Vector<Int>()` are specification notation,
not Faraweave source.

## 10. Delimiters and nesting

Opening and closing delimiters must match:

```text
application   [ ... ]
vector        ( ... )
```

Bracketed applications may nest without a language-defined depth limit; active
execution profiles may impose parser resource limits. Parentheses cannot nest
because vector elements are scalar literals only. Empty `[]` denotes a
zero-argument application only when preceded immediately by a primitive name.
FARAWEAVE-SPEC-0006 adds tuple brackets in expression position, including standalone
empty `[]`; immediately adjacent `primitive_name[]` remains a zero-argument
application.
FARAWEAVE-SPEC-0007 adds the reserved `fanout[...]` bracket pair and matching branch
braces. Those braces may nest ordinary balanced call, tuple, and vector
delimiters, but are not expressions outside a fan-out branch list.

A mismatched closing delimiter is reported at that delimiter. If end of source
is reached with an opening delimiter unmatched, the missing-close diagnostic
uses a zero-length span at end of source and retains the opening-delimiter span
as context. No parser recovery may turn bytes after a mismatched delimiter into
a separate successful root.

## 11. Required source spans and diagnostics

### 11.1 Stored spans

Every parsed node retains its complete expression span. A primitive-call node
also retains:

- the primitive-name span;
- the opening- and closing-bracket spans for bracketed syntax, when present;
- the prefix separator span for prefix syntax, when present;
- one complete span per argument in source order; and
- the complete call span.

A scalar literal span includes its sign, decimal point, exponent, or special
spelling. A vector literal span begins at its type name for a typed empty vector
or at `(` for a nonempty vector, and ends after its closing `)`. Each nonempty
vector element retains its scalar-literal span.

Leading and trailing separator whitespace is excluded from an expression or
argument span. Internal whitespace and nested delimiters are included in the
containing expression's span.

### 11.2 Primary diagnostic locations

Syntax diagnostics use these deterministic primary spans:

| Error | Primary span |
| --- | --- |
| Invalid byte or bare CR | The offending byte. |
| Malformed or out-of-range literal | The complete literal candidate. |
| Missing sibling separator | The first byte of the following expression. |
| Mismatched closing delimiter | The offending closing delimiter. |
| Missing closing delimiter | Zero-length end-of-source insertion span. |
| Bare `()` | The complete `()` span. |
| Heterogeneous vector | The first element whose type differs from element 1. |
| Expression after a root | The first trailing-input byte. |
| Unknown primitive | The primitive-name span. |

Structured context additionally retains the containing vector or call span and
the related opening delimiter where applicable.

Post-parse primitive diagnostics use source spans as follows:

- `ArityError` identifies the primitive-name span and carries the complete call
  span plus supplied argument spans;
- `TypeError` and `ShapeMismatch` identify the offending one-based argument's
  span and carry the primitive-name and call spans;
- `ResourceError` identifies the primitive-name span and carries the call span;
  and
- `DomainError` identifies the primitive-name span, carries the call and
  argument spans, and retains any zero-based lifted result index required by
  FARAWEAVE-SPEC-0001.

Diagnostic presentation may render only a primary line and column, but the
structured result must retain the spans above so tests and backends do not need
to recover context from message text.

## 12. Valid and invalid examples

### 12.1 Valid source

```text
true
-9223372036854775808
-0.0
nan
(1 2 3)
Double()
inc 5
inc inc 5
add[1 2.5]
inc iota 3
add[(1 2 3) 10]
equals[true (false true)]
add[Int() 0.5]
```

Each example is one root expression. Equivalent LF and CRLF files, blank lines,
and omission of the last line terminator are valid.

### 12.2 Invalid source

| Source | Required failure |
| --- | --- |
| `False` | Invalid literal/name casing. |
| `-0` | Noncanonical Int zero. |
| `1.` | Malformed Double. |
| `(1 2.0)` | Heterogeneous vector at `2.0`. |
| `()` | Missing empty-vector element type. |
| `Vector<Int>()` | Semantic notation is not source syntax. |
| `((1 2))` | Nested/grouping parentheses are unsupported. |
| `(inc 1)` | Applications cannot be vector elements. |
| `add [1 2]` | Whitespace before application bracket at this language level; superseded by FARAWEAVE-SPEC-0006 as prefix application to a tuple. |
| `add[1, 2]` | Commas are unsupported. |
| `add[1 2` | Missing `]` at end of source. |
| `add[(1 2] 3]` | Mismatched `]` while `)` is required. |
| `add[iota[3]10]` | Missing sibling separator before `10`. |
| `add 1 2` | Trailing input after the unary prefix call. |
| `inc 1 inc 2` | Two roots on one logical record. |

`add[]` and `add 1` are syntactically valid calls. For the initial primitive
table they fail later with `ArityError`, which is distinct from a parse failure.

## 13. Deliberate differences from Anka and Level 1

Faraweave retains [Anka's](https://github.com/tuncb/anka) readable bracketed form
`add[1 2]`, parenthesized vector literals, and concise unary prefix form
`inc 5`. Prefix chains associate to the right, so `inc inc 5` remains compact
and deterministic.

Faraweave deliberately differs by:

- spelling the structural primitive `iota`, not Level 1 and Anka's `ioata`;
- requiring brackets for every multi-argument call;
- treating unary prefix syntax as one-argument application rather than a
  general pipeline or composition mechanism;
- excluding trains, placeholders, user blocks, variables, and implicit
  arity-driven parsing;
- reserving parentheses exclusively for homogeneous rank-1 literals;
- requiring exact element types inside a vector literal; and
- requiring an explicit element type for every empty vector literal.

These are source-language decisions only. Faraweave does not claim behavioral or
architectural compatibility with Anka.

## 14. Required conformance coverage

Tokenizer and parser conformance must include:

1. every primitive-call and literal form in sections 7 through 9;
2. scalar boundaries, signed zero, subnormal rounding, finite binary64 extremes,
   infinities, and NaN;
3. every malformed numeric category in section 8.5;
4. homogeneous empty, singleton, and nonempty vectors of all three element
   types;
5. heterogeneous, nested, application-containing, comma-separated, and untyped
   empty vector failures;
6. zero-, unary-, and multi-argument bracket calls;
7. right-associated unary prefix chains and rejection of implicit binary prefix
   application;
8. nested calls, whitespace after openers and before closers, and required
   sibling separators;
9. empty input, blank lines, LF, CRLF, bare CR, missing final newline, and
   multiline delimited expressions;
10. missing, extra, and mismatched delimiters;
11. trailing input on a root record;
12. valid unknown primitive names and invalid name casing;
13. exact one-based positions and half-open spans for primitives, calls,
    arguments, delimiters, scalar literals, vector literals, and errors; and
14. a unique parse tree or flat-call shape for every required example in
    FARAWEAVE-SPEC-0001.

The conformance fixture must list expected acceptance, node kind, literal or
element type, call arity, argument boundaries, and primary error span rather
than relying only on formatted diagnostic messages.
