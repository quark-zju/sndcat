use crate::input::Input;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use anyhow::Context;
use std::sync::mpsc::sync_channel;
use std::thread;

pub fn mp3(path: &str) -> anyhow::Result<Input> {
    let name = format!("Mp3[{}]", path);
    let file = std::fs::File::open(path).context(path.to_string())?;
    let mut decoder = minimp3::Decoder::new(file);

    let min_buffer_millis = *crate::config::DECODE_BUFFER_MILLIS;
    let mut read_samples = move || -> Option<Samples> {
        let mut combined_samples = Samples::new(StreamInfo::dummy(), Vec::new());
        while let Some(frame) = decoder.next_frame().ok() {
            log::debug!(
                "MP3 frame {} samples, bitrate: {}",
                frame.data.len(),
                frame.bitrate
            );
            let samples = Samples {
                info: StreamInfo {
                    channels: frame.channels as _,
                    sample_rate: frame.sample_rate as _,
                },
                samples: frame
                    .data
                    .into_iter()
                    .map(|v| (v as f32) / (i16::max_value() as f32))
                    .collect(),
            };
            // Buffer samples. Avoid sending small samples frequently.
            if samples.millis() >= min_buffer_millis && combined_samples.is_empty() {
                return Some(samples);
            }
            combined_samples.concat(samples);
            if combined_samples.millis() >= min_buffer_millis {
                return Some(combined_samples);
            }
        }
        if combined_samples.is_empty() {
            None
        } else {
            Some(combined_samples)
        }
    };

    // Decode a frame to get sample rate.
    let (sender, receiver) = sync_channel((1000 / min_buffer_millis).max(30) + 1);
    let samples = read_samples().ok_or_else(|| anyhow::format_err!("mp3 is empty"))?;
    let info = samples.info;
    sender.send(samples).unwrap();

    thread::spawn(move || {
        while let Some(samples) = read_samples() {
            let _ = sender.send(samples);
        }
    });

    let func = move || -> Option<Samples> { receiver.recv().ok() };
    let input = Input {
        name,
        info,
        read: Box::new(func),
    };
    Ok(input)
}
