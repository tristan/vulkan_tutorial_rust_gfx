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
