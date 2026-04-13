import { execSync } from "child_process";
import fs from "fs";
import path from "path";
import { fileURLToPath } from "url";

const __filename = fileURLToPath(import.meta.url);
const __dirname = path.dirname(__filename);

const CDD_CTL_DIR = path.resolve(__dirname, "..");
const SDKS_DIR = path.join(CDD_CTL_DIR, "sdks");
const DEST_DIR = path.join(CDD_CTL_DIR, "cdd-ctl-wasm-sdk", "assets", "wasm");
const SUPPORT_FILE = path.join(
    CDD_CTL_DIR,
    "cdd-ctl-wasm-sdk",
    "assets",
    "wasm-support.json",
);
const DUMMY_WASM_DIR = path.join(CDD_CTL_DIR, "scripts", "dummy-wasm");

if (!fs.existsSync(DEST_DIR)) {
    fs.mkdirSync(DEST_DIR, { recursive: true });
}

console.log("Building cdd-ctl-wasm-sdk...");
try {
    execSync("npm run build", {
        cwd: path.join(CDD_CTL_DIR, "cdd-ctl-wasm-sdk"),
        stdio: "inherit",
    });
} catch (e) {
    console.warn("Failed to build cdd-ctl-wasm-sdk, continuing...");
}

if (!fs.existsSync(SDKS_DIR)) {
    console.warn("Warning: SDKs directory not found at " + SDKS_DIR);
    process.exit(0);
}

// Build dummy wasm
console.log("Building dummy wasm fallback...");
try {
    execSync("cargo build --target wasm32-wasip1 --release", {
        cwd: DUMMY_WASM_DIR,
        stdio: "inherit",
    });
} catch (e) {
    console.warn("Failed to build dummy wasm fallback");
}
const DUMMY_WASM_PATH = path.join(
    DUMMY_WASM_DIR,
    "target",
    "wasm32-wasip1",
    "release",
    "dummy_wasm.wasm",
);
let DUMMY_WASM = Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]);
if (fs.existsSync(DUMMY_WASM_PATH)) {
    DUMMY_WASM = fs.readFileSync(DUMMY_WASM_PATH);
}

const sdks = fs.readdirSync(SDKS_DIR).filter((file) => {
    return (
        fs.statSync(path.join(SDKS_DIR, file)).isDirectory() &&
        file.startsWith("cdd-")
    );
});

const supportMap = {};

