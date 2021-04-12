use crate::input::Input;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use portaudio::stream::InputSettings;
use portaudio::stream::Parameters;
use portaudio::DeviceIndex;
use portaudio::PortAudio;

struct ForceSend<T>(T);
unsafe impl<T> Send for ForceSend<T> {}

pub fn input_device(pa: &PortAudio, i: u32) -> anyhow::Result<Input> {
    let i = DeviceIndex(i);
    let info = pa.device_info(i)?;
    let channel_count = info.max_input_channels;
    let sample_rate = info.default_sample_rate as u32;
    let frame_size = (sample_rate as u64) / 16;
    let params: Parameters<f32> = Parameters::new(i, channel_count as _, true, 0.0);
    let settings = InputSettings::new(params, sample_rate as _, frame_size as _);
    let settings = settings.clone();
    let mut stream = pa.open_blocking_stream(settings)?;
    stream.start()?;
    let stream = ForceSend(stream);
    let name = format!("Device[{:?}]", info.name.replace('\r', " "));
    let info = StreamInfo {
        sample_rate: sample_rate as _,
        channels: channel_count as _,
    };
    let func = move || -> Option<Samples> {
        // https://portaudio.music.columbia.narkive.com/nLRsY3jZ/thread-safety-of-blocking-calls
        // > In general, calls to ReadStream and WriteStream on different streams in different threads are probably safe...
        let stream = &stream.0;
        let buf = stream.read(sample_rate / 100);
        if let Ok(buf) = buf {
            let samples = Samples::new(info, buf.to_vec());
            Some(samples)
        } else {
            None
        }
    };

    Ok(Input {
        name,
        info,
        read: Box::new(func),
    })
}
