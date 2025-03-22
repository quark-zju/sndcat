#![allow(dead_code)]

mod ast;
mod cli;
mod config;
pub(crate) mod dev;
mod input;
mod mixer;
mod oggopus;
mod output;
mod parser;
mod resample;

fn main() {
    env_logger::Builder::from_env("LOG")
        .format_timestamp_millis()
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

    let code = match cli::run(&args) {
        Err(e) => {
            let s = format!("{}", &e);
            let d = format!("{:?}", &e);
            if d == s {
                eprintln!("Error: {}", &s);
            } else {
                eprintln!("Error: {}\nDetail: {}", &s, &d);
            }
            255
        }
        Ok(code) => code,
    };
    std::process::exit(code);
}
