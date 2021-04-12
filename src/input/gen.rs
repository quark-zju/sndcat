use crate::input::Input;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use std::sync::atomic::AtomicUsize;

pub fn sin_wave(sample_rate_hint: Option<u32>, params: &str) -> anyhow::Result<Input> {
    // params: 440
    // params: freq=440;samplerate=44100

    let mut freq: f32 = 440.0;
    let mut power: f32 = 0.5;
    let mut sample_rate: u32 = sample_rate_hint.unwrap_or(48000);
    let mut channels: u16 = 1;

    for s in params.split(';') {
        if s.contains("=") {
            let mut split = s.splitn(2, "=");
            let key = split.next().unwrap();
            let value = split.next().unwrap();
            match key {
                "freq" => freq = value.parse()?,
                "power" => power = value.parse()?,
                "samplerate" => sample_rate = value.parse()?,
                "channel" => channels = value.parse()?,
                _ => anyhow::bail!("unknown key: {}", key),
            }
        } else {
            freq = s.parse()?
        }
    }

    let name = format!("Sin[{}Hz]", freq,);
    let info = StreamInfo {
        sample_rate,
        channels,
    };
    let step: f32 = (freq as f32) * std::f32::consts::PI * 2.0 / (sample_rate as f32);
    let tick = AtomicUsize::new(0);
    let func = move || -> Option<Samples> {
        let n = (sample_rate as usize) * (channels as usize)
            / (crate::mixer::MIXER_SAMPLE_SIZE as usize);
        let mut buf = Vec::with_capacity(n);
        for _ in 0..n {
            let tick = tick.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            for _ in 0..channels {
                buf.push((step * (tick as f32)).sin() * power);
            }
        }
        Some(Samples::new(info, buf))
    };

    Ok(Input {
        name,
        info,
        read: Box::new(func),
    })
}

pub fn silence(sample_rate_hint: Option<u32>) -> anyhow::Result<Input> {
    let sample_rate: u32 = sample_rate_hint.unwrap_or(48000);
    let channels: u16 = 1;

    let name = "Silence[]".to_string();
    let info = StreamInfo {
        sample_rate,
        channels,
    };
    let func = move || -> Option<Samples> {
        let n = info.sample_count_millis(100);
        let buf = vec![0.0; n];
        Some(Samples::new(info, buf))
    };

    Ok(Input {
        name,
        info,
        read: Box::new(func),
    })
}
