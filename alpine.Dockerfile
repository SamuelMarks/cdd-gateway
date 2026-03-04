# Stage 1: Build
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc pkgconfig openssl-dev

# Create a new empty shell project
WORKDIR /usr/src/cdd-ctl
COPY . .

# Build for release
RUN cargo build --release

# Stage 2: Final
FROM alpine:latest

# Install runtime dependencies if needed
RUN apk add --no-cache libgcc openssl

WORKDIR /usr/local/bin
COPY --from=builder /usr/src/cdd-ctl/target/release/cdd-ctl .

EXPOSE 8080

ENTRYPOINT ["cdd-ctl", "--bind", "0.0.0.0:8080"]
