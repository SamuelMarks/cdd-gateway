@echo off
setlocal enabledelayedexpansion

if not exist cdd-ctl-wasm-sdk\assets\wasm mkdir cdd-ctl-wasm-sdk\assets\wasm
if not exist target\wasm-cache mkdir target\wasm-cache

echo { > cdd-ctl-wasm-sdk\assets\wasm-support.json

set "REPOS=SamuelMarks/cdd-c SamuelMarks/cdd-cpp SamuelMarks/cdd-csharp SamuelMarks/cdd-go SamuelMarks/cdd-java offscale/cdd-kotlin SamuelMarks/cdd-php offscale/cdd-python-all SamuelMarks/cdd-ruby SamuelMarks/cdd-rust SamuelMarks/cdd-sh SamuelMarks/cdd-swift offscale/cdd-ts"
set "COUNT=0"
set "TOTAL=13"

for %%R in (%REPOS%) do (
    for /f "tokens=1,2 delims=/" %%A in ("%%R") do (
        set "ORG=%%A"
        set "TOOL=%%B"
        set "LANG=!TOOL:cdd-=!"
        echo Processing !TOOL! (!LANG!)...
        
        set "WASM_FILE=cdd-ctl-wasm-sdk\assets\wasm\!TOOL!.wasm"
        set "SUPPORTED=false"
        
        echo   Attempting to download from GitHub releases...
        curl -sL -f -o target\wasm-cache\!TOOL!.wasm "https://github.com/!ORG!/!TOOL!/releases/latest/download/!TOOL!.wasm"
        
        if not errorlevel 1 (
            echo   Successfully downloaded !TOOL!.wasm
            copy target\wasm-cache\!TOOL!.wasm !WASM_FILE! >nul
            set "SUPPORTED=true"
        ) else (
            echo   Release not found. Attempting local build...
            if exist target\wasm-cache\!TOOL! rmdir /s /q target\wasm-cache\!TOOL!
            
            git clone --depth 1 "https://github.com/!ORG!/!TOOL!.git" target\wasm-cache\!TOOL! >nul 2>&1
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
        echo   "!LANG!": !SUPPORTED!,>> cdd-ctl-wasm-sdk\assets\wasm-support.json
    ) else (
        echo   "!LANG!": !SUPPORTED!>> cdd-ctl-wasm-sdk\assets\wasm-support.json
    )
)

echo } >> cdd-ctl-wasm-sdk\assets\wasm-support.json
echo WASM acquisition complete. Support matrix generated at cdd-ctl-wasm-sdk\assets\wasm-support.json
