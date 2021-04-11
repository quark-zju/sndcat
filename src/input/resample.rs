use crate::input::Input;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use std::sync::mpsc::sync_channel;
use std::thread;

pub fn resample(mut input: Input, sample_rate: u32, quality: Option<usize>) -> Input {
    if sample_rate == input.info.sample_rate {
        return input;
    }

    let name = format!("Resample[{}, {}]", &input.name, sample_rate);
    let info = StreamInfo {
        sample_rate,
        channels: input.info.channels,
    };
    let mut resampler = crate::resample::Resampler::new(
        input.info,
        StreamInfo {
            channels: input.info.channels,
            sample_rate,
        },
        quality,
    );

    let (sender, receiver) = sync_channel(10);
    thread::spawn(move || {
        while let Some(mut samples) = (input.read)() {
            resampler.process(&mut samples);
            let _ = sender.send(samples);
        }
    });

    let func = move || -> Option<Samples> { receiver.recv().ok() };

    Input {
        name,
        info,
        read: Box::new(func),
    }
}
