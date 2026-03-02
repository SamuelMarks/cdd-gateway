//! Command Line Interface parsing for `cdd-ctl`.
//!
//! Provides the data structures and logic required to parse incoming arguments
//! and map them to the corresponding `cdd-*` ecosystem tools.

const std = @import("std");

/// Supported programming languages in the `cdd-*` ecosystem.
pub const Language = enum {
    /// C (C89) (`cdd-c`)
    c,
    /// C++ (`cdd-cpp`)
    cpp,
    /// C# (`cdd-csharp`)
    csharp,
    /// Go (`cdd-go`)
    go,
    /// Java (`cdd-java`)
    java,
    /// Kotlin Multiplatform (`cdd-kotlin`)
    kotlin,
    /// PHP (`cdd-php`)
    php,
    /// Python (`cdd-python-client`)
    python,
    /// Ruby (`cdd-ruby`)
    ruby,
    /// Rust (`cdd-rust`)
    rust,
    /// Shell script (`cdd-sh`)
    sh,
    /// Swift (`cdd-swift`)
    swift,
    /// TypeScript (`cdd-web-ng`)
    typescript,
};

/// The architectural component type to invoke.
pub const ComponentType = enum {
    /// The standard client SDK.
    client,
    /// The client command-line interface.
    client_cli,
    /// The server component.
    server,
};

/// Configuration parsed from command line arguments.
pub const CliConfig = struct {
    /// The specific language target, if provided.
    language: ?Language = null,
    /// The component type target, if provided.
    component_type: ?ComponentType = null,
    /// Whether to start the `cdd-ctl` central JSON-RPC server.
    start_server: bool = false,
    /// The port to use if `start_server` is true.
    port: u16 = 8080,
    /// Optional URI for a remote server instead of launching locally.
    remote_server: ?[]const u8 = null,
    /// Optional path to an already running socket.
    socket_path: ?[]const u8 = null,
    /// The unparsed remaining arguments intended for the underlying `cdd-*` tool.
    pass_through_args: [][]const u8,
};

/// Parses a slice of string arguments into a `CliConfig`.
/// The caller owns the memory of `pass_through_args` which borrows from the input slice.
///
/// * `allocator`: Memory allocator used for dynamic structures.
/// * `args`: The command line arguments excluding the program name.
pub fn parseArgs(allocator: std.mem.Allocator, args: []const []const u8) !CliConfig {
    var config = CliConfig{
        .pass_through_args = &[_][]const u8{},
    };

    var pass_through = std.ArrayList([]const u8).init(allocator);
    errdefer pass_through.deinit();

    var i: usize = 0;
    while (i < args.len) : (i += 1) {
        const arg = args[i];

        if (std.mem.eql(u8, arg, "--server")) {
            config.start_server = true;
        } else if (std.mem.eql(u8, arg, "--port") and i + 1 < args.len) {
            i += 1;
            config.port = try std.fmt.parseInt(u16, args[i], 10);
        } else if (std.mem.eql(u8, arg, "--remote-server") and i + 1 < args.len) {
            i += 1;
            config.remote_server = args[i];
        } else if (std.mem.eql(u8, arg, "--socket") and i + 1 < args.len) {
            i += 1;
            config.socket_path = args[i];
        } else if (std.mem.eql(u8, arg, "--language") and i + 1 < args.len) {
            i += 1;
            config.language = std.meta.stringToEnum(Language, args[i]) orelse return error.UnknownLanguage;
        } else if (std.mem.eql(u8, arg, "--type") and i + 1 < args.len) {
            i += 1;
            config.component_type = std.meta.stringToEnum(ComponentType, args[i]) orelse return error.UnknownComponentType;
        } else if (std.mem.eql(u8, arg, "--")) {
            // Everything after `--` is pass-through
            i += 1;
            while (i < args.len) : (i += 1) {
                try pass_through.append(args[i]);
            }
            break;
        } else {
            try pass_through.append(arg);
        }
    }

    config.pass_through_args = try pass_through.toOwnedSlice();
    return config;
}

test "parseArgs parses language and component type" {
    const allocator = std.testing.allocator;
    const args = &[_][]const u8{ "--language", "rust", "--type", "server", "--", "--verbose" };

    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);

    try std.testing.expectEqual(Language.rust, config.language.?);
    try std.testing.expectEqual(ComponentType.server, config.component_type.?);
    try std.testing.expectEqualStrings("--verbose", config.pass_through_args[0]);
}

test "parseArgs parses remote server and socket path" {
    const allocator = std.testing.allocator;
    const args = &[_][]const u8{ "--remote-server", "http://localhost:9090", "--socket", "/tmp/cdd.sock" };

    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);

    try std.testing.expectEqualStrings("http://localhost:9090", config.remote_server.?);
    try std.testing.expectEqualStrings("/tmp/cdd.sock", config.socket_path.?);
}

test "parseArgs parses server and port" {
    const allocator = std.testing.allocator;
    const args = &[_][]const u8{ "--server", "--port", "9090" };

    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);

    try std.testing.expect(config.start_server);
    try std.testing.expectEqual(@as(u16, 9090), config.port);
}

test "parseArgs handles unknown language error" {
    const allocator = std.testing.allocator;
    const args = &[_][]const u8{ "--language", "brainfuck" };
    try std.testing.expectError(error.UnknownLanguage, parseArgs(allocator, args));
}

test "parseArgs handles unknown component type error" {
    const allocator = std.testing.allocator;
    const args = &[_][]const u8{ "--type", "magic_box" };
    try std.testing.expectError(error.UnknownComponentType, parseArgs(allocator, args));
}
