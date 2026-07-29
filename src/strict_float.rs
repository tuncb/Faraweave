//! Architecture boundary for Faraweave binary64 semantics.
//!
//! The language requires arithmetic to ignore the caller's rounding, trap,
//! denormal, and flush controls, then restore every supported control/status
//! bit.  The small unsafe sections below only read and write the documented
//! x86-64 or AArch64 floating-point registers.  All arithmetic and bit
//! conversion APIs exposed to the rest of the crate are safe.
#![allow(unsafe_code)]

const EXPONENT_MASK: u64 = 0x7ff0_0000_0000_0000;
const FRACTION_MASK: u64 = 0x000f_ffff_ffff_ffff;
const SIGN_MASK: u64 = 0x8000_0000_0000_0000;
const CANONICAL_NAN_BITS: u64 = 0x7ff8_0000_0000_0000;

#[derive(Clone, Copy)]
pub(crate) enum Binary64Operation {
    Add,
    Subtract,
    Multiply,
}

pub(crate) fn int_to_binary64(value: i64) -> f64 {
    if value == 0 {
        return 0.0;
    }

    let negative = value < 0;
    let magnitude = if negative {
        (-(value + 1)) as u64 + 1
    } else {
        value as u64
    };
    let mut most_significant = 63 - magnitude.leading_zeros();
    let mut significand;
    if most_significant <= 52 {
        significand = magnitude << (52 - most_significant);
    } else {
        let shift = most_significant - 52;
        significand = magnitude >> shift;
        let remainder_mask = (1_u64 << shift) - 1;
        let remainder = magnitude & remainder_mask;
        let halfway = 1_u64 << (shift - 1);
        if remainder > halfway || (remainder == halfway && significand & 1 != 0) {
            significand += 1;
            if significand == 1_u64 << 53 {
                significand >>= 1;
                most_significant += 1;
            }
        }
    }

    let sign = if negative { SIGN_MASK } else { 0 };
    let exponent = u64::from(most_significant + 1023) << 52;
    f64::from_bits(sign | exponent | (significand & FRACTION_MASK))
}

pub(crate) fn arithmetic(left: f64, right: f64, operation: Binary64Operation) -> f64 {
    let left_bits = left.to_bits();
    let right_bits = right.to_bits();
    let signs_differ = (left_bits ^ right_bits) & SIGN_MASK != 0;
    let invalid_infinity_arithmetic = is_infinity_bits(left_bits)
        && is_infinity_bits(right_bits)
        && match operation {
            Binary64Operation::Add => signs_differ,
            Binary64Operation::Subtract => !signs_differ,
            Binary64Operation::Multiply => false,
        };
    let invalid_infinity_product = matches!(operation, Binary64Operation::Multiply)
        && ((is_infinity_bits(left_bits) && is_zero_bits(right_bits))
            || (is_zero_bits(left_bits) && is_infinity_bits(right_bits)));
    if is_nan_bits(left_bits)
        || is_nan_bits(right_bits)
        || invalid_infinity_arithmetic
        || invalid_infinity_product
    {
        return f64::from_bits(CANONICAL_NAN_BITS);
    }

    // SAFETY: `StrictEnvironment` saves the complete supported host state,
    // installs masked round-to-nearest with gradual underflow, and restores the
    // saved bytes in `Drop`. Volatile operands/result keep the single operation
    // inside that guard and prevent excess evaluation or constant folding.
    let result = unsafe {
        let environment = StrictEnvironment::begin();
        let strict_left = core::ptr::read_volatile(&left);
        let strict_right = core::ptr::read_volatile(&right);
        let mut strict_result = match operation {
            Binary64Operation::Add => strict_left + strict_right,
            Binary64Operation::Subtract => strict_left - strict_right,
            Binary64Operation::Multiply => strict_left * strict_right,
        };
        let result = core::ptr::read_volatile(&strict_result);
        core::ptr::write_volatile(&mut strict_result, 0.0);
        drop(environment);
        result
    };
    canonicalize(result)
}

pub(crate) fn backend_native_sqrt(value: f64) -> f64 {
    // SAFETY: `StrictEnvironment` saves the complete supported host state,
    // installs masked round-to-nearest with gradual underflow, and restores the
    // saved bytes in `Drop`. The volatile input/result keep the direct
    // `f64::sqrt` call inside that guard.
    let result = unsafe {
        let environment = StrictEnvironment::begin();
        let strict_value = core::ptr::read_volatile(&value);
        let mut strict_result = f64::sqrt(strict_value);
        let result = core::ptr::read_volatile(&strict_result);
        core::ptr::write_volatile(&mut strict_result, 0.0);
        drop(environment);
        result
    };
    canonicalize(result)
}

