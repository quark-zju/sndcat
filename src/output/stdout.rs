use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use crate::resample::Resampler;
use std::io::Write;
use std::sync::Arc;

pub fn stdout_i16(info: StreamInfo) -> Output {
    let name = format!("Stdout[{}]", &info);

    let output = StdoutOutput {
        info,
        resampler: None,
    };

    Output {
        name,
        sample_rate_hint: Some(info.sample_rate),
        writer: Box::new(output),
    }
}

struct StdoutOutput {
    info: StreamInfo,
    resampler: Option<Resampler>,
}

impl OutputWriter for StdoutOutput {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        let samples = crate::mixer::normalize_cow(samples, self.info, &mut self.resampler)?;
        // Convert samples to i16. Write as i16LE.
        let mut out = Vec::<u8>::with_capacity(samples.samples.len() * 2);
        for &f in &samples.samples {
            let v = (f * 30000.0) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        std::io::stdout().write_all(&out)?;
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        // Wait for clients?
        std::io::stdout().flush()?;
        Ok(())
    }
}
