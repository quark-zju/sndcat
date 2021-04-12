use crate::input::Input;
use crate::mixer::Samples;

pub fn level(mut input: Input, db: f32) -> Input {
    let name = format!("Level[{}, {:+0.3}db]", &input.name, db,);
    let info = input.info;
    if db == 0.0 {
        return input;
    }

    let func = move || -> Option<Samples> {
        let mut samples = (input.read)();
        if let Some(samples) = samples.as_mut() {
            samples.adjust_level(db);
        }
        samples
    };

    Input {
        name,
        info,
        read: Box::new(func),
    }
}
