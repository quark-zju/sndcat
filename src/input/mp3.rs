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
    let (sender, receiver) = sync_channel(10);

    let mut read_samples = move || -> Option<Samples> {
        let frame = decoder.next_frame().ok();
        frame.map(|frame| {
            log::debug!(
                "MP3 frame {} samples, bitrate: {}",
                frame.data.len(),
                frame.bitrate
            );
            Samples {
                info: StreamInfo {
                    channels: frame.channels as _,
                    sample_rate: frame.sample_rate as _,
                },
                samples: frame
                    .data
                    .into_iter()
                    .map(|v| (v as f32) / (i16::max_value() as f32))
                    .collect(),
            }
        })
    };

    // Decode a frame to get sample rate.
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
