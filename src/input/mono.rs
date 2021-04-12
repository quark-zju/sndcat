use crate::input::Input;
use crate::mixer::Samples;
use std::sync::mpsc::sync_channel;
use std::thread;

pub fn mono(mut input: Input) -> Input {
    if input.info.channels == 1 {
        return input;
    }

    let name = format!("Mono[{}]", &input.name);
    let mut info = input.info;
    info.channels = 1;
    let (sender, receiver) = sync_channel(10);
    thread::spawn(move || {
        while let Some(mut samples) = (input.read)() {
            if samples.normalize_channels(info).is_err() {
                break;
            }
            let _ = sender.send(samples);
        }
    });
    let func = move || -> Option<Samples> { receiver.recv().ok() };
    Input {
        name,
        info,
        read: Box::new(func),
    }
}
