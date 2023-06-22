use std::ffi::CString;

use clap::Parser;
use escapepod::Args;
use nix::unistd::{execvp, fork, ForkResult};

fn main() {
    let args = Args::parse();

    assert!(args.exec.len() > 0);

    unsafe {
        match fork().expect("failed to fork") {
            ForkResult::Parent { child } => {
                println!("child: {child:?}");
            }
            ForkResult::Child => {
                let exec = args
                    .exec
                    .iter()
                    .map(|i| CString::new(i.as_bytes().to_vec()).unwrap())
                    .collect::<Vec<_>>();
                execvp(exec[0].as_c_str(), &exec[..]).expect("failed to exec");
                unreachable!()
            }
        }
    }
}
