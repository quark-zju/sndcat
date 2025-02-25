use super::Output;
use super::OutputWriter;
use crate::mixer::StreamInfo;
use anyhow::Context;
use std::io::BufWriter;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write as _;

/// 16-bit PCM WAV output.
pub fn wav(path: &str) -> anyhow::Result<Output> {
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .context(path.to_string())?;
    let out = BufWriter::with_capacity(512000, file);
    let wav = WavWriter {
        header_written: false,
        out: Some(out),
    };
    Ok(Output {
        name: format!("Wav[{}]", path),
        sample_rate_hint: None,
        writer: Box::new(wav),
    })
}

struct WavWriter {
    header_written: bool,
    out: Option<BufWriter<std::fs::File>>,
}

impl WavWriter {
    fn write_header(&mut self, info: StreamInfo) -> anyhow::Result<()> {
        if self.header_written {
            return Ok(());
        }
        if let Some(out) = self.out.as_mut() {
            // Master RIFF chunk.
            out.write_all(b"RIFF")?;
            out.write_all(&0u32.to_le_bytes())?; // Placeholder for the total size.
            out.write_all(b"WAVE")?;

            // Chunk describing the data format.
            out.write_all(b"fmt ")?;
            out.write_all(&16u32.to_le_bytes())?; // Size of the fmt chunk (this chunk).
            out.write_all(&1u16.to_le_bytes())?; // PCM integer (i16).
            out.write_all(&(info.channels as u16).to_le_bytes())?; // Number of channels.
            out.write_all(&(info.sample_rate as u32).to_le_bytes())?; // Sample rate (in Hz).
            out.write_all(&(info.sample_rate as u32 * info.channels as u32 * 2).to_le_bytes())?; // BytePerSec. 2: i16.
            out.write_all(&(info.channels as u16 * 2).to_le_bytes())?; // BytePerBloc (channels * bytes per sample).
            out.write_all(&16u16.to_le_bytes())?; // Bits per sample.

            // Chunk containing the sampled data.
            out.write_all(b"data")?;
            out.write_all(&0u32.to_le_bytes())?; // Placeholder for the total size.
        }
        self.header_written = true;
        Ok(())
    }
}

impl OutputWriter for WavWriter {
    fn write(&mut self, samples: std::sync::Arc<crate::mixer::Samples>) -> anyhow::Result<()> {
        self.write_header(samples.info)?;
        for &f in &samples.samples {
            let v = (f * 32767.0) as i16;
            if let Some(out) = self.out.as_mut() {
                out.write_all(&v.to_le_bytes())?;
            }
        }
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        if self.header_written {
            if let Some(out) = self.out.take() {
                // Fill the placeholders.
                let mut file = out.into_inner()?;
                let total_size = file.seek(SeekFrom::Current(0))? as u32;
                let riff_file_size = total_size - 8;
                file.seek(SeekFrom::Start(4))?;
                file.write_all(&riff_file_size.to_le_bytes())?;
                let data_chunk_size = total_size - 44;
                file.seek(SeekFrom::Start(40))?;
                file.write_all(&data_chunk_size.to_le_bytes())?;
                file.flush()?;
            }
        }
        Ok(())
    }
}
