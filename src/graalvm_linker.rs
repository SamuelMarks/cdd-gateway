#![cfg(not(tarpaulin_include))]

//! GraalVM Linker bindings

use std::collections::HashMap;
use wasmtime::{Caller, Linker, Memory};

/// State object to hold JS mock references for GraalVM.
pub struct GraalVmState {
    /// Simulated JS heap for interop
    pub js_objects: HashMap<u32, Box<dyn std::any::Any + Send + Sync>>,
    next_id: u32,
}

impl Default for GraalVmState {
    fn default() -> Self {
        Self::new()
    }
}

impl GraalVmState {
    /// Creates a new `GraalVmState`.
    pub fn new() -> Self {
        Self {
            js_objects: HashMap::new(),
            next_id: 1,
        }
    }

    /// Inserts a mock JS object into state.
    pub fn insert_object(&mut self, obj: Box<dyn std::any::Any + Send + Sync>) -> u32 {
        let id = self.next_id;
        self.next_id += 1;
        self.js_objects.insert(id, obj);
        id
    }

    /// Retrieves a mock JS object from state.
    pub fn get_object(&self, id: u32) -> Option<&(dyn std::any::Any + Send + Sync)> {
        self.js_objects.get(&id).map(|b| &**b)
    }
}

/// Helper to read a string from memory
pub fn read_string<T>(
    memory: &Memory,
    caller: &mut Caller<'_, T>,
    ptr: i32,
    len: i32,
) -> Result<String, String> {
    let mut buf = vec![0; len as usize];
    memory
        .read(caller, ptr as usize, &mut buf)
        .map_err(|e| e.to_string())?;
    String::from_utf8(buf).map_err(|e| e.to_string())
}

/// Helper to write a string to memory
pub fn write_string<T>(
    memory: &Memory,
    caller: &mut Caller<'_, T>,
    ptr: i32,
    s: &str,
) -> Result<(), String> {
    memory
        .write(caller, ptr as usize, s.as_bytes())
        .map_err(|e| e.to_string())?;
    Ok(())
}

/// Linker implementation for GraalVM `jsbody` and `interop`.
pub struct GraalVmLinker;

impl GraalVmLinker {
    /// Links the stubs required by GraalVM into the given linker.
    pub fn add_to_linker<T: 'static + Send>(linker: &mut Linker<T>) -> Result<(), String> {
        linker
            .func_wrap(
                "interop",
                "stdoutWriter.printChars",
                |mut _caller: Caller<'_, T>, _ptr: i32, _len: i32| {
                    // Stub for Phase 3
                },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "interop",
                "stderrWriter.printChars",
                |mut _caller: Caller<'_, T>, _ptr: i32, _len: i32| {
                    // Stub for Phase 3
                },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap("interop", "Date.now", |mut _caller: Caller<'_, T>| -> f64 {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
                    * 1000.0
            })
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "interop",
                "performance.now",
                |mut _caller: Caller<'_, T>| -> f64 {
                    std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs_f64()
                        * 1000.0
                },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "interop",
                "runtime.setExitCode",
                |mut _caller: Caller<'_, T>, _code: i32| {
                    // Stub for Phase 3
                },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        // Mock jsbody methods
        linker
            .func_wrap(
                "jsbody",
                "_JSObject.stringValue___String",
                |mut _caller: Caller<'_, T>| -> i32 { 0 },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "jsbody",
                "_JSNumber.javaDouble___Double",
                |mut _caller: Caller<'_, T>| -> f64 { 0.0 },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "jsbody",
                "_JSConversion.extractJavaScriptString___String_Object",
                |mut _caller: Caller<'_, T>| -> i32 { 0 },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "jsbody",
                "_JSObject.get___Object_Object",
                |mut _caller: Caller<'_, T>| -> i32 { 0 },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        // Mock compat methods
        linker
            .func_wrap(
                "compat",
                "f64rem",
                |mut _caller: Caller<'_, T>, a: f64, b: f64| -> f64 { a % b },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "compat",
                "f64log",
                |mut _caller: Caller<'_, T>, a: f64| -> f64 { a.ln() },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "compat",
                "f64log10",
                |mut _caller: Caller<'_, T>, a: f64| -> f64 { a.log10() },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        linker
            .func_wrap(
                "compat",
                "f64pow",
                |mut _caller: Caller<'_, T>, a: f64, b: f64| -> f64 { a.powf(b) },
            )
            .map_err(|e| format!("Link error: {}", e))?;

        Ok(())
    }
}
