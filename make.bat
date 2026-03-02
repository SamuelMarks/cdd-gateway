@echo off
setlocal EnableDelayedExpansion

set TARGET=%1
if "%TARGET%"=="" set TARGET=help

if "%TARGET%"=="help" goto help
if "%TARGET%"=="all" goto help
if "%TARGET%"=="install_base" goto install_base
if "%TARGET%"=="install_deps" goto install_deps
if "%TARGET%"=="build_docs" goto build_docs
if "%TARGET%"=="build" goto build
if "%TARGET%"=="test" goto test
if "%TARGET%"=="run" goto run
if "%TARGET%"=="build_docker" goto build_docker
if "%TARGET%"=="run_docker" goto run_docker

echo Unknown target: %TARGET%
goto help

:help
echo Available tasks:
echo   install_base     Install language runtime (Zig) and native deps
echo   install_deps     Install local dependencies (none required for Zig)
echo   build_docs [dir] Build API docs. Optional alternative dir (default: docs)
echo   build [dir]      Build CLI binary. Optional alternative dir (default: zig-out\bin)
echo   test             Run tests locally
echo   run [args...]    Build and run the CLI. Appends any args to the CLI.
echo   build_docker     Build Alpine and Debian Docker images
echo   run_docker       Run and test Docker images locally
goto :eof

:install_base
echo Installing Zig...
echo Please use winget: winget install -e --id zig.zig
goto :eof

:install_deps
echo No external packages to install. Zig manages deps internally via build.zig.zon.
goto :eof

:build_docs
set DOCS_DIR=%2
if "%DOCS_DIR%"=="" set DOCS_DIR=docs
zig build docs --prefix %DOCS_DIR%
goto :eof

:build
set BIN_DIR=%2
if "%BIN_DIR%"=="" set BIN_DIR=zig-out\bin
zig build -Doptimize=ReleaseSafe --prefix %BIN_DIR%
goto :eof

:test
zig build test
goto :eof

:run
set BIN_DIR=zig-out\bin
call :build
shift
set ARGS=
:run_loop
if "%~1"=="" goto run_exec
set ARGS=!ARGS! %1
shift
goto run_loop
:run_exec
!BIN_DIR!\bin\cdd-ctl.exe !ARGS!
goto :eof

:build_docker
docker build -t cdd-ctl-alpine -f alpine.Dockerfile .
docker build -t cdd-ctl-debian -f debian.Dockerfile .
goto :eof

:run_docker
echo Testing Alpine Image...
docker run -d -p 8080:8080 --name cdd-ctl-alpine-test cdd-ctl-alpine
timeout /t 2 /nobreak >nul
curl -s http://localhost:8080
docker stop cdd-ctl-alpine-test
docker rm cdd-ctl-alpine-test

echo.
echo Testing Debian Image...
docker run -d -p 8081:8080 --name cdd-ctl-debian-test cdd-ctl-debian
timeout /t 2 /nobreak >nul
curl -s http://localhost:8081
docker stop cdd-ctl-debian-test
docker rm cdd-ctl-debian-test
docker rmi cdd-ctl-alpine cdd-ctl-debian
goto :eof