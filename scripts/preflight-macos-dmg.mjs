#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, rmSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const RW_DMG_NAME = /^rw\.\d+\..+\.dmg$/;

function run(command, args) {
  return spawnSync(command, args, { encoding: 'utf8' });
}

function collectProjectRwDevices(repoRoot) {
  const result = run('hdiutil', ['info']);
  if (result.status !== 0 || !result.stdout) {
    return [];
  }

  const lines = result.stdout.split(/\r?\n/);
  const devices = new Set();
  const projectMarker = `${repoRoot}/src-tauri/target/`;
  let imagePath = '';

  for (const line of lines) {
    if (line.startsWith('image-path')) {
      const separator = line.indexOf(':');
      imagePath = separator >= 0 ? line.slice(separator + 1).trim() : '';
      continue;
    }

    const devMatch = line.match(/^\/dev\/disk\d+\b/);
    if (!devMatch) {
      if (line.startsWith('===')) {
        imagePath = '';
      }
      continue;
    }

    if (
      imagePath.includes(projectMarker) &&
      /\/bundle\/macos\/rw\.\d+\..+\.dmg$/.test(imagePath)
    ) {
      devices.add(devMatch[0]);
    }
  }

  return Array.from(devices);
}

function detachProjectRwDevices(repoRoot) {
  const devices = collectProjectRwDevices(repoRoot);
  for (const device of devices) {
    let detached = run('hdiutil', ['detach', device]);
    if (detached.status === 0) {
      continue;
    }

    detached = run('hdiutil', ['detach', '-force', device]);
    if (detached.status !== 0) {
      const stderr = detached.stderr?.trim();
      console.warn(
        `[preflight-dmg] Failed to detach ${device}${
          stderr ? `: ${stderr}` : ''
        }`
      );
    }
  }
  return devices.length;
}

function removeStaleRwDmgs(repoRoot) {
  const macosBundleDirs = [
    join(repoRoot, 'src-tauri', 'target', 'debug', 'bundle', 'macos'),
    join(repoRoot, 'src-tauri', 'target', 'release', 'bundle', 'macos'),
  ];

  let removed = 0;
  for (const bundleDir of macosBundleDirs) {
    if (!existsSync(bundleDir)) {
      continue;
    }

    for (const entry of readdirSync(bundleDir)) {
      if (!RW_DMG_NAME.test(entry)) {
        continue;
      }
      rmSync(join(bundleDir, entry), { force: true });
      removed += 1;
    }
  }

  return removed;
}

function main() {
  if (process.platform !== 'darwin') {
    return;
  }

  const __filename = fileURLToPath(import.meta.url);
  const __dirname = dirname(__filename);
  const repoRoot = resolve(__dirname, '..');

  const detached = detachProjectRwDevices(repoRoot);
  const removed = removeStaleRwDmgs(repoRoot);

  if (detached > 0 || removed > 0) {
    console.log(
      `[preflight-dmg] detached ${detached} mount(s), removed ${removed} stale interstitial image(s).`
    );
  }
}

main();
