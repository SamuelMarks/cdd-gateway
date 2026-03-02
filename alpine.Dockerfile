FROM alpine:3.20 AS builder

RUN apk add --no-cache wget tar xz ca-certificates

RUN ARCH=$(uname -m) &&     if [ "$ARCH" = "x86_64" ] || [ "$ARCH" = "aarch64" ]; then         wget -q https://ziglang.org/download/0.13.0/zig-linux-${ARCH}-0.13.0.tar.xz &&         tar -xf zig-linux-${ARCH}-0.13.0.tar.xz &&         mv zig-linux-${ARCH}-0.13.0/ /usr/local/zig &&         ln -s /usr/local/zig/zig /usr/local/bin/zig;     else         echo "Unsupported architecture for pre-built Alpine Zig binary" && exit 1;     fi

WORKDIR /app
COPY . .
RUN zig build -Doptimize=ReleaseSafe

FROM alpine:3.20
WORKDIR /app
COPY --from=builder /app/zig-out/bin/cdd-ctl /usr/local/bin/cdd-ctl
EXPOSE 8080
ENTRYPOINT ["cdd-ctl", "--server", "--port", "8080", "--daemon"]
