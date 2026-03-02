//! The cdd-ctl SDK.
//! 
//! This module acts as the core library for managing cdd-* language applications.
//! It provides high-level components for CLI parsing, JSON-RPC serving, and 
//! cross-platform process management.

/// Exposes the Command Line Interface parsers.
pub const cli = @import("cli.zig");

/// Exposes the JSON-RPC over HTTP server interfaces.
pub const server = @import("server.zig");

/// Exposes the cross-platform systemd/initd process management interfaces.
pub const process = @import("process.zig");

/// Exposes structured logging tools for daemonizing processes.
pub const logger = @import("logger.zig");

test "SDK root module validation" {
    _ = cli;
    _ = server;
    _ = process;
    _ = logger;
}
