# FARAWEAVE-SPEC-0005: Typed Program Parameters and Runtime Arguments

**Status:** Proposed

**Related issue:** [#42](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/42)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Scalar domains:** [FARAWEAVE-SPEC-0003](faraweave-spec-0003-scalar-domain-semantics.md)

**Execution profiles:** [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md)

**Structural tuples:** [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)

**Explicit sequential fan-out:** [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Target:** Faraweave programs, the public evaluator, the file runner, emitted C11,
and native executables

**Compatibility:** This contract deliberately extends the Level 2 language and
execution surfaces without preserving Level 2 invalid-program, evaluator,
emission, or diagnostic behavior.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and
do not override the semantic requirements.

## 2. Outcome and scope

A complete Faraweave program may declare an ordered list of required positional
scalar parameters. Each parameter has one of the existing scalar types:

```text
Bool
Int
Double
```

For example:

```faraweave
parameters[n Int delta Double enabled Bool]
add[iota[n] delta]
not[enabled]
```

This header declares three immutable scalar values in this exact order. The
program requires all three values on every execution, including values for
parameters that are unused by executable roots.

This specification defines:

- the parameter-header grammar, declaration and reference spans, and name rules;
- whole-program static analysis before argument binding;
- ordered typed arguments for public embeddings;
- portable ASCII text arguments for the runner and generated programs;
- structured `ArgumentError` data and deterministic failure precedence;
- `faraweave run`'s `--` boundary and generated-program `argv` behavior;
- dynamic shape, resource, domain, formatting, and output behavior;
- parameter-count representability and execution-profile interaction; and
- a requirement-to-test plan for every execution surface and supported target.

Parameters are a complete-program concept. They are not persistent bindings,
variables, named options, primitive arguments discovered at runtime, or an
untyped process command line.

## 3. Header grammar

### 3.1 Program placement

`parameters` is a reserved program-header keyword. A program may contain at
most one parameter header. When present, it must be the first nonblank logical
record.

Blank records and horizontal whitespace may precede the header. Once the first
nonblank logical record is an executable root, no later record may be a
parameter header. A second header is always invalid.

At delimiter depth zero, the closing `]` completes the header. Only horizontal
whitespace and then a line terminator or end of source may follow it on that
logical record. LF, CRLF, optional final newline, byte positions, and logical
records have the meanings defined by FARAWEAVE-SPEC-0002 sections 3 through 5.

### 3.2 Productions

The header grammar is:

```text
parameter_header = "parameters" "[" parameter_interior "]"

parameter_interior = optional_separator
                     [ parameter_name separator parameter_type
                       { separator parameter_name separator parameter_type } ]
                     optional_separator

parameter_name = lowercase_letter
                 { lowercase_letter | digit | "_" }

parameter_type = "Bool" | "Int" | "Double"
```

`lowercase_letter`, `digit`, `separator`, and `optional_separator` are the
productions from FARAWEAVE-SPEC-0002. Separators inside the brackets may contain LF
or CRLF, so one header may span several physical lines while remaining the
first nonblank logical record.

The keyword and opening bracket must be adjacent:

```text
parameters[n Int]      valid
parameters [n Int]     invalid: whitespace before the header bracket
```

Each declaration is an ordered name/type pair. Commas, colons, semicolons,
equals signs, defaults, and type inference are not accepted.

### 3.3 Empty and malformed interiors

`parameters[]` is valid and declares zero parameters. It has the same required
argument count as an absent header. It does not create a value or executable
root.

A nonempty interior must contain complete name/type pairs:

```text
parameters[n Int delta Double]   valid
parameters[n]                    invalid: missing type after n
parameters[n Int delta]          invalid: missing type after delta
parameters[Int n]                invalid: expected a parameter name first
parameters[n int]                invalid: expected exact type Bool, Int, or Double
```

For a missing name or type immediately before `]` or end of source, the primary
diagnostic span is the zero-length insertion position before that byte. For an
unexpected existing token, the primary span is that complete token. Section 5
defines the complete phase-1 winner algorithm, stable reasons, and exact spans;
implementations must not select a winner from parser accident or recovery order.

### 3.4 Header with no executable roots

A header followed by no executable roots is a valid program:

```faraweave
parameters[n Int enabled Bool]
```

Every execution still requires exactly two arguments and validates both. After
successful binding, execution returns zero root values and publishes zero
stdout bytes. A missing, extra, malformed, or invalid typed argument still
returns `ArgumentError` even though no parameter is referenced.

## 4. Names, references, and immutability

### 4.1 Reserved and unique names

Parameter names are ASCII, case-sensitive, lowercase identifiers. Every name
must be unique in the header and must not equal any of:

- the header keyword `parameters`;
- the construct keyword `fanout` defined by FARAWEAVE-SPEC-0007;
- the literal words `true`, `false`, `inf`, or `nan`;
- the type names `Bool`, `Int`, or `Double`;
- a primitive source name in the primitive descriptor table used to analyze the
  program, including the initial `inc`, `add`, `equals`, `not`, and `iota` names.

The uppercase type names cannot satisfy `parameter_name`, but they remain
reserved so changing identifier grammar cannot silently turn them into legal
parameter names.

A duplicate declaration fails at the later name and retains the earlier name
span as related context. A reserved-word or primitive-name collision fails at
the declaration name. Parameter order is declaration order and is not changed
by name sorting, first use, or root order.

### 4.2 Parameter-reference expression

A declared parameter name written as a bare expression is a parameter
reference:

```text
expression = scalar_literal
           | vector_literal
           | typed_empty_vector
           | parameter_reference
           | bracket_application
           | prefix_application

parameter_reference = parameter_name
```

A reference has the declared scalar type and rank-0 shape. It denotes the same
immutable bound value every time it is evaluated. Repeated and unused
parameters are valid. There is no assignment, mutation, address identity, or
consume-on-read behavior.

The syntactic form determines whether a name is a reference or a call. After
the header `parameters[n Int]`:

| Source root | Interpretation |
| --- | --- |
| `n` | Reference to parameter `n`. |
| `n[1]` | Bracketed primitive-call syntax; `n` is diagnosed as unknown. |
| `n 1` | Prefix primitive-call syntax; `n` is diagnosed as unknown. |

Parameter references are valid roots and primitive-call arguments, including
the operand of unary prefix syntax. FARAWEAVE-SPEC-0002's vector-literal interior
remains scalar literals only, so `(n 1)` is invalid rather than a vector built
from one parameter and one literal.

Parameter declarations cannot use primitive names, and parameter references
cannot name the `parameters` keyword.

### 4.3 Resolution boundary

A bare lowercase name resolves only against the program's parameter header.
There are no local scopes, shadowing, implicit globals, or lookup by argument
text. A bare name absent from the header is `UnknownName` at that name span.
A call name resolves only against the primitive descriptor table and never
against parameter declarations.

The REPL and both `evaluate_expression` overloads reject a parameter header and
a bare parameter reference. A header is rejected at its keyword span as a
program-only construct. A bare name is rejected as `UnknownName`. The REPL does
not retain parameter declarations or argument values between submissions.

## 5. Required source spans and declaration diagnostics

A parsed parameter header retains:

- the complete header span from `p` in `parameters` through the closing `]`;
- the keyword span;
- the opening- and closing-bracket spans;
- each complete declaration span from the first name byte through the last type
  byte, excluding surrounding separators;
- each declaration name span and type span separately; and
- each parameter-reference span plus the corresponding declaration index and
  declaration-name span.

These spans use FARAWEAVE-SPEC-0002's one-based byte coordinates and half-open
representation. Internal separators are included in the complete header and
complete declaration spans but excluded from name, type, and reference spans.

Phase 1 uses the following closed structural-reason vocabulary for malformed or
misplaced parameter headers:

```text
second_parameter_header
parameter_header_after_root
expected_header_open
expected_parameter_name
expected_parameter_type
missing_header_close
unexpected_header_token
trailing_header_bytes
```

The winner is selected independently of parser recovery. Form every applicable
phase-1 candidate, select the candidate with the earliest primary-span start
byte, and, when candidates start at the same byte, select the first reason in
the ordered list above. A zero-length insertion candidate starts at its
insertion byte. An existing offending byte or token always uses a nonempty
half-open span. This ordering makes a second header win over header-after-root
at the same keyword, and makes an expected type win over a missing closing
bracket at the same end-of-source insertion position.

This table is the complete mapping from parameter-header structural failures to
stable reasons and spans. The examples use one-based byte coordinates on one
logical source buffer and half-open `[start,end)` notation.

| Source shape or exact source | Stable reason | Exact primary span | Related context |
| --- | --- | --- | --- |
| Bare `parameters` | `expected_header_open` | End-of-source `[11,11)` | Keyword `[1,11)`. |
| `parameters]` | `expected_header_open` | Offending `]` `[11,12)` | Keyword `[1,11)`. |
| `parameters [n Int]` | `expected_header_open` | First non-adjacent byte, the space `[11,12)` | Keyword `[1,11)`; later `[` is not a recovery winner. |
| `parameters[` | `missing_header_close` | End-of-source `[12,12)` | Opening `[` `[11,12)`. The empty interior is otherwise complete. |
| `parameters[n` | `expected_parameter_type` | End-of-source `[13,13)` | Incomplete declaration `[12,13)`; this beats `missing_header_close` at `[13,13)`. |
| `parameters[n]` | `expected_parameter_type` | Before `]`, `[13,13)` | Incomplete declaration `[12,13)`. |
| `parameters[n Int delta` | `expected_parameter_type` | End-of-source `[23,23)` | Incomplete declaration `[18,23)`; this beats `missing_header_close` at `[23,23)`. |
| A type appears where a name is required, as in `parameters[Int]` | `expected_parameter_name` | Complete `Int` token `[12,15)` | Opening `[` `[11,12)`. |
| A token is not exact `Bool`, `Int`, or `Double`, as in `parameters[n Integer]` | `unexpected_header_token` | Complete `Integer` token `[14,21)` | Declaration name `[12,13)`. |
| Punctuation or another token is not permitted by the production | `unexpected_header_token` | Complete offending token | The declaration or delimiter that established the expectation. |
| A complete pair reaches end of source, as in `parameters[n Int` | `missing_header_close` | End-of-source `[17,17)` | Opening `[` `[11,12)`. |
| Non-whitespace follows the closing bracket on its logical record | `trailing_header_bytes` | Complete first trailing token | Complete header span. |
| `parameters[]\nparameters[]` | `second_parameter_header` | Second keyword `[14,24)` | First header `[1,13)`. |
| `1\nparameters[]` | `parameter_header_after_root` | Keyword `[3,13)` | First root `[1,2)`. |
| `parameters[]\n1\nparameters[]` | `second_parameter_header` | Second keyword `[16,26)` | First header `[1,13)`; this beats `parameter_header_after_root` at `[16,26)`. |

The exact rows above are normative fixtures, not illustrative prose. CRLF and
multiline variants apply the same byte-based algorithm to their actual source
bytes. Once phase 1 succeeds, later diagnostics use these primary spans:

| Failure | Primary span | Related context |
| --- | --- | --- |
| Duplicate name | Later name span | Earlier name span. |
| Reserved or primitive collision | Declaration name span | Colliding category or primitive identity. |
| Unknown bare name | Bare name span | Complete root/call span when applicable. |

Rendered source diagnostics may show one primary line and column, but structured
results must retain the stable reason, spans, and related context above.

## 6. Whole-program static analysis

### 6.1 No binding during analysis

Source analysis never receives, decodes, inspects, or executes concrete runtime
arguments. It produces a typed program with indexed parameter-reference nodes.
Primitive overload selection uses declared parameter types.

Every root must complete all applicable static phases before argument count is
checked or one argument is validated. Therefore a static failure anywhere in
the program precedes every `ArgumentError` and every dynamic failure, including
a dynamic failure in an earlier root.

### 6.2 Static phase order

A conforming implementation applies these whole-program phases in this exact
order:

```text
1. source-byte, lexical, delimiter, parse, and header-placement validation
2. declaration-count and parameter-index representability
3. declaration validation and parameter/primitive name resolution
4. primitive arity validation
5. scalar/container type analysis and signature selection
6. statically knowable shape agreement
```

An invalid primitive descriptor table remains an implementation/configuration
failure before program analysis, as required by FARAWEAVE-SPEC-0001.

Phase 2 has one program-level candidate at the complete header span. Within
phases 1 and 3, declarations, names, and records are considered in source-span
order. Within phases 4 through 6, roots are considered in source order and calls
in each root are considered in left-to-right postorder, so a child is valid
before its parent is analyzed. The first failure in that order is returned. If
two candidates have the same primary span, the earlier item in the phase list
and then the lower one-based argument position wins.

The implementation may fuse passes internally only if it preserves this
observable result. It must not bind arguments after validating only a prefix of
the roots.

The phases are program-wide, not a complete per-root pipeline. For example:

| Root 1 defect | Root 2 defect | Required winner |
| --- | --- | --- |
| `TypeError` | `ArityError` | Root 2 `ArityError`, because phase 4 completes for every root before phase 5 begins. |
| Dynamic `DomainError` | Static `TypeError` | Root 2 `TypeError`, before binding or execution. |
| `ArityError` | `ArityError` | Root 1 error, because roots are ordered by source within the phase. |

Once all static phases succeed, section 11's runtime source order replaces this
category precedence for dynamic failures.

FARAWEAVE-SPEC-0006 preserves phases 1 through 3 and the program-wide
arity-before-type-before-shape category precedence, but makes candidate
discovery dependency-aware for tuple-spreading calls. An outer prefix arity
cannot be formed until its operand has a valid static structural type. Its
section 8.4 is the exact amendment for tuple-capable programs; the examples
above remain unchanged when no tuple-producing expression occurs.

FARAWEAVE-SPEC-0007 preserves that category precedence and adds fan-out branch
dependencies. The complete program and every branch are structurally validated
before execution; a fan-out operand type is derived before substitution at each
branch-local placeholder; and branch candidates are ordered by source branch
and left-to-right postorder as its section 6 defines.

### 6.3 Static and dynamic shape classes

Static shape analysis uses these classes:

```text
StaticScalar
StaticVector(n)
DynamicVector
```

The rules are:

- every scalar literal and parameter reference is `StaticScalar`;
- a vector literal, including a typed empty vector, is `StaticVector(n)` where
  `n` is its literal element count;
- `iota` returns `DynamicVector` even when its input expression is a literal;
- an elementwise call over only scalar arguments is `StaticScalar`;
- an elementwise call with one or more vector arguments is `DynamicVector` when
  any vector argument is dynamic;
- otherwise its vector result is `StaticVector(n)` after equal known lengths
  agree.

For one elementwise call, the first `StaticVector(n)` argument establishes the
known length. The first later `StaticVector(m)` with `m != n` is a static
`ShapeMismatch`, even if another argument is `DynamicVector`. Dynamic vectors
do not establish or change a known length during analysis. At execution, every
dynamic vector must agree with the known length, or the first dynamic vector
establishes the runtime length when no static vector did.

Consequences:

| Source root | Classification |
| --- | --- |
| `add[(1 2) (3 4 5)]` | Static `ShapeMismatch`; `emit-c` and `build` reject it. |
| `add[(1 2) iota[n]]` | Statically valid; a completed dynamic length other than two fails dynamically. |
| `add[iota[2] iota[3]]` | Statically valid; the completed vectors fail dynamically. |

The last example remains dynamic even though evaluating its literals would
predict the mismatch. Static analysis does not constant-evaluate structural
primitives.

### 6.4 Value-dependent work is never a static rejection

After literal grammar and range validation, analysis does not execute
primitives or reject a program because concrete literal values predict:

- an `iota` result length;
- a dynamic shape mismatch;
- a profile limit or unavailable allocation;
- signed integer overflow or another domain failure; or
- formatted runtime output.

For example, `inc[9223372036854775807]` is statically valid and fails with
`DomainError(integer_overflow)` only when executed. A source literal outside its
own grammar or range remains a static source error under FARAWEAVE-SPEC-0002; it is
not a deferred argument or domain error.

## 7. Ordered typed embedding API

### 7.1 Public operation

The public direction is a free-function operation conceptually equivalent to:

```cpp
ProgramResult evaluate_source(
    std::string_view source,
    std::span<const Value> arguments);

ProgramResult evaluate_source(
    std::string_view source,
    std::span<const Value> arguments,
    const EvaluationConfiguration &configuration);
```

The concrete implementation may extend existing plain result and error structs,
but it must not introduce an object hierarchy or virtual binding interface.
Existing zero-argument `evaluate_source` overloads behave exactly as calls with
an empty argument span.

The argument span is borrowed only for the duration of the call. Successful
binding copies already-valid scalar data into immutable indexed parameter slots
without rewriting its bits.
The result retains no pointer, reference, iterator, or view into caller-owned
arguments. Binding allocates no Faraweave vector and performs no text conversion.

A declared parameter requires an exact typed scalar. Parameter binding performs
no `Int -> Double` promotion. Ordinary primitive signature selection may later
promote a bound `Int` parameter when the program uses it in a call covered by
FARAWEAVE-SPEC-0001.

### 7.2 Typed-value validation order

Argument count is checked before any supplied `Value` is inspected. If counts
match, positions are validated from one through the required count. At each
position, the exact order is:

1. reject an unknown top-level container tag as `invalid_typed_value`;
2. reject a valid vector container as `container_mismatch`, without inspecting
   or validating its vector payload;
3. validate the scalar type tag, active field, inactive fields, canonical NaN,
   and every other public scalar invariant; reject failure as
   `invalid_typed_value`;
4. compare the valid scalar type with the declaration and reject an unequal type
   as `type_mismatch`; and
5. copy the scalar into its parameter slot.

Thus a valid `Vector<Int>` supplied for an `Int` parameter is
`container_mismatch`, even if its payload would be invalid under vector
validation. A valid Bool supplied for an Int parameter is `type_mismatch`. An
unknown scalar type, noncanonical NaN, or invalid inactive scalar field is
`invalid_typed_value`.

This operation validates an already-formed caller-supplied stored `Value`; it is
not a raw binary64 construction or backend-ingress boundary. In particular, it
must reject a noncanonical NaN as `invalid_typed_value` with invariant
`noncanonical_nan` and must not normalize the caller's bits. FARAWEAVE-SPEC-0003
section 8 separately requires normalization while constructing a Faraweave value
from raw backend bits, before those bits become a valid stored `Value`.

The lowest failing one-based position wins. No later value is inspected after a
failure.

## 8. Structured ArgumentError

### 8.1 Distinct error category and reasons

Argument binding returns a distinct `ArgumentError`, not a syntax `TypeError`,
primitive `ArityError`, `ResourceError`, or `DomainError`. Its stable reasons
are exact lowercase identifiers:

```text
missing
extra
invalid_literal
out_of_range
invalid_typed_value
container_mismatch
type_mismatch
```

Future specifications may add reasons without changing these meanings.
Rendered prose is not the conformance interface.

### 8.2 Required context

Every `ArgumentError` carries:

| Field | Requirement |
| --- | --- |
| `reason` | One reason from section 8.1. |
| `required_count` | Number of declarations. |
| `supplied_count` | Number of typed or text values supplied. |
| `position` | One-based failing position. |
| `parameter_name` | Required when `position <= required_count`; absent for `extra`. |
| `expected_type` | Required when `position <= required_count`; absent for `extra`. |
| `declaration_span` | Required when `position <= required_count`; absent for `extra`. |
| `actual_container` | Present exactly as specified below; otherwise absent. |
| `actual_type` | Present exactly as specified below; otherwise absent. |
| `invalid_value_invariant` | Required for every `invalid_typed_value`; otherwise absent. |

Raw argument text is never stored in `ArgumentError` and is omitted from every
diagnostic. This avoids terminal-control, escaping, privacy, and unbounded-text
ambiguity.

The closed `invalid_value_invariant` vocabulary for this scalar-only boundary is:

```text
unknown_container
unknown_scalar_type
inactive_scalar_field
noncanonical_nan
```

`inactive_vector_payload` and `invalid_boolean_element` are public vector
invariants, but they cannot be returned here: a recognized vector container
returns `container_mismatch` before its payload or element tag is inspected.
Every `invalid_typed_value` must identify exactly one invariant from the closed
list; `none`, an absent field, or an implementation-defined value is invalid.

Optional context presence and values are exact:

| Reason/invariant | `actual_container` | `actual_type` | `invalid_value_invariant` |
| --- | --- | --- | --- |
| `invalid_typed_value` / `unknown_container` | `unknown` | absent | `unknown_container` |
| `invalid_typed_value` / `unknown_scalar_type` | `scalar` | `unknown` | `unknown_scalar_type` |
| `invalid_typed_value` / `inactive_scalar_field` | `scalar` | Exact recognized `Bool`, `Int`, or `Double` tag. | `inactive_scalar_field` |
| `invalid_typed_value` / `noncanonical_nan` | `scalar` | `Double` | `noncanonical_nan` |
| `container_mismatch` | `vector` | absent; the vector element tag is not inspected | absent |
| `type_mismatch` | `scalar` | Exact valid supplied `Bool`, `Int`, or `Double` type. | absent |
| `missing`, `extra`, `invalid_literal`, or `out_of_range` | absent | absent | absent |

`parameter_name`, `expected_type`, and `declaration_span` are present if and
only if `position <= required_count`; therefore they are absent exactly for
`extra` and present for every other current reason. The values identify the
declaration at `position`. No other optional `ArgumentError` field may be
populated.

### 8.3 Count failures

Count mismatch is decided without decoding or inspecting any value:

```text
supplied_count < required_count
  reason   = missing
  position = supplied_count + 1

supplied_count > required_count
  reason   = extra
  position = required_count + 1
```

A `missing` error includes the first missing parameter's name, type, and
declaration span. An `extra` error has no corresponding declaration, so its
name, expected type, and declaration span are absent. Only the first missing or
extra position is reported.

Count failures precede all malformed, out-of-range, invalid-value, container,
and type failures, including a malformed value at an earlier supplied position.

### 8.4 Stable process serialization

The runner and every generated/native artifact serialize `ArgumentError` to one
ASCII stderr line in exactly this field order:

```text
faraweave_argument_error reason=<reason> required_count=<uint> supplied_count=<uint> position=<uint> parameter_name=<value> expected_type=<value> declaration_span=<value> actual_container=<value> actual_type=<value> invalid_value_invariant=<value>
```

The code block contains the record without its required final LF byte. `<uint>`
is the shortest nonnegative decimal representation with no sign or leading zero
except the value zero. A present enum or parameter name uses its exact ASCII
spelling from this specification. A present declaration span is
`<start-byte>:<start-line>:<start-column>-<end-byte>:<end-line>:<end-column>`,
with all six numbers encoded as `<uint>` and the end excluded. An absent optional
field is exactly one hyphen (`-`). The current present string values contain
only ASCII letters, digits, and underscore. If a future specification permits
another byte in a string value, each byte outside `[A-Za-z0-9_]` must be encoded
as `%HH` with uppercase hexadecimal digits; `%` and `-` must therefore be
escaped when present data uses them. There are single ASCII spaces between
fields, no leading/trailing space, and one final LF byte. Native Windows text
translation may expose that logical LF as CRLF; parsers remove exactly one
platform line terminator before parsing the ASCII record.

No prose prefix or suffix is part of this record, and raw argument text never
appears. Conformance parses the fixed field names, order, absence marker,
escaping, and values; it does not infer structured data from human prose.

## 9. Portable text arguments

### 9.1 Boundary and common rules

Only `faraweave run` and an emitted/native program decode text arguments. The
expected declaration type selects exactly one grammar below. Decoding uses
portable ASCII, is locale independent, consumes the complete argument, and
does not accept whitespace around the text.

A token that does not match the selected grammar completely is
`invalid_literal`. A grammar-valid Int outside Int64 or a grammar-valid finite
Double that rounds to infinity is `out_of_range`.

### 9.2 Bool

Bool arguments are exactly:

```text
false
true
```

Case variants, numeric truth values, prefixes, suffixes, and surrounding
whitespace are `invalid_literal`.

### 9.3 Int

Int arguments use FARAWEAVE-SPEC-0002 section 8.2 exactly:

```text
unsigned_int = "0" | nonzero_digit { digit }
int_argument = unsigned_int | "-" nonzero_digit { digit }
```

There is no leading plus, `-0`, leading zero on a nonzero magnitude,
underscore, base prefix, suffix, or partial parse. The mathematical result must
be in `-9223372036854775808` through `9223372036854775807`.

Examples:

| Text | Result for Int |
| --- | --- |
| `0` | Int zero. |
| `-1` | Int negative one. |
| `-9223372036854775808` | `INT64_MIN`. |
| `+1`, `-0`, `00`, `01`, `1x` | `invalid_literal`. |
| `9223372036854775808` | `out_of_range`. |

### 9.4 Double

Double arguments use FARAWEAVE-SPEC-0002 sections 8.3 and 8.4 exactly:

```text
finite_double = [ "-" ] unsigned_mantissa
                ( fraction [ exponent ] | exponent )

special_double = "inf" | "-inf" | "nan"
```

A finite Double must contain a decimal point or exponent. A bare integer token
such as `2`, `0`, or `-2` is `invalid_literal` for a Double parameter. There is
no leading mantissa plus, no extra leading zero in the integral mantissa,
missing digit beside a decimal point, missing exponent digit, underscore,
hexadecimal form, suffix, `+inf`, or `-nan`. Integral mantissa `0` remains valid,
including in `0.5` and `0e0`.

Conversion is one correctly rounded decimal-to-binary64 conversion using
round-to-nearest, ties-to-even. Finite underflow to signed zero is valid and a
negative spelling retains negative zero. A grammar-valid finite spelling whose
magnitude rounds to infinity is `out_of_range`; infinity must use `inf` or
`-inf`. Every NaN result is normalized to FARAWEAVE-SPEC-0003's canonical NaN.

Examples:

| Text | Result for Double |
| --- | --- |
| `0.0`, `0e0` | Positive zero. |
| `-0.0`, `-0e0` | Negative zero. |
| `0.5`, `2.0`, `2e0`, `1.25E-2` | Finite Double. |
| `inf`, `-inf`, `nan` | Exact special class. |
| `2`, `+2.0`, `01.0`, `.5`, `1.`, `1e`, `1.0x` | `invalid_literal`. |
| `1e9999` | `out_of_range`. |

No text decoder may accept a valid prefix and ignore remaining bytes.

## 10. Runner command boundary

The exact runner form is:

```text
faraweave run <source> [-- <arguments...>]
```

The runner first validates this command shape. After `<source>`:

- no token means an empty script-argument list;
- if any token follows, the first must be exactly `--`;
- that first `--` is removed and every later token is argument data; and
- a token before the separator is a CLI usage error, not `ArgumentError`.

A usage error occurs before source-file loading or analysis. After a valid
command shape, source-file I/O and all static analysis precede argument binding.
Therefore a source error wins over malformed text following a valid separator.

Examples:

```text
faraweave run program.faraweave                 empty argument list
faraweave run program.faraweave --              explicit empty argument list
faraweave run program.faraweave -- -1 -0.0 true three inert argument strings
faraweave run program.faraweave -1              CLI usage error: missing --
faraweave run program.faraweave -- --           one argument whose text is --
```

A zero-parameter program succeeds with either no separator or a bare separator.
Any text after the separator is `ArgumentError(extra)`. A parameterized program
without the separator supplies zero arguments and therefore returns
`ArgumentError(missing)`.

`emit-c` and `build` accept source and their existing output/compiler options;
they do not accept runtime parameter values and do not use the runner's `--`
separator.

## 11. Runtime binding and execution order

### 11.1 Direct evaluator

After all static phases succeed, the direct evaluator applies this observable
order:

```text
1. compare supplied and required argument counts
2. validate typed arguments from position 1 upward
3. create one execution resource context
4. execute roots in source order
5. within a root, execute calls in left-to-right postorder
6. within each call, check dynamic shape, then resource preflight, then domain
   or scalar-kernel/structural execution
7. return all structured root Values
```

FARAWEAVE-SPEC-0007 refines steps 5 and 6 for a fan-out node: evaluate its operand
once, admit the profile-v2 result table, then execute complete branches
sequentially from left to right. Its operand/table/branch first-failure and
transactional cleanup order is part of the same complete-program execution; no
later branch or root begins after failure.

The first failure stops the sequence. Within dynamic shape agreement, the first
one-based mismatching argument position wins. Within a lifted kernel, the
lowest zero-based failing result index wins as required by FARAWEAVE-SPEC-0001.
Across roots, an earlier root's dynamic failure wins over every dynamic failure
in a later root.

Typed validation completes before the resource context begins. The evaluator
does not format a successful `Value`, construct pending stdout, write stdout or
stderr, or return a formatting/output-device failure.

### 11.2 Runner and artifact presentation

The runner and generated/native process adapters own text decoding,
presentation, and host output. After static analysis has succeeded for the
runner, or at process start for an already-published artifact, they perform
count comparison and lowest-position text decoding before creating the resource
context. They then execute steps 3 through 6 above and continue in this exact
order:

```text
7. format root Values into one pending byte batch in one-based root order
8. write the complete pending batch to stdout
9. flush stdout
```

No primitive receives raw text or performs argument decoding. All execution and
all formatting complete before step 8 begins.

Formatting failure is the stable structured class `FormattingError`. Its closed
reasons are `invalid_value` and `conversion_failure`. Context always contains
the one-based `root_position` and complete `root_span`; `invalid_value` also
contains one exact public `ValueInvariant`, while `conversion_failure` has no
invariant. The primary source position is the start of `root_span`. Roots are
formatted in source order, so the lowest failing `root_position` wins. This
failure occurs before publication and publishes zero stdout bytes.

Host stdout failure is the stable structured class `OutputError`. Its closed
reasons are `write_failed` and `flush_failed`. Context always contains
`pending_byte_count` and `accepted_byte_count`. For `write_failed`, the latter
is the byte count accepted by the single step-8 write and must be less than the
pending count. For `flush_failed`, it equals the pending count because the write
accepted the complete batch. Its deterministic `output_position` is the
zero-based first unaccepted byte for `write_failed` and the pending byte count
for `flush_failed`. No source span is attached. `OutputError` can occur only
after execution and formatting have succeeded and publication has begun; an
external device may therefore have observed a prefix. It returns process
failure and best-effort stderr rather than a successful language result.

Runner and artifact process serialization for these failures is also stable.
It uses section 8.4's `<uint>`, span, absence, spacing, escaping, and logical-line
terminator rules, with exactly one of these complete ASCII records and no prose:

```text
faraweave_formatting_error reason=<reason> root_position=<uint> root_span=<span> invalid_value_invariant=<value>
faraweave_output_error reason=<reason> pending_byte_count=<uint> accepted_byte_count=<uint> output_position=<uint>
```

For `FormattingError(invalid_value)`, `invalid_value_invariant` is the exact
public invariant; for `conversion_failure`, it is the absence marker `-`.
`<span>` is section 8.4's six-component half-open span without angle brackets.
An `OutputError` record is best effort because the host may also make stderr
unwritable, but whenever the record is observable its bytes and fields are the
stable conformance interface.

### 11.3 Generated and native programs

A generated or native program receives runtime arguments directly:

```text
./program <arguments...>
```

The process program name is not an argument. The ordered text argument list is
`argv[1]` through `argv[argc - 1]`; no `--` is required or removed. If a caller
passes `--`, it is ordinary first-argument text and normally fails the expected
scalar grammar.

On a hosted C entry where `argc <= 1`, the supplied script-argument count is
exactly zero. The program must not read `argv[1]`, form a pointer to it, or
subtract one into an unsigned count. Only an `argc > 1` branch may convert
`argc - 1` to the checked internal count and inspect `argv[1]` onward.

Static analysis already succeeded before the artifact was published. At each
invocation the artifact begins with section 11.2's count and decoding boundary,
then creates a fresh resource context and performs the same dynamic and
presentation order. Emitted C and a
native executable built from it must return the same values, structured
failure class/reason/context, stderr information, exit success/failure, and
stdout bytes.

### 11.4 Dynamic examples

```faraweave
parameters[n Int]
iota[n]
```

`n <= 0` produces an empty Int vector. Positive `n` produces `1` through `n`
when representable, admitted, and allocated. A representability, profile, or
allocation failure is a runtime `ResourceError`.

```faraweave
parameters[n Int]
add[(10 20) iota[n]]
```

The program is statically valid. `n = 2` succeeds; any `n` producing a vector
length other than two returns dynamic `ShapeMismatch` before the outer `add`
allocates its result or executes an element. If the child `iota` first fails its
own resource preflight or execution, that postorder child failure wins.

```faraweave
parameters[n Int]
inc[n]
```

Binding `9223372036854775807` succeeds. Execution returns
`DomainError(integer_overflow)`.

```faraweave
parameters[n Int]
inc[n]
add[(1 2) iota[n]]
```

For `n = 9223372036854775807`, the first root's integer overflow wins over the
later root's shape/resource outcome. No root output is published.

## 12. Emission, diagnostics, and stdout

### 12.1 Emitter and builder acceptance

`emit-c` and `build` accept every program that passes section 6. They require no
concrete runtime values. They must not constant-evaluate otherwise valid calls
to reject predictable dynamic shape, resource, or domain failures.

Existing source/output I/O, unsupported-target, compiler selection/invocation,
and destination-publication failures remain tool failures. They do not make a
statically valid Faraweave program semantically invalid.

Emission retains:

- ordered parameter names and scalar types;
- declaration and reference spans needed by structured results;
- typed parameter-slot indices;
- typed primitive selections;
- static and dynamic shape information; and
- dynamic source spans needed by runtime diagnostics.

The generated C11 program decodes directly into scalar slots before executing
the typed lowering. It does not add runtime overload dispatch.

### 12.2 Presentation by surface

The public evaluator returns structured success or error data and writes no
stdout or stderr.

The runner writes source and dynamic diagnostics to stderr in the established
form:

```text
<source-path>:<line>:<column>: <category>: <message>
```

`emit-c` and `build` use the same source-path form for header and other static
source failures. They exit nonzero and preserve the existing publish-last
destination contract.

An argument diagnostic is exactly section 8.4's stable ASCII record. It exits
nonzero and writes no stdout.

A generated/native program has no source file dependency. Dynamic diagnostics
use the embedded logical label `faraweave-source` plus the original one-based line
and column:

```text
faraweave-source:<line>:<column>: <category>: <message>
```

Its argument diagnostics use the same section 8.4 serialization as the runner.
Generated and native failures write one diagnostic record to stderr, exit
nonzero, and write no stdout unless the failure is itself an stdout-device
failure.

Exact human-readable prose for other diagnostics is not stable. The section 8.4
argument record and section 11.2 formatting/output records are wholly stable.
For every category, error kind, stable reason, structured context,
primary/output position, channel, and success/failure status are the
conformance interface.

### 12.3 Transactional language output

The evaluator returns all structured root Values or one failure and performs no
presentation I/O. The runner and artifacts evaluate and format every root into
one pending output batch before the first stdout write. Missing, extra,
malformed, invalid typed, shape, resource, domain, or `FormattingError` failure
publishes zero stdout bytes, including when an earlier root succeeded.

Once publication to an external stdout device begins, `OutputError` may mean a
byte prefix became externally visible. The process must still return nonzero
and report the stable reason/context on best-effort stderr.
This unavoidable device behavior is not a relaxation allowing language or
argument failures to stream partial root results.

## 13. Resources, ownership, and representability

### 13.1 Parameter storage and accounting

Typed validation and text decoding occur before the evaluation resource context
starts. The initial parameter domain contains only three scalar types. Binding
copies scalar bits into ordinary slots and creates no Faraweave vector, owned
container, primitive workspace, or user-visible allocation.

Parameter slots therefore charge:

```text
max_vector_bytes           0
max_live_evaluation_bytes  0
max_work_units             0
```

Calls that reference parameter slots charge exactly the canonical vector,
live-byte, and work events required by FARAWEAVE-SPEC-0004. Every direct complete
program, runner execution, and generated/native invocation starts one fresh
resource context after successful binding. `trusted-local-v1` and `bounded-v1`
otherwise remain unchanged.

Hosts may need bounded implementation storage for analysis, argument metadata,
or pending diagnostics. Such host bookkeeping is not a Faraweave vector or profile
charge, but allocation failure must remain explicit and must not escape as a
host exception or partial language result.

### 13.2 No language-level parameter maximum

There is no language-defined maximum parameter count. The grammar and semantics
do not impose a number such as 255 or 65,535.

Practical bounds remain:

- the finite source byte length;
- representability by the implementation's source offsets, declaration count,
  parameter index, and public span size;
- available host storage during analysis and binding; and
- the host process command-line byte and `argc` limits for text boundaries.

Every count, index, table size, and generated constant must be checked before
conversion, addition, multiplication, allocation, or indexing. An
implementation that cannot represent required source-side bookkeeping fails
source analysis explicitly with `ResourceError(size_overflow)` at the header;
it must not wrap, truncate, alias slots, or impose an undocumented accepted
count. A public typed span is already bounded by its representable span size.

A generated process whose host cannot supply the declared number of arguments
returns `ArgumentError(missing)` for the count it actually receives. A host
command-line limit is an invocation limit, not a smaller Faraweave language maximum.

## 14. Complete valid and invalid examples

### 14.1 Valid programs and bindings

```faraweave
parameters[]
```

Requires zero arguments, has zero roots, and produces no output.

```faraweave
parameters[unused Bool n Int]
iota[n]
```

Both values are required and validated. `unused` need not be referenced.

```faraweave
parameters[x Double]
add[x x]
```

The same immutable value is referenced twice. Text `2.0` succeeds; text `2`
fails with `invalid_literal`. A typed Double succeeds; a typed Int fails with
`type_mismatch`.

```faraweave
parameters[n Int]
iota[n]
inc[n]
```

The runner invocation `faraweave run p.faraweave -- -2` succeeds with an empty vector
then `-1`. The generated invocation `./p -2` has the same logical output.

### 14.2 Invalid declarations and references

| Source | Required static failure |
| --- | --- |
| `parameters [n Int]` | Header syntax at `[`. |
| `parameters[n]` | Missing parameter type. |
| `parameters[n Integer]` | Unexpected type token. |
| `parameters[n Int n Bool]` | Duplicate later `n`. |
| `parameters[add Int]` | Primitive-name collision. |
| `parameters[true Bool]` | Reserved-literal collision. |
| `parameters[n Int]` followed later by `parameters[]` | Second-header placement error. |
| `1` followed later by `parameters[n Int]` | Header-after-root placement error. |
| `parameters[n Int]` followed by root `m` | Unknown bare parameter reference `m`. |

### 14.3 Argument failures

For `parameters[n Int delta Double enabled Bool]`:

| Supplied values | Required failure |
| --- | --- |
| none | `missing`, counts 3/0, position 1, `n`, Int. |
| `1` | `missing`, counts 3/1, position 2, `delta`, Double. |
| `1 2.0 true extra` | `extra`, counts 3/4, position 4, no parameter fields. |
| text `1 2 true` | `invalid_literal` at position 2 because Double requires visible Double spelling. |
| text `1 1e9999 true` | `out_of_range` at position 2. |
| text `01 2.0 true` | `invalid_literal` at position 1. |
| typed Vector<Int>, Double, Bool | `container_mismatch` at position 1. |
| typed Bool, Double, Bool | `type_mismatch` at position 1. |
| typed invalid Int scalar, Double, Bool | `invalid_typed_value` at position 1. |

If four values are supplied and the first is malformed or invalid, `extra`
still wins because count precedes inspection.

## 15. Deliberate Level 2 and Anka differences

Level 2 has no parameter declaration, bare parameter-reference expression,
runtime script-argument list, or typed argument overload. Its public evaluator
executes concrete source directly, and its emitter may rely on evaluator-backed
concrete results. This specification introduces a typed analysis/execution
boundary: analysis selects calls without values, while evaluator and artifact
execution bind values later. Existing source using no header keeps a required
count of zero, but prior invalid-program and diagnostic behavior is not a
compatibility promise.

[Anka](https://github.com/tuncb/anka) remains language-design inspiration, not
a compatibility target. Faraweave chooses one explicit program header, required
ordered scalar positions, exact `Bool`/`Int`/`Double` declarations, a typed
embedding span, and an explicit runner separator. It does not import Anka's
broader variables, user blocks, command-line application conventions, parser
representation, or runtime architecture. No Anka behavior constrains Faraweave's
`ArgumentError`, whole-program analysis, resource profiles, or emitted-C
agreement.

## 16. Non-goals and fixed boundaries

This specification does not add:

- String, vector, tuple, or other container parameters;
- named options or named embedding arguments;
- optional, defaulted, repeated, or variadic parameters;
- environment-variable or standard-input decoding;
- persistent REPL declarations or bindings;
- user variables, assignment, functions, closures, or blocks;
- runtime primitive overload dispatch;
- argument-dependent static type selection;
- shell parsing, quote removal, wildcard expansion, or response files; or
- a language-level parameter-count cap.

Any such feature requires a separate specification. Future parameter container
kinds must define ownership, decoding, resource charging, and portable Windows
and C11 text boundaries before they are accepted.

## 17. Requirement-to-test plan

Issue #42 records the plan only. Implementation issues must add executable tests
and then add their exact identifiers to `tests/spec-traceability.tsv`.

Supported target-native journeys mean Linux x64, Windows x64, and macOS arm64.
Strict C11 means GCC/Clang-compatible compilation with `-std=c11 -Wall -Wextra
-Werror -pedantic-errors` where applicable and MSVC compilation with `/std:c11
/W4 /WX` on Windows.

| Plan ID | Normative requirement | Required future evidence |
| --- | --- | --- |
| `PARG-001-HEADER` | Sections 3 through 5 grammar, placement, empty form, names, links, exact malformed-header reasons/spans, and reserved primitive descriptor. | Parser/analysis fixtures for valid LF/CRLF/multiline headers and every table row, including bare `parameters`, missing `[`, `parameters[n` at EOS, second header, header after root, and both same-byte overlaps; assert exact stable reason and half-open primary/related spans. Reject a primitive descriptor whose source name is `parameters`; cover duplicate, collision, unknown-reference, and call/reference distinctions. |
| `PARG-002-STATIC-ORDER` | Section 6 whole-program phase and traversal order. | Multi-root and nested fixtures combining name, arity, type, static shape, malformed typed arguments, and predicted dynamic errors; direct evaluator proves the required static winner before any argument inspection. |
| `PARG-003-SHAPE-ANALYSIS` | Static versus dynamic shape classes. | Direct analysis fixtures for literal-vector mismatch, parameter scalars, literal-fed `iota`, two dynamic vectors, and mixed known/dynamic vectors; emit/build reject only the static mismatch. |
| `PARG-004-TYPED-API` | Section 7 ordered typed API, exact types, ownership, and validation order. | Public C++ tests using plain `Value` arrays for all scalar types, repeated/unused parameters, invalid tags/inactive fields/noncanonical NaN, valid vectors, wrong scalar types, caller mutation after return, and empty-span compatibility. Inject the same raw noncanonical NaN through FARAWEAVE-SPEC-0003 construction normalization and through a caller-supplied stored `Value`; prove normalization only at raw ingress and `invalid_typed_value(noncanonical_nan)` at the public stored-Value boundary. |
| `PARG-005-ARGUMENT-ERROR` | Section 8 reasons, fields, count precedence, lowest position, closed invariant vocabulary, and stable process record. | Structured direct-result assertions for every reason, invariant, and exact optional-field presence/value; multiple missing/extra values, malformed-before-extra, and two invalid equal-count positions. Runner/C/native tests parse the fixed ASCII field order, absence marker, span form, and escaping; tests never infer fields from prose. |
| `PARG-006-TEXT-GRAMMAR` | Section 9 portable Bool/Int/Double grammar and range. | Table-driven runner and generated-C decoder corpus covering signs, zeroes, boundaries, partial parses, locale independence, underflow/signed zero, infinities, NaN, finite overflow, visible Double spelling, and raw-text omission. |
| `PARG-007-RUNNER-SEPARATOR` | Section 10 exact `--` boundary. | CLI process tests for no separator, bare separator, negative values, stray pre-separator values, a second `--` as data, zero-parameter extra values, and CLI-usage-before-file precedence on every supported target. |
| `PARG-008-ZERO-ROOTS` | Empty declaration and declared parameters with no roots. | Direct, runner, emitted-C, and native tests proving required validation, exact zero values/stdout on success, and count/decode failures despite no references. |
| `PARG-009-DYNAMIC-IOTA-SHAPE` | Sections 6 and 11 dynamic `iota` and shape behavior. | Differential direct/runner/emitted/native corpus for negative, zero, matching, and mismatching parameter lengths; structured first mismatching argument and zero partial output. |
| `PARG-010-RUNTIME-ORDER` | Section 11 root/call/dynamic failure order. | Multi-root and nested tests combining dynamic shape, bounded resource refusal, and Int overflow; compare winner, span, argument/index context, status, stderr class, and zero stdout across paths. |
| `PARG-011-PROFILES` | Section 13 unchanged `trusted-local-v1`/`bounded-v1` accounting. | Instrumented resource tests prove binding emits no vector/live/work charge, calls emit existing canonical charges, binding failure creates no context, and each invocation resets. Replay bounded refusals across evaluator, C, and native. |
| `PARG-012-EMIT-C` | Sections 11 and 12 typed emission without concrete values. | Deterministic emitted-byte checks across source paths and argument values; compile every valid dynamic fixture as strict C11; execute success and all argument/dynamic failures; inspect generated source for no runtime overload selection. |
| `PARG-013-NATIVE` | Generated/native direct-argv equivalence. | Build and invoke native artifacts without `--`; compare byte-exact success output and parsed section 8.4 failure records with emitted C and direct structured paths on Linux, Windows, and macOS. |
| `PARG-014-DIAGNOSTICS` | Section 12 source/argument channels, embedded spans, raw-text omission, and statuses. | Direct no-I/O tests; runner path/line/column tests; generated `faraweave-source` position tests; parse every section 8.4 field rather than prose; stderr/nonzero/no-stdout assertions; hostile control-byte argument proves no raw echo. |
| `PARG-015-ATOMIC-STDOUT` | Complete-program transactional output and device-failure exception. | Programs with an early successful root and later argument-independent shape/resource/domain/format failure publish zero bytes on runner/C/native; parse the exact formatting record and assert reason, root position/span, and invariant presence. Injected short-write/final-flush tests parse the exact output record and assert reason, pending/accepted counts, and output position; a prefix may be observed but status is nonzero. |
| `PARG-016-REPRESENTABILITY` | No language cap and checked count/index/host boundaries. | Synthetic source-analysis seams exercise count/index/byte overflow without huge allocation; public span boundaries; generated/native hosted fixtures call the argument adapter with `argc <= 1` and prove zero supplied arguments, no `argv[1]` access, no unsigned underflow, and checked `argc > 1` conversion; host-limit documentation; no wrapping or truncated slot aliases. |
| `PARG-017-REGRESSION` | Header-absent zero-argument compatibility and program-only rejection. | Existing complete Release CTest remains green; zero-argument direct/runner/C/native corpus remains byte exact; REPL and `evaluate_expression` reject headers/references and recover as specified. |
| `PARG-018-PLATFORMS` | Portable ASCII process boundary. | Target-native matrix executes Bool, Int extrema, visible Double, signed zero, infinity, NaN, negative values, missing/extra/malformed cases, `argc <= 1`, strict C11, and native build on Linux x64, Windows x64, and macOS arm64; compare logical results and parse exact section 8.4 records. |

No one path substitutes for another: direct evaluator tests establish structured
API data, runner tests establish `--` and file diagnostics, emitted-C tests
establish portable C decoding, native tests establish builder/process behavior,
and the three-platform matrix establishes target argument handling.