pub(crate) fn backend_native_exp(value: f64) -> f64 {
    // SAFETY: `StrictEnvironment` saves the complete supported host state,
    // installs masked round-to-nearest with gradual underflow, and restores the
    // saved bytes in `Drop`. The volatile input/result keep the direct
    // `f64::exp` call inside that guard.
    let result = unsafe {
        let environment = StrictEnvironment::begin();
        let strict_value = core::ptr::read_volatile(&value);
        let mut strict_result = f64::exp(strict_value);
        let result = core::ptr::read_volatile(&strict_result);
        core::ptr::write_volatile(&mut strict_result, 0.0);
        drop(environment);
        result
    };
    canonicalize(result)
}

pub(crate) fn backend_native_log(value: f64) -> f64 {
    // SAFETY: `StrictEnvironment` saves the complete supported host state,
    // installs masked round-to-nearest with gradual underflow, and restores the
    // saved bytes in `Drop`. The volatile input/result keep the direct
    // `f64::ln` call inside that guard.
    let result = unsafe {
        let environment = StrictEnvironment::begin();
        let strict_value = core::ptr::read_volatile(&value);
        let mut strict_result = f64::ln(strict_value);
        let result = core::ptr::read_volatile(&strict_result);
        core::ptr::write_volatile(&mut strict_result, 0.0);
        drop(environment);
        result
    };
    canonicalize(result)
}

pub(crate) fn negate(value: f64) -> f64 {
    canonicalize(f64::from_bits(value.to_bits() ^ SIGN_MASK))
}

pub(crate) fn absolute(value: f64) -> f64 {
    canonicalize(f64::from_bits(value.to_bits() & !SIGN_MASK))
}

pub(crate) fn equal(left: f64, right: f64) -> bool {
    let left = left.to_bits();
    let right = right.to_bits();
    !is_nan_bits(left)
        && !is_nan_bits(right)
        && ((is_zero_bits(left) && is_zero_bits(right)) || left == right)
}

pub(crate) fn less_than(left: f64, right: f64) -> bool {
    let left = left.to_bits();
    let right = right.to_bits();
    !is_nan_bits(left)
        && !is_nan_bits(right)
        && !(is_zero_bits(left) && is_zero_bits(right))
        && order_key(left) < order_key(right)
}

pub(crate) fn is_positive(value: f64) -> bool {
    let bits = value.to_bits();
    !is_nan_bits(bits) && !is_zero_bits(bits) && bits & SIGN_MASK == 0
}

pub(crate) fn is_negative(value: f64) -> bool {
    let bits = value.to_bits();
    !is_nan_bits(bits) && !is_zero_bits(bits) && bits & SIGN_MASK != 0
}

fn canonicalize(value: f64) -> f64 {
    if is_nan_bits(value.to_bits()) {
        f64::from_bits(CANONICAL_NAN_BITS)
    } else {
        value
    }
}

fn is_nan_bits(bits: u64) -> bool {
    bits & EXPONENT_MASK == EXPONENT_MASK && bits & FRACTION_MASK != 0
}

fn is_infinity_bits(bits: u64) -> bool {
    bits & !SIGN_MASK == EXPONENT_MASK
}

fn is_zero_bits(bits: u64) -> bool {
    bits & !SIGN_MASK == 0
}

fn order_key(bits: u64) -> u64 {
    if bits & SIGN_MASK != 0 {
        !bits
    } else {
        bits | SIGN_MASK
    }
}

#[cfg(target_arch = "x86_64")]
#[repr(C)]
struct StrictEnvironment {
    mxcsr: u32,
    x87: [u8; 28],
}

