# Canonical FWIR v1 artifact encoding

**Status:** Normative encoding decision for
[issue #10](https://github.com/tuncb/Faraweave/issues/10). Encoder, decoder,
and public-surface implementation belong to issues
[#11](https://github.com/tuncb/Faraweave/issues/11) through
[#13](https://github.com/tuncb/Faraweave/issues/13).

**Semantic contract:** [Typed FWIR semantic contract](typed-fwir-semantic-contract.md).

## 1. Scope and conformance

FWIR v1 is a canonical, sectioned binary representation of one immutable
`VerifiedProgram`. Faraweave is the only authoritative v1 producer; producer
metadata is informational and does not authenticate an artifact. Signing,
encryption, compression, transport, and third-party production are outside
this version.

Keywords **MUST**, **MUST NOT**, **SHOULD**, and **MAY** are normative. A
canonical encoder emits exactly one byte sequence for the same verified
program and producer options. A decoder first validates this physical
contract, constructs untrusted `RawProgram` records, and only publishes a
`VerifiedProgram` after the semantic verifier succeeds.

## 2. Wire primitives

- All integers are unsigned fixed-width `u8`, `u16`, `u32`, or `u64` in
  little-endian order unless a field is explicitly an `i64`.
- An `i64` is the two's-complement 64-bit bit pattern written little-endian.
- A binary64 is its exact IEEE-754 `u64` bits. The semantic verifier rejects
  any noncanonical NaN; no encoder or decoder normalizes floating-point bits.
- Indexes and counts are `u32`; byte offsets and byte lengths are `u64`.
- `NONE` for an optional `u32` index is `0xffff_ffff`.
- Booleans are one byte: `0` or `1`; every other value is noncanonical.
- Strings are unnormalized, length-delimited UTF-8 bytes. NUL has no special
  meaning.
- Reserved fields and unused variant payloads MUST be zero.
- Records have the sizes below regardless of Rust layout, pointer width,
  alignment, or host byte order. There is no padding between records or
  sections.

## 3. File header and section directory

The 32-byte header is:

| Offset | Width | Field | Canonical v1 value |
| ---: | ---: | --- | --- |
| 0 | 8 | magic | `46 57 49 52 0d 0a 1a 0a` (`FWIR\r\n\x1a\n`) |
| 8 | 2 | format major | `1` |
| 10 | 2 | format minor | `0` |
| 12 | 4 | header size | `32` |
| 16 | 2 | directory-entry size | `24` |
| 18 | 2 | reserved | `0` |
| 20 | 4 | section count | number of directory entries |
| 24 | 8 | directory offset | `32` |

Exactly `section_count` 24-byte entries immediately follow the header:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 2 | section ID |
| 2 | 2 | flags |
| 4 | 4 | fixed record size, or `0` for a variable payload |
| 8 | 8 | absolute payload offset |
| 16 | 8 | payload byte length |

Flag bit 0 means mandatory-to-understand and bit 1 means the section
participates in canonical program identity. All other flag bits are reserved.
Directory entries are strictly increasing by section ID. Payloads occur in
that same order, start immediately after the directory, are contiguous, and
end exactly at end-of-file; overlap, gaps, padding, and trailing bytes are
noncanonical.

`MODL` is always present. Every other known semantic section is omitted
exactly when it has zero records, so an omitted known section decodes as an
empty arena. A present fixed-record section has nonzero length divisible by
its specified record size. A known section appears at most once.

## 4. Canonical sections

| ID | Name | Flags | Record size | Meaning |
| ---: | --- | ---: | ---: | --- |
| 1 | `MODL` | `3` | 8 | module metadata |
| 2 | `FEAT` | `3` | 4 | semantic features |
| 3 | `STRS` | `3` | 0 | canonical string pool |
| 4 | `SRCU` | `3` | 8 | source units |
| 5 | `PARM` | `3` | 20 | parameters |
| 6 | `TYPE` | `3` | 12 | type records |
| 7 | `TYEL` | `3` | 4 | tuple type elements |
| 8 | `CONS` | `3` | 20 | constants |
| 9 | `COEL` | `3` | 12 | vector constant elements |
| 10 | `ORIG` | `3` | 28 | diagnostic origins |
| 11 | `EDGE` | `3` | 24 | ordered node operands |
| 12 | `SHCK` | `3` | 4 | dynamic shape-check positions |
| 13 | `BRAN` | `3` | 20 | fan-out branches |
| 14 | `NODE` | `3` | 56 | executable nodes |
| 15 | `OWNR` | `3` | 12 | logical ownership and release |
| 16 | `ROOT` | `3` | 8 | ordered program roots |
| 17 | `APPL` | `3` | 8 | explicit application plans |
| 18 | `OPRF` | `3` | 16 | stable built-in operation references |
| 32769 | `PROD` | `0` | 0 | optional producer metadata |

Every fixed-record section contains only consecutive records; its record count
is `payload_length / record_size`.

### 4.1 Module and derived ranges

`MODL` contains exactly one record: `semantic_major:u16`,
`semantic_minor:u16`, and `parameter_header_origin:u32` (`NONE` when absent).
`ProgramRanges` is not duplicated on the wire: verification requires every
arena range to be `{ start: 0, count: decoded_arena_length }`, so the decoder
reconstructs every field exactly from section record counts.

### 4.2 Features and strings

A `FEAT` record is `id:u16`, `class:u8`, `reserved:u8`. Class `0` is a
mandatory semantic feature and class `1` is optional advisory metadata.
Current `VerifiedProgram.features` entries are emitted in strictly increasing
ID order with class `0`; optional entries are not added to that vector.
Current IDs are `1=StableSemanticIds`, `2=Tuples`, `3=PrefixSpread`,
`4=FanOut`, `5=ApplicationPlans`, `6=OperationReferences`, and
`7=BackendNativeMathV1`; zero is invalid. IDs 1 through 7 are semantic capabilities and
MUST have class `0`: pairing a known current ID with class `1` is a
`NonCanonicalRecord` error rather than an advisory feature.
Feature 5 requires semantic minor 1 and physical format minor 1.
Feature 6 requires semantic minor 1 and physical format minor 1.
Feature 7 requires semantic minor 1 but does not by itself raise the physical
format minor.

`STRS` begins with `count:u32`, followed by `count` descriptors
`offset:u32, length:u32`, followed by one concatenated byte area. Offsets are
relative to the start of the byte area. Strings are unique and strictly
increasing by unsigned UTF-8 byte sequence, descriptors cover the byte area
contiguously from offset zero, and unused strings are forbidden. Source-unit
diagnostic names and parameter names reference this pool by `u32` index.

### 4.3 Sources, parameters, types, and constants

`SRCU` is `diagnostic_name:u32, byte_length:u32`.

`PARM` is `slot:u32, name:u32, scalar_type:u8, reserved[3],
declaration_origin:u32, name_origin:u32`.

`TYPE` is `kind:u8, scalar_type:u8, reserved:u16, element_start:u32,
element_count:u32`. Kinds are `1=Scalar`, `2=Vector`, and `3=Tuple`.
`scalar_type` is `1=Bool`, `2=Int`, `3=Double`; tuple uses zero.
`element_start` and `element_count` are zero except for Tuple. `TYEL` records
are `type_index:u32`.

`CONS` is `kind:u8, scalar_type:u8, reserved:u16, element_start:u32,
element_count:u32, payload:u64`. Kinds are `1=Scalar` and `2=Vector`.
Scalar uses zero element range and stores Bool, Int, or Double bits in
`payload`; Bool permits only `0` or `1`, Int uses the exact two's-complement
`i64` bit pattern, and Double uses exact IEEE-754 bits. Vector uses zero
payload and identifies its typed `COEL` range. `COEL` is
`scalar_type:u8, reserved[3], payload:u64` with the same scalar payload rules.

### 4.4 Provenance and graph sidecars

`ORIG` is seven `u32` fields in order: `source_unit`, begin `offset`, `line`,
`column`, then end `offset`, `line`, `column`.

`EDGE` is `producer:u32, argument_position:u32, access:u8,
cardinality_kind:u8, conversion:u8, ownership:u8, access_index:u32,
cardinality_length:u32, origin:u32`. Access is `1=WholeValue`,
`2=TupleElement`, or `3=FanOutOperandBorrow`; only TupleElement uses
`access_index`. Cardinality is `0=None`, `1=StaticScalar`,
`2=StaticVector`, or `3=DynamicVector`; only StaticVector uses
`cardinality_length`. Conversion is `1=Identity` or
`2=PromoteIntToDouble`. Ownership is `1=OwnedInput`,
`2=ImmutableBorrow`, or `3=InfallibleTransfer`.

`SHCK` is `argument_position:u32`.

`BRAN` is `node_start:u32, node_count:u32, root:u32,
placeholder_origin:u32, origin:u32`.

### 4.5 Nodes, ownership, and roots

`NODE` has the following fixed 56-byte shape:

| Offset | Width | Field |
| ---: | ---: | --- |
| 0 | 1 | kind |
| 1 | 1 | cardinality kind |
| 2 | 1 | lift mode |
| 3 | 1 | result element scalar type |
| 4 | 4 | result type |
| 8 | 4 | cardinality length |
| 12 | 4 | edge start |
| 16 | 4 | edge count |
| 20 | 4 | origin |
| 24 | 32 | eight variant `u32` words `a0` through `a7` |

Kinds are `1=Constant`, `2=ParameterBorrow`, `3=TupleConstruct`,
`4=SelectedApply`, `5=PrefixSpreadPrepare`, and `6=FanOut`. Cardinality uses
the `EDGE` tags. Lift is `0` outside SelectedApply and otherwise `1=Scalar`,
`2=Vector`, or `3=DynamicVector`; result-element scalar type is likewise zero
outside SelectedApply.

- Constant: `a0=constant`.
- ParameterBorrow: `a0=parameter`.
- TupleConstruct and PrefixSpreadPrepare: all variant words are zero.
- SelectedApply: `a0=primitive_id`, `a1=signature_id`,
  `a2=implementation_id`, `a3=primitive_origin`,
  `a4=static_anchor` (`NONE` when absent), `a5=dynamic_check_start`,
  `a6=dynamic_check_count`, and `a7=operation_reference_plus_one`. The last
  field is zero for ordinary applications; `foldl` and `scanl` require feature
  6 and store their zero-based `OPRF` index plus one. Stable semantic IDs fit `u16`,
  so the upper 16 bits of `a0` through `a2` MUST be zero. Lift adds
  `4=ContainerScalar` and `5=ContainerVector` only under feature 5.
- FanOut: `a0=branch_start`, `a1=branch_count`,
  `a2=keyword_origin`; `a3` through `a7` are zero.

`OWNR` is `owner:u32, release_kind:u8, reserved[3], release_index:u32`.
Release kind is `1=Node` or `2=Root`. `ROOT` is
`node:u32, origin:u32`.

`APPL` is present exactly when feature 5 is present and at least one
SelectedApply exists. Each record is `node:u32, application_plan_id:u16,
reserved:u16`; records cover every SelectedApply exactly once in ascending
node order. Application-plan IDs are nonzero, fit `u16`, and must match the
selected implementation's registry descriptor. A v1.0 artifact omits `APPL`
and reconstructs the plan from its validated implementation identity.

`OPRF` is `primitive_id:u16, signature_id:u16, implementation_id:u16,
reserved:u16, origin:u32, reserved:u32`. Both reserved fields are zero.
Verification requires the three IDs to identify one closed registered
elementwise descriptor and requires `origin` to be in range; inconsistent or
structural identities are malformed rather than runtime-dispatched by name.
Every `foldl` or `scanl` SelectedApply links exactly one `OPRF` record through
nonzero `NODE.a7`; the referenced descriptor must have the container element
type as both parameter types and result type. Other applications require zero, and missing,
out-of-range, structurally invalid, or type-incompatible links are malformed.

### 4.6 Producer metadata

`PROD` is advisory and excluded from program identity. Its payload is
`producer_name_length:u32`, producer-name UTF-8 bytes,
`producer_version_length:u32`, producer-version UTF-8 bytes,
`source_digest_algorithm:u16`, `source_digest_length:u16`, and digest bytes.
Algorithms are `0=None` with zero length and `1=SHA-256` with length 32.
When present in canonical v1 output, producer name is exactly the ASCII bytes
`faraweave`, producer version is the nonempty ASCII Cargo package version
without a leading `v`, and no bytes follow the declared digest. The field
asserts provenance but conveys no trust.

## 5. Complete in-memory-field coverage

| In-memory field | Wire source |
| --- | --- |
| `ModuleMetadata.semantic_major/minor` | `MODL` |
| `parameter_header_origin` | `MODL` |
| every `ProgramRanges` member | exact derived range for its decoded section |
| `features` | mandatory-class `FEAT` records |
| `source_units` | `SRCU` plus `STRS` |
| `parameters` | `PARM` plus `STRS` |
| `types`, `type_elements` | `TYPE`, `TYEL` |
| `constants`, `constant_elements` | `CONS`, `COEL` |
| `nodes`, except the application-plan sidecar | `NODE` |
| `SelectedApply.application_plan_id` | `APPL` with feature 5; reconstructed from the validated implementation identity in v1.0 |
| `SelectedApply.operation_reference` | `NODE.a7 - 1` when nonzero, validated against `OPRF`; zero means absent |
| `edges`, `shape_checks` | `EDGE`, `SHCK` |
| `origins` | `ORIG` |
| `operation_references` | `OPRF` |
| `branches` | `BRAN` |
| `ownership` | `OWNR` |
| `roots` | `ROOT` |

No Rust discriminant, `usize`, native address, host path, struct bytes, or
backend handle is serialized.

## 6. Canonical ordering and program identity

Arena record order is the already-semantic `VerifiedProgram` order; an encoder
MUST NOT sort, deduplicate, or renumber those records. Features are strictly
increasing by numeric ID, the string pool is sorted as specified above, and
directory order is numeric section-ID order. These rules make repeated
encoding byte-identical without relying on map iteration or native layout.

The **canonical program identity** is the complete canonical file containing
only identity-participating semantics. In v1 that is the header, recalculated
directory, and section payloads with flag bit 1 set, except that class-1
advisory `FEAT` records are filtered out and the resulting `FEAT` section is
omitted if empty. `PROD` and any accepted unknown optional advisory sections
are omitted and all following offsets are recalculated. This byte string—not a
host-language hash value—is normative: byte equality means identity equality.
A UI MAY display a named cryptographic digest of those bytes, but the
algorithm and spelling are not part of FWIR v1.

Producer version, source digest, source filesystem path, compilation time,
execution profile, resource limits, and target platform do not participate in
identity. Diagnostic source-unit names and all origins do
participate because they affect structured errors.

## 7. Version, feature, and extension compatibility

The physical format version and semantic contract version are separate.

- A decoder MUST reject a format major other than `1`.
- Format minor increments are additive. A v1.0 decoder MAY accept a greater
  format minor only after it has proved that every directory entry and feature
  is known or explicitly optional; otherwise it rejects before payload
  allocation.
- The decoded `MODL` semantic version is passed unchanged to semantic
  verification. The current semantic verifier accepts major `1` and a minor
  no greater than its supported minor.
- Format minor 1 is emitted when feature 5 or feature 6 is present; either
  sidecar feature at format or semantic minor 0 is noncanonical/unsupported.
- `OPRF` records or feature `6=OperationReferences` require semantic version
  1.1 and physical format minor 1. Semantic 1.0/physical 1.0 artifacts cannot
  opt into that mandatory sidecar capability.
- A `foldl` or `scanl` SelectedApply requires feature 6, a nonzero in-range
  `NODE.a7`, and one compatible stable reducer identity; other applications
  require `NODE.a7=0`.
- A lower format minor is accepted when the major matches and every required
  v1 section/record rule used by the artifact is supported.

Unknown directory entries with mandatory bit 0 set are rejected before
decoding any section payload. Unknown entries with mandatory bit clear and
identity bit clear may be bounds-checked and skipped. An unknown entry claiming
identity participation is rejected even when its mandatory bit is clear,
because a v1 decoder cannot reconstruct the program identity projection.

Unknown mandatory feature IDs are rejected before `RawProgram` construction.
Unknown class-1 advisory feature IDs may be skipped. Unknown feature classes,
node kinds, type kinds, scalar types, access modes, conversions, ownership
modes, release kinds, primitive IDs, signature IDs, implementation IDs, or
application-plan IDs are always rejected. When any SelectedApply exists,
feature 5 requires mandatory section 17 and one explicit nonzero plan per
SelectedApply; section 17 without feature 5 is rejected. A known feature ID
with class `1` is likewise rejected as
`NonCanonicalRecord`; optionality is granted only to an unknown feature ID
explicitly marked class `1`, not to a known semantic capability or an unknown
enum value inside a known semantic record.

The v1 public decoder is strict for every known v1.0 field: wrong order,
redundant empty known sections, unused strings, or a known advisory section
with noncanonical content produces a noncanonical-artifact error rather than
normalization. It accepts and skips a well-framed unknown optional section only
for a greater format minor, and accepts/skips an unknown class-1 advisory
feature; those forward-compatible bytes are not retained in
`VerifiedProgram`. Decode-encode is therefore byte-identical for supported
canonical v1.0 artifacts, while an accepted forward-minor artifact re-encodes
as canonical v1.0 with advisory extensions omitted. A semantic 1.1 artifact
using mandatory feature 5 re-encodes as canonical physical v1.1.

## 8. Checked decoding and hostile lengths

Decoding untrusted bytes MUST be iterative and use checked arithmetic. The
decoder accepts caller-supplied limits at least for `max_artifact_bytes`,
`max_sections`, `max_records_per_section`, `max_total_records`, and
`max_string_bytes`; defaults are product policy and are not encoded in program
identity.

Before reserving an arena or copying a string, the decoder performs this
deterministic physical-validation sequence:

1. reject input longer than `max_artifact_bytes` or shorter than the header;
2. validate magic, header size, reserved bytes, and physical major/minor;
3. checked-multiply section count by 24, apply `max_sections`, and prove the
   complete directory is in the input;
4. scan the whole directory without allocation, proving strict IDs, flags,
   known record sizes, mandatory support, contiguous checked extents, exact
   end-of-file, and no zero-record section other than `MODL`;
5. derive each fixed record count from its byte length, decode the string count
   only after proving four bytes exist, and enforce per-section and checked
   total-record limits;
6. validate string descriptors, checked byte-area bounds, contiguity, UTF-8,
   uniqueness/order, total string bytes, and reference-use completeness;
7. validate every reserved byte, feature ID/class pair (including rejecting
   class `1` on known IDs 1 through 7), tag, boolean, optional sentinel, unused
   variant word, and stable-ID width while decoding records in section and
   record order;
8. reserve each destination vector with `try_reserve_exact`, copy only after
   successful reservation, reconstruct `ProgramRanges`, then run the complete
   semantic verifier.

Every offset-plus-length, count-times-size, range end, index conversion, and
aggregate count uses checked arithmetic before access or allocation.
Allocation refusal returns a structured decode allocation error; it never
panics and never publishes a partial `RawProgram` or `VerifiedProgram`.
Physical format errors precede allocation and semantic errors according to the
sequence above; within a step, directory then section then record order wins.

There are no trailing bytes. `PROD` lengths and digest length are subject to
the same checked bounds and are fully validated even though the section is
advisory. A source digest is compared only when a caller separately supplies
the corresponding source bytes; mismatch is not a semantic-program failure.

The stable structured rejection categories are
`ArtifactTooLarge`, `Truncated`, `InvalidHeader`, `UnsupportedFormatVersion`,
`UnknownMandatoryExtension`, `NonCanonicalDirectory`, `InvalidSectionLength`,
`ResourceLimit`, `AllocationUnavailable`, `InvalidUtf8`, `NonCanonicalRecord`,
and `MalformedProgram(VerifyError)`. Each physical error carries a byte offset
and, once known, section ID and record index; semantic verifier errors retain
their existing record and field identity. The category separation and winner
order are part of the accepted v1 behavior.

## 9. Exact representative bytes

The checked-in examples are whitespace-separated hexadecimal; whitespace is
not part of the artifact.

- [Empty verified program](examples/fwir-v1-empty.hex) is exactly 64 bytes:
  one `MODL` section with semantic version 1.0 and no parameter-header origin.
- [Scalar `true` program](examples/fwir-v1-scalar-true.hex) is exactly 422
  bytes: source name `example.fw`, one source unit, Bool type, Bool constant
  with payload 1, one one-based origin spanning bytes 1 through 5 at line 1
  columns 1 through 5, one Constant node, one root release, and one root.
- [Semantic 1.0 encoder surface](examples/fwir-v1-complete.hex) is a
  4,413-byte golden produced from one verified source program covering all 16
  semantic-1.0 sections, all six node opcodes, every graph sidecar, sorted
  strings, and exact binary64 payload bits. Operation-reference section 18 is
  covered by constructed semantic-1.1 codec tests because no source consumer
  exists in issue #38.

Run:

```text
python tools/validation/fwir_v1_measurements.py examples --examples-dir spec/examples
```

The measurement model constructs these bytes while a separate decoder reads
the header, directory, and every field used by the examples, proves exact
extents, compares the decoded logical records with an independently stated
program, and rejects trailing data. The encoder goldens and independently
authored malformed corpus are executable in the Rust conformance suites.

## 10. Stable library and CLI boundaries

The following names and phase separation are accepted v1 product boundaries:

```text
encode_fwir(program: &VerifiedProgram, options: &FwirEncodeOptions)
    -> Result<Vec<u8>, FwirEncodeError>
decode_fwir(bytes: &[u8], limits: &FwirDecodeLimits)
    -> Result<VerifiedProgram, FwirDecodeError>
compile_source_to_fwir(source: &str, options: &FwirEncodeOptions)
    -> Result<Vec<u8>, CompileFwirError>
inspect_fwir(program: &VerifiedProgram)
    -> Result<String, FwirInspectError>
```

`encode_fwir` accepts no `RawProgram`. `decode_fwir` performs physical decoding
and semantic verification before returning. Inspection text is deterministic
and exact-value-safe but is not executable FWIR and never participates in
identity.

The explicit CLI spellings are:

```text
faraweave compile-ir <source> -o <artifact.fwir>
faraweave inspect-ir <artifact.fwir>
faraweave run-ir <artifact.fwir> [-- <arguments...>]
```

Commands never infer source versus FWIR from the extension. Every file
publication is atomic, rejects input/output aliases, and preserves an existing
destination on compile, decode, verify, format, or publication failure.
Argument count and decoding begin only after a complete artifact has become a
`VerifiedProgram`.

## 11. Alternatives and measurement decision

[Reproducible measurements](fwir-v1-encoding-measurements.md) compare the
selected format with canonical JSON and an explicit-schema Protocol Buffers
wire model. The sectioned format is selected for bounded nonrecursive framing,
exact fixed-width values, zero production/schema-runtime dependencies, and
straightforward checked safe-Rust implementation, not because it wins every
size case. Protocol Buffers is smaller in all measured fixtures but its official
[encoding rules](https://protobuf.dev/programming-guides/encoding/) do not
guarantee field serialization order, requiring another canonicalization layer;
canonical JSON is retained only as an inspection model because exact integer
and binary64 bits need additional string conventions.

No dependency is added by this decision or measurement harness. An encoder or
decoder proposal that adds one must make a new dependency decision in its own
issue rather than treating this comparison as authorization.

## 12. Accepted product, producer, and security policy

Physical formats 1.0 and 1.1, semantic contracts 1.0 and 1.1, canonical
program-identity bytes, the `.fwir` extension, and the library and CLI
spellings above are the stable FWIR v1 product contract. A compatible addition
uses the same-major
forward-minor and explicitly optional advisory mechanisms in section 7;
changing identity-participating meaning, a stable semantic ID, or a mandatory
record requires a new incompatible version.

Faraweave is the authoritative v1 producer. A consumer validates artifact
bytes without trusting who produced them, and `PROD` is advisory provenance,
not an authenticity assertion; third-party producer tooling and compatibility
guarantees are unsupported. Authenticity, source-digest comparison, signing,
distribution provenance, and release policy are caller or release-layer
responsibilities outside the FWIR semantic identity.

Unsupported v1 capabilities include optimizers, additional backends, user
functions, general control flow, parallel or nested fan-out, tuple-aware
primitive signatures, multidimensional arrays, and unknown mandatory
features. Consumers reject unsupported mandatory semantics rather than
guessing, defaulting, or redispatching by a source name.

The decoder treats bytes as hostile, applies checked bounds and caller limits,
and yields no interpreter input before complete verification. That is an
input-safety boundary, not a sandbox: interpreter execution runs with the
invoking process's authority.

FWIR is not confidential or encrypted. Canonical bytes and inspection output
can reveal diagnostic source names, literal constants, type and node graphs,
parameter names, provenance positions, and producer metadata; source digests
can also support offline guessing. Producers must not place secrets in an
artifact and must use a separate confidentiality mechanism when required.
