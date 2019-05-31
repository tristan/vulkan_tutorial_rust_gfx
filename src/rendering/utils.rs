const NANOS_PER_SEC: u32 = 1_000_000_000;

pub fn as_float_secs(duration: &std::time::Duration) -> f32 {
    (duration.as_secs() as f32) + (duration.as_nanos() as f32) / (NANOS_PER_SEC as f32)
}

pub fn ratio(width: u32, height: u32) -> f32 {
    (width as f32) / (height as f32)
}
