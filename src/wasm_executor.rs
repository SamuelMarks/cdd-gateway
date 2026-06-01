use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;

use wasmtime::{Config, Engine, Linker, Module, Store};
use wasmtime_wasi::p1::WasiP1Ctx;
use wasmtime_wasi::p2::pipe::MemoryOutputPipe;
use wasmtime_wasi::{DirPerms, FilePerms, WasiCtxBuilder};

/// A generated file output from a WASM execution.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GeneratedFile {
    /// The relative path of the file.
    pub path: String,
    /// The raw byte content of the file.
    pub content: Vec<u8>,
}

/// Errors that can occur during WASM execution.
#[derive(derive_more::Display, Debug, derive_more::Error)]
pub enum WasmError {
    /// The execution of the WASM binary failed.
    #[display("WASM execution failed: {_0}")]
    #[error(ignore)]
    ExecutionFailed(String),

    /// Loading the WASM module from disk or instantiating it failed.
    #[display("WASM module load failed: {_0}")]
    #[error(ignore)]
    LoadFailed(String),

    /// The provided configuration for WASI was invalid.
    #[display("Invalid configuration: {_0}")]
    #[error(ignore)]
    InvalidConfig(String),
}

/// A standard trait for executing SDK generators.
pub trait WasmExecutor: Send + Sync {
    /// Executes the target returning a list of generated files from the `/out` directory.
    fn execute(
        &self,
        target: &str,
        input: &str,
        args: &[String],
    ) -> Result<Vec<GeneratedFile>, WasmError>;
    /// Executes the target returning the raw stdout bytes (typically for JSON outputs).
    fn execute_to_stdout(
        &self,
        target: &str,
        input: &str,
        args: &[String],
    ) -> Result<Vec<u8>, WasmError>;
    /// Executes a raw WASI command for the CLI, returning (stdout, stderr).
    fn execute_cli(
        &self,
        target: &str,
        input_dir: Option<&Path>,
        mount_current_dir: bool,
        args: &[String],
    ) -> Result<(Vec<u8>, Vec<u8>), WasmError>;
}

/// Native implementation using the `wasmtime` embedded engine.
pub struct NativeWasmExecutor {
    engine: Engine,
    module_cache: Arc<Mutex<HashMap<String, Module>>>,
}

/// A globally shared instance of the native WASM executor.
pub static WASM_EXECUTOR: Lazy<NativeWasmExecutor> =
    Lazy::new(|| NativeWasmExecutor::new().expect("Failed to initialize WASM engine"));

