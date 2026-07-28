const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

#[derive(Clone, Copy, Debug)]
enum Operation {
    Sqrt,
    Exp,
    Log,
    Log10,
    Sin,
    Cos,
    Tan,
    Floor,
    Ceil,
    Trunc,
}

#[derive(Clone, Copy)]
struct FiniteCase {
    operation: Operation,
    input: u64,
    reference: u64,
    max_ulps: u64,
    max_absolute: f64,
}

fn invoke(operation: Operation, input: f64) -> f64 {
    match operation {
        Operation::Sqrt => input.sqrt(),
        Operation::Exp => input.exp(),
        Operation::Log => input.ln(),
        Operation::Log10 => input.log10(),
        Operation::Sin => input.sin(),
        Operation::Cos => input.cos(),
        Operation::Tan => input.tan(),
        Operation::Floor => input.floor(),
        Operation::Ceil => input.ceil(),
        Operation::Trunc => input.trunc(),
    }
}

fn normalized_bits(value: f64) -> u64 {
    if value.is_nan() {
        CANONICAL_NAN_BITS
    } else {
        value.to_bits()
    }
}

fn order_key(bits: u64) -> u64 {
    if bits & (1_u64 << 63) == 0 {
        bits | (1_u64 << 63)
    } else {
        !bits
    }
}

fn conforms(actual: f64, case: FiniteCase) -> bool {
    let reference = f64::from_bits(case.reference);
    if !actual.is_finite() || actual.is_sign_negative() != reference.is_sign_negative() {
        return false;
    }
    if reference == 0.0 {
        return actual.to_bits() == case.reference;
    }
    let ulps = order_key(actual.to_bits()).abs_diff(order_key(case.reference));
    ulps <= case.max_ulps || (actual - reference).abs() <= case.max_absolute
}

#[test]
fn backend_native_math_envelope_handles_underflow_zero_asymmetry() {
    let positive_minimum_reference = FiniteCase {
        operation: Operation::Exp,
        input: 0,
        reference: 1,
        max_ulps: 1,
        max_absolute: 0.0,
    };
    assert!(conforms(0.0, positive_minimum_reference));
    assert!(!conforms(-0.0, positive_minimum_reference));
    assert!(!conforms(f64::NAN, positive_minimum_reference));
    assert!(!conforms(f64::INFINITY, positive_minimum_reference));

    for (reference, matching_zero, wrong_zero, same_sign_nonzero) in [
        (0, 0.0, -0.0, f64::from_bits(1)),
        (1_u64 << 63, -0.0, 0.0, f64::from_bits((1_u64 << 63) | 1)),
    ] {
        let signed_zero_reference = FiniteCase {
            operation: Operation::Exp,
            input: 0,
            reference,
            max_ulps: u64::MAX,
            max_absolute: f64::MAX,
        };
        assert!(conforms(matching_zero, signed_zero_reference));
        assert!(!conforms(wrong_zero, signed_zero_reference));
        assert!(!conforms(same_sign_nonzero, signed_zero_reference));
    }
}

#[test]
fn backend_native_math_rust_reference_vectors_meet_policy() {
    let cases = [
        FiniteCase {
            operation: Operation::Sqrt,
            input: 0x4000_0000_0000_0000,
            reference: 0x3ff6_a09e_667f_3bcd,
            max_ulps: 1,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Sqrt,
            input: 0x0000_0000_0000_0001,
            reference: 0x1e60_0000_0000_0000,
            max_ulps: 1,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Sqrt,
            input: 0x7fef_ffff_ffff_ffff,
            reference: 0x5fef_ffff_ffff_ffff,
            max_ulps: 1,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Exp,
            input: 0x3ff0_0000_0000_0000,
            reference: 0x4005_bf0a_8b14_5769,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Exp,
            input: 0xc087_4000_0000_0000,
            reference: 0x0000_0000_0000_0002,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Exp,
            input: 0x4086_2e42_fefa_39ef,
            reference: 0x7fef_ffff_ffff_ff2a,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Log,
            input: 0x0000_0000_0000_0001,
            reference: 0xc087_4385_446d_71c3,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Log,
            input: 0x3ff0_0000_0000_0001,
            reference: 0x3caf_ffff_ffff_ffff,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Log10,
            input: 0x0000_0000_0000_0001,
            reference: 0xc074_34e6_420f_4374,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Log10,
            input: 0x3ff0_0000_0000_0001,
            reference: 0x3c9b_cb7b_1526_e50d,
            max_ulps: 4,
            max_absolute: 0.0,
        },
        FiniteCase {
            operation: Operation::Sin,
            input: 0x3ff0_0000_0000_0000,
            reference: 0x3fea_ed54_8f09_0cee,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Sin,
            input: 0x0000_0000_0000_0001,
            reference: 0x0000_0000_0000_0001,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Sin,
            input: 0x7e37_e43c_8800_759c,
            reference: 0xbfea_2c16_b010_e385,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Cos,
            input: 0x4009_21fb_5444_2d18,
            reference: 0xbff0_0000_0000_0000,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Cos,
            input: 0x0000_0000_0000_0001,
            reference: 0x3ff0_0000_0000_0000,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Cos,
            input: 0x7e37_e43c_8800_759c,
            reference: 0xbfe2_6990_22ad_c4c1,
            max_ulps: 8,
            max_absolute: 2_f64.powi(-48),
        },
        FiniteCase {
            operation: Operation::Tan,
            input: 0x3ff9_21fb_5444_2d17,
            reference: 0x4329_153d_9443_ed0b,
            max_ulps: 16,
            max_absolute: 2_f64.powi(-46),
        },
        FiniteCase {
            operation: Operation::Tan,
            input: 0x0000_0000_0000_0001,
            reference: 0x0000_0000_0000_0001,
            max_ulps: 16,
            max_absolute: 2_f64.powi(-46),
        },
        FiniteCase {
            operation: Operation::Tan,
            input: 0x3ff9_21fb_5444_2d19,
            reference: 0xc336_17a1_5494_767a,
            max_ulps: 16,
            max_absolute: 2_f64.powi(-46),
        },
        FiniteCase {
            operation: Operation::Tan,
            input: 0x7e37_e43c_8800_759c,
            reference: 0x3ff6_be41_1f37_ac77,
            max_ulps: 16,
            max_absolute: 2_f64.powi(-46),
        },
    ];

    for case in cases {
        let actual = invoke(case.operation, f64::from_bits(case.input));
        assert!(
            conforms(actual, case),
            "{:?}({:#018x}) produced {:#018x}, reference {:#018x}",
            case.operation,
            case.input,
            actual.to_bits(),
            case.reference
        );
    }
}

