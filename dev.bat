@echo off
set ORT_DYLIB_PATH=C:\Users\amazi\Documents\meeting_scribe\ort-runtime\onnxruntime-win-x64-1.22.0\lib\onnxruntime.dll
$env:CMAKE_GENERATOR = "Ninja"
$env:CFLAGS = "/MD"
$env:CXXFLAGS = "/MD"
$env:PROTOC = "C:\Users\amazi\AppData\Local\Microsoft\WinGet\Links\protoc.exe"
$env:PROTOC_INCLUDE = "C:\protoc\include"
pnpm tauri dev
