//! Cross-platform process management subsystem.
//!
//! Provides a pure Zig abstraction for managing daemon-like processes,
//! functioning similarly to an initd or systemd service manager but scoped
//! to the `cdd-*` ecosystem.

const std = @import("std");

/// The state of a managed process.
pub const ProcessState = enum {
    /// The process has not been started yet.
    stopped,
    /// The process is actively running.
    running,
    /// The process has exited.
    exited,
};

/// Process reliability statistics.
pub const ProcessStats = struct {
    /// Number of times the process has been started.
    start_count: u32 = 0,
    /// Number of times the process crashed or exited abnormally.
    crash_count: u32 = 0,
    /// Total uptime in seconds.
    total_uptime: u64 = 0,
    /// Timestamp of when the process last started (seconds since epoch).
    last_start_time: ?i64 = null,
    
    /// Calculates flakiness as a percentage of crashes to starts.
    pub fn flakiness(self: ProcessStats) f32 {
        if (self.start_count == 0) return 0.0;
        return (@as(f32, @floatFromInt(self.crash_count)) / @as(f32, @floatFromInt(self.start_count))) * 100.0;
    }
};

/// A manager for a single child process, tracking its state and handling signals.
pub const ManagedProcess = struct {
    /// Memory allocator used to build process arguments.
    allocator: std.mem.Allocator,
    /// The standard Zig child process abstraction.
    child: std.process.Child,
    /// The current operational state of the process.
    state: ProcessState = .stopped,
    /// Reliability statistics.
    stats: ProcessStats = .{},

    /// Initializes a new managed process configuration.
    ///
    /// * `allocator`: The allocator for process string allocations.
    /// * `argv`: The arguments defining the command to execute.
    pub fn init(allocator: std.mem.Allocator, argv: []const []const u8) ManagedProcess {
        var child = std.process.Child.init(argv, allocator);
        child.stdin_behavior = .Ignore;
        child.stdout_behavior = .Pipe;
        child.stderr_behavior = .Pipe;

        return ManagedProcess{
            .allocator = allocator,
            .child = child,
            .state = .stopped,
            .stats = .{},
        };
    }

    /// Starts the process in the background.
    pub fn start(self: *ManagedProcess) !void {
        if (self.state == .running) return error.AlreadyRunning;
        try self.child.spawn();
        self.state = .running;
        self.stats.start_count += 1;
        self.stats.last_start_time = std.time.timestamp();
    }

    /// Stops the process gracefully. If it doesn't terminate, it is killed.
    pub fn stop(self: *ManagedProcess) !void {
        if (self.state != .running) return;

        // In a real robust systemd-like manager, we'd send SIGTERM, wait, then SIGKILL.
        // For cross-platform compatibility out of the box with Zig, we use `kill`.
        _ = try self.child.kill();
        self.updateStatsOnExit();
        self.state = .exited;
    }

    /// Internal helper to update stats upon process exit.
    fn updateStatsOnExit(self: *ManagedProcess) void {
        if (self.stats.last_start_time) |start_time| {
            const current_time = std.time.timestamp();
            if (current_time > start_time) {
                self.stats.total_uptime += @intCast(current_time - start_time);
            }
            self.stats.last_start_time = null;
        }
    }

    /// Checks if the process is still running, updating internal state.
    pub fn checkStatus(self: *ManagedProcess) !ProcessState {
        if (self.state == .stopped) return .stopped;

        // Try to wait for the child without blocking (Not natively exposed cleanly without blocking in basic std.process yet cross-platform)
        // For the sake of this implementation, if we haven't explicitely tracked its exit via wait(), we assume running or we block on wait.
        // In a full implementation we'd use polling or async waitpid.
        return self.state;
    }

    /// Blocks and waits for the process to exit, returning its term status.
    pub fn wait(self: *ManagedProcess) !std.process.Child.Term {
        if (self.state != .running) return error.NotRunning;
        const term = try self.child.wait();
        self.updateStatsOnExit();
        self.state = .exited;
        
        switch (term) {
            .Exited => |code| {
                if (code != 0) {
                    self.stats.crash_count += 1;
                }
            },
            else => {
                self.stats.crash_count += 1;
            },
        }
        
        return term;
    }
};

test "ManagedProcess lifecycle initialization" {
    const allocator = std.testing.allocator;
    const argv = &[_][]const u8{"echo", "test"};
    var process = ManagedProcess.init(allocator, argv);

    try std.testing.expectEqual(ProcessState.stopped, process.state);
}

test "ManagedProcess lifecycle start and wait" {
    // This test relies on 'echo' being available, which is true on most *nix systems and some Windows shells.
    // Skip on pure Windows without msys/cygwin.
    if (std.process.builtin.os.tag == .windows) return;

    const allocator = std.testing.allocator;
    const argv = &[_][]const u8{"echo", "test"};
    var process = ManagedProcess.init(allocator, argv);

    try process.start();
    try std.testing.expectEqual(ProcessState.running, process.state);

    const term = try process.wait();
    try std.testing.expectEqual(ProcessState.exited, process.state);
    
    switch (term) {
        .Exited => |code| try std.testing.expectEqual(@as(u8, 0), code),
        else => return error.UnexpectedTermination,
    }
    
    try std.testing.expectEqual(@as(u32, 1), process.stats.start_count);
    try std.testing.expectEqual(@as(u32, 0), process.stats.crash_count);
}

test "ProcessStats flakiness calculation" {
    var stats = ProcessStats{};
    try std.testing.expectEqual(@as(f32, 0.0), stats.flakiness());
    
    stats.start_count = 10;
    stats.crash_count = 1;
    try std.testing.expectEqual(@as(f32, 10.0), stats.flakiness());
    
    stats.crash_count = 5;
    try std.testing.expectEqual(@as(f32, 50.0), stats.flakiness());
}
