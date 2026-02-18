#!/usr/bin/env node

import { existsSync, readFileSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const MIN_DEPLOYMENT_TARGET = '11.0';

function parseVersion(value) {
  return String(value)
    .trim()
    .split('.')
    .map((part) => Number.parseInt(part, 10))
    .filter((n) => Number.isFinite(n));
}

function compareVersions(a, b) {
  const left = parseVersion(a);
  const right = parseVersion(b);
  const length = Math.max(left.length, right.length);

  for (let i = 0; i < length; i += 1) {
    const l = left[i] ?? 0;
    const r = right[i] ?? 0;
    if (l < r) {
      return -1;
    }
    if (l > r) {
      return 1;
    }
  }

  return 0;
}

function detectConfiguredDeploymentTarget(cacheContents) {
  const targetLine = cacheContents.match(
    /^CMAKE_OSX_DEPLOYMENT_TARGET:STRING=(.+)$/m
  );
  if (targetLine?.[1]) {
    return targetLine[1].trim();
  }

  const flagLine = cacheContents.match(
    /^CMAKE_CXX_FLAGS:STRING=.*-mmacosx-version-min=([0-9.]+)/m
  );
  if (flagLine?.[1]) {
    return flagLine[1].trim();
  }

  return null;
}

function cleanStaleLlamaBuildDirs(repoRoot) {
  const profiles = ['debug', 'release'];
  let cleaned = 0;

  for (const profile of profiles) {
    const buildRoot = join(repoRoot, 'src-tauri', 'target', profile, 'build');
    if (!existsSync(buildRoot)) {
      continue;
    }

    for (const entry of readdirSync(buildRoot)) {
      if (!entry.startsWith('llama-cpp-sys-2-')) {
        continue;
      }

      const outDir = join(buildRoot, entry, 'out');
      const cachePath = join(outDir, 'build', 'CMakeCache.txt');
      if (!existsSync(cachePath)) {
        continue;
      }

      let cache;
      try {
        cache = readFileSync(cachePath, 'utf8');
      } catch {
        continue;
      }

      const configuredTarget = detectConfiguredDeploymentTarget(cache);
      if (!configuredTarget) {
        continue;
      }

      if (compareVersions(configuredTarget, MIN_DEPLOYMENT_TARGET) >= 0) {
        continue;
      }

      rmSync(outDir, { recursive: true, force: true });
      cleaned += 1;
      console.log(
        `[preflight-llama] Removed stale llama CMake cache (${profile}) with deployment target ${configuredTarget}.`
      );
    }
  }

  return cleaned;
}

function main() {
  if (process.platform !== 'darwin') {
    return;
  }

  const __filename = fileURLToPath(import.meta.url);
  const __dirname = dirname(__filename);
  const repoRoot = resolve(__dirname, '..');

  const cleaned = cleanStaleLlamaBuildDirs(repoRoot);
  if (cleaned > 0) {
    console.log(
      `[preflight-llama] cleaned ${cleaned} stale llama-cpp-sys CMake build director${cleaned === 1 ? 'y' : 'ies'}.`
    );
  }
}

main();
