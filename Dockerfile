FROM rust:buster as builder
WORKDIR /ipbot
COPY . .
RUN cargo install --path . --root .

FROM debian:buster-slim

RUN apt-get update && \
    DEBIAN_FRONTEND=noninteractive apt-get install -qq -y libssl1.1 ca-certificates && \
    rm -rf /var/lib/apt/lists/*

EXPOSE 7443/tcp

ENV TELOXIDE_TOKEN=""
ENV REDIS_SOCKET="redis+unix:///run/redis.sock"

COPY --from=builder /ipbot/bin/ipbot /ipbot/ipbot
WORKDIR /ipbot
VOLUME /run

CMD ["/ipbot/ipbot"]
