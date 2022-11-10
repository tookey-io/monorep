FROM rust:1.65 as builder

WORKDIR /app

COPY . /app/
RUN cargo build --release

CMD ["/app/target/release/manager"]

##
# Prepare environment
##

FROM debian:buster-slim

ENV TZ=Etc/UTC
ENV ROCKET_ADDRESS=0.0.0.0
ENV ROCKET_PORT=3000

USER root
RUN useradd -u 1000 -s /bin/bash -M -d /app monal && \
    \
    apt-get update && \
    apt-get install -y --no-install-recommends \
    tzdata ca-certificates netcat-openbsd wget libssl1.1 && \
    \
    rm -rf /var/lib/apt/lists/* && rm -rf /var/lib/apt/lists.d/* && apt-get autoremove -y && apt-get clean && apt-get autoclean

RUN mkdir -p /app/bin && \
    wget https://github.com/EmperDeon/healthcheck/releases/download/v0.1.1/healthcheck-linux-amd64 -O /app/bin/healthcheck --quiet

WORKDIR /app
COPY --from=builder /app/target/release/manager /app/target/release/relay /app/
COPY ./bin/*.sh /app/bin/

RUN chown -R 1000:1000 /app && \
    chmod +x /app/bin/*

USER 1000
EXPOSE 3000

HEALTHCHECK --interval=15s CMD /app/bin/liveliness.sh

CMD ["/app/manager"]
