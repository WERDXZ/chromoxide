use chrox::cli;
use clap::Parser;
use std::error::Error as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args = cli::Args::parse();
    match cli::run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err}");
            let mut source = err.source();
            while let Some(cause) = source {
                eprintln!("  caused by: {cause}");
                source = cause.source();
            }
            ExitCode::FAILURE
        }
    }
}
