#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, '..');

function runOrExit(command, args) {
  const result = spawnSync(command, args, {
    cwd: repoRoot,
    stdio: 'inherit',
    shell: process.platform === 'win32',
  });

  if (result.error) {
    console.error(`[tauri-wrapper] Failed to run ${command}: ${result.error.message}`);
    process.exit(1);
  }

  if (typeof result.status === 'number' && result.status !== 0) {
    process.exit(result.status);
  }
}

function main() {
  const args = process.argv.slice(2);

  if (process.platform === 'darwin') {
    // Ensure native C/C++ dependencies (cmake-based crates) use the app baseline.
    process.env.MACOSX_DEPLOYMENT_TARGET ??= '11.0';
    process.env.CMAKE_OSX_DEPLOYMENT_TARGET ??= '11.0';
  }

  // Keep ONNX runtime staging consistent for dev/build/release flows.
  runOrExit('pnpm', ['stage:onnx-runtime']);

  // macOS preflight checks for reliable local builds.
  if (process.platform === 'darwin' && args[0] === 'build') {
    runOrExit('node', ['scripts/preflight-macos-llama-cmake.mjs']);
    runOrExit('node', ['scripts/preflight-macos-dmg.mjs']);
  }

  runOrExit('tauri', args);
}

main();