#[cfg(target_arch = "x86_64")]
impl StrictEnvironment {
    unsafe fn begin() -> Self {
        use core::arch::asm;

        let mut environment = Self {
            mxcsr: 0,
            x87: [0; 28],
        };
        // SAFETY: both destinations are valid, suitably sized writable
        // objects for the architectural state images.
        unsafe {
            asm!(
                "stmxcsr [{mxcsr}]",
                mxcsr = in(reg) &mut environment.mxcsr,
                options(nostack, preserves_flags)
            );
            asm!(
                "fnstenv [{x87}]",
                x87 = in(reg) environment.x87.as_mut_ptr(),
                options(nostack, preserves_flags)
            );
        }

        let saved_control = u16::from_ne_bytes([environment.x87[0], environment.x87[1]]);
        let strict_x87_control = (saved_control | 0x003f) & !0x0c00;
        let strict_mxcsr = (environment.mxcsr | 0x1f80) & !(0x003f | 0x0040 | 0x6000 | 0x8000);
        // SAFETY: the values mask all exceptions, select nearest-even, and
        // clear DAZ/FTZ and pending SIMD exception flags.
        unsafe {
            asm!(
                "fldcw [{control}]",
                control = in(reg) &strict_x87_control,
                options(nostack, preserves_flags)
            );
            asm!(
                "ldmxcsr [{mxcsr}]",
                mxcsr = in(reg) &strict_mxcsr,
                options(nostack, preserves_flags)
            );
        }
        environment
    }
}

#[cfg(target_arch = "x86_64")]
impl Drop for StrictEnvironment {
    fn drop(&mut self) {
        use core::arch::asm;

        // SAFETY: these are the exact state images captured by `begin`, and
        // the guard cannot outlive its own backing storage.
        unsafe {
            asm!(
                "fldenv [{x87}]",
                x87 = in(reg) self.x87.as_ptr(),
                options(nostack, preserves_flags)
            );
            asm!(
                "ldmxcsr [{mxcsr}]",
                mxcsr = in(reg) &self.mxcsr,
                options(nostack, preserves_flags)
            );
        }
    }
}

#[cfg(target_arch = "aarch64")]
struct StrictEnvironment {
    control: u64,
    status: u64,
}

#[cfg(target_arch = "aarch64")]
impl StrictEnvironment {
    unsafe fn begin() -> Self {
        use core::arch::asm;

        let mut environment = Self {
            control: 0,
            status: 0,
        };
        // SAFETY: MRS only copies the current thread's architectural state
        // into ordinary integer registers.
        unsafe {
            asm!("mrs {value}, fpcr", value = out(reg) environment.control);
            asm!("mrs {value}, fpsr", value = out(reg) environment.status);
        }
        let strict_control = environment.control & !(0x0000_9f00 | 0x00c0_0000 | 0x0300_0000);
        let clear_status = 0_u64;
        // SAFETY: the masked value disables exceptions, nearest-even is zero,
        // and FZ/DN are cleared. ISB makes the control change effective before
        // the guarded operation.
        unsafe {
            asm!(
                "msr fpcr, {value}",
                "isb",
                value = in(reg) strict_control,
                options(nostack)
            );
            asm!("msr fpsr, {value}", value = in(reg) clear_status, options(nostack));
        }
        environment
    }
}

#[cfg(target_arch = "aarch64")]
impl Drop for StrictEnvironment {
    fn drop(&mut self) {
        use core::arch::asm;

        // SAFETY: restore the exact control/status pair captured by `begin`.
        unsafe {
            asm!(
                "msr fpcr, {value}",
                "isb",
                value = in(reg) self.control,
                options(nostack)
            );
            asm!(
                "msr fpsr, {value}",
                value = in(reg) self.status,
                options(nostack)
            );
        }
    }
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
compile_error!("Faraweave requires an x86-64 or AArch64 floating-point environment");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_conversion_matches_every_normative_boundary() {
        let cases = [
            (i64::MIN, 0xc3e0_0000_0000_0000),
            (-9_007_199_254_740_995, 0xc340_0000_0000_0002),
            (-9_007_199_254_740_994, 0xc340_0000_0000_0001),
            (-9_007_199_254_740_993, 0xc340_0000_0000_0000),
            (-9_007_199_254_740_992, 0xc340_0000_0000_0000),
            (-9_007_199_254_740_991, 0xc33f_ffff_ffff_ffff),
            (-1, 0xbff0_0000_0000_0000),
            (0, 0x0000_0000_0000_0000),
            (1, 0x3ff0_0000_0000_0000),
            (9_007_199_254_740_991, 0x433f_ffff_ffff_ffff),
            (9_007_199_254_740_992, 0x4340_0000_0000_0000),
            (9_007_199_254_740_993, 0x4340_0000_0000_0000),
            (9_007_199_254_740_994, 0x4340_0000_0000_0001),
            (9_007_199_254_740_995, 0x4340_0000_0000_0002),
            (i64::MAX, 0x43e0_0000_0000_0000),
        ];
        for (value, expected) in cases {
            assert_eq!(int_to_binary64(value).to_bits(), expected);
        }
    }

