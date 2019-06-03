use gfx_hal::image::SubresourceRange;
use gfx_hal::format;

pub(super) const COLOR_RANGE: SubresourceRange = SubresourceRange {
    aspects: format::Aspects::COLOR,
    levels: 0..1,
    layers: 0..1,
};