sdks.forEach((sdk) => {
    const sdkPath = path.join(SDKS_DIR, sdk);
    const cargoTomlPath = path.join(sdkPath, "Cargo.toml");
    const goModPath = path.join(sdkPath, "go.mod");
    const packageJsonPath = path.join(sdkPath, "package.json");
    const pyProjectPath = path.join(sdkPath, "pyproject.toml");
    const makefilePath = path.join(sdkPath, "Makefile");
    const cmakePath = path.join(sdkPath, "CMakeLists.txt");
    const gradlePath = path.join(sdkPath, "build.gradle.kts");
    const isJavaRaw = fs.existsSync(path.join(sdkPath, "src", "main", "java"));
    const mavenPath = path.join(sdkPath, "pom.xml");
    const buildWasmScriptLegacy = path.join(sdkPath, "scripts", "build-wasm.sh");
    const buildWasmScriptRoot = path.join(sdkPath, "build_wasm.sh");

    let success = false;
    const wasmDest = path.join(DEST_DIR, sdk + ".wasm");

    console.log(`\nAnalyzing ${sdk}...`);

    try {
        if (fs.existsSync(buildWasmScriptRoot)) {
            console.log(`[${sdk}] Found custom build script at root. Executing...`);
            execSync(`bash ${buildWasmScriptRoot}`, {
                cwd: sdkPath,
                stdio: "inherit",
            });
            const customWasmSource = path.join(sdkPath, "target", "wasm", `${sdk}.wasm`);
            if (fs.existsSync(customWasmSource)) {
                fs.copyFileSync(customWasmSource, wasmDest);
                success = true;
            }
        } else if (fs.existsSync(buildWasmScriptLegacy)) {
            console.log(`[${sdk}] Found custom build script in scripts/. Executing...`);
            execSync(`bash ${buildWasmScriptLegacy}`, {
                cwd: sdkPath,
                stdio: "inherit",
            });
            const customWasmSource = path.join(sdkPath, "build", `${sdk}.wasm`);
            if (fs.existsSync(customWasmSource)) {
                fs.copyFileSync(customWasmSource, wasmDest);
                success = true;
            }
        } else if (fs.existsSync(cargoTomlPath)) {
            console.log(
                `[${sdk}] Rust project detected. Building via Cargo for wasm32-wasip1...`,
            );
            execSync("cargo build --target wasm32-wasip1 --release", {
                cwd: sdkPath,
                stdio: "inherit",
                env: {
                    ...process.env,
                    RUSTFLAGS: "-C target-feature=+multivalue",
                },
            });
            const wasmSource = path.join(
                sdkPath,
                "target/wasm32-wasip1/release/" + sdk + ".wasm",
            );
            if (fs.existsSync(wasmSource)) {
                fs.copyFileSync(wasmSource, wasmDest);
                success = true;
            }
        } else if (sdk === "cdd-sh") {
            console.log(`[${sdk}] Shell project detected. Running make build_wasm...`);
            execSync("make build_wasm", { cwd: sdkPath, stdio: "inherit" });
            const wasmSource = path.join(sdkPath, "wasm_build", `${sdk}.wasm`);
            if (fs.existsSync(wasmSource)) {
                fs.copyFileSync(wasmSource, wasmDest);
                success = true;
            }
        } else if (fs.existsSync(goModPath)) {
            console.log(
                `[${sdk}] Go project detected. Building via Go compiler for wasip1/wasm...`,
            );
            execSync(
                `GOOS=wasip1 GOARCH=wasm go build -o ${wasmDest} ./cmd/${sdk}`,
                { cwd: sdkPath, stdio: "inherit" },
            );
            if (fs.existsSync(wasmDest)) {
                success = true;
            }
        } else if (fs.existsSync(packageJsonPath)) {
            console.log(
                `[${sdk}] Node/TS project detected. Running make build_wasm...`,
            );
            execSync("make build_wasm", { cwd: sdkPath, stdio: "inherit" });
            const wasmSource = path.join(sdkPath, "bin", `${sdk}.wasm`);
            if (fs.existsSync(wasmSource)) {
                fs.copyFileSync(wasmSource, wasmDest);
                success = true;
            }
        } else if (fs.existsSync(pyProjectPath) || fs.existsSync(path.join(sdkPath, "setup.py"))) {
            console.log(
                `[${sdk}] Python project detected. Zipping source code for Pyodide fallback...`,
            );
            const sourceFiles = fs.existsSync(pyProjectPath) ? "src pyproject.toml" : "cdd setup.py";
            execSync(`zip -r ${wasmDest} ${sourceFiles}`, {
                cwd: sdkPath,
                stdio: "inherit",
            });
            if (fs.existsSync(wasmDest)) {
                success = true;
            }
        } else if (fs.existsSync(makefilePath) || fs.existsSync(cmakePath)) {
            console.log(
                `[${sdk}] C/C++ project detected. Requires wasi-sdk / emscripten to compile. Running make build_wasm...`,
            );
            execSync("make build_wasm", { cwd: sdkPath, stdio: "inherit" });
            const wasmSource = path.join(sdkPath, "bin", `${sdk}.wasm`);
            const wasmSourceDist = path.join(sdkPath, "dist", `${sdk}.wasm`);
            if (fs.existsSync(wasmSource)) {
                fs.copyFileSync(wasmSource, wasmDest);
                success = true;
            } else if (fs.existsSync(wasmSourceDist)) {
                fs.copyFileSync(wasmSourceDist, wasmDest);
                success = true;
            }
        } else if (
            fs.existsSync(gradlePath) ||
            fs.existsSync(mavenPath) ||
            isJavaRaw
        ) {
            console.log(
                `[${sdk}] JVM project detected (Java/Kotlin). Requires TeaVM or GraalVM WASM target. Running make build_wasm...`,
            );
            execSync("make build_wasm", { cwd: sdkPath, stdio: "inherit" });
            const wasmSource = path.join(sdkPath, "bin", `${sdk}.wasm`);
            if (fs.existsSync(wasmSource)) {
                fs.copyFileSync(wasmSource, wasmDest);
                success = true;
            }
        } else {
            console.log(
                `[${sdk}] No known build configuration found for WASM compilation.`,
            );
        }
    } catch (e) {
        console.warn(
            `[${sdk}] Native WASM build failed or is unsupported:`,
            e.message,
        );
    }

    if (!success) {
        console.log(`[${sdk}] Falling back to dummy WASM binary.`);
        fs.writeFileSync(wasmDest, DUMMY_WASM);
        success = false;
    }

    let lang = sdk.replace("cdd-", "");
    supportMap[lang] = success;
});

fs.writeFileSync(SUPPORT_FILE, JSON.stringify(supportMap, null, 2));
