use crate::input::Input;
use crate::mixer::Samples;
use anyhow::Context;
use std::sync::mpsc::sync_channel;
use std::thread;

pub fn opus(path: &str) -> anyhow::Result<Input> {
    let name = format!("Opus[{}]", path);
    let file = std::fs::File::open(path).context(path.to_string())?;
    let file = std::io::BufReader::with_capacity(1048576, file);

    let mut ogg_reader = ogg::PacketReader::new(file);
    ogg_reader.delete_unread_packets();

    let info = {
        let head = match ogg_reader.read_packet()? {
            Some(head) => head,
            None => anyhow::bail!("lack of Opus head"),
        };
        let _tags = ogg_reader.read_packet()?;
        let header = crate::oggopus::Header::deserialize_head(&head.data)?;
        let info = header.info;
        info
    };

    let mut opus_decoder = {
        let channels = match info.channels {
            1 => audiopus::Channels::Mono,
            2 => audiopus::Channels::Stereo,
            n => anyhow::bail!("unsupported channels: {}", n),
        };
        audiopus::coder::Decoder::new(audiopus::SampleRate::Hz48000, channels)?
    };

    let mut read_samples = {
        let mut out_buf = vec![0f32; (info.sample_rate as usize) * 3];
        move || -> Option<Samples> {
            let mut combined_samples = Samples::new(info, Vec::new());
            while let Some(Some(pkt)) = ogg_reader.read_packet().ok() {
                let pkt = Some(&pkt.data);
                let len = opus_decoder.decode_float(pkt, &mut out_buf, false).ok()?;
                let samples = out_buf[..(len * (info.channels as usize))].to_vec();
                let samples = Samples::new(info, samples);
                // Buffer samples. Avoid sending small samples frequently.
                combined_samples.concat(samples);
                if combined_samples.millis() >= 50 {
                    return Some(combined_samples);
                }
            }
            if combined_samples.is_empty() {
                None
            } else {
                Some(combined_samples)
            }
        }
    };

    let (sender, receiver) = sync_channel(10);
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
