use crate::input::resample;
use crate::input::Input;
use crate::mixer::Mixer;
use std::thread;

pub fn mix(inputs: Vec<Input>) -> Input {
    let name = format!(
        "Mix[{}]",
        inputs
            .iter()
            .map(|i| i.name.as_str())
            .collect::<Vec<_>>()
            .join(", ")
    );
    let mut mixer = Mixer::new();
    let sample_rate = inputs.first().map(|i| i.info.sample_rate).unwrap_or(44100);
    for input in inputs {
        let mut input = resample::resample(input, sample_rate, None);
        let buf = mixer.allocate_input_buffer(input.info);
        thread::spawn({
            move || {
                while let Some(samples) = (input.read)() {
                    log::debug!("read {}: {}", &input.name, &samples);
                    buf.extend(samples);
                }
                buf.end();
            }
        });
    }
    let info = mixer.pick_output_info();
    let func = move || match mixer.mix(info) {
        Ok(Some(samples)) => Some(samples),
        Ok(None) => None,
        Err(_) => None,
    };
    Input {
        name,
        info,
        read: Box::new(func),
    }
}
