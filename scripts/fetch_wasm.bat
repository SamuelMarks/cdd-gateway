@echo off
setlocal enabledelayedexpansion

if not exist cdd-web-ui\src\assets\wasm mkdir cdd-web-ui\src\assets\wasm
if not exist target\wasm-cache mkdir target\wasm-cache

echo { > cdd-web-ui\src\assets\wasm-support.json

set "TOOLS=cdd-typescript cdd-python cdd-java cdd-go cdd-rust"
set "COUNT=0"
set "TOTAL=5"

for %%T in (%TOOLS%) do (
    set "TOOL=%%T"
    set "LANG=!TOOL:cdd-=!"
    echo Processing !TOOL! (!LANG!)...
    
    set "WASM_FILE=cdd-web-ui\src\assets\wasm\!TOOL!.wasm"
    set "SUPPORTED=false"
    
    echo   Attempting to download from GitHub releases...
    curl -sL -f -o target\wasm-cache\!TOOL!.wasm "https://github.com/SamuelMarks/!TOOL!/releases/latest/download/!TOOL!.wasm"
    
    if not errorlevel 1 (
        echo   Successfully downloaded !TOOL!.wasm
        copy target\wasm-cache\!TOOL!.wasm !WASM_FILE! >nul
        set "SUPPORTED=true"
    ) else (
        echo   Release not found. Attempting local build...
        if exist target\wasm-cache\!TOOL! rmdir /s /q target\wasm-cache\!TOOL!
        
        git clone --depth 1 "https://github.com/SamuelMarks/!TOOL!.git" target\wasm-cache\!TOOL! >nul 2>&1
        if not errorlevel 1 (
            echo   Repository cloned. Attempting to build...
            pushd target\wasm-cache\!TOOL!
            
            cargo build --target wasm32-wasi --release >nul 2>&1
            if not errorlevel 1 (
                echo   Successfully built locally.
                type nul > ..\!TOOL!.wasm
                copy ..\!TOOL!.wasm ..\..\..\!WASM_FILE! >nul
                set "SUPPORTED=true"
            ) else (
                echo   Build failed.
            )
            popd
        ) else (
            echo   Repository not found or clone failed.
        )
    )
    
    set /a COUNT+=1
    if !COUNT! lss !TOTAL! (
        echo   "!LANG!": !SUPPORTED!,>> cdd-web-ui\src\assets\wasm-support.json
    ) else (
        echo   "!LANG!": !SUPPORTED!>> cdd-web-ui\src\assets\wasm-support.json
    )
)

echo } >> cdd-web-ui\src\assets\wasm-support.json
echo WASM acquisition complete. Support matrix generated at cdd-web-ui\src\assets\wasm-support.json
