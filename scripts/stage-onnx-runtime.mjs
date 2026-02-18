#!/usr/bin/env node

import { spawnSync } from 'node:child_process';
import {
  copyFileSync,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readdirSync,
  readFileSync,
  rmSync,
  statSync,
  symlinkSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { createWriteStream } from 'node:fs';
import { tmpdir } from 'node:os';
import { basename, dirname, join, resolve } from 'node:path';
import { pipeline } from 'node:stream/promises';
import { Readable } from 'node:stream';
import { fileURLToPath } from 'node:url';

const ONNX_RUNTIME_VERSION = '1.22.0';
const RELEASE_BASE_URL = `https://github.com/microsoft/onnxruntime/releases/download/v${ONNX_RUNTIME_VERSION}`;

const __filename = fileURLToPath(import.meta.url);
const __dirname = dirname(__filename);
const repoRoot = resolve(__dirname, '..');
const runtimeDir = join(repoRoot, 'src-tauri', 'resources', 'runtime');
const manifestPath = join(runtimeDir, '.onnxruntime-staged.json');

function platformSpec() {
  const platform = process.platform;
  const arch = process.arch;

  if (platform === 'darwin' && arch === 'arm64') {
    return {
      platform,
      arch,
      asset: `onnxruntime-osx-arm64-${ONNX_RUNTIME_VERSION}.tgz`,
      expectedLibRegex: /^libonnxruntime.*\.dylib$/,
    };
  }

  if (platform === 'darwin' && arch === 'x64') {
    return {
      platform,
      arch,
      asset: `onnxruntime-osx-x86_64-${ONNX_RUNTIME_VERSION}.tgz`,
      expectedLibRegex: /^libonnxruntime.*\.dylib$/,
    };
  }

  if (platform === 'linux' && arch === 'x64') {
    return {
      platform,
      arch,
      asset: `onnxruntime-linux-x64-${ONNX_RUNTIME_VERSION}.tgz`,
      expectedLibRegex: /^libonnxruntime.*\.so(\..*)?$/,
    };
  }

  if (platform === 'win32' && arch === 'x64') {
    return {
      platform,
      arch,
      asset: `onnxruntime-win-x64-${ONNX_RUNTIME_VERSION}.zip`,
      expectedLibRegex: /^onnxruntime.*\.dll$/i,
    };
  }

  throw new Error(
    `Unsupported platform for ONNX runtime staging: ${platform}/${arch}.`
  );
}

function walkFiles(startDir) {
  const files = [];
  const stack = [startDir];

  while (stack.length > 0) {
    const dir = stack.pop();
    if (!dir) {
      continue;
    }

    for (const entry of readdirSync(dir)) {
      const fullPath = join(dir, entry);
      const stats = lstatSync(fullPath);
      if (stats.isDirectory()) {
        stack.push(fullPath);
      } else if (stats.isFile() || stats.isSymbolicLink()) {
        files.push(fullPath);
      }
    }
  }

  return files;
}

function readManifest() {
  if (!existsSync(manifestPath)) {
    return null;
  }

  try {
    return JSON.parse(readFileSync(manifestPath, 'utf8'));
  } catch {
    return null;
  }
}

function hasExpectedRuntimeFiles(spec, files) {
  if (!Array.isArray(files) || files.length === 0) {
    return false;
  }
  return files.every((fileName) => {
    if (typeof fileName !== 'string') {
      return false;
    }
    return (
      spec.expectedLibRegex.test(fileName) &&
      existsSync(join(runtimeDir, fileName))
    );
  });
}

function isManifestUsable(spec, manifest) {
  if (!manifest) {
    return false;
  }

  if (
    manifest.version !== ONNX_RUNTIME_VERSION ||
    manifest.platform !== spec.platform ||
    manifest.arch !== spec.arch ||
    manifest.asset !== spec.asset
  ) {
    return false;
  }

  return hasExpectedRuntimeFiles(spec, manifest.files);
}

function resolveCodesignIdentity() {
  const identity = process.env.APPLE_SIGNING_IDENTITY;
  if (typeof identity === 'string' && identity.trim() !== '') {
    return identity.trim();
  }
  return '-';
}

function codesignMacRuntimeLibraries(files) {
  if (process.platform !== 'darwin') {
    return;
  }
  if (!Array.isArray(files) || files.length === 0) {
    return;
  }

  const identity = resolveCodesignIdentity();
  const signableFiles = files
    .map((name) => join(runtimeDir, name))
    .filter((filePath) => {
      try {
        return lstatSync(filePath).isFile();
      } catch {
        return false;
      }
    });

  for (const filePath of signableFiles) {
    const signed = spawnSync('codesign', ['--force', '--sign', identity, filePath], {
      encoding: 'utf8',
      stdio: 'pipe',
    });
    if (signed.error) {
      throw new Error(`Failed to run codesign: ${signed.error.message}`);
    }
    if (typeof signed.status === 'number' && signed.status !== 0) {
      const details = [signed.stdout, signed.stderr].filter(Boolean).join('\n').trim();
      throw new Error(
        `codesign failed for ${filePath} with identity "${identity}"${
          details ? `: ${details}` : ''
        }`
      );
    }
  }

  console.log(
    `[onnx] Re-signed ${signableFiles.length} runtime dylib(s) with identity "${identity}"`
  );
}

async function downloadFile(url, destinationPath) {
  const response = await fetch(url);
  if (!response.ok || !response.body) {
    throw new Error(
      `Failed to download ${url}: ${response.status} ${response.statusText}`
    );
  }

  await pipeline(Readable.fromWeb(response.body), createWriteStream(destinationPath));
}

function extractArchive(archivePath, extractDir) {
  const isZip = archivePath.toLowerCase().endsWith('.zip');
  if (!isZip) {
    const tar = spawnSync('tar', ['-xzf', archivePath, '-C', extractDir], {
      stdio: 'inherit',
    });
    if (tar.status !== 0) {
      throw new Error(`Failed to extract archive with tar (exit ${tar.status ?? 'unknown'})`);
    }
    return;
  }

  const isWindows = process.platform === 'win32';
  if (!isWindows) {
    const unzip = spawnSync('unzip', ['-q', archivePath, '-d', extractDir], {
      stdio: 'inherit',
    });
    if (unzip.status !== 0) {
      throw new Error(
        `Failed to extract zip archive with unzip (exit ${unzip.status ?? 'unknown'})`
      );
    }
    return;
  }

  const script = `Expand-Archive -Path "${archivePath}" -DestinationPath "${extractDir}" -Force`;
  const powershell = spawnSync('powershell', ['-NoProfile', '-Command', script], {
    stdio: 'inherit',
  });
  if (powershell.status !== 0) {
    throw new Error(
      `Failed to extract zip archive with PowerShell (exit ${powershell.status ?? 'unknown'})`
    );
  }
}

function clearStaleRuntimeFiles() {
  if (!existsSync(runtimeDir)) {
    return;
  }

  for (const entry of readdirSync(runtimeDir)) {
    const filePath = join(runtimeDir, entry);
    let stats;
    try {
      stats = lstatSync(filePath);
    } catch {
      continue;
    }
    if (!stats.isFile() && !stats.isSymbolicLink()) {
      continue;
    }

    const isRuntimeLib =
      /^libonnxruntime.*\.dylib$/.test(entry) ||
      /^libonnxruntime.*\.so(\..*)?$/.test(entry) ||
      /^onnxruntime.*\.dll$/i.test(entry);
    const isManifest = entry === '.onnxruntime-staged.json';
    if (isRuntimeLib || isManifest) {
      rmSync(filePath, { force: true });
    }
  }
}

function relinkCanonicalRuntime(spec, copiedFiles) {
  if (spec.platform === 'win32') {
    return;
  }

  const canonicalName =
    spec.platform === 'linux' ? 'libonnxruntime.so' : 'libonnxruntime.dylib';
  if (!copiedFiles.includes(canonicalName)) {
    return;
  }

  const versioned = copiedFiles.find((name) => {
    if (name === canonicalName) {
      return false;
    }
    if (spec.platform === 'linux') {
      return /^libonnxruntime\.so\.\S+/.test(name);
    }
    return /^libonnxruntime\.\S+\.dylib$/.test(name);
  });
  if (!versioned) {
    return;
  }

  const canonicalPath = join(runtimeDir, canonicalName);
  try {
    unlinkSync(canonicalPath);
  } catch {
    // Ignore if file is already absent.
  }
  symlinkSync(versioned, canonicalPath);
}

async function main() {
  const spec = platformSpec();
  const releaseUrl = `${RELEASE_BASE_URL}/${spec.asset}`;
  const forceRestage = process.argv.includes('--force');
  const existingManifest = readManifest();

  mkdirSync(runtimeDir, { recursive: true });

  if (!forceRestage && isManifestUsable(spec, existingManifest)) {
    codesignMacRuntimeLibraries(existingManifest.files);
    console.log(
      `[onnx] Runtime ${ONNX_RUNTIME_VERSION} already staged for ${spec.platform}/${spec.arch}`
    );
    return;
  }

  console.log(`[onnx] Staging ONNX Runtime ${ONNX_RUNTIME_VERSION} (${spec.platform}/${spec.arch})`);
  clearStaleRuntimeFiles();

  const workingDir = mkdtempSync(join(tmpdir(), 'meeting-scribe-onnx-'));

  const archivePath = join(workingDir, spec.asset);
  const extractDir = join(workingDir, 'extract');
  mkdirSync(extractDir, { recursive: true });

  try {
    console.log(`[onnx] Downloading ${releaseUrl}`);
    await downloadFile(releaseUrl, archivePath);

    console.log('[onnx] Extracting archive');
    extractArchive(archivePath, extractDir);

    const selected = new Map();
    for (const filePath of walkFiles(extractDir)) {
      const fileName = basename(filePath);
      if (!spec.expectedLibRegex.test(fileName)) {
        continue;
      }
      let targetStats;
      try {
        targetStats = statSync(filePath);
      } catch {
        continue;
      }
      if (!targetStats.isFile()) {
        continue;
      }
      if (!selected.has(fileName)) {
        selected.set(fileName, filePath);
      }
    }

    if (selected.size === 0) {
      throw new Error('No ONNX runtime libraries found in extracted archive.');
    }

    const copied = [];
    for (const [fileName, srcPath] of selected.entries()) {
      const destPath = join(runtimeDir, fileName);
      copyFileSync(srcPath, destPath);
      copied.push(fileName);
    }

    copied.sort();
    relinkCanonicalRuntime(spec, copied);
    codesignMacRuntimeLibraries(copied);
    const manifest = {
      version: ONNX_RUNTIME_VERSION,
      platform: spec.platform,
      arch: spec.arch,
      asset: spec.asset,
      url: releaseUrl,
      files: copied,
    };
    writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`, 'utf8');

    console.log(`[onnx] Staged ${copied.length} runtime file(s) into ${runtimeDir}`);
  } finally {
    rmSync(workingDir, { recursive: true, force: true });
  }
}

main().catch((error) => {
  console.error(
    `[onnx] Failed to stage ONNX Runtime: ${
      error instanceof Error ? error.message : String(error)
    }`
  );
  process.exit(1);
});
