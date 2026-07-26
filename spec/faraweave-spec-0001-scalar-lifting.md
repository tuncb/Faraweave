# FARAWEAVE-SPEC-0001: Scalar Primitive Lifting over Scalars and Vectors

**Status:** Proposed

**Related issue:** [#23](https://github.com/tuncbahreadingmaterial-ops/Faraweave/issues/23)

**Implementation plan:** [FARAWEAVE-SPEC-0001 Implementation Plan](faraweave-spec-0001-implementation-plan.md)

**Source syntax:** [FARAWEAVE-SPEC-0002](faraweave-spec-0002-application-and-literal-syntax.md)

**Scalar domains:** [FARAWEAVE-SPEC-0003](faraweave-spec-0003-scalar-domain-semantics.md)

**Execution profiles:** [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md)

**Program parameters:** [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md)

**Structural tuples:** [FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md)

**Explicit sequential fan-out:** [FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md)

**Target:** Faraweave language rewrite; rank-0 and rank-1 values

**Compatibility:** The rewrite does not preserve source, semantic, API, ABI, or
implementation compatibility with the bootstrap Level 1 language.

## 1. Normative language

The key words **must**, **must not**, **required**, **shall**, **shall not**,
**should**, **should not**, and **may** describe requirements and permissions in
this specification.

Sections explicitly marked non-normative provide implementation guidance and
do not override the semantic requirements.

## 2. Purpose

Faraweave permits selected pure scalar primitives to operate element by element on
homogeneous vectors. This feature is called **scalar primitive lifting** or
**scalar extension**.

For example, scalar addition can be applied to two scalars, a scalar and a
vector, or two equal-length vectors. The scalar kernel remains the only
definition of the pointwise operation; lifting determines containers,
promotion, shape agreement, projection, and result construction.

This is not full rank polymorphism. General rank polymorphism requires
multidimensional shapes, cell ranks, frames, and axis rules, which are outside
this specification.

## 3. Non-goals

This specification does not define:

- multidimensional arrays;
- nested or boxed arrays;
- reductions, scans, filters, indexing, axes, or array views;
- user-defined functions, closures, or partial application;
- evaluation order for effectful expressions;
- source constructs beyond those defined by FARAWEAVE-SPEC-0002,
  FARAWEAVE-SPEC-0005, FARAWEAVE-SPEC-0006, and FARAWEAVE-SPEC-0007;
- scalar domains outside the initial kernels defined by FARAWEAVE-SPEC-0003,
  including integer division;
- optimizer, allocator, or generated-C implementation strategy; or
- compatibility with the bootstrap Level 1 implementation.

Only pure scalar primitives may use implicit elementwise lifting. A future
effect system must define separate rules before an effectful primitive can
lift.

## 4. Value domain

### 4.1 Scalar types

The scalar types in this core are:

```text
Bool
Int
Double
```

Their domains are:

- `Bool` contains `false` and `true`.
- `Int` is a signed 64-bit integer with range
  `-9223372036854775808` through `9223372036854775807`.
- `Double` is IEEE 754 binary64, including signed zero and infinities. NaN
  payload and sign are not language-observable in this specification.

A scalar value has shape `[]` and rank 0.

### 4.2 Vector values

A vector is an ordered, homogeneous sequence of scalar values:

```text
Vector<T>(x0, x1, ..., x(n-1))
```

where `T` is `Bool`, `Int`, or `Double`, every `xi` has type `T`, and `n >= 0`.
A vector has shape `[n]` and rank 1.

Empty vectors are valid and retain their element type:

```text
Vector<Bool>()
Vector<Int>()
Vector<Double>()
```

These are distinct typed values even though ordinary formatting renders each
as `()`.

### 4.3 Scalars and singleton vectors

A scalar is not a one-element vector:

```text
42          : Scalar<Int>
(42)        : Vector<Int> with shape [1]
```

The values remain observably different under shape inspection and lifting.
A singleton vector does not broadcast to a longer vector.

### 4.4 Element type and shape

```text
element_type(Scalar<T>(x))       = T
element_type(Vector<T>(items))   = T

shape(Scalar<T>(x))              = []
shape(Vector<T>(n items))        = [n]
```

Element type is available without inspecting vector elements. In particular,
it is available for an empty vector.

## 5. Primitive declarations

### 5.1 Descriptor

A primitive descriptor contains:

1. a stable primitive identity;
2. a source name;
3. one or more signatures;
4. a lifting mode; and
5. an implementation identity or dispatch entry.

```text
lifting none
lifting elementwise
```

Lifting mode is explicit metadata. It must not be inferred from a host-language
function pointer, argument representation, return type, or naming convention.

### 5.2 Elementwise primitives

An elementwise primitive declares one or more **scalar kernel signatures**:

```text
primitive inc
  lifting elementwise
  scalar signatures:
    Int    -> Int
    Double -> Double

primitive add
  lifting elementwise
  scalar signatures:
    Int Int       -> Int
    Double Double -> Double

primitive equals
  lifting elementwise
  scalar signatures:
    Bool Bool     -> Bool
    Int Int       -> Bool
    Double Double -> Bool

primitive not
  lifting elementwise
  scalar signatures:
    Bool -> Bool
```

Each scalar kernel signature has arity at least one. Its parameters and result
are scalar types. The selected signature determines the result element type.

### 5.3 Structural primitives

A structural primitive uses its declared containers exactly and does not lift
implicitly:

```text
primitive iota
  lifting none
  signatures:
    Scalar<Int> -> Vector<Int>

primitive length
  lifting none
  signatures:
    Vector<Bool>   -> Scalar<Int>
    Vector<Int>    -> Scalar<Int>
    Vector<Double> -> Scalar<Int>
```

The initial `iota` behavior is:

```text
iota[n] = Vector<Int>(1, 2, ..., n)  when n > 0
iota[n] = Vector<Int>()              when n <= 0
```

`iota` is the required spelling.

Structural signatures require exact container and element types unless that
primitive's separate specification explicitly declares a conversion. The
implicit numeric promotion in this specification is for elementwise scalar
kernel selection only.

Examples:

```text
iota[3]
=> (1 2 3)

inc[iota[3]]
=> (2 3 4)

iota[(3)]
=> TypeError: iota argument 1 expected Scalar<Int>, got Vector<Int>
```

### 5.4 Descriptor validity

Before evaluating source, an implementation must validate its primitive table.
A valid table has:

- unique primitive identities and names;
- no primitive source name equal to the reserved program-header keyword
  `parameters` from FARAWEAVE-SPEC-0005;
- at least one signature per primitive;
- no duplicate signatures;
- no overload ambiguity under the promotion rules in section 7;
- only scalar parameters and a scalar result for an elementwise signature;
- arity of at least one for an elementwise signature; and
- a valid implementation dispatch entry for every signature.

An invalid built-in descriptor is an implementation/configuration failure, not
an ordinary Faraweave program error. Descriptor conformance must include a table
whose otherwise-valid primitive is rejected solely because its source name is
`parameters`.

## 6. Application syntax

FARAWEAVE-SPEC-0002 defines the normative source grammar. General primitive calls
use bracketed application:

```text
p[e1 e2 ... ek]
```

Unary calls may also use right-associative prefix application:

```text
p e
```

Thus `inc 5` and `inc[5]` denote the same one-argument call, while a
multi-argument call such as `add[1 2]` requires brackets. Parenthesized source
values denote vectors, not grouping. `Bool()`, `Int()`, and `Double()` are the
source spellings of typed empty vectors.

Right-hand sides such as `Vector<Int>()` expose the semantic element type for
clarity and are not canonical output text. Section 14 defines ordinary
formatting, under which every typed empty vector is printed as `()`.

## 7. Numeric promotion and overload selection

### 7.1 Permitted promotion

There is one implicit element-type promotion:

```text
Int -> Double
```

There are no implicit conversions:

- from `Double` to `Int`;
- between `Bool` and a numeric type; or
- between scalar and vector containers.

Promotion of an `Int` vector is conceptual and pointwise. It must not require
materializing a converted vector before applying the scalar kernel.

### 7.2 Binary64 conversion

`Int -> Double` conversion uses IEEE 754 round-to-nearest, ties-to-even. The
conversion is type-based, unconditional, and may lose precision.

FARAWEAVE-SPEC-0003 defines the exact conversion result at every Int64 value and
provides normative bit-pattern vectors at the precision-loss boundaries.

For example, the integer `9007199254740993` is not exactly representable as
binary64:

```text
add[9007199254740993 0.0]
=> 9007199254740992.0
```

Assuming ordinary IEEE numeric equality for the scalar `equals` kernel:

```text
equals[9007199254740993 9007199254740992.0]
=> true
```

This specification does not permit a value-dependent "promote only if exact"
rule.

### 7.3 Selection algorithm

For an elementwise call with `k` arguments:

1. Retain the primitive's scalar signatures with arity `k`.
2. If none exist, return `ArityError`.
3. For every remaining signature, compare each argument's element type with the
   corresponding scalar parameter type.
4. Retain a signature when every comparison is either an exact match or the
   permitted `Int -> Double` promotion.
5. If none remain, return `TypeError`.
6. Assign each remaining signature a cost equal to the number of promoted
   arguments. A promoted vector counts once, not once per element.
7. Select the unique signature having minimum cost.

Primitive descriptor validation must prevent equal-cost ambiguity, so a valid
runtime primitive table always produces a unique selection.

For example, this overload set is invalid:

```text
primitive ambiguous
  lifting elementwise
  scalar signatures:
    Double Int -> Double
    Int Double -> Double
```

For `ambiguous[1 2]`, either signature would require one promotion. The
primitive table must reject the descriptor instead of making the runtime call
depend on declaration order.

Examples using the `add` signatures from section 5:

```text
add[1 2]
```

has candidates:

```text
Int Int       -> Int       cost 0
Double Double -> Double    cost 2
```

and therefore returns `3 : Scalar<Int>`.

```text
add[1 2.5]
```

rejects the `Int` signature and selects:

```text
Double Double -> Double    cost 1
```

It therefore returns `3.5 : Scalar<Double>`.

Selection uses element types rather than elements, so typed empty vectors work
without invoking the scalar kernel:

```text
add[Int() 0.5]
=> Vector<Double>()
```

## 8. Shape agreement

Define `agree` over rank-0 and rank-1 shapes:

```text
agree([],     [])      = []
agree([],     [n])     = [n]
agree([n],    [])      = [n]
agree([n],    [n])     = [n]
agree([n],    [m])     = undefined, when n != m
```

For more than two arguments, agreement is applied from left to right. The first
vector argument establishes the expected vector length. Scalars do not
establish or change it. The first later vector with a different length causes
`ShapeMismatch`.

Consequences:

- scalars agree with every vector shape;
- vectors agree only when their lengths are equal;
- a one-element vector does not broadcast to a longer vector;
- no truncation, cycling, padding, Cartesian product, or implicit reshape
  occurs; and
- an empty vector agrees with scalars and other empty vectors, but not with a
  nonempty vector.

Examples:

```text
add[10 (1 2 3)]
=> (11 12 13)

add[(10) (1 2 3)]
=> ShapeMismatch: add argument 2 expected shape [1], got [3]

add[Int() 10]
=> Vector<Int>()

add[Int() (1)]
=> ShapeMismatch: add argument 2 expected shape [0], got [1]
```

For a hypothetical ternary elementwise `sum3`:

```text
sum3[10 (1 2) (3 4 5)]
=> ShapeMismatch: sum3 argument 3 expected shape [2], got [3]
```

## 9. Dynamic lifting semantics

Let `p0` be the selected scalar kernel for an elementwise primitive `p`.

### 9.1 Projection

At result index `i`:

```text
project(Scalar<T>(s), i)      = s
project(Vector<T>(v), i)      = v[i]
```

If the selected signature requires `Int -> Double`, conversion occurs when the
argument is projected into the scalar kernel. Scalar extension and promotion do
not require a repeated-scalar or converted-vector allocation.

### 9.2 Scalar result

When every argument is scalar:

```text
p[s1 ... sk] => p0(convert(s1), ..., convert(sk))
```

The result is `Scalar<R>`, where `R` is the selected scalar signature's result
type.

### 9.3 Vector result

When agreed shape is `[n]`:

```text
p[a1 ... ak]
  => Vector<R>(
       p0(convert(project(a1, 0)), ..., convert(project(ak, 0))),
       p0(convert(project(a1, 1)), ..., convert(project(ak, 1))),
       ...,
       p0(convert(project(a1, n-1)), ..., convert(project(ak, n-1))))
```

The result is homogeneous and has shape `[n]`. The result element type may
differ from every argument element type, as with comparisons returning `Bool`.

### 9.4 Empty-vector result

When agreed shape is `[0]`, the result is an empty vector whose element type is
the selected scalar signature's result type. The scalar kernel is invoked zero
times.

```text
add[Int() 10]
=> Vector<Int>()

add[Int() 0.5]
=> Vector<Double>()

equals[Int() 10]
=> Vector<Bool>()
```

If a scalar argument would be outside the scalar kernel's domain, the empty
result still succeeds because there is no result position at which to invoke
the kernel. Assuming division by zero is a scalar domain error:

```text
divide[Int() 0]
=> Vector<Int>()
```

## 10. Static typing rule

This section is a metatheoretic rule and a contract for a future static checker.
The rewrite may initially implement the same decisions dynamically from tagged
values.

FARAWEAVE-SPEC-0005 requires equivalent whole-program static type and shape
analysis before argument binding for program execution and before emission.
That requirement supersedes the initial dynamic-checking permission for every
surface governed by FARAWEAVE-SPEC-0005.

Let:

- `Ci` be the actual container of argument `i`, either `Scalar` or `Vector`;
- `Ai` be its actual element type;
- `Pi` be parameter `i` of the selected scalar kernel signature; and
- `R` be that signature's scalar result type.

Define container join:

```text
join(Scalar, Scalar) = Scalar
join(Scalar, Vector) = Vector
join(Vector, Scalar) = Vector
join(Vector, Vector) = Vector
```

Let `Ai <= Pi` mean either `Ai = Pi` or `Ai = Int` and `Pi = Double`.
Numeric selection and promotion occur before the lifting type rule.

For:

```text
p : P1 P2 ... Pk -> R
Gamma |- ei : Ci<Ai>
Ai <= Pi for every i
C = join(C1, C2, ..., Ck)
```

the lifted call has type:

```text
Gamma |- p[e1 ... ek] : C<R>
```

Vector lengths are checked dynamically unless a future type system tracks
shapes. Structural primitives use their exact declared container-aware
signatures instead of this lifting rule.

## 11. Validation and failure order

For every primitive call, validation and execution occur in this observable
order:

```text
1. arity
2. signature selection and element types
3. shape agreement
4. resource and allocation preflight
5. scalar-kernel or structural-primitive execution
```

All arity, type, shape, representability, and active execution-profile checks
complete before an elementwise scalar kernel is invoked at any result position.

This order resolves calls with multiple defects deterministically.

### 11.1 Arity before type

```text
add[true]
=> ArityError
```

### 11.2 Type before shape

```text
add[(1 2) (true false true)]
=> TypeError
```

The call also contains unequal vector lengths, but signature selection fails
before shape agreement.

### 11.3 Shape before domain

Assuming a scalar `divide` kernel that rejects a zero divisor:

```text
divide[(10 20) (2 0 5)]
=> ShapeMismatch
```

The scalar division kernel is invoked zero times.

### 11.4 Resource preflight before domain

If the agreed output cannot be represented, is rejected by the active execution
profile, or cannot be allocated, the call returns `ResourceError` before
testing any scalar value for a domain error.

## 12. Resource safety and execution profiles

Scalar lifting has mathematical semantics independent of finite machines.
`iota` and other vector-producing primitives therefore have no magic
primitive-specific element cap in the core language.

For every positive `n`, the semantic result of `iota[n]` is a vector containing
the integers from 1 through `n`. A call is not a type or domain error merely
because `n` exceeds a fixed threshold such as one million:

```text
iota[1000001]
=> Vector<Int> with 1000001 elements, when resources are sufficient
```

Finite implementations still require overflow-safe sizing, explicit allocation
failure, and optional limits for deployments that evaluate untrusted or
resource-sensitive programs. Those concerns are centralized rather than
encoded separately in `iota`, lifting, vector literals, or future primitives.

### 12.1 Mandatory representability and allocation checks

Before producing a vector or reserving primitive workspace, an implementation
must:

1. prove that the logical element count is representable by the implementation's
   index and container-size types;
2. calculate payload and workspace byte counts with overflow-safe arithmetic;
3. consult the active execution profile, when it defines a relevant limit;
4. obtain the complete required allocation before scalar-kernel execution; and
5. return an explicit `ResourceError` without a partial language value if any
   check or allocation fails.

Host allocation exceptions or process termination must not serve as normal
Faraweave error control flow. A result is either fully constructed or absent.

### 12.2 Central resource boundary

Every allocation-capable operation must use the same evaluation resource
boundary, conceptually:

```text
allocate_vector(context, element_type, element_count)
reserve_workspace(context, byte_count)
```

This includes:

| Operation | Resource behavior |
| --- | --- |
| `iota` | Allocates a vector whose length is derived from a scalar value. |
| Vector literals | Allocate their declared element count when introduced. |
| Lifted `inc`, `add`, `equals`, and similar calls | Allocate a result vector with the agreed input length. |
| `filter` | May allocate up to its input length. |
| `sort` | May allocate a result and temporary workspace. |
| `length`, `sum`, and scalar-only results | Normally allocate no result vector, though an implementation must still account for any workspace it uses. |

Primitive implementations must not embed unrelated per-primitive allocation
constants. An operation may impose a semantic bound only when that bound is
part of the operation itself rather than a substitute for resource management.

### 12.3 Optional execution profiles

An execution profile may define limits such as:

```text
max_vector_bytes
max_live_evaluation_bytes
max_work_units
```

Concrete values are product or deployment configuration, not scalar-lifting
semantics. A named profile must document its limits, units, reset boundaries,
and resource-error behavior. Execution backends claiming the same named profile
must enforce the same observable profile contract.

A local trusted CLI profile may omit an arbitrary logical work cap while still
performing all mandatory representability and allocation checks. An untrusted
service profile may impose strict memory and work limits.

[FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md) resolves this
delegated decision: it defines the initial named profiles `trusted-local-v1`
and `bounded-v1`, their limit units and charging events, resource-context
reset boundaries, `ResourceError` presentation, the cross-backend agreement
contract, and deterministic conformance testing.

### 12.4 Optional logical work accounting

A profile may count logically processed or produced elements as a work budget:

```text
iota[3]          may charge 3 work units
inc[iota[3]]     may charge another 3 work units
```

This is a computation or denial-of-service policy, not a measurement of memory.
It is not required by the core language. If enabled, the profile must define
logical charging so optimizer fusion or storage reuse does not silently change
the work limit.

### 12.5 Rationale for centralization

A primitive-local cap can be a reasonable bootstrap guard when one sequence
constructor is the only operation capable of allocating an arbitrarily large
vector. It becomes incomplete once vector literals, scalar lifting, filtering,
sorting, and other producers can allocate results.

Central resource handling preserves one rule for all present and future
allocators. It also separates language meaning from a deployment's memory and
work policy.

## 13. Errors and diagnostic context

Argument positions in diagnostics are one-based. Vector result indices are
zero-based.

Errors carry structured context where applicable. Presentation code may render
that context as text, but conformance tests must not need to parse a diagnostic
message to recover primitive, argument, shape, or element information.

### 13.1 ArityError

No signature for the named primitive has the supplied arity.

Required context:

```text
primitive identity or name
supplied arity
declared arities
call source location
```

### 13.2 TypeError

At least one signature has the supplied arity, but no signature accepts all
argument element types and containers after permitted promotion.

Required context:

```text
primitive identity or name
actual argument container and element types
accepted signatures
call source location
```

When one argument can be identified as the first argument that eliminates the
last remaining candidate, its one-based argument position should also be
reported.

### 13.3 ShapeMismatch

Two vector arguments have unequal lengths.

Required context:

```text
primitive identity or name
one-based mismatching argument position
expected shape established by the first vector argument
actual shape
call source location
```

Example:

```text
add[(1 2) (10 20 30)]
=> ShapeMismatch: add argument 2 expected shape [2], got [3]
```

### 13.4 ResourceError

A result or required workspace cannot be represented, is refused by the active
execution profile, or cannot be allocated completely.

Required context:

```text
primitive identity or name, when associated with a call
reason: size_overflow, profile_limit, or allocation_unavailable
requested element and byte counts, when available
profile name, limit kind, configured limit, and current usage, when applicable
call source location
```

### 13.5 DomainError

The selected scalar kernel is undefined for a projected value combination, or
a structural primitive rejects an otherwise well-typed value.

Examples include division by zero and signed integer overflow when the scalar
primitive's own contract defines those conditions as errors.

For a lifted vector call, the observable error is the lowest failing result
index. The diagnostic includes that zero-based index. An implementation may
evaluate positions in parallel only if it preserves this deterministic result.

```text
divide[(8 9 10) (2 0 5)]
=> DomainError: divide failed at result index 1: division by zero
```

No partial vector is returned. Scalar calls omit the result-index field.

FARAWEAVE-SPEC-0003 defines `integer_overflow`, the sole domain reason used by the
initial scalar kernels, and the additional structured scalar context.

## 14. Canonical value formatting

Canonical formatting is observable across the REPL, runner, emitted C, and
native executables.

### 14.1 Scalars

```text
Bool    false or true
Int     signed decimal without a leading plus sign
Double  canonical binary64 spelling described below
```

A finite `Double` uses the shortest decimal spelling that round-trips to the
same binary64 value under round-to-nearest, ties-to-even. Formatting then:

- uses lowercase `e` for an exponent;
- removes an exponent's redundant leading plus sign and leading zeroes;
- includes `.0` when the shortest spelling otherwise contains neither a decimal
  point nor an exponent, so `Int` and `Double` remain visibly distinct; and
- preserves negative zero as `-0.0`.

Special values format as:

```text
positive infinity  inf
negative infinity  -inf
NaN                nan
```

NaN payload and sign are not preserved by canonical presentation.

Examples:

```text
1        Scalar<Int>
1.0      Scalar<Double>
-0.0     Scalar<Double>
true     Scalar<Bool>
```

### 14.2 Vectors

Vectors use parentheses, one ASCII space between elements, and no commas:

```text
(1 2 3)
(1.0 2.5 3.0)
(false true false)
()
```

Ordinary formatting does not display an empty vector's element type. The type
remains available to subsequent primitive selection and introspection.

## 15. Required examples

```text
iota[3]
=> (1 2 3)

iota[0]
=> Vector<Int>()

iota[-3]
=> Vector<Int>()

inc[iota[3]]
=> (2 3 4)

inc 5
=> 6

inc inc 5
=> 7

add[1 2]
=> 3

add[1 2.5]
=> 3.5

add[10 (1 2 3)]
=> (11 12 13)

add[(1 2 3) 10]
=> (11 12 13)

add[(1 2 3) (10 20 30)]
=> (11 22 33)

add[(1 2 3) 0.5]
=> (1.5 2.5 3.5)

equals[2 (1 2 3 2)]
=> (false true false true)

equals[true (false true)]
=> (false true)

not[(false true)]
=> (true false)

add[(10) (1 2 3)]
=> ShapeMismatch: add argument 2 expected shape [1], got [3]

add[(1 2) (10 20 30)]
=> ShapeMismatch: add argument 2 expected shape [2], got [3]

add[(1 2) (true false)]
=> TypeError

add[Int() 0.5]
=> Vector<Double>()

equals[Int() 10]
=> Vector<Bool>()

divide[Int() 0]
=> Vector<Int>()

divide[(8 9 10) (2 0 5)]
=> DomainError at result index 1
```

`divide` examples assume a separately declared scalar division primitive whose
scalar contract treats division by zero as a domain error.

## 16. Required conformance coverage

Every elementwise primitive must be tested for every applicable category:

1. scalar arguments and scalar result;
2. each argument position receiving a vector while the others are scalar;
3. equal-length vectors in multiple argument positions;
4. unequal-length vectors;
5. a singleton vector remaining a vector and not broadcasting;
6. empty vector with scalars;
7. equal-typed empty vector with empty vector;
8. mixed numeric typed empty vectors and promotion;
9. empty vector with nonempty vector mismatch;
10. exact overload preference;
11. permitted `Int -> Double` promotion in scalar and vector positions;
12. the signed 64-bit precision-loss boundary during promotion;
13. rejected type combinations, including Bool/numeric mixing;
14. result element type different from every input element type;
15. wrong arity;
16. type-before-shape error precedence;
17. shape-before-domain error precedence;
18. resource-error-before-domain precedence;
19. lowest-index deterministic domain failure;
20. zero scalar-kernel invocations for an empty result;
21. zero scalar-kernel invocations after an arity, type, shape, or resource
    failure; and
22. scalar-kernel consistency at numeric and domain boundaries.

Every structural primitive must be tested for exact container and element-type
matching, rejected lifting, empty results where applicable, domain errors, and
resource failures. Primitive-table validation must also reject duplicate,
missing-implementation, and equal-cost ambiguous descriptors. Resource tests
must cover size-arithmetic overflow, an injected profile limit, allocation
failure without a partial value, and use of the shared resource boundary by
multiple vector-producing primitives.

For every supported program, all shipped execution paths must accept or reject
the same program and produce the same canonical values or source diagnostics:

```text
direct evaluator
REPL and file runner
emitted C compiled and executed
native build output
```

Successful differential tests must cover arbitrary lifted vector contents, not
only sequences reconstructible from a vector length.

## 17. Semantic laws

For every pure elementwise primitive `p` and valid selected scalar kernel `p0`:

### Scalar consistency

```text
lift(p, s1, ..., sk) = p0(convert(s1), ..., convert(sk))
```

### Shape preservation

If argument shapes agree to `S`, then:

```text
shape(lift(p, args)) = S
```

### Result-type preservation

If the selected scalar signature returns `R`, then a scalar lifted result has
type `Scalar<R>` and a vector lifted result has element type `R`.

### Pointwise correspondence

For result vector `r` and every valid index `i`:

```text
r[i] = p0(convert(project(arg1, i)), ..., convert(project(argk, i)))
```

### Empty preservation

If agreed shape is `[0]`, result shape is `[0]`, result element type comes from
the selected scalar signature, and `p0` is invoked zero times.

### Deterministic failure

Call validation follows section 11. If multiple projected calls would fail,
the observable `DomainError` reports the lowest failing result index.

## 18. Non-normative C++ representation guidance

This section illustrates a data-oriented representation compatible with
Faraweave's contributor constraints. Exact field names are not normative.

### 18.1 Values

```cpp
enum class ScalarType {
  boolean,
  integer,
  double_precision,
};

enum class ContainerKind {
  scalar,
  vector,
};

struct ScalarValue {
  ScalarType type;
  bool boolean;
  std::int64_t integer;
  double double_precision;
};

struct VectorValue {
  ScalarType element_type;
  std::vector<std::uint8_t> booleans;
  std::vector<std::int64_t> integers;
  std::vector<double> doubles;
};

struct Value {
  ContainerKind container;
  ScalarValue scalar;
  VectorValue vector;
};
```

The active vector buffer is selected once by `element_type`. Bool elements use
bytes rather than C++ `std::vector<bool>`. Elements do not carry individual type
tags.

Required representation invariants include:

- exactly one vector buffer is selected by `element_type`;
- inactive vector buffers are empty;
- every active-buffer element has the selected element type;
- vector length is the active buffer's length;
- a typed empty vector retains `element_type`; and
- rank-1 shape is derived as `[length]` rather than stored in a dynamic shape
  vector.

A standard-library tagged union of typed buffers is another valid
implementation, provided expected lookup failures remain explicit and Faraweave
does not use host exceptions for recoverable control flow.

### 18.2 Primitive metadata

```cpp
struct ValueType {
  ContainerKind container;
  ScalarType element;
};

struct PrimitiveSignature {
  const ValueType *parameters;
  std::size_t parameter_count;
  ValueType result;
};

enum class LiftingMode {
  none,
  elementwise,
};

struct PrimitiveDescriptor {
  PrimitiveId id;
  std::string_view name;
  LiftingMode lifting;
  const PrimitiveSignature *signatures;
  std::size_t signature_count;
};
```

Static-lifetime parameter and signature tables avoid one dynamic allocation per
descriptor. Scalar-kernel dispatch may use primitive identity plus selected
signature index. Metadata remains explicit and is not inferred from the
host-language function's type.

### 18.3 Shared application path

One free function should own elementwise application:

```text
apply_primitive(context, descriptor, arguments, call_location)
  validate arity
  select signature and promotion
  validate shape
  preflight representability and the active resource profile
  allocate typed result when vector-shaped
  project, convert, and invoke the scalar kernel
  return a scalar, vector, or explicit error
```

Primitive-specific code implements scalar kernels and structural operations;
it does not enumerate scalar/vector combinations.

### 18.4 Flat expression representation

The rewrite can preserve Faraweave's flat forward-evaluation architecture while
generalizing calls:

```cpp
enum class ExpressionKind {
  scalar_literal,
  vector_literal,
  primitive_call,
};

struct ExpressionNode {
  ExpressionKind kind;
  SourceLocation location;
  PrimitiveId primitive;
  std::size_t first_argument;
  std::size_t argument_count;
};

struct Program {
  std::vector<ExpressionNode> nodes;
  std::vector<std::size_t> arguments;
  std::vector<std::size_t> roots;
};
```

Call arguments refer to earlier nodes through a contiguous argument-index arena.
This retains visible ownership and iterative evaluation without one expression
tag per primitive or one fixed argument field per call.

### 18.5 Backend boundary

The evaluator and C emitter should consume the same validated typed program or
equivalent semantic IR. The C backend must preserve actual vector values and
element types; it must not reconstruct arbitrary vectors from length alone.

For programs without runtime parameters, whether generated C embeds validated
constants or lowers typed primitive loops is an implementation decision.
FARAWEAVE-SPEC-0005 requires parameterized programs to retain typed parameter slots
and value-dependent execution in the artifact. Either strategy may still
optimize statically safe work, but it must preserve the differential conformance
requirement in section 16 and the configured resource profile.

## 19. Generalization path

A later specification may replace `lifting elementwise` with argument cell
ranks:

```text
add     : cell ranks [0, 0] -> 0
sum     : cell ranks [1]    -> 0
reverse : cell ranks [1]    -> 1
```

At that point, each argument is split into a frame and cells, frames agree under
a separately documented rule, and the primitive is applied to corresponding
cells. The scalar/vector semantics here become the rank-0/rank-1 special case.

The rewrite should add multidimensional shape storage only when that capability
is specified and implemented.

## 20. Decisions intentionally delegated to other specifications

FARAWEAVE-SPEC-0002 resolves the rewrite's application, scalar-literal,
vector-literal, typed-empty-vector, whitespace, delimiter, line-ending, and
source-span syntax. FARAWEAVE-SPEC-0003 resolves the initial kernels' Int overflow,
binary64 arithmetic, floating equality, NaN, promotion, and domain-error
semantics. [FARAWEAVE-SPEC-0004](faraweave-spec-0004-execution-profiles.md) resolves
which execution profiles Faraweave ships and their optional memory and work
limits. [FARAWEAVE-SPEC-0005](faraweave-spec-0005-program-parameters.md) resolves typed
program parameters, argument binding, and the analysis/execution boundary.
[FARAWEAVE-SPEC-0006](faraweave-spec-0006-structural-tuples-and-profile-v2.md) resolves
structural tuple values and types, static one-level prefix spreading, and the
versioned profile-v2 resource boundary. Tuple arguments do not extend scalar
primitive lifting and are rejected unless a later specification adds a
tuple-aware signature.
[FARAWEAVE-SPEC-0007](faraweave-spec-0007-explicit-sequential-fanout.md) resolves the
explicit branch-template syntax that borrows one operand across sequential
primitive calls and returns their independently owned results as a tuple. It
does not change primitive lifting, direct application, or tuple-aware signature
selection. The following remaining decisions do not change scalar
lifting itself
but must be defined before their associated primitives or product surfaces
become conforming:

- integer division semantics;
- multidimensional arrays and general rank polymorphism.
