use crate::mixer::Samples;
use crate::output::Output;
use crate::output::OutputWriter;
use std::sync::mpsc::sync_channel;
use std::sync::mpsc::SyncSender;
use std::sync::Arc;
use std::thread;
use thread_priority::ThreadPriority;

/// Put output loop in a thread.
pub fn background(
    mut output: Output,
    priority: Option<ThreadPriority>,
    backlog: usize,
) -> anyhow::Result<Output> {
    let (sender, receiver) = sync_channel(backlog);
    let name = format!("Background[{}]", &output.name);
    let sample_rate_hint = output.sample_rate_hint;
    let handle = thread::Builder::new()
        .name(format!("Output: {}", &output.name))
        .spawn(move || -> anyhow::Result<()> {
            if let Some(priority) = priority {
                let _ = thread_priority::set_current_thread_priority(priority);
            }
            while let Ok(samples) = receiver.recv() {
                output.writer.write(samples)?;
            }
            output.writer.close()?;
            Ok(())
        })?;

    let background = Background {
        sender: Some(sender),
        handle: Some(handle),
    };
    Ok(Output {
        name,
        sample_rate_hint,
        writer: Box::new(background),
    })
}

struct Background {
    sender: Option<SyncSender<Arc<Samples>>>,
    handle: Option<thread::JoinHandle<anyhow::Result<()>>>,
}
impl OutputWriter for Background {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()> {
        if let Some(sender) = self.sender.as_mut() {
            sender.send(samples)?;
        }
        Ok(())
    }

    fn close(&mut self) -> anyhow::Result<()> {
        // Drop sender to notify remote.
        self.sender = None;
        if let Some(handle) = self.handle.take() {
            let close_result = handle.join().map_err(|e| anyhow::format_err!("{:?}", e))?;
            close_result?;
        }
        Ok(())
    }
}
