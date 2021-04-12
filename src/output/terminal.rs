use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use std::collections::VecDeque;
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
        meters: (0..20).map(|_| 0.0).collect(),
        meter_max: 0.1,
        meter_current: 0.0,
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
    meters: VecDeque<f32>,
    meter_max: f32,
    meter_current: f32, // Current
}

impl StatsWriter {
    fn write_stats(&self) {
        let millis = self.total * 100 / (self.info.sample_count_millis(100) as u64);
        let seconds = millis / 1000;
        let millis = millis % 1000;
        let msg = format!(
            "{:02}:{:02}.{:01} {}khz {}ch",
            seconds / 60,
            seconds % 60,
            millis / 100,
            self.info.sample_rate / 1000,
            self.info.channels,
        );
        eprint!("{} {}\r", msg, self.render_bar());
        log::debug!("{}", msg);
    }

    fn render_bar(&self) -> String {
        let chars = &['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];
        self.meters
            .iter()
            .map(|&v| {
                if v == 0.0 {
                    ' '
                } else {
                    let i = (v * (chars.len() as f32) / self.meter_max) as usize;
                    chars[i.min(chars.len() - 1)]
                }
            })
            .collect()
    }
}

impl OutputWriter for StatsWriter {
    fn write(&mut self, samples: &Samples) -> anyhow::Result<()> {
        if self.total == 0 {
            self.info = samples.info;
        }

        // Update "meters" (for rendering vu meters) and "total".
        let mut total = self.total;
        let meter_sample_rate = (self.info.sample_rate / 10) as u64;
        let mut next_total = (total / meter_sample_rate + 1) * meter_sample_rate;
        for &s in &samples.samples {
            self.meter_current += s * s;
            total += 1;
            if total >= next_total {
                if self.meter_current > 0.0 {
                    self.meter_current = self.meter_current.log2();
                }
                if self.meter_current > self.meter_max {
                    self.meter_max = self.meter_current;
                }
                self.meters.pop_front();
                self.meters.push_back(self.meter_current);
                self.meter_current = 0.0;
                next_total += meter_sample_rate;
            }
        }
        self.total = total;

        // Writing to terminal can be slow. Limit its freq.
        if self.mtime.elapsed() >= Duration::from_millis(100) {
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
