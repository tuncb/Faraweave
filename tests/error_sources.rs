use faraweave::{
    Arena, BuildError, CompileFwirError, Error, ErrorKind, FwirDecodeError, FwirDecodeErrorKind,
    FwirEncodeError, FwirEncodeOptions, FwirInspectError, Invariant, LocatedError,
    MalformedProgram, RecordKind, SourceLocation, VerifyError, compile_source_to_fwir,
};
use std::error::Error as StdError;

fn required_source<'a>(error: &'a (dyn StdError + 'static)) -> &'a (dyn StdError + 'static) {
    error.source().expect("wrapper source")
}

#[test]
fn public_error_wrappers_expose_their_error_chains() {
    let located = LocatedError {
        source_name: "input.faraweave".to_owned(),
        error: Error::new(
            ErrorKind::SyntaxError,
            SourceLocation::start(),
            "invalid source",
        ),
    };
    let located_source = required_source(&located);
    assert_eq!(
        located_source.downcast_ref::<Error>(),
        Some(&located.error),
        "located error should expose its contained evaluation error"
    );
    assert!(located_source.source().is_none());

    let compile =
        compile_source_to_fwir("inc[", &FwirEncodeOptions::default()).expect_err("invalid source");
    let compile_source = required_source(&compile);
    let CompileFwirError::Compile(expected_compile_source) = &compile else {
        panic!("invalid source should fail during compilation");
    };
    assert_eq!(
        compile_source.downcast_ref::<Error>(),
        Some(expected_compile_source),
        "compile wrapper should expose its contained source diagnostic"
    );
    assert!(compile_source.source().is_none());

    let compile_encode = CompileFwirError::Encode(FwirEncodeError::InvalidProducerVersion);
    let encode_source = required_source(&compile_encode);
    let CompileFwirError::Encode(expected_encode_source) = &compile_encode else {
        panic!("fixture should contain an encoder error");
    };
    assert_eq!(
        encode_source.downcast_ref::<FwirEncodeError>(),
        Some(expected_encode_source),
        "compile wrapper should expose its contained encoder error"
    );
    assert!(encode_source.source().is_none());

    let inspect = FwirInspectError::Encode(FwirEncodeError::InvalidProducerVersion);
    let inspect_source = required_source(&inspect);
    let FwirInspectError::Encode(expected_inspect_source) = &inspect else {
        panic!("fixture should contain an encoder error");
    };
    assert_eq!(
        inspect_source.downcast_ref::<FwirEncodeError>(),
        Some(expected_inspect_source),
        "inspection wrapper should expose its contained encoder error"
    );
    assert!(inspect_source.source().is_none());

    let decode = FwirDecodeError {
        kind: FwirDecodeErrorKind::MalformedProgram(VerifyError::MalformedProgram(
            MalformedProgram {
                invariant: Invariant::InvalidRecord,
                record: RecordKind::Node,
                index: Some(0),
                field: "kind",
            },
        )),
        offset: 32,
        section_id: Some(12),
        record_index: Some(0),
    };
    let verify_source = required_source(&decode);
    let FwirDecodeErrorKind::MalformedProgram(expected_verify_source) = &decode.kind else {
        panic!("fixture should contain a verifier error");
    };
    assert_eq!(
        verify_source.downcast_ref::<VerifyError>(),
        Some(expected_verify_source),
        "decode wrapper should expose its contained verifier error"
    );
    assert!(verify_source.source().is_none());
}

#[test]
fn public_leaf_errors_and_non_wrapping_variants_have_no_source() {
    let evaluation = Error::new(
        ErrorKind::DomainError,
        SourceLocation::start(),
        "division by zero",
    );
    assert!(evaluation.source().is_none());
    assert!(FwirEncodeError::InvalidProducerVersion.source().is_none());
    assert!(
        VerifyError::AllocationUnavailable {
            site: faraweave::VerifyAllocationSite::ReachabilityBits,
        }
        .source()
        .is_none()
    );
    assert!(
        BuildError::AllocationUnavailable { arena: Arena::Node }
            .source()
            .is_none()
    );
    assert!(FwirInspectError::SizeOverflow.source().is_none());
    assert!(FwirInspectError::AllocationUnavailable.source().is_none());

    let decode = FwirDecodeError {
        kind: FwirDecodeErrorKind::InvalidUtf8,
        offset: 0,
        section_id: None,
        record_index: None,
    };
    assert!(decode.source().is_none());
}
