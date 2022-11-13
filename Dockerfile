# Base rust image (configured with rust_toolchain)
FROM debian:bullseye-slim AS rust_base
COPY rust-toolchain /
RUN apt-get update && \
    apt-get install -y --no-install-recommends ca-certificates libssl1.1 libssl-dev pkg-config libudev-dev curl build-essential gcc gcc-multilib git openssh-client libgmp-dev && \
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o /rustup.sh && chmod +x /rustup.sh && \
    /rustup.sh --default-toolchain `cat rust-toolchain` -y && \
    ln -s /root/.cargo/bin/cargo /usr/bin/

WORKDIR /app
RUN cargo install cargo-chef --locked

##
# Prepare dependency file
##
FROM rust_base as planner
COPY . /app
RUN cargo chef prepare --recipe-path recipe.json

##
# Install dependencies and build applications
##
FROM rust_base as builder

# Build dependencies - this is the caching Docker layer!
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Build actual executables
COPY . /app
RUN cargo build --release --bin relay --bin manager

##
# Prepare executables
##
FROM debian:bullseye-slim

ENV TZ=Etc/UTC
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=3000

USER root
RUN useradd -u 1000 -s /bin/bash -M -d /app monal && \
    \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    tzdata ca-certificates netcat-openbsd libgmp-dev && \
    \
    rm -rf /var/lib/apt/lists/* && rm -rf /var/lib/apt/lists.d/* && apt-get autoremove -y && apt-get clean && apt-get autoclean

WORKDIR /app
COPY --from=builder /app/target/release/manager /app/target/release/relay /app/
COPY ./bin/*.sh /app/bin/

RUN chown -R 1000:1000 /app && \
    chmod +x /app/bin/*

USER 1000
EXPOSE 3000

HEALTHCHECK --interval=15s CMD /app/bin/liveliness.sh

CMD ["/app/manager"]
