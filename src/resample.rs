use crate::config;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;

pub struct Resampler {
    pub input_info: StreamInfo,
    pub output_info: StreamInfo,
    // Per channel.
    state: Vec<speexdsp_resampler::State>,
    quality: usize,
}

impl Resampler {
    pub fn new(input_info: StreamInfo, output_info: StreamInfo, quality: Option<usize>) -> Self {
        assert_eq!(input_info.channels, output_info.channels);
        let channel_count = input_info.channels;
        let quality = quality
            .unwrap_or_else(|| *config::RESAMPLE_QUALITY)
            .max(0)
            .min(10);
        let state = (0..channel_count)
            .map(|_| {
                speexdsp_resampler::State::new(
                    input_info.channels as _,
                    input_info.sample_rate as _,
                    output_info.sample_rate as _,
                    quality,
                )
                .unwrap()
            })
            .collect();
        Self {
            state,
            input_info,
            output_info,
            quality,
        }
    }

    pub fn process(&mut self, samples: &mut Samples) {
        assert_eq!(samples.info, self.input_info);
        assert_eq!(samples.info.channels as usize, self.state.len());
        log::debug!(
            "resample {} to {}hz with quality {}",
            &samples,
            self.output_info.sample_rate,
            self.quality,
        );
        let in_len = samples.samples.len() / (samples.info.channels as usize);
        if in_len > 0 {
            if self.input_info.channels == 1 {
                let out_len = in_len * (self.output_info.sample_rate as usize)
                    / (self.input_info.sample_rate as usize);
                let mut out = vec![0f32; out_len];
                self.state[0]
                    .process_float(0, &mut samples.samples, &mut out)
                    .unwrap();
                samples.samples = out;
            } else {
                let out_len = in_len * (self.output_info.sample_rate as usize)
                    / (self.input_info.sample_rate as usize);
                let channels = self.input_info.channels as usize;
                // Single channel buffer
                let mut in_buf = vec![0.0; in_len];
                let mut out_buf = vec![0.0; out_len];
                // Mixed channel buffer
                let mut final_out_buf = vec![0.0; out_len * channels];
                for c in 0..channels {
                    for i in 0..in_len {
                        in_buf[i] = samples.samples[i * channels + c];
                    }
                    self.state[c]
                        .process_float(0, &mut in_buf, &mut out_buf)
                        .unwrap();
                    for i in 0..out_len {
                        final_out_buf[i * channels + c] = out_buf[i];
                    }
                }
                samples.samples = final_out_buf;
            }
        }
        samples.info.sample_rate = self.output_info.sample_rate;
        log::debug!("resample completed {}", &samples);
    }
}
