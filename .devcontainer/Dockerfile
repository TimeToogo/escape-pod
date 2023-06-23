FROM mcr.microsoft.com/devcontainers/rust:latest

RUN DEBIAN_FRONTEND=noninteractive \
    apt update && \
    apt install -y gdb procps

WORKDIR /app
ENTRYPOINT ["bash"]
