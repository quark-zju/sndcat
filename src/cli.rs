use crate::ast::Expr;
use crate::config;
use crate::input;
use crate::output;
use portaudio::PortAudio;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering::SeqCst;
use std::sync::Arc;
use thread_priority::ThreadPriority;

fn print_device_list(pa: &PortAudio, filter: Option<&str>) -> Result<(), portaudio::Error> {
    let ds = pa.devices()?;
    for d in ds {
        let (i, d) = d?;
        if let Some(filter) = filter {
            if !d.name.contains(filter) {
                continue;
            }
        }

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
            i.e. 10 means dev(10).

        mp3(path)
            MP3 stream of the given file path.
            Alias: path, if path ends with '.mp3'.
            i.e. a.mp3 means mp3('a.mp3').

        opus(path)
            OggOpus stream of the given file path.
            Alias: path, if path ends with '.opus'.
            i.e. a.opus means opus('a.opus').

        sin(freq)
            Sin wave with given frequency.

        silence()
            Generate silence stream. Useful to keep audio device busy.
            Alias: nul

        level(input, db)
            Adjust loudness. db: float +3 (2x louder), -3 (half louder).

        mix(input, input, ...)
            Mix multiple streams together.
            The mixed stream ends when one of the input stream ends.

        resample(input, rate, quality=4)
            Resample a stream. Max quality is 10.
            Note: if quality is too high and CPU cannot catch up, it might
            cause "output underflow" error!

        mono(input)
            Convert an input stream to Mono.

    Multiple inputs like '-i X -i Y' is equivalent to 'mix(X, Y)'.

    For endless streams (ex. dev, or sin(x)), press Ctrl+C to end the input.

OUTPUT:
    An expression specifying an output stream. Supported functions are:

        dev(i, max_channels=2)
            Audio output device with index i.
            Use 'sndcat list' to see device indexes.
            Alias: i, if i is an integer.
            i.e. 10 means dev(10).

        opus(path, samplerate=16000, channels=1, mode=audio)
            Encode into an OggOpus file at the given path.
            mode can be 'audio' or 'voip'.
            Alias: path, if path ends with '.opus'.
            i.e. a.opus means opus('a.opus').

        wav(path)
            Write a 16-bit PCM WAV file at the given path.
            If path exists, it will be appended, assuming
            channel and sample rate are the same.
            Alias: path, if path ends with '.wav'.
            i.e. a.wav means wav('a.wav').

        stats()
            Print statistics to stderr.
            Alias: -

        tcp16le(port, samplerate=16000, channels=1)
            Start a TCP server at 127.0.0.1:port. Provide a stream of raw
            16bit little endian integer samples. Useful as input for other
            programs. For example, it can be relatively easily read from
            Python, then integrate with some ML tools or services.

    Example:
        -o dev(10) -o stats() -o opus('1.opus', 24000, 2)

Other commands:

    sndcat list [name]  List devices.
    sndcat help         Print this message.

Environment variables:

    LOG             Debug logging (ex. info, debug, trace).
    SNDCAT_RESAMPLE_QUALITY
                    Default resampling quality (0-10, default: 4).
    SNDCAT_DECODE_BUFFER_MILLIS
                    Minimal decoder (mp3, opus) buffer size
                    (milliseconds, default: 5).
    SNDCAT_MAIN_THREAD_PRIORITY
                    Main thread priority (0-100, default: 80).
    SNDCAT_MAX_INPUT_CHANNELS
    SNDCAT_MAX_OUTPUT_CHANNELS
                    Maximum channels for devices. (1-64, default: 2).
"#;
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
            "list" | "--list" | "-l" | "/l" | "l" => {
                arg_index += 1;
                let filter = args.get(arg_index).map(|&s| s);
                print_device_list(&pa, filter)?;
                return Ok(0);
            }
            "help" | "--help" | "-h" | "/?" | "h" => {
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
            log::debug!("creating output: {}", expr);
            let expr = Expr::parse(&expr)?;
            let output = output::eval_output(&ctx, &expr)?;
            if let Some(hint) = output.sample_rate_hint {
                sample_rate_hint = Some(hint);
            }
            log::info!("output: {}", &output.name);
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
        log::debug!("creating input: {}", expr);
        let expr = Expr::parse(&expr)?;
        let input = input::eval_input(&ctx, &expr)?;
        log::info!("input: {}", &input.name);
        input
    };

    let running = Arc::new(AtomicBool::new(true));

    ctrlc::set_handler({
        let running = running.clone();
        move || {
            running.store(false, SeqCst);
        }
    })?;

    let _ = thread_priority::set_current_thread_priority(ThreadPriority::Specific(
        *config::MAIN_THREAD_PRIORITY,
    ));
    while let Some(samples) = (input.read)() {
        let samples = Arc::new(samples);
        for output in &mut outputs {
            output.writer.write(samples.clone())?;
        }
        if !running.load(SeqCst) {
            break;
        }
    }
    log::trace!("dropping inputs and outputs");
    drop(input);
    for output in &mut outputs {
        log::info!("closing output {}", &output.name);
        output.writer.close()?;
    }
    drop(outputs);
    log::trace!("dropped inputs and outputs");

    Ok(0)
}
