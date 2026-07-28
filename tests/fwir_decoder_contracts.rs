use faraweave::{
    FwirDecodeError, FwirDecodeErrorKind, FwirDecodeLimits, FwirEncodeOptions, decode_fwir,
    encode_fwir,
};

fn example_bytes(name: &str) -> Vec<u8> {
    let text = match name {
        "empty" => include_str!("../spec/examples/fwir-v1-empty.hex"),
        "complete" => include_str!("../spec/examples/fwir-v1-complete.hex"),
        _ => panic!("unknown example"),
    };
    match text
        .split_ascii_whitespace()
        .map(|byte| u8::from_str_radix(byte, 16))
        .collect()
    {
        Ok(bytes) => bytes,
        Err(error) => panic!("invalid checked-in example: {error}"),
    }
}

#[test]
fn public_decoder_returns_only_verified_programs_and_preserves_canonical_bytes() {
    for name in ["empty", "complete"] {
        let bytes = example_bytes(name);
        let verified = match decode_fwir(&bytes, &FwirDecodeLimits::default()) {
            Ok(value) => value,
            Err(error) => panic!("{name} failed to decode: {error:?}"),
        };
        assert_eq!(
            encode_fwir(&verified, &FwirEncodeOptions::default()),
            Ok(bytes)
        );
    }
}

#[test]
fn public_decoder_reports_artifact_failures_without_source_errors_or_partial_results() {
    let bytes = example_bytes("complete");
    for length in 0..bytes.len() {
        assert!(matches!(
            decode_fwir(&bytes[..length], &FwirDecodeLimits::default()),
            Err(FwirDecodeError { .. })
        ));
    }

    let mut invalid_graph = bytes;
    let section_count = u32::from_le_bytes([
        invalid_graph[20],
        invalid_graph[21],
        invalid_graph[22],
        invalid_graph[23],
    ]) as usize;
    let mut root_offset = None;
    for index in 0..section_count {
        let entry = 32 + index * 24;
        let id = u16::from_le_bytes([invalid_graph[entry], invalid_graph[entry + 1]]);
        if id == 16 {
            let encoded = u64::from_le_bytes([
                invalid_graph[entry + 8],
                invalid_graph[entry + 9],
                invalid_graph[entry + 10],
                invalid_graph[entry + 11],
                invalid_graph[entry + 12],
                invalid_graph[entry + 13],
                invalid_graph[entry + 14],
                invalid_graph[entry + 15],
            ]);
            root_offset = usize::try_from(encoded).ok();
            break;
        }
    }
    let root_offset = match root_offset {
        Some(value) => value,
        None => panic!("complete fixture has no ROOT section"),
    };
    invalid_graph[root_offset..root_offset + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        decode_fwir(&invalid_graph, &FwirDecodeLimits::default()),
        Err(FwirDecodeError {
            kind: FwirDecodeErrorKind::MalformedProgram(_),
            ..
        })
    ));
}
