//! Platform-specific audio session management.
//!
//! Currently only iOS requires explicit audio session configuration.
//! Other platforms are no-ops.
//!
//! Design (MAYA DRY KISS):
//! - Initialize once, keep active for app lifetime
//! - No deactivation (avoids 0.5s blocking delays)
//! - Thread-safe lazy initialization via OnceLock

use std::sync::OnceLock;

use common::error::{InstrumentError, Result};

#[cfg(target_os = "ios")]
use objc::runtime::{Object, BOOL, YES};
#[cfg(target_os = "ios")]
use objc::{msg_send, sel, sel_impl};

/// Global flag tracking whether audio session has been configured
static AUDIO_SESSION_CONFIGURED: OnceLock<bool> = OnceLock::new();

/// Ensure platform audio session is configured for duplex audio.
/// Idempotent - safe to call multiple times.
/// On non-iOS platforms, this is a no-op.
pub fn ensure_configured() -> Result<()> {
    // Fast path: already configured
    if AUDIO_SESSION_CONFIGURED.get().is_some() {
        return Ok(());
    }

    // Platform-specific configuration
    #[cfg(target_os = "ios")]
    unsafe {
        configure_ios()?;
        log::info!("Audio session configured on iOS");
    }

    // Mark as configured
    AUDIO_SESSION_CONFIGURED.set(true).map_err(|_| {
        // This should never happen (race condition in theory)
        InstrumentError::DeviceUnavailable
    })?;

    Ok(())
}

#[cfg(target_os = "ios")]
unsafe fn configure_ios() -> Result<()> {
    // Get the shared AVAudioSession singleton instance
    let audio_session: *mut Object = msg_send![class!(AVAudioSession), sharedInstance];

    // Create category string for playAndRecord (duplex audio)
    let category_str: *mut Object = msg_send![
        class!(NSString),
        stringWithUTF8String: "AVAudioSessionCategoryPlayAndRecord\0".as_ptr()
    ];

    // Create mode string for default mode
    let mode_str: *mut Object = msg_send![
        class!(NSString),
        stringWithUTF8String: "AVAudioSessionModeMeasurement\0".as_ptr()
    ];

    // Set category and mode with options
    // Options: 0x8 = DefaultToSpeaker, 0x4 = AllowBluetooth
    let options: u64 = 0x8 | 0x4; // DefaultToSpeaker | AllowBluetooth
    let mut error: *mut Object = std::ptr::null_mut();

    let success: BOOL = msg_send![
        audio_session,
        setCategory: category_str
        mode: mode_str
        options: options
        error: &mut error
    ];

    if success != YES {
        log::error!("Failed to set iOS audio session category");
        if !error.is_null() {
            let description: *mut Object = msg_send![error, localizedDescription];
            let c_str: *const std::os::raw::c_char = msg_send![description, UTF8String];
            if !c_str.is_null() {
                let err_msg = std::ffi::CStr::from_ptr(c_str).to_string_lossy();
                log::error!("Error details: {}", err_msg);
            }
        }
        return Err(InstrumentError::DeviceUnavailable.into());
    }

    // Set preferred sample rate to 48kHz for better quality
    let preferred_rate: f64 = 48000.0;
    let mut error: *mut Object = std::ptr::null_mut();
    let _: BOOL = msg_send![
        audio_session,
        setPreferredSampleRate: preferred_rate
        error: &mut error
    ];

    if !error.is_null() {
        log::warn!("Could not set preferred sample rate to 48kHz, using device default");
    }

    // Set preferred I/O buffer duration for low latency (256 samples @ 48kHz ≈ 5.3ms)
    let preferred_duration: f64 = 256.0 / 48000.0;
    let mut error: *mut Object = std::ptr::null_mut();
    let _: BOOL = msg_send![
        audio_session,
        setPreferredIOBufferDuration: preferred_duration
        error: &mut error
    ];

    if !error.is_null() {
        log::warn!("Could not set preferred I/O buffer duration, using device default");
    }

    // Activate the audio session (and keep it active for app lifetime)
    let mut error: *mut Object = std::ptr::null_mut();
    let success: BOOL = msg_send![
        audio_session,
        setActive: YES
        error: &mut error
    ];

    if success != YES {
        log::error!("Failed to activate iOS audio session");
        if !error.is_null() {
            let description: *mut Object = msg_send![error, localizedDescription];
            let c_str: *const std::os::raw::c_char = msg_send![description, UTF8String];
            if !c_str.is_null() {
                let err_msg = std::ffi::CStr::from_ptr(c_str).to_string_lossy();
                log::error!("Error details: {}", err_msg);
            }
        }
        return Err(InstrumentError::DeviceUnavailable.into());
    }

    log::info!(
        "iOS audio session configured for duplex audio (will remain active for app lifetime)"
    );
    Ok(())
}
