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
