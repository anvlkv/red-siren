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