impl NativeWasmExecutor {
    /// Initializes a new embedded `wasmtime` engine.
    pub fn new() -> Result<Self, String> {
        let mut config = Config::new();
        config.wasm_gc(true);
        config.wasm_function_references(true);
        config.wasm_multi_memory(true);
        config.wasm_memory64(true);

        let engine = Engine::new(&config).map_err(|e| e.to_string())?;

        Ok(Self {
            engine,
            module_cache: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    fn run_python(
        &self,
        target: &str,
        _input_dir: Option<&Path>,
        _args: &[String],
    ) -> Result<(Vec<u8>, Vec<u8>), WasmError> {
        // Here we will embed QuickJS to orchestrate the Pyodide WebAssembly module.
        use rquickjs::{Context, Runtime};

        let rt = Runtime::new().map_err(|e| {
            WasmError::ExecutionFailed(format!("Failed to create quickjs runtime: {}", e))
        })?;
        let _ctx = Context::full(&rt).map_err(|e| {
            WasmError::ExecutionFailed(format!("Failed to create quickjs context: {}", e))
        })?;

        // This is a stub implementation representing Phase 2 of PLANNN.md
        // To be fully implemented with pyodide.mjs injection.
        let stdout = format!(
            "Executing {} via Pyodide logic inside rquickjs (STUB)",
            target
        )
        .into_bytes();
        let stderr = vec![];

        Ok((stdout, stderr))
    }

    fn get_module(&self, wasm_file: &str) -> Result<Module, WasmError> {
        let mut cache = self
            .module_cache
            .lock()
            .map_err(|e| WasmError::ExecutionFailed(format!("Mutex lock failed: {}", e)))?;
        if let Some(module) = cache.get(wasm_file) {
            return Ok(module.clone());
        }

        let module = Module::from_file(&self.engine, wasm_file)
            .map_err(|e| WasmError::LoadFailed(format!("Failed to load {}: {}", wasm_file, e)))?;

        cache.insert(wasm_file.to_string(), module.clone());
        Ok(module)
    }

    fn run_wasi(
        &self,
        target: &str,
        input_dir: Option<&Path>,
        mount_current_dir: bool,
        args: &[String],
        wasm_file_override: Option<&str>,
    ) -> Result<(Vec<u8>, Vec<u8>), WasmError> {
        let wasm_file = wasm_file_override
            .map(|s| s.to_string())
            .unwrap_or_else(|| format!("cdd-ctl-wasm-sdk/assets/wasm/{}.wasm", target));
        let module = self.get_module(&wasm_file)?;

        let mut linker: Linker<WasiP1Ctx> = Linker::new(&self.engine);
        wasmtime_wasi::p1::add_to_linker_sync(&mut linker, |ctx| ctx)
            .map_err(|e| WasmError::InvalidConfig(format!("Failed to link WASI: {}", e)))?;

        let stdout = MemoryOutputPipe::new(1024 * 1024 * 10); // 10MB
        let stderr = MemoryOutputPipe::new(1024 * 1024 * 10);

        let mut builder = WasiCtxBuilder::new();
        builder.stdout(stdout.clone()).stderr(stderr.clone());

        if let Some(dir) = input_dir {
            builder
                .preopened_dir(dir, "/workspace", DirPerms::all(), FilePerms::all())
                .map_err(|e| {
                    WasmError::InvalidConfig(format!("Failed to mount /workspace: {}", e))
                })?;
        }
        if mount_current_dir {
            builder
                .preopened_dir(".", ".", DirPerms::all(), FilePerms::all())
                .map_err(|e| WasmError::InvalidConfig(format!("Failed to mount .: {}", e)))?;
        }

        let mut wasi_args = vec![wasm_file.clone()];
        wasi_args.extend(args.iter().cloned());
        builder.args(&wasi_args);

        let ctx = builder.build_p1();
        let mut store = Store::new(&self.engine, ctx);

        let instance = linker.instantiate(&mut store, &module).map_err(|e| {
            WasmError::ExecutionFailed(format!("Failed to instantiate module: {}", e))
        })?;

        let start = instance
            .get_typed_func::<(), ()>(&mut store, "_start")
            .map_err(|e| WasmError::ExecutionFailed(format!("No _start function: {}", e)))?;

        if let Err(err) = start.call(&mut store, ()) {
            let msg = err.to_string();
            if !msg.contains("exit status 0") {
                let stderr_bytes = stderr.contents();
                let stderr_str = String::from_utf8_lossy(&stderr_bytes);
                return Err(WasmError::ExecutionFailed(format!(
                    "Execution failed: {}\nStderr: {}",
                    err, stderr_str
                )));
            }
        }

        Ok((stdout.contents().into(), stderr.contents().into()))
    }
}

impl WasmExecutor for NativeWasmExecutor {
    fn execute(
        &self,
        _target: &str,
        _input: &str,
        _args: &[String],
    ) -> Result<Vec<GeneratedFile>, WasmError> {
        Ok(vec![])
    }

    fn execute_to_stdout(
        &self,
        target: &str,
        input: &str,
        args: &[String],
    ) -> Result<Vec<u8>, WasmError> {
        let input_path = std::path::Path::new(input)
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(input));
        let input_dir = input_path.parent();
        let filename = input_path
            .file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();

        let mut run_args = vec![];
        for a in args {
            run_args.push(a.clone());
        }
        run_args.push("-i".to_string());
        run_args.push(format!("/workspace/{}", filename));

        let (stdout, _) = if target == "cdd-python" || target == "cdd-python-all" {
            self.run_python(target, input_dir, &run_args)?
        } else if target == "cdd-sh" {
            let mut sh_args = vec!["/workspace/script.sh".to_string()];
            sh_args.extend(run_args);
            self.run_wasi(
                "dash",
                input_dir,
                false,
                &sh_args,
                Some("cdd-ctl-wasm-sdk/assets/wasm/dash.wasm"),
            )?
        } else {
            self.run_wasi(target, input_dir, false, &run_args, None)?
        };
        Ok(stdout)
    }

    fn execute_cli(
        &self,
        target: &str,
        input_dir: Option<&Path>,
        mount_current_dir: bool,
        args: &[String],
    ) -> Result<(Vec<u8>, Vec<u8>), WasmError> {
        if target == "cdd-python" || target == "cdd-python-all" {
            self.run_python(target, input_dir, args)
        } else if target == "cdd-sh" {
            let mut sh_args = vec!["/workspace/script.sh".to_string()];
            sh_args.extend(args.iter().cloned());
            self.run_wasi(
                "dash",
                input_dir,
                mount_current_dir,
                &sh_args,
                Some("cdd-ctl-wasm-sdk/assets/wasm/dash.wasm"),
            )
        } else {
            self.run_wasi(target, input_dir, mount_current_dir, args, None)
        }
    }
}
