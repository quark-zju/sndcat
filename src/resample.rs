use crate::mixer::Samples;
use crate::mixer::StreamInfo;

pub struct Resampler {
    state: speexdsp_resampler::State,
    pub input_info: StreamInfo,
    pub output_info: StreamInfo,
    quality: usize,
}

fn default_quality() -> usize {
    let mut result = 4;
    if let Ok(v) = std::env::var("SNDCAT_RESAMPLE_QUALITY") {
        if let Ok(v) = v.parse() {
            result = v;
        }
    }
    result
}

impl Resampler {
    pub fn new(input_info: StreamInfo, output_info: StreamInfo, quality: Option<usize>) -> Self {
        assert_eq!(input_info.channels, output_info.channels);
        let quality = quality.unwrap_or_else(default_quality).max(0).min(10);
        let state = speexdsp_resampler::State::new(
            input_info.channels as _,
            input_info.sample_rate as _,
            output_info.sample_rate as _,
            quality,
        )
        .unwrap();
        Self {
            state,
            input_info,
            output_info,
            quality,
        }
    }

    pub fn process(&mut self, samples: &mut Samples) {
        assert_eq!(samples.info, self.input_info);
        log::debug!(
            "resample {} to {}hz with quality {}",
            &samples,
            self.output_info.sample_rate,
            self.quality,
        );
        let in_len = samples.samples.len();
        if in_len > 0 {
            let out_len = in_len * (self.output_info.sample_rate as usize)
                / (self.input_info.sample_rate as usize);
            let mut out = vec![0f32; out_len];
            self.state
                .process_float(0, &mut samples.samples, &mut out)
                .unwrap();
            samples.samples = out;
        }
        samples.info.sample_rate = self.output_info.sample_rate;
        log::debug!("resample completed {}", &samples);
    }
}
