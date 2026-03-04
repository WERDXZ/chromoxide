use chrox::cli;
use clap::Parser;

fn main() -> Result<(), cli::Error> {
    let args = cli::Args::parse();
    cli::run(args)
}
