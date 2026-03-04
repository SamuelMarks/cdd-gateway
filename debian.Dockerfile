# Stage 1: Build
FROM rust:slim-bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y pkg-config libssl-dev gcc build-essential && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/src/cdd-ctl
COPY . .

# Build for release
RUN cargo build --release

# Stage 2: Final
FROM debian:bookworm-slim

# Install runtime dependencies if needed
RUN apt-get update && apt-get install -y libssl3 && rm -rf /var/lib/apt/lists/*

WORKDIR /usr/local/bin
COPY --from=builder /usr/src/cdd-ctl/target/release/cdd-ctl .

EXPOSE 8080

ENTRYPOINT ["cdd-ctl", "--bind", "0.0.0.0:8080"]
