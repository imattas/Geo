use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(name = "geo")]
#[command(about = "The Geo programming language compiler")]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    Check {
        input: PathBuf,
        #[arg(long)]
        target: Option<String>,
    },
    EmitAsm {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        target: Option<String>,
    },
    EmitObj {
        input: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
        #[arg(long)]
        target: Option<String>,
    },
    Build {
        input: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        target: Option<String>,
    },
    Run {
        input: PathBuf,
        #[arg(long)]
        target: Option<String>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    Fmt {
        input: PathBuf,
    },
    Test {
        path: PathBuf,
    },
}
