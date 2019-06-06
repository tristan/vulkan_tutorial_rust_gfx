use std::hash::{Hash, Hasher};

const NANOS_PER_SEC: u64 = 1_000_000_000;

#[inline]
pub fn as_float_secs(duration: &std::time::Duration) -> f32 {
    // TODO: replace when duration_float is stable
    // https://github.com/rust-lang/rust/issues/54361
    let secs = duration.as_secs();
    let nanos = duration.as_nanos() - ((secs * NANOS_PER_SEC) as u128);
    (secs as f32) + (nanos as f32) / (NANOS_PER_SEC as f32)
}

pub fn ratio(width: u32, height: u32) -> f32 {
    (width as f32) / (height as f32)
}

const SIGN_MASK: u64 = 0x8000000000000000u64;
const EXP_MASK: u64 = 0x7ff0000000000000u64;
const MAN_MASK: u64 = 0x000fffffffffffffu64;

const CANONICAL_NAN_BITS: u64 = 0x7ff8000000000000u64;
const CANONICAL_ZERO_BITS: u64 = 0x0u64;

// https://github.com/reem/rust-ordered-float/blob/master/src/lib.rs#L497
#[inline]
pub fn hash_float<H: Hasher>(f: f32, state: &mut H) {
    raw_double_bits(f).hash(state);
}

#[inline]
fn raw_double_bits(f: f32) -> u64 {
    if f.is_nan() {
        return CANONICAL_NAN_BITS;
    }

    let (man, exp, sign) = integer_decode_f32(f);
    if man == 0 {
        return CANONICAL_ZERO_BITS;
    }

    let exp_u64 = unsafe { std::mem::transmute::<i16, u16>(exp) } as u64;
    let sign_u64 = if sign > 0 { 1u64 } else { 0u64 };
    (man & MAN_MASK) | ((exp_u64 << 52) & EXP_MASK) | ((sign_u64 << 63) & SIGN_MASK)
}


// https://github.com/rust-num/num-traits/blob/master/src/float.rs#L1883
fn integer_decode_f32(f: f32) -> (u64, i16, i8) {
    let bits: u32 = unsafe { std::mem::transmute(f) };
    let sign: i8 = if bits >> 31 == 0 { 1 } else { -1 };
    let mut exponent: i16 = ((bits >> 23) & 0xff) as i16;
    let mantissa = if exponent == 0 {
        (bits & 0x7fffff) << 1
    } else {
        (bits & 0x7fffff) | 0x800000
    };
    // Exponent bias + mantissa shift
    exponent -= 127 + 23;
    (mantissa as u64, exponent, sign)
}
