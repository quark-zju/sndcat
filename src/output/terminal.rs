use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use std::time::Duration;
use std::time::Instant;

pub fn stats() -> Output {
    let name = "Stats[]".to_string();

    let writer = StatsWriter {
        total: 0,
        mtime: Instant::now(),
        info: StreamInfo {
            channels: 1,
            sample_rate: 44100,
        },
    };

    Output {
        name,
        sample_rate_hint: None,
        writer: Box::new(writer),
    }
}

struct StatsWriter {
    total: u64,
    info: StreamInfo,
    mtime: Instant,
}

impl StatsWriter {
    fn write_stats(&self) {
        let msg = format!(
            "{} samples {}hz {}ch",
            self.total, self.info.sample_rate, self.info.channels,
        );
        eprint!("{}\r", msg);
        log::debug!("{}", msg);
    }
}

impl OutputWriter for StatsWriter {
    fn write(&mut self, samples: &Samples) -> anyhow::Result<()> {
        if self.total == 0 {
            self.info = samples.info;
        }
        self.total += samples.samples.len() as u64;
        // Writing to terminal can be slow. Limit its freq.
        if self.mtime.elapsed() > Duration::from_millis(200) {
            self.write_stats();
            self.mtime = Instant::now();
        }
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        self.write_stats();
        Ok(())
    }
}
