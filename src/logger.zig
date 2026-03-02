//! Logging utility for `cdd-ctl`.
//!
//! Provides a mechanism to output logs either as human-readable plaintext
//! or as structured JSONL (JSON Lines) for daemon environments.

const std = @import("std");

/// The severity level of the log entry.
pub const LogLevel = enum {
    info,
    warning,
    error,
    debug,
};

/// Logs a formatted message.
///
/// If `is_daemon` is true, it outputs a single-line JSON object (JSONL)
/// adhering to a simple structured schema (`timestamp`, `level`, `message`).
/// Otherwise, it outputs a standard bracketed plaintext format.
///
/// * `allocator`: Memory allocator used to format the message string.
/// * `is_daemon`: Whether to use JSONL daemon output mode.
/// * `level`: The log severity level.
/// * `fmt`: The format string.
/// * `args`: The arguments to interpolate into the format string.
pub fn log(allocator: std.mem.Allocator, is_daemon: bool, level: LogLevel, comptime fmt: []const u8, args: anytype) void {
    const out = std.io.getStdOut().writer();
    
    if (is_daemon) {
        // We catch errors silently here to avoid crashing the daemon on logging failure
        const msg = std.fmt.allocPrint(allocator, fmt, args) catch return;
        defer allocator.free(msg);
        
        const log_obj = .{
            .timestamp = std.time.timestamp(),
            .level = @tagName(level),
            .message = msg,
        };
        
        std.json.stringify(log_obj, .{}, out) catch return;
        out.writeByte('
') catch return;
    } else {
        out.print("[{s}] ", .{@tagName(level)}) catch return;
        out.print(fmt, args) catch return;
        out.writeByte('
') catch return;
    }
}

test "logger formats and writes" {
    const allocator = std.testing.allocator;
    
    // Test plaintext
    log(allocator, false, .info, "Testing plain {s}", .{"message"});
    
    // Test JSONL
    log(allocator, true, .warning, "Testing json {s} number {d}", .{"format", 42});
}