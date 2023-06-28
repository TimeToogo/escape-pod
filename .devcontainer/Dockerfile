FROM mcr.microsoft.com/devcontainers/rust:latest

RUN DEBIAN_FRONTEND=noninteractive \
    apt update && \
    apt install -y gdb procps

RUN cargo install cargo-show-asm

WORKDIR /app
ENTRYPOINT ["bash"]
