use crate::safe_area::DEFAULT_SAFE_AREA;

/// Common desktop display resolutions in pixels
pub const DESKTOP_SCREEN_SIZES: [(u32, u32); 6] = [
    (1920, 1080), // Full HD
    (1366, 768),
    (1536, 864),
    (1280, 720),
    (1440, 900),
    (1600, 900),
];

/// Common mobile display resolutions in pixels
pub const MOBILE_SCREEN_SIZES: [(u32, u32); 5] =
    [(360, 800), (390, 844), (393, 873), (412, 915), (414, 896)];

/// Common tablet display resolutions in pixels
pub const TABLET_SCREEN_SIZES: [(u32, u32); 5] = [
    (768, 1024),
    (810, 1080),
    (820, 1180),
    (1280, 800),
    (800, 1280),
];

/// Safe area insets (pixels) for common mobile layouts (portrait).
pub const MOBILE_SAFE_AREA_INSETS: [(i64, i64, i64, i64); 3] = [
    // (top, right, bottom, left)
    (44, 0, 34, 0), // iPhone X, 11, 12, 13, 14, 15 series w/ notch
    (24, 0, 16, 0), // Typical Android (navigation bar, status bar)
    (0, 0, 0, 0),   // Devices without a notch or special gesture area
];

/// Safe area insets (pixels) for common tablet layouts (portrait).
pub const TABLET_SAFE_AREA_INSETS: [(i64, i64, i64, i64); 2] = [
    (24, 0, 20, 0), // Typical iPad Pro with home indicator area
    (0, 0, 0, 0),   // Older iPads, Android tablets, no system gesture zone
];

pub fn test_cases() -> impl Iterator<Item = (&'static (u32, u32), &'static (i64, i64, i64, i64))> {
    DESKTOP_SCREEN_SIZES
        .iter()
        .zip(DESKTOP_SCREEN_SIZES.iter().map(|_| {
            &(
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
                DEFAULT_SAFE_AREA,
            )
        }))
        .chain(
            MOBILE_SCREEN_SIZES
                .iter()
                .zip(MOBILE_SAFE_AREA_INSETS.iter().cycle()),
        )
        .chain(
            TABLET_SCREEN_SIZES
                .iter()
                .zip(TABLET_SAFE_AREA_INSETS.iter().cycle()),
        )
}
