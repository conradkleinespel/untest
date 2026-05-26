mod commands;

use clap::Parser;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand, Debug)]
enum Commands {
    /// Strip tests from a directory
    Strip(StripArgs),
}

#[derive(Parser, Debug)]
pub struct StripArgs {
    /// Input directory
    pub input: PathBuf,

    /// Output directory
    pub output: PathBuf,

    /// Paths to exclude (relative to input directory or absolute)
    #[arg(short, long)]
    pub exclude: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Strip(args) => commands::strip::run(args),
    }
}
