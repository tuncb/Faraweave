# FARAWEAVE-SPEC-0003: Scalar Primitive Domain Semantics

**Status:** Proposed

**Related issues:** [#25](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/25),
[#55](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/55)

**Parent specification:** [FARAWEAVE-SPEC-0001](faraweave-spec-0001-scalar-lifting.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Target:** Faraweave language rewrite; Bool, Int, and Double scalar kernels

**Compatibility:** These semantics deliberately replace, rather than extend,
the bootstrap Level 1 scalar contract.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and
do not override the semantic requirements.

## 2. Outcome and scope

This specification gives every initial elementwise scalar kernel a complete,
host-independent value-domain, conversion, result, and error contract:

```text
inc    : Int -> Int
inc    : Double -> Double
dec    : Int -> Int
dec    : Double -> Double
neg    : Int -> Int
neg    : Double -> Double
abs    : Int -> Int
abs    : Double -> Double
add    : Int Int -> Int
add    : Double Double -> Double
sub    : Int Int -> Int
sub    : Double Double -> Double
mul    : Int Int -> Int
mul    : Double Double -> Double
equals : Bool Bool -> Bool
equals : Int Int -> Bool
equals : Double Double -> Bool
not    : Bool -> Bool
```

It defines:

- signed Int64 overflow behavior for `inc`, `dec`, `neg`, `abs`, `add`,
  `sub`, and `mul`;
- strict IEEE 754 binary64 arithmetic and rounding behavior;
- normalization and non-observability of NaN sign, payload, and signaling state;
- Double equality for finite values, signed zero, infinities, and NaN;
- unconditional `Int -> Double` conversion at every Int64 value;
- the scalar `DomainError` reason and required structured context; and
- exact-value and binary64-bit-pattern conformance vectors.

FARAWEAVE-SPEC-0001 continues to own overload selection, lifting, shape agreement,
resource preflight, error precedence, and lowest-result-index reporting.
FARAWEAVE-SPEC-0002 continues to own literal syntax and decimal-to-binary64
conversion.

## 3. Domains and notation

`Int` is the mathematical integer interval:

```text
INT64_MIN = -9223372036854775808 = -2^63
INT64_MAX =  9223372036854775807 =  2^63 - 1
```

`Double` is the IEEE 754 binary64 interchange format: one sign bit, an
11-bit biased exponent, and a 52-bit trailing significand field. It includes
finite normal values, finite subnormal values, signed zero, signed infinity,
and NaN.

Binary64 bit patterns in this specification are written as exactly 16
lowercase hexadecimal digits after `0x`. The leftmost digit contains the sign
bit. These patterns describe interchange bits, not host byte order.

The following names are used in tables:

| Name | Binary64 bits |
| --- | --- |
| positive zero | `0x0000000000000000` |
| negative zero | `0x8000000000000000` |
| positive one | `0x3ff0000000000000` |
| negative one | `0xbff0000000000000` |
| smallest positive subnormal | `0x0000000000000001` |
| largest positive subnormal | `0x000fffffffffffff` |
| smallest positive normal | `0x0010000000000000` |
| largest positive finite | `0x7fefffffffffffff` |
| positive infinity | `0x7ff0000000000000` |
| negative infinity | `0xfff0000000000000` |
| canonical NaN | `0x7ff8000000000000` |

`Bool` contains exactly `false` and `true`.

## 4. Scalar-kernel result model

Every scalar-kernel invocation returns exactly one of:

```text
success(result scalar)
DomainError(structured context)
```

The initial kernels have these result domains:

| Selected scalar signature | Result contract |
| --- | --- |
| `inc : Int -> Int` | Success when the mathematical result is in the Int domain; otherwise `integer_overflow`. |
| `inc : Double -> Double` | Success for every Double input. |
| `dec : Int -> Int` | Success when the mathematical result is in the Int domain; otherwise `integer_overflow`. |
| `dec : Double -> Double` | Success for every Double input. |
| `neg : Int -> Int` | Success except for `INT64_MIN`, which reports `integer_overflow`. |
| `neg : Double -> Double` | Always succeeds by complementing the sign bit, followed by NaN normalization. |
| `abs : Int -> Int` | Success except for `INT64_MIN`, which reports `integer_overflow`. |
| `abs : Double -> Double` | Always succeeds by clearing the sign bit, followed by NaN normalization. |
| `add : Int Int -> Int` | Success when the mathematical result is in the Int domain; otherwise `integer_overflow`. |
| `add : Double Double -> Double` | Success for every Double input pair. |
| `sub : Int Int -> Int` | Success when the mathematical result is in the Int domain; otherwise `integer_overflow`. |
| `sub : Double Double -> Double` | Success for every Double input pair. |
| `mul : Int Int -> Int` | Success when the mathematical result is in the Int domain; otherwise `integer_overflow`. |
| `mul : Double Double -> Double` | Success for every Double input pair. |
| `equals : Bool Bool -> Bool` | Always succeeds. |
| `equals : Int Int -> Bool` | Always succeeds. |
| `equals : Double Double -> Bool` | Always succeeds. |
| `not : Bool -> Bool` | Always succeeds. |

Infinity, underflow, inexact rounding, and an arithmetic NaN are successful
Double results. They are not `DomainError` conditions. A hardware
floating-point exception, signal, trap, host exception, or process termination
must not replace the language result.

## 5. Int arithmetic

### 5.1 Increment

For `x : Int`, define the mathematical integer `r = x + 1`.

```text
inc(x) = success(Int(r))                 when r <= INT64_MAX
inc(x) = DomainError(integer_overflow)   otherwise
```

Therefore only `inc(INT64_MAX)` fails. Faraweave does not wrap it to `INT64_MIN`,
saturate it to `INT64_MAX`, or rely on signed host arithmetic that has undefined
or implementation-specific behavior.

Required exact cases:

| Input | Expected result |
| ---: | --- |
| `INT64_MIN` | `-9223372036854775807` |
| `-1` | `0` |
| `0` | `1` |
| `INT64_MAX - 1` | `INT64_MAX` |
| `INT64_MAX` | `DomainError(integer_overflow)` |

### 5.2 Addition

For `x, y : Int`, define the mathematical integer `r = x + y` without first
performing a potentially overflowing fixed-width addition.

```text
add(x, y) = success(Int(r))                 when INT64_MIN <= r <= INT64_MAX
add(x, y) = DomainError(integer_overflow)   otherwise
```

Overflow depends only on the mathematical sum. Operand order does not change
the result or error.

Required exact cases:

| Left | Right | Expected result |
| ---: | ---: | --- |
| `INT64_MIN` | `-1` | `DomainError(integer_overflow)` |
| `INT64_MIN` | `0` | `INT64_MIN` |
| `INT64_MIN` | `1` | `-9223372036854775807` |
| `INT64_MIN` | `INT64_MAX` | `-1` |
| `-1` | `1` | `0` |
| `INT64_MAX` | `-1` | `9223372036854775806` |
| `INT64_MAX` | `0` | `INT64_MAX` |
| `INT64_MAX` | `1` | `DomainError(integer_overflow)` |
| `INT64_MAX` | `INT64_MAX` | `DomainError(integer_overflow)` |

The same table with left and right exchanged has the same expected results.

### 5.3 Decrement

For `x : Int`, define the mathematical integer `r = x - 1`.

```text
dec(x) = success(Int(r))                 when r >= INT64_MIN
dec(x) = DomainError(integer_overflow)   otherwise
```

Only `dec(INT64_MIN)` fails. Required successful boundaries are
`dec(INT64_MIN + 1) = INT64_MIN` and `dec(INT64_MAX) = INT64_MAX - 1`.

### 5.4 Negation and absolute value

For `x : Int`, `neg` computes the mathematical integer `-x`, and `abs` computes
`x` when `x >= 0` or `-x` otherwise. Both operations fail with
`integer_overflow` exactly when `x == INT64_MIN`; every other input succeeds.
Required boundaries include:

| Input | `neg` | `abs` |
| ---: | ---: | ---: |
| `INT64_MIN` | `DomainError(integer_overflow)` | `DomainError(integer_overflow)` |
| `INT64_MIN + 1` | `INT64_MAX` | `INT64_MAX` |
| `-1` | `1` | `1` |
| `0` | `0` | `0` |
| `INT64_MAX` | `-INT64_MAX` | `INT64_MAX` |

### 5.5 Subtraction

For `x, y : Int`, define the mathematical integer `r = x - y` without first
negating `y` or performing a potentially overflowing fixed-width subtraction.

```text
sub(x, y) = success(Int(r))                 when INT64_MIN <= r <= INT64_MAX
sub(x, y) = DomainError(integer_overflow)   otherwise
```

Subtraction is ordered: exchanging operands generally changes the result and
may change whether the operation overflows. Required boundary cases are:

| Left | Right | Expected result |
| ---: | ---: | --- |
| `INT64_MIN` | `1` | `DomainError(integer_overflow)` |
| `INT64_MIN` | `-1` | `INT64_MIN + 1` |
| `-1` | `INT64_MAX` | `INT64_MIN` |
| `0` | `INT64_MIN` | `DomainError(integer_overflow)` |
| `INT64_MAX` | `-1` | `DomainError(integer_overflow)` |
| `INT64_MAX` | `1` | `INT64_MAX - 1` |

### 5.6 Multiplication

For `x, y : Int`, define the mathematical integer `r = x * y` without first
performing a potentially overflowing fixed-width multiplication.

```text
mul(x, y) = success(Int(r))                 when INT64_MIN <= r <= INT64_MAX
mul(x, y) = DomainError(integer_overflow)   otherwise
```

Multiplication is commutative. Zero times any Int is zero. Required boundary
cases, including their exchanged forms, are:

| Left | Right | Expected result |
| ---: | ---: | --- |
| `INT64_MIN` | `-1` | `DomainError(integer_overflow)` |
| `INT64_MIN` | `0` | `0` |
| `INT64_MIN` | `1` | `INT64_MIN` |
| `INT64_MAX` | `-1` | `-INT64_MAX` |
| `3037000499` | `3037000499` | `9223372030926249001` |
| `3037000500` | `3037000500` | `DomainError(integer_overflow)` |

## 6. Binary64 execution contract

Every Double `inc`, `dec`, `add`, `sub`, and `mul` operation is exactly one IEEE
754 binary64 arithmetic operation. `neg` and `abs` are exact sign-bit
transformations. Arithmetic is correctly rounded directly to binary64 using
round-to-nearest, ties-to-even, also called `roundTiesToEven`.

This rule is independent of the ambient host rounding mode. A conforming
implementation must:

- produce a binary64 result after the specified operation, without observable
  excess intermediate precision;
- preserve gradual underflow and subnormal inputs and results;
- preserve the IEEE result sign for zero and infinity;
- prevent an enabled host floating-point trap from escaping as Faraweave behavior;
- avoid reassociation, finite-only assumptions, flush-to-zero,
  denormals-are-zero, and unsafe fast-math transformations; and
- restore any host rounding mode it changes before returning to its caller.

Floating-point status flags and NaN payload propagation are not Faraweave-observable
state. An implementation must not read them to choose a language result.

## 7. Double arithmetic

### 7.1 Double increment

For `x : Double`:

```text
inc(x) = binary64_add_roundTiesToEven(x, 1.0)
```

The result is then normalized by section 8.

Required bit-pattern cases:

| Input bits | Expected result bits | Purpose |
| --- | --- | --- |
| `0xbff0000000000000` | `0x0000000000000000` | Exact cancellation yields positive zero. |
| `0x8000000000000000` | `0x3ff0000000000000` | Negative zero increments to one. |
| `0x0000000000000001` | `0x3ff0000000000000` | The subnormal addend is below half an ulp at one. |
| `0x7fefffffffffffff` | `0x7fefffffffffffff` | Adding one cannot change the largest finite value. |
| `0x7ff0000000000000` | `0x7ff0000000000000` | Positive infinity remains positive infinity. |
| `0xfff0000000000000` | `0xfff0000000000000` | Negative infinity remains negative infinity. |
| `0x7ff8123456789abc` | `0x7ff8000000000000` | A quiet NaN with payload normalizes. |
| `0x7ff0000000000001` | `0x7ff8000000000000` | A raw signaling-NaN boundary input normalizes. |

The raw NaN rows exercise normalization at the value boundary immediately
before kernel dispatch; noncanonical NaNs are not valid stored operands.

### 7.2 Double addition

For `x, y : Double`:

```text
add(x, y) = binary64_add_roundTiesToEven(x, y)
```

The result is then normalized by section 8.

Required bit-pattern cases:

| Left bits | Right bits | Expected result bits | Purpose |
| --- | --- | --- | --- |
| `0x0000000000000000` | `0x8000000000000000` | `0x0000000000000000` | Opposite signed zeros produce positive zero. |
| `0x8000000000000000` | `0x8000000000000000` | `0x8000000000000000` | Two negative zeros produce negative zero. |
| `0x3ff0000000000000` | `0x3ca0000000000000` | `0x3ff0000000000000` | Halfway addition selects the even significand. |
| `0x3ff0000000000000` | `0x3cb8000000000000` | `0x3ff0000000000002` | The next halfway case selects the even upper significand. |
| `0x0000000000000001` | `0x0000000000000001` | `0x0000000000000002` | Exact subnormal addition is preserved. |
| `0x0010000000000000` | `0x800fffffffffffff` | `0x0000000000000001` | Normal-minus-subnormal produces the smallest subnormal. |
| `0x7fefffffffffffff` | `0x7fefffffffffffff` | `0x7ff0000000000000` | Finite overflow produces positive infinity. |
| `0xffefffffffffffff` | `0xffefffffffffffff` | `0xfff0000000000000` | Finite overflow produces negative infinity. |
| `0x7ff0000000000000` | `0xfff0000000000000` | `0x7ff8000000000000` | Opposite infinities produce canonical NaN. |
| `0x7ff0000000000001` | `0x3ff0000000000000` | `0x7ff8000000000000` | A signaling NaN produces canonical NaN. |
| `0xfff8123456789abc` | `0x3ff0000000000000` | `0x7ff8000000000000` | NaN sign and payload do not propagate. |

The raw NaN rows exercise normalization at the value boundary immediately
before kernel dispatch; noncanonical NaNs are not valid stored operands.

### 7.3 Double decrement

```text
dec(x) = binary64_subtract_roundTiesToEven(x, 1.0)
```

Required cases include positive and negative zero producing negative one,
negative infinity remaining negative infinity, the largest finite value
remaining unchanged, and every NaN producing canonical NaN. The largest-finite
case must remain unchanged under an ambient directed rounding mode and the
ambient mode must be restored.

### 7.4 Double negation

`neg` complements only the binary64 sign bit and then applies section 8
normalization. It maps positive zero to negative zero, negative zero to positive
zero, positive infinity to negative infinity, and negative infinity to positive
infinity. NaN remains the canonical positive NaN because its sign is not
observable.

### 7.5 Double absolute value

`abs` clears only the binary64 sign bit and then applies section 8 normalization.
It maps either signed zero to positive zero, negative infinity to positive
infinity, and every finite value to the same magnitude with a clear sign bit.
NaN remains canonical NaN.

### 7.6 Double subtraction

```text
sub(x, y) = binary64_subtract_roundTiesToEven(x, y)
```

Required bit-pattern cases are:

| Left bits | Right bits | Expected result bits | Purpose |
| --- | --- | --- | --- |
| `0x0000000000000000` | `0x8000000000000000` | `0x0000000000000000` | Positive zero minus negative zero is positive zero. |
| `0x8000000000000000` | `0x0000000000000000` | `0x8000000000000000` | Negative zero minus positive zero is negative zero. |
| `0x0010000000000000` | `0x000fffffffffffff` | `0x0000000000000001` | Normal-minus-subnormal preserves the smallest subnormal. |
| `0x3ff0000000000000` | `0x3c90000000000000` | `0x3ff0000000000000` | Halfway subtraction selects the even significand. |
| `0x7ff0000000000000` | `0x7ff0000000000000` | `0x7ff8000000000000` | Infinity minus itself produces canonical NaN. |
| `0xfff0000000000000` | `0x7ff0000000000000` | `0xfff0000000000000` | Negative infinity minus positive infinity remains negative infinity. |

### 7.7 Double multiplication

```text
mul(x, y) = binary64_multiply_roundTiesToEven(x, y)
```

Required bit-pattern cases are:

| Left bits | Right bits | Expected result bits | Purpose |
| --- | --- | --- | --- |
| `0x8000000000000000` | `0x4000000000000000` | `0x8000000000000000` | Negative zero times positive two is negative zero. |
| `0x0000000000000001` | `0x4000000000000000` | `0x0000000000000002` | Exact subnormal multiplication is preserved. |
| `0x3ff0000000000001` | `0x3ff0000000000001` | `0x3ff0000000000002` | Rounding-sensitive finite multiplication. |
| `0x7fefffffffffffff` | `0x4000000000000000` | `0x7ff0000000000000` | Finite overflow produces positive infinity. |
| `0x7ff0000000000000` | `0x0000000000000000` | `0x7ff8000000000000` | Infinity times zero produces canonical NaN. |
| `0xbff0000000000000` | `0x7ff0000000000000` | `0xfff0000000000000` | Negative one times positive infinity is negative infinity. |

## 8. NaN normalization and observability

Every binary64 encoding with an all-ones exponent and a nonzero trailing
significand is the single semantic Faraweave NaN class. NaN sign, payload, and
quiet-versus-signaling state are not language-observable.

A valid stored Faraweave `Double` must use this canonical NaN encoding:

```text
0x7ff8000000000000
```

Every value-construction, raw backend-ingress, conversion, and scalar-kernel
result boundary must normalize any other NaN encoding to the canonical encoding
before the value is stored or returned. Raw backend ingress means binary64 bits
that have not yet become a public stored Faraweave `Value`; it does not include
validation of a caller-supplied `Value` record. A signaling NaN therefore cannot
remain in a valid Faraweave value and cannot cause a host trap. Conformance tests
must still inject positive and negative raw signaling-NaN patterns into the
normalization boundary to verify this rule.

FARAWEAVE-SPEC-0005's typed argument API receives an already-formed public stored
`Value`. It must validate that record without rewriting it and reject a
noncanonical NaN as `ArgumentError(invalid_typed_value)` with
`invalid_value_invariant = noncanonical_nan`. Normalizing that caller-supplied
record would hide a violated public-value invariant and is forbidden. The same
raw bits are therefore normalized when they enter through a construction/raw
backend boundary and rejected when they are presented as an allegedly valid
stored `Value`.

Required normalization vectors are:

| Raw boundary bits | Expected stored bits | Class |
| --- | --- | --- |
| `0x7ff8123456789abc` | `0x7ff8000000000000` | Positive quiet NaN with payload. |
| `0xfff8123456789abc` | `0x7ff8000000000000` | Negative quiet NaN with payload. |
| `0x7ff0000000000001` | `0x7ff8000000000000` | Positive signaling NaN. |
| `0xfff0000000000001` | `0x7ff8000000000000` | Negative signaling NaN. |

Canonical formatting remains `nan`, as defined by FARAWEAVE-SPEC-0001 and
FARAWEAVE-SPEC-0002.

## 9. Equality

### 9.1 Bool equality

`equals : Bool Bool -> Bool` returns `true` exactly when both operands are the
same Boolean value.

| Left | Right | Result |
| --- | --- | --- |
| `false` | `false` | `true` |
| `false` | `true` | `false` |
| `true` | `false` | `false` |
| `true` | `true` | `true` |

### 9.2 Int equality

`equals : Int Int -> Bool` returns `true` exactly when both mathematical Int
values are equal. Every input pair succeeds.

### 9.3 Double equality

`equals : Double Double -> Bool` uses IEEE numeric equality with an explicit
NaN rule:

- positive zero and negative zero compare equal;
- finite nonzero values compare equal exactly when they have the same numeric
  value;
- positive infinity compares equal only to positive infinity;
- negative infinity compares equal only to negative infinity; and
- NaN compares unequal to every value, including NaN itself.

The implementation must classify NaN without allowing a raw signaling-NaN test
fixture to raise a host floating-point exception. Required cases are:

| Left bits | Right bits | Result |
| --- | --- | --- |
| `0x0000000000000000` | `0x8000000000000000` | `true` |
| `0x7ff0000000000000` | `0x7ff0000000000000` | `true` |
| `0xfff0000000000000` | `0xfff0000000000000` | `true` |
| `0x7ff0000000000000` | `0xfff0000000000000` | `false` |
| `0x0000000000000001` | `0x0000000000000001` | `true` |
| `0x7fefffffffffffff` | `0x7fefffffffffffff` | `true` |
| `0x7ff8000000000000` | `0x7ff8000000000000` | `false` |
| `0xfff8123456789abc` | `0x3ff0000000000000` | `false` |
| `0x7ff0000000000001` | `0x7ff0000000000001` | `false` after boundary normalization |

## 10. Int-to-Double conversion

The only implicit scalar conversion remains:

```text
Int -> Double
```

For every Int value, conversion returns the nearest binary64 value using
round-to-nearest, ties-to-even. Conversion is unconditional and succeeds even
when it loses precision. It never returns `DomainError`.

The conversion result is independent of the ambient host rounding mode and of
the C or C++ implementation's default integer-to-floating conversion choice.

Required exact vectors are:

| Int input | Expected Double bits | Exact Double value |
| ---: | --- | ---: |
| `INT64_MIN` | `0xc3e0000000000000` | `-9223372036854775808` |
| `-9007199254740995` | `0xc340000000000002` | `-9007199254740996` |
| `-9007199254740994` | `0xc340000000000001` | `-9007199254740994` |
| `-9007199254740993` | `0xc340000000000000` | `-9007199254740992` |
| `-9007199254740992` | `0xc340000000000000` | `-9007199254740992` |
| `-9007199254740991` | `0xc33fffffffffffff` | `-9007199254740991` |
| `-1` | `0xbff0000000000000` | `-1` |
| `0` | `0x0000000000000000` | `0` |
| `1` | `0x3ff0000000000000` | `1` |
| `9007199254740991` | `0x433fffffffffffff` | `9007199254740991` |
| `9007199254740992` | `0x4340000000000000` | `9007199254740992` |
| `9007199254740993` | `0x4340000000000000` | `9007199254740992` |
| `9007199254740994` | `0x4340000000000001` | `9007199254740994` |
| `9007199254740995` | `0x4340000000000002` | `9007199254740996` |
| `INT64_MAX` | `0x43e0000000000000` | `9223372036854775808` |

Consequently, overload selection may make distinct Int values compare equal
after promotion:

```text
equals[9007199254740993 9007199254740992.0]
=> true
```

This is the required type-based promotion behavior, not a value-dependent
exception.

## 11. Bool operations and rejected conversions

For `x : Bool`:

```text
not(false) = true
not(true)  = false
```

There is no conversion between `Bool` and `Int` or between `Bool` and `Double`.
There is no `Double -> Int` conversion. A call needing one of these conversions
fails overload selection with `TypeError` before scalar-kernel execution, as
defined by FARAWEAVE-SPEC-0001; it is not a scalar `DomainError`.

## 12. DomainError reason and context

The only `DomainError` reason used by the checked arithmetic scalar kernels is
the stable,
lowercase identifier:

```text
integer_overflow
```

The primitive identity already distinguishes each checked operation, so
separate operation-specific overflow reasons are not used. Future primitive
specifications may add reason identifiers without changing this one. This
specification does not reserve or define reasons for primitives outside the
initial set.

An `integer_overflow` error must carry these structured fields:

| Field | Requirement |
| --- | --- |
| error kind | `DomainError` |
| reason | `integer_overflow` |
| primitive | Stable primitive identity and source name. |
| selected scalar signature | Ordered parameter types and result type, not a declaration-order index. |
| operands | Ordered projected scalar operands as typed values, not preformatted message text. |
| call source span | The call span defined by FARAWEAVE-SPEC-0002. |
| result index | Absent for a scalar result; the lowest failing zero-based index for a lifted vector result. |

Rendered diagnostic prose may change without changing the reason or structured
fields. Conformance tests must inspect the fields directly rather than parse a
message. No partial vector result is returned with an error.

## 13. Interaction with lifting

Overload selection and permitted promotion occur before the selected scalar
kernel executes. Therefore:

- an empty lifted result invokes the scalar kernel zero times and cannot report
  an otherwise possible overflow;
- an arity, type, shape, or resource failure invokes no scalar kernel;
- `Int -> Double` conversion occurs at scalar projection and is governed by
  section 10 for scalar and vector operands alike;
- a lifted Int overflow reports `integer_overflow` at the lowest failing result
  index; and
- evaluation returns no partial vector even if earlier indices succeeded.

The error precedence remains:

```text
ArityError
TypeError
ShapeMismatch
ResourceError
DomainError
```

## 14. C++20 and C11 backend requirements

The C++ evaluator and emitted C11 backend must implement the same language
contract, not merely the behavior their current hosts happen to provide.

A supported target must establish or prove all of the following before being
treated as conforming:

- an eight-byte binary64 `double` with radix 2, 53 significand bits, and maximum
  exponent 1024;
- a two's-complement-capable exact signed 64-bit storage type for Faraweave Int;
- bit-preserving binary64 transfer through an alias-safe operation such as
  `memcpy` rather than type-punning with undefined behavior;
- checked Int arithmetic before any possibly overflowing signed operation;
- correctly rounded `Int -> Double` conversion for every vector in section 10;
- strict binary64 addition, subtraction, and multiplication plus exact sign-bit
  negation and absolute value under the environment in section 6; and
- canonical NaN normalization at every Faraweave value boundary.

An implementation may use hardware arithmetic, the C/C++ floating-point
environment, explicit bit algorithms, or another proven method. It must reject
an unsupported build or execution target explicitly rather than silently use
different scalar semantics.

Compiler configuration is part of conformance evidence. Builds that enable
unsafe fast-math, assume finite values, flush subnormals, reassociate additions,
or ignore required floating-point-environment access are not conforming.

## 15. Required conformance coverage

At minimum, automated tests must cover:

1. every Int exact-value case in section 5, with operands exchanged for `add`
   and `mul` and ordered boundary cases for `sub`;
2. overflow immediately outside both Int64 result boundaries;
3. scalar and lifted `integer_overflow` structured fields;
4. lowest-index lifted overflow and absence of a partial result;
5. every Double bit-pattern vector in sections 7 through 10, including `dec`,
   `neg`, `abs`, `sub`, and `mul`;
6. positive and negative zero results and comparisons;
7. smallest and largest subnormals, smallest normal, and largest finite values;
8. positive and negative finite overflow to infinity;
9. equal and opposite infinities;
10. positive and negative quiet NaNs with differing payloads;
11. positive and negative raw signaling NaNs at the normalization boundary;
12. canonical NaN storage and canonical `nan` presentation;
13. `Int -> Double` neighbors on both sides of `2^53` and both Int64 extrema;
14. rejected Bool/numeric and `Double -> Int` conversions;
15. all Bool `equals` and `not` results;
16. execution under an ambient non-nearest rounding mode while still producing
    the required results and restoring that mode;
17. strict C++20 and strict C11 builds without unsafe floating optimizations;
    and
18. differential agreement among the direct evaluator, emitted C11, and native
    execution paths.

Tests of Double arithmetic and conversion must compare the 64-bit interchange
pattern. Decimal output comparison alone is insufficient. Equality tests
compare the exact Bool result. Tests involving NaN compare the canonical NaN
pattern after normalization, not a host-propagated payload.

## 16. Non-goals and deliberate boundaries

This specification does not define integer division, a `divide` primitive,
decimal floating point, arbitrary-precision integers, numeric-to-Bool truthiness,
floating-point ordering, total ordering, approximate equality, bit casts,
user-observable floating-point flags, or source syntax beyond FARAWEAVE-SPEC-0002.

Faraweave deliberately chooses checked Int overflow instead of modular or saturating
arithmetic, and one canonical semantic NaN instead of observable payload or
signaling behavior. Checked `dec`, `neg`, `abs`, `sub`, and `mul` deliberately
reuse the same lifting, overload, promotion, and `integer_overflow` model as
`inc` and `add`. These choices prioritize data integrity, deterministic
cross-backend behavior, and a small scalar contract. They are Faraweave decisions;
they do not assert compatibility with Anka or the bootstrap Level 1
implementation.
