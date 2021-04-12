use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use crate::output::Output;
use crate::output::OutputWriter;
use crate::resample::Resampler;
use parking_lot::Mutex;
use std::io::Write;
use std::net::TcpListener;
use std::net::TcpStream;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::SyncSender;
use std::sync::mpsc::TrySendError;
use std::sync::Arc;
use std::thread;

pub fn tcp_i16_server(port: u16, info: StreamInfo) -> anyhow::Result<Output> {
    let name = format!("Tcp[127.0.0.1:{}, {}]", port, &info);

    let listener = TcpListener::bind(format!("127.0.0.1:{}", port))?;
    let channels: Senders = Default::default();
    log::info!("tcp i16 listening: 127.0.0.1:{} ({})", port, &info);

    thread::Builder::new()
        .name(format!("TcpServer {}", port))
        .spawn({
            let channels = Arc::clone(&channels);
            move || -> anyhow::Result<()> {
                for stream in listener.incoming() {
                    let (sender, receiver) = sync_channel::<Arc<Vec<u8>>>(50);
                    channels.lock().push(sender);
                    handle_client(stream?, receiver);
                }
                Ok(())
            }
        })?;

    let service = TcpService {
        info,
        resampler: None,
        channels,
    };

    Ok(Output {
        name,
        sample_rate_hint: Some(info.sample_rate),
        writer: Box::new(service),
    })
}

fn handle_client(mut stream: TcpStream, receiver: Receiver<Arc<Vec<u8>>>) {
    log::info!("client connected: {:?}", stream.peer_addr());
    let _ = thread::Builder::new()
        .name(format!("TcpClient {:?}", stream.peer_addr()))
        .spawn(move || {
            while let Ok(data) = receiver.recv() {
                if stream.write_all(&data).is_err() {
                    break;
                }
            }
        });
}

struct TcpService {
    info: StreamInfo,
    resampler: Option<Resampler>,
    channels: Senders,
}

type Senders = Arc<Mutex<Vec<SyncSender<Arc<Vec<u8>>>>>>;

impl OutputWriter for TcpService {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        let samples = crate::mixer::normalize_cow(samples, self.info, &mut self.resampler)?;
        // Convert samples to i16. Write as i16LE.
        let mut out = Vec::<u8>::with_capacity(samples.samples.len() * 2);
        for &f in &samples.samples {
            let v = (f * 30000.0) as i16;
            out.extend_from_slice(&v.to_le_bytes());
        }
        let out = Arc::new(out);
        let mut new_senders = Vec::new();
        for sender in self.channels.lock().drain(..) {
            // Just drop samples if the client cannot catch up.
            match sender.try_send(out.clone()) {
                Err(TrySendError::Full(_)) | Ok(_) => {
                    new_senders.push(sender);
                }
                Err(TrySendError::Disconnected(_)) => {
                    log::info!("client disconnected");
                }
            }
        }
        self.channels.lock().extend(new_senders);
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        // Wait for clients?
        Ok(())
    }
}
