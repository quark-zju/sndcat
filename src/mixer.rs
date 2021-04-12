use crate::resample::Resampler;
use parking_lot::RwLock;
use std::collections::VecDeque;
use std::fmt;
use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

pub struct Mixer {
    /// Mutable input buffers.
    streams: Vec<Arc<MixBuffer>>,
}

pub struct MixBuffer {
    info: StreamInfo,
    buf: RwLock<VecDeque<f32>>,
    ended: AtomicBool,
}

#[derive(Debug, Copy, Clone, Eq, PartialEq)]
pub struct StreamInfo {
    pub sample_rate: u32,
    pub channels: u16,
}

#[derive(Debug, Clone)]
pub struct Samples {
    pub info: StreamInfo,
    pub samples: Vec<f32>,
}

pub(crate) const MIXER_SAMPLE_SIZE: u32 = 100;

impl MixBuffer {
    /// Add samples. Might block if the buffer is relatively full.
    pub fn extend(&self, samples: Samples) {
        // Already has 2 seconds of content?
        while self.buf.read().len() > (self.info.sample_rate as usize) * 2 {
            // Wait for buffer to be consumed.
            thread::sleep(Duration::from_millis(10));
        }
        log::trace!("buf len {}", self.buf.read().len());
        assert_eq!(self.info, samples.info);
        self.buf.write().extend(samples.samples);
    }

    /// Mark the stream as ended.
    pub fn end(&self) {
        self.ended.store(true, SeqCst);
    }
}

impl StreamInfo {
    pub fn sample_count_millis(&self, millis: usize) -> usize {
        (self.channels as usize) * (self.sample_rate as usize) * millis / 1000
    }

    fn sample_count_internal_batch_size(&self) -> u32 {
        // 10ms
        (self.channels as u32) * self.sample_rate / MIXER_SAMPLE_SIZE
    }
}

impl fmt::Display for StreamInfo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}Hz {}ch", self.sample_rate, self.channels)
    }
}

impl fmt::Display for Samples {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} samples ({})", self.samples.len(), self.info)
    }
}

impl Samples {
    pub fn new(info: StreamInfo, samples: Vec<f32>) -> Self {
        assert_eq!(samples.len() % (info.channels as usize), 0);
        Self { info, samples }
    }

    pub fn millis(&self) -> usize {
        self.samples.len() * 1000 / (self.info.channels as usize) / (self.info.sample_rate as usize)
    }

    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    /// Connect another samples in-place.
    pub fn concat(&mut self, other: Self) {
        if self.samples.is_empty() {
            self.info = other.info;
        }
        assert_eq!(self.info, other.info);
        self.samples.extend(other.samples);
    }

    /// Mix with another samples in-place.
    fn mix(&mut self, mut other: Samples) -> anyhow::Result<()> {
        log::debug!("mixing {}", &other);
        other.normalize_channels(self.info)?;
        assert_eq!(self.info, other.info);
        if self.samples.is_empty() {
            self.samples = other.samples;
        } else {
            if other.samples.len() != self.samples.len() {
                anyhow::bail!(
                    "sample count mismatch: {} != {}",
                    other.samples.len(),
                    self.samples.len()
                );
            }
            for (i, v) in other.samples.into_iter().enumerate() {
                self.samples[i] += v;
            }
        }
        Ok(())
    }

    /// Normalize to spec.
    pub fn normalize_channels(&mut self, info: StreamInfo) -> anyhow::Result<()> {
        self.rechannels(info.channels)?;
        if info.sample_rate != self.info.sample_rate {
            anyhow::bail!(
                "sample rate mismatch {} != {}",
                info.sample_rate,
                self.info.sample_rate,
            );
        }
        assert_eq!(self.info, info);
        Ok(())
    }
    /// Normalize to spec.
    pub fn normalize_both(
        &mut self,
        info: StreamInfo,
        resampler: &mut Option<Resampler>,
    ) -> anyhow::Result<()> {
        self.rechannels(info.channels)?;
        if info.sample_rate != self.info.sample_rate {
            if resampler.is_none() {
                *resampler = Some(Resampler::new(self.info, info, None));
            }
            let resampler = resampler.as_mut().unwrap();
            anyhow::ensure!(
                resampler.input_info.sample_rate == self.info.sample_rate,
                "cannot re-initialize resampler to a different setup"
            );
            resampler.process(self);
        }
        assert_eq!(self.info, info);
        Ok(())
    }