    #[test]
    fn arithmetic_preserves_required_interchange_bits() {
        let cases = [
            (
                Binary64Operation::Add,
                0x3ff0_0000_0000_0000,
                0x3ca0_0000_0000_0000,
                0x3ff0_0000_0000_0000,
            ),
            (
                Binary64Operation::Add,
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0001,
                0x0000_0000_0000_0002,
            ),
            (
                Binary64Operation::Subtract,
                0x0010_0000_0000_0000,
                0x000f_ffff_ffff_ffff,
                0x0000_0000_0000_0001,
            ),
            (
                Binary64Operation::Multiply,
                0x3ff0_0000_0000_0001,
                0x3ff0_0000_0000_0001,
                0x3ff0_0000_0000_0002,
            ),
            (
                Binary64Operation::Multiply,
                0x7ff0_0000_0000_0000,
                0,
                CANONICAL_NAN_BITS,
            ),
        ];
        for (operation, left, right, expected) in cases {
            assert_eq!(
                arithmetic(f64::from_bits(left), f64::from_bits(right), operation).to_bits(),
                expected
            );
        }
    }

    #[test]
    fn backend_native_sqrt_preserves_exact_special_values_and_canonical_nan() {
        for (input, expected) in [
            (0x0000_0000_0000_0000, 0x0000_0000_0000_0000),
            (0x8000_0000_0000_0000, 0x8000_0000_0000_0000),
            (0x7ff0_0000_0000_0000, 0x7ff0_0000_0000_0000),
            (0xbff0_0000_0000_0000, CANONICAL_NAN_BITS),
            (0xfff0_0000_0000_0000, CANONICAL_NAN_BITS),
            (0x7ff8_0000_0000_0000, CANONICAL_NAN_BITS),
        ] {
            assert_eq!(
                backend_native_sqrt(f64::from_bits(input)).to_bits(),
                expected
            );
        }
    }

    #[test]
    fn backend_native_exp_preserves_exact_special_values_and_canonical_nan() {
        for (input, expected) in [
            (0x0000_0000_0000_0000, 0x3ff0_0000_0000_0000),
            (0x8000_0000_0000_0000, 0x3ff0_0000_0000_0000),
            (0xfff0_0000_0000_0000, 0x0000_0000_0000_0000),
            (0x7ff0_0000_0000_0000, 0x7ff0_0000_0000_0000),
            (0x7ff8_0000_0000_0000, CANONICAL_NAN_BITS),
        ] {
            assert_eq!(
                backend_native_exp(f64::from_bits(input)).to_bits(),
                expected
            );
        }
    }

    #[test]
    fn backend_native_log_preserves_exact_special_values_and_canonical_nan() {
        for (input, expected) in [
            (0x0000_0000_0000_0000, 0xfff0_0000_0000_0000),
            (0x8000_0000_0000_0000, 0xfff0_0000_0000_0000),
            (0x3ff0_0000_0000_0000, 0x0000_0000_0000_0000),
            (0xbff0_0000_0000_0000, CANONICAL_NAN_BITS),
            (0xfff0_0000_0000_0000, CANONICAL_NAN_BITS),
            (0x7ff0_0000_0000_0000, 0x7ff0_0000_0000_0000),
            (0x7ff8_0000_0000_0000, CANONICAL_NAN_BITS),
        ] {
            assert_eq!(
                backend_native_log(f64::from_bits(input)).to_bits(),
                expected
            );
        }
    }

