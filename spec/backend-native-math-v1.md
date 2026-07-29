# Backend-native math v1 semantic and conformance policy

**Status:** Accepted for implementation

**Authority:** This document is the normative exception to the otherwise exact
binary64 backend parity required by the
[typed FWIR semantic contract](typed-fwir-semantic-contract.md). It is accepted
by [issue #39](https://github.com/tuncb/Faraweave/issues/39) and its
[decision record](../decisions/issue-39-backend-native-math-policy.md).

The key words **must**, **must not**, **required**, and **may** are normative.

## 1. Covered identities and backend calls (`FWIR-MATH-001`)

The exception covers only these reserved unary `Double -> Double`
elementwise operations:

| Primitive ID | Source name | Rust call | C call |
| ---: | --- | --- | --- |
| 29 | `sqrt` | `f64::sqrt` | `sqrt` |
| 30 | `exp` | `f64::exp` | `exp` |
| 31 | `log` (natural log) | `f64::ln` | `log` |
| 32 | `log10` | `f64::log10` | `log10` |
| 33 | `sin` | `f64::sin` | `sin` |
| 34 | `cos` | `f64::cos` | `cos` |
| 35 | `tan` | `f64::tan` | `tan` |
| 36 | `floor` | `f64::floor` | `floor` |
| 37 | `ceil` | `f64::ceil` | `ceil` |
| 38 | `trunc` | `f64::trunc` | `trunc` |

Signature and implementation IDs remain unassigned until each owning primitive
issue adds its registry row and direct backend dispatch. Implementations must
call the named backend function; Faraweave must not substitute a software algorithm, bit-level
rounding implementation, polynomial, or custom range reduction, and must not
add a math dependency.

All earlier and later identities are exact-parity operations unless another
accepted policy names them. Integer-to-Double promotion happens before the
covered call and retains the existing exact conversion contract.

## 2. Exact portable results (`FWIR-MATH-002`)

Every produced NaN is normalized immediately to Faraweave's canonical quiet
NaN bits `0x7ff8000000000000`. NaN payload, sign, signaling state, backend
`errno`, and floating exception flags are not language values.

The following results are exact across interpreter, generated C, and native
execution:

| Operation | Exact requirements |
| --- | --- |
| `sqrt` | `sqrt(+0)=+0`, `sqrt(-0)=-0`, `sqrt(+inf)=+inf`; negative finite values and `-inf` produce canonical NaN; NaN produces canonical NaN. |
| `exp` | `exp(±0)=1`, `exp(-inf)=+0`, `exp(+inf)=+inf`; NaN produces canonical NaN. |
| `log`, `log10` | either zero produces `-inf`, `1` produces `+0`, and `+inf` produces `+inf`; negative finite values and `-inf` produce canonical NaN; NaN produces canonical NaN. |
| `sin`, `tan` | signed zero is preserved; either infinity and NaN produce canonical NaN. |
| `cos` | either zero produces `1`; either infinity and NaN produce canonical NaN. |
| `floor`, `ceil`, `trunc` | every finite result is the exact mathematical directed integral binary64 result; signed zero, either infinity, and already integral finite inputs are preserved; NaN produces canonical NaN. |

Thus `floor(+min_subnormal)=+0`, `floor(-min_subnormal)=-1`,
`ceil(+min_subnormal)=1`, `ceil(-min_subnormal)=-0`, and
`trunc(±min_subnormal)=±0`. No integer cast may stand in for a rounding call.

## 3. Finite accuracy envelopes (`FWIR-MATH-003`)

Conformance uses independently computed, checked-in binary64 reference values.
For same-sign finite values, `ulp_distance` is the distance between their
monotonic IEEE-754 bit-order keys; `absolute_error` is the binary64 absolute
difference. A covered finite result conforms when:

| Operation | Envelope |
| --- | --- |
| `sqrt` | `ulp_distance <= 1` |
| `exp` | `ulp_distance <= 4` |
| `log`, `log10` | `ulp_distance <= 4` |
| `sin`, `cos` | `ulp_distance <= 8` **or** `absolute_error <= 2^-48` |
| `tan` | `ulp_distance <= 16` **or** `absolute_error <= 2^-46` |
| `floor`, `ceil`, `trunc` | exact result bits |

An envelope never permits a wrong sign, a NaN in place of a finite result, or
an infinity in place of a finite reference. If the reference is infinity or
signed zero, the result must match that exact classification and sign.
Gradual-underflow references, including subnormals, use the same envelope;
flushing an expected nonzero result to zero is allowed only when zero itself
falls inside that operation's stated envelope.

Accuracy vectors must include signed zero, the smallest subnormal, ordinary
values, values adjacent to important boundaries, overflow and underflow
neighbors for `exp`, values adjacent to one for logarithms, quadrant and
binary64 pole neighbors for trigonometric functions, inputs near `2^52` for
rounding, the largest finite input where defined, and difficult huge finite
trigonometric inputs. Large-argument range reduction and a finite tangent
value near a mathematical pole remain backend-native; tests use the envelope
for the concrete binary64 input and do not infer an infinity from a symbolic
description such as π/2.

## 4. Floating environment and failures (`FWIR-MATH-004`)

Calls execute under the existing Faraweave strict environment: round-to-nearest
with gradual underflow and masked exceptions. The implementation restores the
caller's supported control and status state after each scalar call. It must
not inspect or translate C `errno` or floating exception flags into a
Faraweave error, and results do not depend on the caller's hostile rounding,
trap, denormal, or flush controls.

Native domain, pole, overflow, and underflow outcomes described above are
successful Double values, not `DomainError`. Existing argument count, type,
shape, profile, resource, allocation, formatting, and output-device failures
retain their exact categories, provenance, winner order, and transactional
publication behavior.

## 5. Differential comparison and reproducibility (`FWIR-MATH-005`)

Differential tests compare covered numeric leaves using sections 2 and 3.
They compare every other value bit, type, shape, root order, diagnostic byte
and span, resource admission/usage/release event, ownership event, exit code,
stderr byte, and output-transaction effect exactly. A tolerant numeric
comparison must never skip or relax surrounding structural or failure data.

FWIR bytes, lowering identities, execution order, and nonnumeric observations
remain reproducible. Finite covered result bits may vary with the Rust
standard library, C implementation, target, compiler, or platform math
library even when every result conforms; callers requiring bit-reproducible
transcendentals must not use these operations.

## 6. FWIR compatibility (`FWIR-MATH-006`)

This additive capability retains physical format 1.0 and uses semantic version
1.1. Any program containing primitive ID 29 through 38 must contain the
mandatory feature `7=BackendNativeMathV1`; lowering adds it in numeric order.
The verifier rejects a missing feature, and a consumer that does not know
feature 7 rejects the artifact before constructing a `RawProgram`.

Feature 7 applies to every covered implementation, including the portable
exact rounding operations, because direct backend dispatch and floating-state
handling are part of their semantics. It is never advisory, and it does not
authorize unknown primitive, signature, or implementation IDs.

## 7. Requirement-to-evidence map

| Requirement | Executable evidence |
| --- | --- |
| `FWIR-MATH-001` | `rust:src/semantic_registry.rs::backend_native_math_primitive_reservation_is_narrow`<br>`rust:tests/backend_native_sqrt.rs::sqrt_uses_reserved_ids_double_selection_lifting_and_feature_seven`<br>`rust:tests/backend_native_exp.rs::exp_uses_contiguous_ids_double_selection_lifting_and_feature_seven`<br>`rust:tests/backend_native_log.rs::log_uses_contiguous_ids_lifting_promotion_and_shared_feature` |
| `FWIR-MATH-002` | `rust:tests/backend_native_math_policy.rs::backend_native_math_special_values_and_rounding_are_exact`<br>`rust:tests/backend_native_sqrt.rs::sqrt_special_values_finite_envelope_and_vectors_are_public_semantics`<br>`rust:tests/backend_native_exp.rs::exp_special_values_thresholds_finite_envelope_and_vectors_are_public_semantics`<br>`rust:tests/backend_native_log.rs::log_special_domain_and_finite_envelope_are_public_semantics`<br>`command:strict-c11-journey` |
| `FWIR-MATH-003` | `rust:tests/backend_native_math_policy.rs::backend_native_math_rust_reference_vectors_meet_policy`<br>`rust:tests/backend_native_sqrt.rs::sqrt_special_values_finite_envelope_and_vectors_are_public_semantics`<br>`rust:tests/backend_native_exp.rs::exp_special_values_thresholds_finite_envelope_and_vectors_are_public_semantics`<br>`rust:tests/backend_native_log.rs::log_special_domain_and_finite_envelope_are_public_semantics`<br>`command:strict-c11-journey` |
| `FWIR-MATH-004` | `rust:src/strict_float.rs::hostile_x86_environment_is_ignored_and_exactly_restored`<br>`rust:src/strict_float.rs::hostile_aarch64_environment_is_ignored_and_exactly_restored`<br>`command:strict-c11-journey` |
| `FWIR-MATH-005` | `rust:tests/backend_native_sqrt.rs::sqrt_resource_work_and_allocation_refusals_are_exact`<br>`rust:tests/backend_native_exp.rs::exp_resources_diagnostics_and_allocation_refusals_are_exact`<br>`rust:tests/backend_native_log.rs::log_resources_failures_cleanup_and_diagnostics_are_exact`<br>`rust:tests/fwir_public_contracts.rs::public_source_artifact_execution_c_and_resource_traces_are_differential`<br>`command:strict-c11-journey` |
| `FWIR-MATH-006` | `rust:tests/backend_native_sqrt.rs::sqrt_uses_reserved_ids_double_selection_lifting_and_feature_seven`<br>`rust:tests/backend_native_exp.rs::exp_fwir_roundtrip_and_malformed_identities_are_checked`<br>`rust:tests/backend_native_log.rs::log_fwir_roundtrip_malformed_identities_and_version_are_checked`<br>`rust:tests/fwir_conformance.rs::same_major_optional_compatibility_and_mandatory_rejection_are_exact`<br>`rust:src/fwir_decoder.rs::directory_extensions_and_feature_compatibility_are_explicit` |
