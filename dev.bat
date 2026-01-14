@echo off
setlocal

REM Native ONNX Runtime DLL used by the `ort` crate (load-dynamic feature)
set "ORT_DYLIB_PATH=C:\Users\amazi\Documents\meeting_scribe\ort-runtime\onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll"

REM Build toolchain hints (for crates that use CMake / C/C++)
set "CMAKE_GENERATOR=Ninja"
set "CFLAGS=/MD"
set "CXXFLAGS=/MD"

REM Protobuf compiler paths (if your Rust deps need protoc at build time)
set "PROTOC=C:\Users\amazi\AppData\Local\Microsoft\WinGet\Links\protoc.exe"
set "PROTOC_INCLUDE=C:\protoc\include"

pnpm tauri dev
