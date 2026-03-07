# OculOS — Linux container build
# Note: UI automation requires a running desktop session (AT-SPI2).
# This image is useful for building from source or CI/CD pipelines.

FROM rust:1.82-bookworm AS builder

RUN apt-get update && apt-get install -y \
    libatspi2.0-dev \
    libdbus-1-dev \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app
COPY . .
RUN cargo build --release

FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    libatspi2.0-0 \
    libdbus-1-3 \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/oculos /usr/local/bin/oculos

EXPOSE 7878

ENTRYPOINT ["oculos"]
CMD ["--bind", "0.0.0.0:7878"]
