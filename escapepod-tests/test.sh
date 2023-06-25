# we have this script until https://rust-lang.github.io/rfcs/3028-cargo-binary-dependencies.html is stablised
cargo build -p escapepod -p escapepod-restore && cargo test $@
