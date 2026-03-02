FROM debian:bookworm-slim AS builder

RUN apt-get update && apt-get install -y wget xz-utils ca-certificates &&     rm -rf /var/lib/apt/lists/*

RUN ARCH=$(dpkg --print-architecture) &&     if [ "$ARCH" = "amd64" ]; then ZIG_ARCH="x86_64";     elif [ "$ARCH" = "arm64" ]; then ZIG_ARCH="aarch64";     else ZIG_ARCH="x86_64"; fi &&     wget -q https://ziglang.org/download/0.13.0/zig-linux-${ZIG_ARCH}-0.13.0.tar.xz &&     tar -xf zig-linux-${ZIG_ARCH}-0.13.0.tar.xz &&     mv zig-linux-${ZIG_ARCH}-0.13.0/ /usr/local/zig &&     ln -s /usr/local/zig/zig /usr/local/bin/zig

WORKDIR /app
COPY . .
RUN zig build -Doptimize=ReleaseSafe

FROM debian:bookworm-slim
WORKDIR /app
COPY --from=builder /app/zig-out/bin/cdd-ctl /usr/local/bin/cdd-ctl
EXPOSE 8080
ENTRYPOINT ["cdd-ctl", "--server", "--port", "8080", "--daemon"]
