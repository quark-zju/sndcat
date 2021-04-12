use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use portaudio::stream::OutputSettings;
use portaudio::stream::Parameters;
use portaudio::DeviceIndex;
use portaudio::PortAudio;
use std::sync::Arc;

pub fn output_device(pa: &PortAudio, i: u32) -> anyhow::Result<Output> {
    let i = DeviceIndex(i);
    let info = pa.device_info(i)?;
    let channel_count = info.max_output_channels;
    let sample_rate = info.default_sample_rate as u32;
    let frame_size = (sample_rate as u64) / 10;
    let params: Parameters<f32> = Parameters::new(i, channel_count as _, true, 0.0);
    let settings = OutputSettings::new(params, sample_rate as _, frame_size as _);
    let settings = settings.clone();
    let mut stream = pa.open_blocking_stream(settings)?;
    stream.start()?;
    let stream = ForceSend(stream);
    let name = format!("Device[{}]", info.name.replace('\r', " "));
    let info = StreamInfo {
        sample_rate: sample_rate as _,
        channels: channel_count as _,
    };
    let writer = DeviceOutput {
        stream,
        info,
        frame_size,
        resampler: None,
    };

    Ok(Output {
        name,
        sample_rate_hint: Some(sample_rate),
        writer: Box::new(writer),
    })
}

struct ForceSend<T>(T);
unsafe impl<T> Send for ForceSend<T> {}

struct DeviceOutput {
    stream: ForceSend<
        portaudio::Stream<portaudio::Blocking<portaudio::stream::Buffer>, portaudio::Output<f32>>,
    >,
    frame_size: u64,
    info: StreamInfo,
    resampler: Option<crate::resample::Resampler>,
}

impl OutputWriter for DeviceOutput {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        let samples = crate::mixer::normalize_cow(samples, self.info, &mut self.resampler)?;

        // https://portaudio.music.columbia.narkive.com/nLRsY3jZ/thread-safety-of-blocking-calls
        // > In general, calls to ReadStream and WriteStream on different streams in different threads are probably safe...
        let stream = &mut self.stream.0;
        assert_eq!(self.info, samples.info);
        for slice in samples.samples.chunks(self.frame_size as _) {
            let len = slice.len();
            let frames = len / (self.info.channels as usize);
            stream.write(frames as _, move |b: &mut [f32]| {
                assert!(b.len() <= len);
                b.copy_from_slice(&slice[0..b.len()]);
            })?;
        }
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        let stream = &mut self.stream.0;
        stream.stop()?;
        Ok(())
    }
}