#[test]
fn backend_native_math_special_values_and_rounding_are_exact() {
    let positive_zero = f64::from_bits(0);
    let negative_zero = f64::from_bits(1_u64 << 63);
    let nan = f64::from_bits(CANONICAL_NAN_BITS);

    for (operation, input, expected) in [
        (Operation::Sqrt, positive_zero, 0),
        (Operation::Sqrt, negative_zero, 1_u64 << 63),
        (Operation::Sqrt, f64::INFINITY, f64::INFINITY.to_bits()),
        (Operation::Sqrt, -1.0, CANONICAL_NAN_BITS),
        (Operation::Exp, positive_zero, 1.0_f64.to_bits()),
        (Operation::Exp, negative_zero, 1.0_f64.to_bits()),
        (Operation::Exp, f64::NEG_INFINITY, 0),
        (Operation::Exp, f64::INFINITY, f64::INFINITY.to_bits()),
        (Operation::Log, positive_zero, f64::NEG_INFINITY.to_bits()),
        (Operation::Log, negative_zero, f64::NEG_INFINITY.to_bits()),
        (Operation::Log, 1.0, 0),
        (Operation::Log, -1.0, CANONICAL_NAN_BITS),
        (Operation::Log10, positive_zero, f64::NEG_INFINITY.to_bits()),
        (Operation::Log10, 1.0, 0),
        (Operation::Sin, negative_zero, 1_u64 << 63),
        (Operation::Sin, f64::INFINITY, CANONICAL_NAN_BITS),
        (Operation::Cos, negative_zero, 1.0_f64.to_bits()),
        (Operation::Cos, f64::NEG_INFINITY, CANONICAL_NAN_BITS),
        (Operation::Tan, negative_zero, 1_u64 << 63),
        (Operation::Tan, f64::INFINITY, CANONICAL_NAN_BITS),
    ] {
        assert_eq!(normalized_bits(invoke(operation, input)), expected);
    }

    for operation in [
        Operation::Sqrt,
        Operation::Exp,
        Operation::Log,
        Operation::Log10,
        Operation::Sin,
        Operation::Cos,
        Operation::Tan,
        Operation::Floor,
        Operation::Ceil,
        Operation::Trunc,
    ] {
        assert_eq!(normalized_bits(invoke(operation, nan)), CANONICAL_NAN_BITS);
    }

    let minimum = f64::from_bits(1);
    let below_two_to_52 = f64::from_bits(0x432f_ffff_ffff_ffff);
    for (operation, input, expected) in [
        (Operation::Floor, minimum, 0),
        (Operation::Floor, -minimum, (-1.0_f64).to_bits()),
        (Operation::Ceil, minimum, 1.0_f64.to_bits()),
        (Operation::Ceil, -minimum, 1_u64 << 63),
        (Operation::Trunc, minimum, 0),
        (Operation::Trunc, -minimum, 1_u64 << 63),
        (Operation::Floor, 1.5, 1.0_f64.to_bits()),
        (Operation::Floor, -1.5, (-2.0_f64).to_bits()),
        (Operation::Ceil, 1.5, 2.0_f64.to_bits()),
        (Operation::Ceil, -1.5, (-1.0_f64).to_bits()),
        (Operation::Trunc, 1.5, 1.0_f64.to_bits()),
        (Operation::Trunc, -1.5, (-1.0_f64).to_bits()),
        (Operation::Floor, below_two_to_52, 0x432f_ffff_ffff_fffe),
        (Operation::Ceil, below_two_to_52, 0x4330_0000_0000_0000),
        (Operation::Trunc, below_two_to_52, 0x432f_ffff_ffff_fffe),
    ] {
        assert_eq!(normalized_bits(invoke(operation, input)), expected);
    }

    assert!(invoke(Operation::Exp, 710.0).is_infinite());
    assert_eq!(invoke(Operation::Exp, -746.0).to_bits(), 0);
}
