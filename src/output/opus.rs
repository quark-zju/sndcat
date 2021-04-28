use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use anyhow::Context;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use std::sync::Arc;

pub fn opus(path: &str, info: StreamInfo, mode: &str) -> anyhow::Result<Output> {
    use audiopus::Application;
    use audiopus::Channels;
    use audiopus::SampleRate;
    let name = format!("Opus[{}]", path);

    log::debug!("creating opus encoder");
    let encoder = {
        log::trace!("preparing parameters");
        let sample_rate = match info.sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            n => anyhow::bail!(
                "invalid sample rate or opus: {} (valid choices are 8k, 12k, 16k, 24k, 48k)",
                n
            ),
        };
        let channels = match info.channels {
            2 => Channels::Stereo,
            1 => Channels::Mono,
            n => anyhow::bail!("invalid channel for opus: {}", n),
        };
        let mode = match mode {
            "voip" => Application::Voip,
            "audio" => Application::Audio,
            s => anyhow::bail!("invalid mode for opus: {}", s),
        };
        audiopus::coder::Encoder::new(sample_rate, channels, mode)?
    };
    log::info!(
        "opus encoder created, vbr: {}, bitrate: {:?}",
        encoder.vbr()?,
        encoder.bitrate()?,
    );

    let writer = {
        log::debug!("opening file {}", path);
        rename_existing_best_effort(&Path::new(path))?;
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .truncate(true)
            .open(path)
            .context(path.to_string())?;
        let file = std::io::BufWriter::with_capacity(200000, file);
        OggWriter::new(file, info, encoder)?
    };

    let output = Output {
        name,
        sample_rate_hint: None,
        writer: Box::new(writer),
    };
    Ok(output)
}

struct OggWriter<T: std::io::Write> {
    ogg_file: ogg::writing::PacketWriter<T>,
    encoder: audiopus::coder::Encoder,
    info: StreamInfo,

    absgp: u64,
    serial: u32,
    resampler: Option<crate::resample::Resampler>,

    // > opus_encode_float:
    // The passed frame_size must an opus frame size for the encoder's sampling rate. For example,
    // at 48kHz the permitted values are 120, 240, 480, or 960. Passing in a duration of less than
    // 10ms (480 samples at 48kHz) will prevent the encoder from using the LPC or hybrid modes.
    //
    // Opus can encode frames of 2.5, 5, 10, 20, 40, or 60 ms. It can also combine multiple frames into packets of up to 120 ms.
    // Opus uses a 20 ms frame size by default, as it gives a decent mix of low latency and good quality.
    in_buf: VecDeque<f32>,
    out_buf: Vec<u8>,
}

impl<T: std::io::Write + Send + 'static> OggWriter<T> {
    fn new(write: T, info: StreamInfo, encoder: audiopus::coder::Encoder) -> anyhow::Result<Self> {
        // ogg framing format. https://www.xiph.org/vorbis/doc/framing.html
        // absgp: absolute granule position
        let mut ogg_file = ogg::writing::PacketWriter::new(write);
        let serial = 0;
        let absgp = 0;
        let in_buf = VecDeque::with_capacity(info.sample_rate as usize);
        let out_buf = vec![0; (info.sample_rate as usize) * 5];

        // Write OggOpus header.
        let header = crate::oggopus::Header { info };

        // First 2 headers must be in 2 pages (each page only has 1 packet).
        let inf = ogg::writing::PacketWriteEndInfo::EndPage;
        ogg_file.write_packet(header.serialize_head()?.into_boxed_slice(), serial, inf, 0)?;
        ogg_file.write_packet(header.serialize_tags()?.into_boxed_slice(), serial, inf, 0)?;

        let writer = Self {
            ogg_file,
            encoder,
            info,
            absgp,
            serial,
            resampler: None,
            in_buf,
            out_buf,
        };
        Ok(writer)
    }

    fn encode_packet(
        &mut self,
        millis: usize,
        reserve_millis: usize,
    ) -> anyhow::Result<Option<Box<[u8]>>> {
        let n = self.info.sample_count_millis(millis);
        let reserve_n = self.info.sample_count_millis(reserve_millis);
        if self.in_buf.len() >= n + reserve_n {
            let buf: Vec<f32> = (0..n).map(|_| self.in_buf.pop_front().unwrap()).collect();
            let samples = Samples::new(self.info, buf);
            log::debug!("opus encoding {}", &samples);
            let len = self
                .encoder
                .encode_float(&samples.samples, &mut self.out_buf)?;
            log::debug!("opus encoding into {} bytes", &len);
            let boxed = self.out_buf[0..len].to_vec().into_boxed_slice();
            log::trace!("opus packet: {:?}", &boxed);
            // From https://wiki.xiph.org/OggOpus#Granule_Position:
            // The granule position of an audio page is in units of PCM audio samples at a fixed
            // rate of 48 kHz (per channel; a stereo stream’s granule position does not increment
            // at twice the speed of a mono stream). It is possible to run a decoder at other
            // sampling rates, but the format and this specification always count samples assuming
            // a 48 kHz decoding rate.
            self.absgp += 48000 / 1000 * (millis as u64);
            Ok(Some(boxed))
        } else {
            Ok(None)
        }
    }
}

impl<T: std::io::Write + Send + 'static> OutputWriter for OggWriter<T> {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        let samples = crate::mixer::normalize_cow(samples, self.info, &mut self.resampler)?;
        assert_eq!(self.info, samples.info);
        self.in_buf.extend(samples.samples.iter().cloned());

        // Feed 120ms data.
        while let Some(encoded) = self.encode_packet(120, 20)? {
            self.ogg_file.write_packet(
                encoded,
                self.serial,
                ogg::writing::PacketWriteEndInfo::NormalPacket,
                self.absgp as _,
            )?;
        }

        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        // TODO: Padding?
        let mut packets = Vec::new(); // (encoded, absgp)

        // Feed 20ms data.
        while let Some(encoded) = self.encode_packet(20, 0)? {
            packets.push((encoded, self.absgp));
        }
        anyhow::ensure!(packets.len() > 0);

        // Write output.
        let len = packets.len();
        for (i, (encoded, absgp)) in packets.into_iter().enumerate() {
            let inf = if i + 1 == len {
                ogg::writing::PacketWriteEndInfo::EndStream
            } else {
                ogg::writing::PacketWriteEndInfo::NormalPacket
            };
            self.ogg_file
                .write_packet(encoded, self.serial, inf, absgp)?;
        }
        self.ogg_file.inner_mut().flush()?;
        log::debug!("Flushed OGG file");
        Ok(())
    }
}

/// Rename existing files to avoid overwriting files accidentally.
///
/// This is a simple implementation and could be racy.
fn rename_existing_best_effort(path: &Path) -> std::io::Result<()> {
    if path.exists() {
        let ext = path.extension().unwrap_or_default();
        let without_ext = path.with_extension("").display().to_string();
        for i in 1usize.. {
            let new_path_str = format!("{}-{}", &without_ext, i);
            let new_path = Path::new(&new_path_str).with_extension(ext);
            if new_path.exists() {
                continue;
            }
            return fs::rename(path, new_path);
        }
    }
    Ok(())
}
