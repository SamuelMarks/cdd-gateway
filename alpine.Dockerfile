# Stage 1: Build
FROM rust:alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev gcc pkgconfig openssl-dev postgresql-dev

# Create a new empty shell project
WORKDIR /usr/src/cdd-gateway
COPY . .

# Build for release
RUN cargo build --release

# Stage 2: Final
FROM alpine:latest

# Install runtime dependencies if needed
RUN apk add --no-cache libgcc openssl curl libpq

WORKDIR /usr/local/bin
COPY --from=builder /usr/src/cdd-gateway/target/release/cdd-gateway .

EXPOSE 8080

ENTRYPOINT ["cdd-gateway", "--bind", "0.0.0.0:8080"]
