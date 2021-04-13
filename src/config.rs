use once_cell::sync::Lazy;

pub static RESAMPLE_QUALITY: Lazy<usize> = Lazy::new(|| {
    let mut result = 4;
    if let Ok(v) = std::env::var("SNDCAT_RESAMPLE_QUALITY") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result
});

pub static DECODE_BUFFER_MILLIS: Lazy<usize> = Lazy::new(|| {
    let mut result = 50;
    if let Ok(v) = std::env::var("SNDCAT_DECODE_BUFFER_MILLIS") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result
});

pub static MAIN_THREAD_PRIORITY: Lazy<u32> = Lazy::new(|| {
    let mut result = 80;
    if let Ok(v) = std::env::var("SNDCAT_MAIN_THREAD_PRIORITY") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result.min(100).max(0)
});

pub static MAX_OUTPUT_CHANNELS: Lazy<i32> = Lazy::new(|| {
    let mut result = 2;
    if let Ok(v) = std::env::var("SNDCAT_MAX_OUTPUT_CHANNELS") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result.min(64).max(1)
});

pub static MAX_INPUT_CHANNELS: Lazy<i32> = Lazy::new(|| {
    let mut result = 2;
    if let Ok(v) = std::env::var("SNDCAT_MAX_INPUT_CHANNELS") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result.min(64).max(1)
});
