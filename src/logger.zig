//! Logging utility for `cdd-ctl`.
//!
//! Provides a mechanism to output logs either as human-readable plaintext
//! or as structured JSONL (JSON Lines) for daemon environments.

const std = @import("std");

/// The severity level of the log entry.
pub const LogLevel = enum {
    info,
    warning,
    err,
    debug,
};

/// Logs a formatted message.
pub fn log(allocator: std.mem.Allocator, is_daemon: bool, level: LogLevel, comptime fmt: []const u8, args: anytype) void {
    const out = std.io.getStdOut().writer();
    
    if (is_daemon) {
        const msg = std.fmt.allocPrint(allocator, fmt, args) catch return;
        defer allocator.free(msg);
        
        const log_obj = .{
            .timestamp = std.time.timestamp(),
            .level = @tagName(level),
            .message = msg,
        };
        
        std.json.stringify(log_obj, .{}, out) catch return;
        out.writeByte('\n') catch return;
    } else {
        out.print("[{s}] ", .{@tagName(level)}) catch return;
        out.print(fmt, args) catch return;
        out.writeByte('\n') catch return;
    }
}

test "logger formats and writes" {
    const allocator = std.testing.allocator;
    log(allocator, false, .info, "Testing plain {s}", .{"message"});
    log(allocator, true, .warning, "Testing json {s} number {d}", .{"format", 42});
}