    /// Scale channels in place.
    fn rechannels(&mut self, channels: u16) -> io::Result<()> {
        if self.info.channels != channels {
            log::debug!(
                "rechannel {} -> {} for {} samples",
                self.info.channels,
                channels,
                self.samples.len()
            );
            match (self.info.channels, channels) {
                (1, n) => {
                    // Mono => n channel.
                    let new_samples = self
                        .samples
                        .drain(..)
                        .flat_map(|v| std::iter::repeat(v).take(n as _))
                        .collect();
                    self.samples = new_samples;
                }
                (n, 1) => {
                    // n channel => Mono.
                    assert_eq!(self.samples.len() % (self.info.channels as usize), 0);
                    let new_samples = self
                        .samples
                        .chunks(n as _)
                        .map(|s| s.iter().sum::<f32>() / (n as f32))
                        .collect();
                    self.samples = new_samples;
                }
                (a, b) => {
                    let msg = format!("cannot remix {} channels to {} channels", a, b);
                    return Err(io::Error::new(io::ErrorKind::InvalidData, msg));
                }
            }
            self.info.channels = channels;
        }
        Ok(())
    }
}

impl Mixer {
    pub fn new() -> Self {
        Self {
            streams: Vec::new(),
        }
    }

    /// Pick ut a "suitable" output info.
    pub fn pick_output_info(&self) -> StreamInfo {
        match self.streams.first() {
            Some(s) => s.info,
            None => StreamInfo {
                channels: 2,
                sample_rate: 44100,
            },
        }
    }

    /// Try to mix some inputs to output.
    pub fn mix(&self, output_info: StreamInfo) -> anyhow::Result<Option<Samples>> {
        // How many samples can we take for each input channels?
        // Unit: (1 / MIXER_SAMPLE_SIZE) seconds.
        let duration = self
            .streams
            .iter()
            .map(|s| (s.buf.read().len() as u32) / s.info.sample_count_internal_batch_size())
            .min()
            .unwrap_or(0);
        let ended = self.streams.iter().any(|s| {
            s.ended.load(SeqCst)
                && s.buf.read().len() < (s.info.sample_count_internal_batch_size() as usize)
        });
        if ended {
            return Ok(None);
        }
        let mut mixed = Samples::new(output_info, Vec::new());
        if duration > 0 {
            log::debug!("mixer duration: {}", duration);
            for s in self.streams.iter() {
                // Take samples out from the beginning of the input buffer.
                let count = (duration * s.info.sample_count_internal_batch_size()) as usize;
                let mut samples = Samples::new(s.info, Vec::with_capacity(count));
                let mut stream = s.buf.write();
                for _ in 0..count {
                    samples.samples.push(stream.pop_front().unwrap());
                }
                drop(stream);
                // Mix it.
                mixed.mix(samples)?;
            }
            log::debug!("mixed: {}", &mixed);
        }
        Ok(Some(mixed))
    }

    /// Allocate an input stream buffer. The buffer should be written by the callsite.
    pub fn allocate_input_buffer(&mut self, info: StreamInfo) -> Arc<MixBuffer> {
        assert!(
            info.sample_rate % MIXER_SAMPLE_SIZE == 0,
            "sample rate {} is not a multiple of {}",
            info.sample_rate,
            MIXER_SAMPLE_SIZE
        );
        let buf = MixBuffer {
            info,
            // Buffer size: 3 seconds.
            buf: RwLock::new(VecDeque::with_capacity(info.sample_count_millis(3000))),
            ended: AtomicBool::new(false),
        };
        let buf = Arc::new(buf);
        self.streams.push(buf.clone());
        buf
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mixer_single_stream() {
        let mut mixer = Mixer::new();
        let info = StreamInfo {
            channels: 1,
            sample_rate: 8000,
        };
        let stream = mixer.allocate_input_buffer(info);

        assert_eq!(mixer.pick_output_info(), info);

        // Write some samples.
        let samples = Samples::new(info, repeat(&[1.0, -1.0], 160));
        stream.extend(samples);
        let mixed = mixer.mix(info).unwrap().unwrap();
        assert_eq!(mixed.samples, repeat(&[1.0, -1.0], 160));

        let mixed = mixer.mix(info).unwrap().unwrap();
        assert!(mixed.samples.is_empty());

        // Mono -> Stereo
        let samples = Samples::new(info, repeat(&[1.0, -1.0], 160));
        stream.extend(samples);
        let mixed = mixer
            .mix(StreamInfo {
                channels: 2,
                sample_rate: 8000,
            })
            .unwrap()
            .unwrap();
        assert_eq!(mixed.samples, repeat(&[1., 1., -1., -1.], 160));

        // Cannot mix streams with different sample rates.
        let samples = Samples::new(info, sin_wave(0.1, 160));
        stream.extend(samples);
        let mixed = mixer.mix(StreamInfo {
            channels: 1,
            sample_rate: 16000,
        });
        assert!(mixed.is_err());

        // End of stream.
        stream.end();
        let mixed = mixer.mix(info).unwrap();
        assert!(mixed.is_none());
    }

    fn repeat(seq: &[f32], n: usize) -> Vec<f32> {
        (0..n).flat_map(|_| seq.to_vec()).collect()
    }

    fn sin_wave(step: f32, n: usize) -> Vec<f32> {
        (0..n).map(|i| (step * (i as f32)).sin()).collect()
    }
}
