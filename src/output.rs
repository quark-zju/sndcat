use crate::ast::Expr;
use crate::mixer::Samples;
use crate::mixer::StreamInfo;
use portaudio::PortAudio;
use std::sync::Arc;

mod background;
mod dev;
mod opus;
mod tcp;
mod terminal;

pub trait OutputWriter: Send + 'static {
    fn write(&mut self, samples: Arc<Samples>) -> anyhow::Result<()>;
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
            "dev" => {
                anyhow::ensure!(args.len() > 0);
                let i = args[0].to_i64()? as u32;
                let max_channels = match args.get(1) {
                    Some(a) => Some(a.to_i64()? as i32),
                    None => None,
                };
                let output = dev::output_device(ctx.pa, i, max_channels)?;
                // Not moving to background. Want the blocking behavior.
                Ok(output)
            }
            "opus" => {
                anyhow::ensure!(args.len() > 0);
                let path = args[0].to_string();
                let sample_rate = match args.get(1) {
                    Some(a) => a.to_i64()? as u32,
                    None => 16000,
                };
                let channels = match args.get(2) {
                    Some(a) => a.to_i64()? as u16,
                    None => 1,
                };
                let mode = args.get(3).and_then(|a| a.to_str().ok()).unwrap_or("audio");
                let info = StreamInfo {
                    sample_rate,
                    channels,
                };
                let output = opus::opus(&path, info, &mode)?;
                // Move to background so it does not block main thread.
                let output = background::background(output, None, 5)?;
                Ok(output)
            }
            "tcp16le" => {
                anyhow::ensure!(args.len() > 0);
                let port: u16 = args[0].to_i64()? as u16;
                let sample_rate = match args.get(1) {
                    Some(a) => a.to_i64()? as u32,
                    None => 16000,
                };
                let channels = match args.get(2) {
                    Some(a) => a.to_i64()? as u16,
                    None => 1,
                };
                let info = StreamInfo {
                    sample_rate,
                    channels,
                };
                let output = tcp::tcp_i16_server(port, info)?;
                Ok(output)
            }
            "stats" => {
                let output = terminal::stats();
                // Move to background so it does not block main thread.
                let output = background::background(output, None, 1)?;
                Ok(output)
            }
            _ => anyhow::bail!("unknown function: {}", name),
        },
    }
}
