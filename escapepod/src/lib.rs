use std::path::PathBuf;

use clap::Parser;
use escapepod_common::nix::sys::signal::Signal;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Args {
    /// signals to escape on
    #[arg(long)]
    pub signal: Vec<Signal>,
    /// port to listen for pods on
    /// command to run when escape signal is received
    #[arg(long)]
    pub launch_pod_command: String,
    #[arg(long)]
    pub port: u16,
    /// sync files under path
    #[arg(long)]
    pub path: Vec<PathBuf>,
    /// child command to exec
    pub exec: Vec<String>,
}
