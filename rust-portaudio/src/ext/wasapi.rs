use crate::HostApiTypeId;
use libc::c_ulong;
use libc::c_void;
use std::ptr;

/// Matches the `PaWinWaveFormatChannelMask` C typedef.
pub type WinWaveFormatChannelMask = c_ulong;
/// Matches the `PaWasapiStreamInfo` C struct.
#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub struct WasapiStreamInfo {
    size: c_ulong,
    host_api_type: HostApiTypeId,
    version: c_ulong,
    flags: c_ulong,
    channel_mask: WinWaveFormatChannelMask,
    host_processor_output: *const c_void,
    host_processor_input: *const c_void,
    thread_priority: WasapiThreadPriority,
    stream_category: WasapiStreamCategory,
    stream_option: WasapiStreamOption,
}

impl WasapiStreamInfo {
    /// Creates a new `WasapiStreamInfo` struct.
    pub fn new() -> Self {
        Self {
            size: std::mem::size_of::<Self>() as _,
            host_api_type: HostApiTypeId::WASAPI,
            version: 1,
            flags: 0,
            channel_mask: 0,
            host_processor_output: ptr::null(),
            host_processor_input: ptr::null(),
            thread_priority: WasapiThreadPriority::None,
            stream_category: WasapiStreamCategory::Other,
            stream_option: WasapiStreamOption::None,
        }
    }
}

/// Matches the `PaWasapiThreadPriority` C struct.
#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub enum WasapiThreadPriority {
    None = 0,
    Audio,
    Capture,
    Distribution,
    Games,
    Playback,
    ProAudio,
    WindowManager,
}

/// Matches the `PaWasapiStreamCategory` C struct.
#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub enum WasapiStreamCategory {
    Other = 0,
    Communications = 3,
    Alerts = 4,
    SoundEffects = 5,
    GameEffects = 6,
    GameMedia = 7,
    GameChat = 8,
    Speech = 9,
    Movie = 10,
    Media = 11,
}

/// Matches the `PaWasapiStreamOption` C struct.
#[doc(hidden)]
#[derive(Copy, Clone, Debug)]
#[repr(C)]
pub enum WasapiStreamOption {
    None = 0,
    Raw = 1,
    MatchFormat = 2,
}
