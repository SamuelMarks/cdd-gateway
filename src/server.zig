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

    /// Starts the server. In a full implementation, this binds to a TCP socket
    /// and listens for incoming HTTP requests.
    pub fn start(self: *RpcServer) !void {
        if (self.is_running) return error.AlreadyRunning;
        // Mock server start
        self.is_running = true;
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

    try server.start();
    try std.testing.expect(server.is_running);

    server.stop();
    try std.testing.expect(!server.is_running);
}

test "JSON-RPC request parsing structure" {
    const allocator = std.testing.allocator;
    const json_str =
        \{
        \  "jsonrpc": "2.0",
        \  "method": "execute",
        \  "id": 42
        \}
    ;

    const parsed = try std.json.parseFromSlice(RpcRequest, allocator, json_str, .{});
    defer parsed.deinit();

    try std.testing.expectEqualStrings("2.0", parsed.value.jsonrpc);
    try std.testing.expectEqualStrings("execute", parsed.value.method);
    try std.testing.expectEqual(@as(i64, 42), parsed.value.id);
}
