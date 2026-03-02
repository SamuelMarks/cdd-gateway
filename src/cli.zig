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
    /// Whether to run in daemon mode (enables structured JSONL logging).
    daemon: bool = false,
    /// The unparsed remaining arguments intended for the underlying `cdd-*` tool.
    pass_through_args: [][]const u8,
};

/// Raw JSON configuration structure.
pub const JsonConfig = struct {
    language: ?[]const u8 = null,
    component_type: ?[]const u8 = null,
    start_server: ?bool = null,
    port: ?u16 = null,
    remote_server: ?[]const u8 = null,
    socket_path: ?[]const u8 = null,
    daemon: ?bool = null,
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

    var config_file_path: ?[]const u8 = null;

    // First pass to check for --config
    var i: usize = 0;
    while (i < args.len) : (i += 1) {
        if (std.mem.eql(u8, args[i], "--config") and i + 1 < args.len) {
            config_file_path = args[i + 1];
            break;
        } else if (std.mem.eql(u8, args[i], "--")) {
            break;
        }
    }

    // Try applying config file if provided
    if (config_file_path) |path| {
        try applyConfigFile(allocator, path, &config);
    }

    // Second pass to apply CLI overrides
    i = 0;
    while (i < args.len) : (i += 1) {
        const arg = args[i];

        if (std.mem.eql(u8, arg, "--config") and i + 1 < args.len) {
            i += 1; // Already processed
        } else if (std.mem.eql(u8, arg, "--server")) {
            config.start_server = true;
        } else if (std.mem.eql(u8, arg, "--daemon")) {
            config.daemon = true;
        } else if (std.mem.eql(u8, arg, "--port") and i + 1 < args.len) {
            i += 1;
            config.port = try std.fmt.parseInt(u16, args[i], 10);
        } else if (std.mem.eql(u8, arg, "--remote-server") and i + 1 < args.len) {
            i += 1;
            // Overwrite JSON value if it exists, use input slice
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

/// Applies settings from a JSON config file into a `CliConfig`.
fn applyConfigFile(allocator: std.mem.Allocator, path: []const u8, config: *CliConfig) !void {
    const file = std.fs.cwd().openFile(path, .{}) catch |err| {
        if (err == error.FileNotFound) return error.ConfigFileNotFound;
        return err;
    };
    defer file.close();

    const file_size = try file.getEndPos();
    if (file_size > 1024 * 1024) return error.ConfigFileTooLarge; // Max 1MB
    const content = try file.readToEndAlloc(allocator, @intCast(file_size));
    defer allocator.free(content);

    const parsed = try std.json.parseFromSlice(JsonConfig, allocator, content, .{ .ignore_unknown_fields = true });
    defer parsed.deinit();

    const j = parsed.value;

    if (j.language) |lang| {
        config.language = std.meta.stringToEnum(Language, lang) orelse return error.UnknownLanguageInConfig;
    }
    if (j.component_type) |ctype| {
        config.component_type = std.meta.stringToEnum(ComponentType, ctype) orelse return error.UnknownComponentTypeInConfig;
    }
    if (j.start_server) |srv| config.start_server = srv;
    if (j.port) |p| config.port = p;
    if (j.daemon) |d| config.daemon = d;
    
    // For strings, we must duplicate because `content` is freed at end of function
    if (j.remote_server) |rs| {
        config.remote_server = try allocator.dupe(u8, rs);
    }
    if (j.socket_path) |sp| {
        config.socket_path = try allocator.dupe(u8, sp);
    }
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
    const args = &[_][]const u8{ "--server", "--port", "9090", "--daemon" };

    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);

    try std.testing.expect(config.start_server);
    try std.testing.expect(config.daemon);
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

test "parseArgs reads config file" {
    const allocator = std.testing.allocator;
    const tmp_path = "test_config.json";
    
    const file = try std.fs.cwd().createFile(tmp_path, .{});
    const json_content = 
        \\{
        \\  "language": "python",
        \\  "component_type": "client",
        \\  "start_server": true,
        \\  "port": 3000
        \\}
    ;
    try file.writeAll(json_content);
    file.close();
    
    defer std.fs.cwd().deleteFile(tmp_path) catch {};

    const args = &[_][]const u8{ "--config", tmp_path };
    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);
    
    try std.testing.expectEqual(Language.python, config.language.?);
    try std.testing.expectEqual(ComponentType.client, config.component_type.?);
    try std.testing.expect(config.start_server);
    try std.testing.expectEqual(@as(u16, 3000), config.port);
}

test "parseArgs config file CLI override" {
    const allocator = std.testing.allocator;
    const tmp_path = "test_override.json";
    
    const file = try std.fs.cwd().createFile(tmp_path, .{});
    const json_content = 
        \\{
        \\  "language": "python",
        \\  "port": 3000
        \\}
    ;
    try file.writeAll(json_content);
    file.close();
    
    defer std.fs.cwd().deleteFile(tmp_path) catch {};

    // Override port to 8080 and change language to go via CLI
    const args = &[_][]const u8{ "--config", tmp_path, "--port", "8080", "--language", "go" };
    const config = try parseArgs(allocator, args);
    defer allocator.free(config.pass_through_args);
    
    try std.testing.expectEqual(Language.go, config.language.?);
    try std.testing.expectEqual(@as(u16, 8080), config.port);
}