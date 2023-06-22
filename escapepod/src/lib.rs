use std::path::PathBuf;

use clap::Parser;
use nix::sys::signal::Signal;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// signals to escape on
    #[arg(long)]
    pub signal: Vec<Signal>,
    /// sync files under path
    #[arg(long)]
    pub path: Vec<PathBuf>,
    /// child command to exec
    pub exec: Vec<String>,
}
