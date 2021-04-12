use crate::ast::Expr;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use portaudio::PortAudio;

mod dev;
mod opus;
mod terminal;

pub trait OutputWriter: Send + 'static {
    fn write(&mut self, samples: &Samples) -> anyhow::Result<()>;
    fn close(&mut self) -> anyhow::Result<()>;
}

pub struct Output {
    pub name: String,
    pub sample_rate_hint: Option<u32>,
    pub writer: Box<dyn OutputWriter>,
}

pub struct EvalContext<'a> {
    pub pa: &'a PortAudio,
}

pub fn eval_output(ctx: &EvalContext, expr: &Expr) -> anyhow::Result<Output> {
    match expr {
        Expr::Name(name) => {
            // Syntax sugar.
            if name.ends_with(".opus") {
                // a.opus => opus(a.opus)
                eval_output(
                    ctx,
                    &Expr::Fn("opus".into(), vec![Expr::Name(name.clone())]),
                )
            } else if name == "-" {
                // - => stats()
                eval_output(ctx, &Expr::Fn("stats".into(), vec![]))
            } else if name.parse::<u16>().is_ok() {
                // 42 => dev(42)
                eval_output(ctx, &Expr::Fn("dev".into(), vec![Expr::Name(name.clone())]))
            } else {
                anyhow::bail!("unknown output: {}", name)
            }
        }
        Expr::Fn(name, args) => match name.as_ref() {
            "dev" => match &args[..] {
                [Expr::Name(i)] if i.parse::<u32>().is_ok() => {
                    let i = i.parse::<u32>()?;
                    Ok(dev::output_device(ctx.pa, i)?)
                }
                _ => anyhow::bail!("unknown args: {:?}", args),
            },
            "opus" => {
                anyhow::ensure!(args.len() > 0);
                let path = args[0].to_string();
                let sample_rate = match args.get(1) {
                    Some(a) => a.to_string().parse()?,
                    None => 16000,
                };
                let channels = match args.get(2) {
                    Some(a) => a.to_string().parse()?,
                    None => 1,
                };
                let mode = args
                    .get(3)
                    .map(|a| a.to_string())
                    .unwrap_or_else(|| "audio".to_string());
                let info = StreamInfo {
                    sample_rate,
                    channels,
                };
                opus::opus(&path, info, &mode)
            }
            "stats" => Ok(terminal::stats()),
            _ => anyhow::bail!("unknown function: {}", name),
        },
    }
}
