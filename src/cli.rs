use crate::ast::Expr;
use crate::input;
use crate::output;
use portaudio::PortAudio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;

fn print_device_list(pa: &PortAudio) -> Result<(), portaudio::Error> {
    let ds = pa.devices()?;
    for d in ds {
        let (i, d) = d?;

        let ty = match (d.max_input_channels, d.max_output_channels) {
            (0, 0) => "NUL",
            (0, _) => "OUT",
            (_, 0) => "IN ",
            (_, _) => "   ",
        };
        let name = d.name.replace('\n', " ").replace('\r', "");
        println!(
            "{:2} {} {} (In {}, Out {}, {}Hz)",
            i.0,
            ty,
            &name,
            d.max_input_channels,
            d.max_output_channels,
            d.default_sample_rate as u32,
        );
    }
    Ok(())
}

fn print_help() {
    let help = r#"sndcat

Convert a stream of input audios to specified outputs.

    sndcat -i INPUT [-i INPUT ...] -o OUTPUT [-o OUTPUT ...]

INPUT:
    An expression specifying an input stream. Supported functions are:

        dev(i)
            Audio input device with index i.
            Use 'sndcat list' to see device indexes.
            Alias: i, if i is an integer.

        mp3(path)
            MP3 stream of the given file path.
            Alias: path, if path ends with '.mp3'.

        opus(path)
            OggOpus stream of the given file path.
            Alias: path, if path ends with '.opus'.

        sin(freq)
            Sin wave with given frequency.

        silence()
            Generate silence stream. Useful to keep audio device busy.
            Alias: nul

        mix(input, input, ...)
            Mix multiple streams together.
            The mixed stream ends when one of the input stream ends.

        resample(input, rate, quality=4)
            Resample a stream. Max quality is 10.
            Note: if quality is too high and CPU cannot catch up, it might
            cause "output underflow" error!

    Multiple inputs like '-i X -i Y' is equivalent to 'mix(X, Y)'.

    For endless streams (ex. dev, or sin(x)), press Ctrl+C to end the input.

OUTPUT:
    An expression specifying an output stream. Supported functions are:

        dev(i)
            Audio output device with index i.
            Use 'sndcat list' to see device indexes.
            Alias: i, if i is an integer.

        opus(path, samplerate=16000, channels=1, mode=audio)
            Encode into an OggOpus file at the given path.
            mode can be 'audio' or 'voip'.
            Alias: path, if path ends with '.opus'.

        stats()
            Print statistics to stderr.
            Alias: -

    Example:
        -o dev(10) -o stats() -o opus('1.opus', 24000, 2)

Other commands:

    sndcat list     List devices.
    sndcat help     Print this message."#;
    println!("{}", help);
}

pub fn run(args: &[&str]) -> anyhow::Result<i32> {
    if args.is_empty() {
        print_help();
        return Ok(0);
    }

    let pa = PortAudio::new()?;
    log::debug!("PortAudio initialized: {}", pa.version_text()?);

    let mut input_args: Vec<&str> = Vec::new();
    let mut output_args: Vec<&str> = Vec::new();
    let mut arg_index = 0;
    while arg_index < args.len() {
        let arg = args[arg_index];
        match arg {
            "-i" => {
                arg_index += 1;
                if let Some(&a) = args.get(arg_index) {
                    input_args.push(a);
                }
            }
            "-o" => {
                arg_index += 1;
                if let Some(&a) = args.get(arg_index) {
                    output_args.push(a);
                }
            }
            "list" => {
                print_device_list(&pa)?;
                return Ok(0);
            }
            "help" => {
                print_help();
                return Ok(0);
            }
            _ => {
                eprintln!("unknown flag: {}", arg);
                return Ok(255);
            }
        }
        arg_index += 1;
    }

    let mut sample_rate_hint = None;
    let mut outputs = {
        let mut outputs = Vec::new();
        let ctx = output::EvalContext { pa: &pa };
        for expr in output_args {
            log::info!("creating output: {}", expr);
            let expr = Expr::parse(&expr)?;
            let output = output::eval_output(&ctx, &expr)?;
            if let Some(hint) = output.sample_rate_hint {
                sample_rate_hint = Some(hint);
            }
            outputs.push(output);
        }
        outputs
    };

    let mut input = {
        let expr = match input_args.len() {
            1 => input_args[0].to_string(),
            _ => format!("mix({})", input_args.join(",")),
        };
        let ctx = input::EvalContext {
            pa: &pa,
            sample_rate_hint,
        };
        log::info!("creating input: {}", expr);
        let expr = Expr::parse(&expr)?;
        input::eval_input(&ctx, &expr)?
    };

    let running = Arc::new(AtomicBool::new(true));

    ctrlc::set_handler({
        let running = running.clone();
        move || {
            running.store(false, SeqCst);
        }
    })?;

    while let Some(samples) = (input.read)() {
        let samples = Arc::new(samples);
        for output in &mut outputs {
            output.writer.write(samples.clone())?;
        }
        if !running.load(SeqCst) {
            break;
        }
    }
    log::trace!("Dropping inputs and outputs");
    drop(input);
    for output in &mut outputs {
        log::info!("Closing output {}", &output.name);
        output.writer.close()?;
    }
    drop(outputs);
    log::trace!("Dropped inputs and outputs");

    Ok(0)
}
