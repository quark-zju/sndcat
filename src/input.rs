use crate::ast::Expr;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use portaudio::PortAudio;

mod dev;
mod gen;
mod mix;
mod mp3;
mod opus;
mod resample;

pub struct Input {
    pub name: String,
    pub info: StreamInfo,
    pub read: Box<dyn (FnMut() -> Option<Samples>) + Send + 'static>,
}

#[derive(Clone)]
pub struct EvalContext<'a> {
    pub pa: &'a PortAudio,
    pub sample_rate_hint: Option<u32>,
}

pub fn eval_input(ctx: &EvalContext, expr: &Expr) -> anyhow::Result<Input> {
    match expr {
        Expr::Name(name) => {
            // Syntax sugar.
            if name.ends_with(".mp3") {
                // a.mp3 => mp3(a.mp3)
                eval_input(ctx, &Expr::Fn("mp3".into(), vec![Expr::Name(name.clone())]))
            } else if name.ends_with(".opus") {
                // a.opus => opus(a.opus)
                eval_input(
                    ctx,
                    &Expr::Fn("opus".into(), vec![Expr::Name(name.clone())]),
                )
            } else if name.parse::<u16>().is_ok() {
                // 42 => dev(42)
                eval_input(ctx, &Expr::Fn("dev".into(), vec![Expr::Name(name.clone())]))
            } else if name.to_ascii_lowercase() == "nul" {
                // nul => silence()
                eval_input(ctx, &Expr::Fn("silence".into(), vec![]))
            } else {
                anyhow::bail!("unknown input: {}", name);
            }
        }
        Expr::Fn(name, args) => match name.as_ref() {
            "dev" => match &args[..] {
                [Expr::Name(i)] if i.parse::<u32>().is_ok() => {
                    let i = i.parse::<u32>()?;
                    Ok(dev::input_device(ctx.pa, i)?)
                }
                _ => anyhow::bail!("unknown args: {:?}", args),
            },
            "sin" => {
                let params = match &args[..] {
                    [Expr::Name(s)] => s,
                    _ => anyhow::bail!("unknown args: {:?}", args),
                };
                gen::sin_wave(ctx.sample_rate_hint, params)
            }
            "silence" => gen::silence(ctx.sample_rate_hint),
            "mix" => {
                let mut inputs = Vec::with_capacity(args.len());
                let mut ctx = ctx.clone();
                let mut first = true;
                for expr in args {
                    let input = eval_input(&ctx, expr)?;
                    if first {
                        ctx.sample_rate_hint = Some(input.info.sample_rate);
                        first = false;
                    }
                    inputs.push(input);
                }
                Ok(mix::mix(inputs))
            }
            "resample" => {
                anyhow::ensure!(args.len() >= 2);
                let sample_rate = args[1].to_string().parse::<u32>()?;
                let quality = if let Some(arg) = args.get(2) {
                    arg.to_string().parse::<usize>().ok()
                } else {
                    None
                };
                let mut ctx = ctx.clone();
                ctx.sample_rate_hint = Some(sample_rate);
                let input = eval_input(&ctx, &args[0])?;
                Ok(resample::resample(input, sample_rate, quality))
            }
            "mp3" => {
                anyhow::ensure!(args.len() >= 1);
                let path = args[0].to_string();
                mp3::mp3(&path)
            }
            "opus" => {
                anyhow::ensure!(args.len() >= 1);
                let path = args[0].to_string();
                opus::opus(&path)
            }
            _ => anyhow::bail!("unknown function: {}", name),
        },
    }
}
