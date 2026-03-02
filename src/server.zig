//! JSON-RPC over HTTP server interface for the `cdd-ctl` SDK.
//!
//! Provides the HTTP listening and JSON payload parsing logic to expose the
//! central interface externally.

const std = @import("std");

/// The generic structure of a JSON-RPC 2.0 Request.
pub const RpcRequest = struct {
    /// Protocol version. Must be "2.0".
    jsonrpc: []const u8,
    /// The name of the method to be invoked.
    method: []const u8,
    /// An identifier established by the Client.
    id: i64,
};

/// A basic mock server configuration tracking state.
pub const RpcServer = struct {
    /// Port on which the server will listen.
    port: u16,
    /// Tracks if the server is currently running.
    is_running: bool = false,

    /// Initializes a new JSON-RPC server.
    ///
    /// * `port`: The TCP port to listen on.
    pub fn init(port: u16) RpcServer {
        return RpcServer{
            .port = port,
        };
    }

    /// Starts the server. Binds to a TCP socket and listens for incoming HTTP requests.
    pub fn start(self: *RpcServer) !void {
        if (self.is_running) return error.AlreadyRunning;
        self.is_running = true;

        const address = try std.net.Address.parseIp4("0.0.0.0", self.port);
        var server = try address.listen(.{ .reuse_address = true });
        defer server.deinit();

        while (self.is_running) {
            const conn = server.accept() catch continue;
            defer conn.stream.close();

            var buf: [1024]u8 = undefined;
            _ = conn.stream.read(&buf) catch continue;

            const response =
                "HTTP/1.1 200 OK\r\n" ++
                "Content-Type: application/json\r\n" ++
                "Connection: close\r\n" ++
                "\r\n" ++
                "{\"jsonrpc\": \"2.0\", \"result\": {\"version\": \"1.0.0\"}, \"id\": 1}\n";

            _ = conn.stream.writeAll(response) catch continue;
        }
    }

    /// Stops the running server.
    pub fn stop(self: *RpcServer) void {
        self.is_running = false;
    }
};

test "RpcServer initialization and lifecycle" {
    var server = RpcServer.init(8080);
    try std.testing.expectEqual(@as(u16, 8080), server.port);
    try std.testing.expect(!server.is_running);

    // Simulate start manually to prevent blocking the test runner
    server.is_running = true;
    try std.testing.expect(server.is_running);

    server.stop();
    try std.testing.expect(!server.is_running);
}

test "JSON-RPC request parsing structure" {
    const allocator = std.testing.allocator;
    const json_str =
        \\{
        \\  "jsonrpc": "2.0",
        \\  "method": "execute",
        \\  "id": 42
        \\}
    ;

    const parsed = try std.json.parseFromSlice(RpcRequest, allocator, json_str, .{});
    defer parsed.deinit();

    try std.testing.expectEqualStrings("2.0", parsed.value.jsonrpc);
    try std.testing.expectEqualStrings("execute", parsed.value.method);
    try std.testing.expectEqual(@as(i64, 42), parsed.value.id);
}