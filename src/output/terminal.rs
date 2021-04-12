use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

pub fn stats() -> Output {
    let name = "Stats[]".to_string();

    let writer = StatsWriter {
        mtime: Instant::now(),
        meters: Meters::new(),
    };

    Output {
        name,
        sample_rate_hint: None,
        writer: Box::new(writer),
    }
}

struct Meters {
    values: VecDeque<(f32, bool)>, // db, clipped
    current_power: f32,            // Current
    clipped: bool,                 // Current, if clipped happened.
    info: StreamInfo,
    total: u64,
}

struct StatsWriter {
    mtime: Instant,
    meters: Meters,
}

impl Meters {
    fn new() -> Self {
        Self {
            values: (0..20).map(|_| (-60.0, false)).collect(),
            current_power: 0.0,
            clipped: false,
            total: 0,
            info: StreamInfo::dummy(),
        }
    }

    fn process(&mut self, samples: &Samples) {
        // Update "meters" (for rendering vu meters) and "total".
        let mut total = self.total;
        if total == 0 {
            self.info = samples.info;
        }
        let meter_sample_rate = (self.info.sample_rate / 10) as u64;
        let mut next_total = (total / meter_sample_rate + 1) * meter_sample_rate;
        for &s in &samples.samples {
            if s > 1.0 || s < -1.0 {
                self.clipped = true;
            }
            self.current_power += s * s;
            total += 1;
            if total >= next_total {
                let db = {
                    let power_0db = meter_sample_rate as f32; // 0db
                    (self.current_power / power_0db).log10() * 10.0
                };
                self.values.pop_front();
                self.values.push_back((db, self.clipped));
                self.current_power = 0.0;
                self.clipped = false;
                next_total += meter_sample_rate;
            }
        }
        self.total = total;
    }

    fn render_bar(&self) -> String {
        static SYMBOLS: &[&str] = &["▁", "▂", "▃", "▄", "▅", "▆", "▇", "█"];
        self.values
            .iter()
            .map(|(db, clipped)| {
                // limit to MIN_DB .. MAX_DB range.
                let db = db.min(0.0);
                if db <= MIN_DB {
                    " ".to_string()
                } else {
                    let i = (db - MIN_DB) / (MAX_DB - MIN_DB) * (SYMBOLS.len() as f32);
                    let i = i as usize;
                    let i = i.min(SYMBOLS.len() - 1).max(0);
                    let sym = SYMBOLS[i].to_string();
                    if *clipped {
                        format!("\x1b[31m{}\x1b[0m", sym)
                    } else {
                        sym
                    }
                }
            })
            .collect::<Vec<_>>()
            .concat()
    }

    fn current_db(&self) -> f32 {
        match self.values.back() {
            Some((db, _)) => *db,
            None => MIN_DB,
        }
    }
}

// Visualize "db" in the given range.
const MIN_DB: f32 = -32.0;
const MAX_DB: f32 = -8.0;

impl StatsWriter {
    fn write_stats(&self) {
        let millis = self.meters.total * 100 / (self.meters.info.sample_count_millis(100) as u64);
        let seconds = millis / 1000;
        let millis = millis % 1000;
        let msg = format!(
            "{:02}:{:02}.{:01} {}khz {}ch",
            seconds / 60,
            seconds % 60,
            millis / 100,
            self.meters.info.sample_rate / 1000,
            self.meters.info.channels,
        );
        let db = self.meters.current_db();
        eprint!("{} {} {:+0.1}dB  \r", msg, self.meters.render_bar(), db);
        log::debug!("{} {:+0.2}dB", msg, db);
    }
}

impl OutputWriter for StatsWriter {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        // Update "meters" (for rendering vu meters) and "total".
        self.meters.process(&samples);

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