    #[cfg(target_arch = "x86_64")]
    #[test]
    fn hostile_x86_environment_is_ignored_and_exactly_restored() {
        use core::arch::asm;

        unsafe fn read_mxcsr() -> u32 {
            let mut value = 0_u32;
            // SAFETY: `value` is a valid four-byte destination.
            unsafe {
                asm!(
                    "stmxcsr [{value}]",
                    value = in(reg) &mut value,
                    options(nostack, preserves_flags)
                );
            }
            value
        }
        unsafe fn write_mxcsr(value: u32) {
            // SAFETY: the caller supplies an MXCSR derived from a previously
            // captured valid value.
            unsafe {
                asm!(
                    "ldmxcsr [{value}]",
                    value = in(reg) &value,
                    options(nostack, preserves_flags)
                );
            }
        }

        // SAFETY: all installed bits are derived from a valid captured MXCSR;
        // the original is restored before any assertion or test return.
        unsafe {
            let reference_exp = backend_native_exp(-744.0).to_bits();
            let reference_log = backend_native_log(f64::from_bits(1)).to_bits();
            let original = read_mxcsr();
            let hostile = (original | 0x0040 | 0x4000 | 0x8000 | 0x1f80) & !0x003f;
            write_mxcsr(hostile);
            let sqrt_result = backend_native_sqrt(f64::from_bits(1)).to_bits();
            let after_sqrt = read_mxcsr();
            let exp_result = backend_native_exp(-744.0).to_bits();
            let after_exp = read_mxcsr();
            let log_result = backend_native_log(f64::from_bits(1)).to_bits();
            let after_log = read_mxcsr();
            let result = arithmetic(
                f64::from_bits(0x0000_0000_0000_0001),
                2.0,
                Binary64Operation::Multiply,
            )
            .to_bits();
            let restored = read_mxcsr();
            write_mxcsr(original);
            assert_eq!(sqrt_result, 0x1e60_0000_0000_0000);
            assert_eq!(after_sqrt, hostile);
            assert_eq!(exp_result, reference_exp);
            assert_eq!(after_exp, hostile);
            assert_eq!(log_result, reference_log);
            assert_eq!(after_log, hostile);
            assert_eq!(result, 0x0000_0000_0000_0002);
            assert_eq!(restored, hostile);
        }
    }

    #[cfg(target_arch = "aarch64")]
    #[test]
    fn hostile_aarch64_environment_is_ignored_and_exactly_restored() {
        use core::arch::asm;

        unsafe fn read_environment() -> (u64, u64) {
            let control;
            let status;
            // SAFETY: MRS copies the current thread's FPCR/FPSR into ordinary
            // integer registers without dereferencing memory.
            unsafe {
                asm!("mrs {value}, fpcr", value = out(reg) control);
                asm!("mrs {value}, fpsr", value = out(reg) status);
            }
            (control, status)
        }

        unsafe fn write_environment(control: u64, status: u64) {
            // SAFETY: the values only modify documented FPCR/FPSR fields and
            // the caller restores the exact captured pair before returning.
            unsafe {
                asm!(
                    "msr fpcr, {value}",
                    "isb",
                    value = in(reg) control,
                    options(nostack)
                );
                asm!("msr fpsr, {value}", value = in(reg) status, options(nostack));
            }
        }

        // SAFETY: the hostile values are derived from a valid captured state
        // and set only documented exception-enable, rounding, FZ/DN, and
        // cumulative-status fields. The original pair is restored before any
        // assertion or test return.
        unsafe {
            let reference_sqrt = backend_native_sqrt(f64::from_bits(1)).to_bits();
            let reference_exp = backend_native_exp(-744.0).to_bits();
            let reference_log = backend_native_log(f64::from_bits(1)).to_bits();
            let reference_arithmetic = arithmetic(
                f64::from_bits(0x0000_0000_0000_0001),
                2.0,
                Binary64Operation::Multiply,
            )
            .to_bits();
            let original = read_environment();
            let requested_control = original.0 | 0x0000_9f00 | 0x00c0_0000 | 0x0300_0000;
            let requested_status = original.1 | 0x0000_009f;
            write_environment(requested_control, requested_status);
            let hostile = read_environment();

            let sqrt_result = backend_native_sqrt(f64::from_bits(1)).to_bits();
            let after_sqrt = read_environment();
            let exp_result = backend_native_exp(-744.0).to_bits();
            let after_exp = read_environment();
            let log_result = backend_native_log(f64::from_bits(1)).to_bits();
            let after_log = read_environment();
            let arithmetic_result = arithmetic(
                f64::from_bits(0x0000_0000_0000_0001),
                2.0,
                Binary64Operation::Multiply,
            )
            .to_bits();
            let after_arithmetic = read_environment();
            write_environment(original.0, original.1);

            assert_eq!(
                hostile.0 & (0x00c0_0000 | 0x0300_0000),
                requested_control & (0x00c0_0000 | 0x0300_0000)
            );
            assert_eq!(hostile.1 & 0x0000_009f, requested_status & 0x0000_009f);
            assert_eq!(sqrt_result, reference_sqrt);
            assert_eq!(after_sqrt, hostile);
            assert_eq!(exp_result, reference_exp);
            assert_eq!(after_exp, hostile);
            assert_eq!(log_result, reference_log);
            assert_eq!(after_log, hostile);
            assert_eq!(arithmetic_result, reference_arithmetic);
            assert_eq!(after_arithmetic, hostile);
        }
    }
}
