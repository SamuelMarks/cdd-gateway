//! Main entry point for the `cdd-ctl` central CLI interface.
//!
//! Handles initialization, argument parsing, and directing execution
//! to either the JSON-RPC server or the specific language process manager.

const std = @import("std");
const root = @import("root.zig");
const cli = root.cli;
const server = root.server;
const process = root.process;
const logger = root.logger;

/// The main entry point for the compiled executable.
pub fn main() !void {
    var gpa = std.heap.GeneralPurposeAllocator(.{}){};
    defer _ = gpa.deinit();
    const allocator = gpa.allocator();

    // Fetch raw process arguments
    var args_iterator = try std.process.argsWithAllocator(allocator);
    defer args_iterator.deinit();

    var args = std.ArrayList([]const u8).init(allocator);
    defer args.deinit();

    // Skip program name
    _ = args_iterator.skip();
    while (args_iterator.next()) |arg| {
        try args.append(arg);
    }

    // Parse CLI arguments
    const config = cli.parseArgs(allocator, args.items) catch |err| {
        std.debug.print("Failed to parse arguments: {}
", .{err});
        std.process.exit(1);
    };
    defer allocator.free(config.pass_through_args);

    // Route command logic
    if (config.start_server) {
        logger.log(allocator, config.daemon, .info, "Starting cdd-ctl JSON-RPC Server on port {d}...", .{config.port});
        var rpc_server = server.RpcServer.init(config.port);
        try rpc_server.start();
        defer rpc_server.stop();
        // Server would block here in a real implementation
    } else {
        if (config.language) |lang| {
            logger.log(allocator, config.daemon, .info, "Routing request to language ecosystem: {s}", .{@tagName(lang)});
        }
        
        if (config.component_type) |ctype| {
            logger.log(allocator, config.daemon, .info, "Component targeted: {s}", .{@tagName(ctype)});
        }
        
        if (config.remote_server) |remote_uri| {
            logger.log(allocator, config.daemon, .info, "Forwarding request to remote server: {s}", .{remote_uri});
            // HTTP client dispatch would go here
            return;
        }
        
        if (config.socket_path) |sock| {
            logger.log(allocator, config.daemon, .info, "Forwarding request to local socket: {s}", .{sock});
            // Unix Domain Socket or Named Pipe dispatch would go here
            return;
        }

        logger.log(allocator, config.daemon, .info, "Executing pass-through toolchain arguments...", .{});
        
        if (config.language != null and config.component_type != null) {
            // Build pseudo command based on requested language
            var cmd = std.ArrayList([]const u8).init(allocator);
            defer cmd.deinit();
            
            // Example map: rust -> "cdd-rust", python -> "cdd-python"
            const exe_name = switch (config.language.?) {
                .rust => "cdd-rust",
                .python => "cdd-python-client",
                .go => "cdd-go",
                .java => "cdd-java",
                .kotlin => "cdd-kotlin",
                .php => "cdd-php",
                .ruby => "cdd-ruby",
                .c => "cdd-c",
                .cpp => "cdd-cpp",
                .csharp => "cdd-csharp",
                .sh => "cdd-sh",
                .swift => "cdd-swift",
                .typescript => "cdd-web-ng",
            };
            
            try cmd.append(exe_name);
            for (config.pass_through_args) |pta| {
                try cmd.append(pta);
            }
            
            var proc = process.ManagedProcess.init(allocator, cmd.items);
            proc.start() catch |err| {
                logger.log(allocator, config.daemon, .error, "Failed to start language process {s}: {}", .{exe_name, err});
                return;
            };
            
            _ = proc.wait() catch |err| {
                logger.log(allocator, config.daemon, .error, "Error waiting on process: {}", .{err});
                return;
            };
            
            logger.log(allocator, config.daemon, .info, "Process finished with stats: Starts={d}, Crashes={d}, Flakiness={d:.2}%", .{
                proc.stats.start_count,
                proc.stats.crash_count,
                proc.stats.flakiness()
            });
        }
    }
}

test "main test wrapper" {
    // Tests that main compiles cleanly and basic structure works
    _ = cli;
    _ = server;
    _ = process;
}
