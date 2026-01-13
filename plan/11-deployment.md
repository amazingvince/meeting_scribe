# 11. Deployment & Distribution

**Goal:** Configure release builds, create platform installers, implement auto-updates, and set up CI/CD pipelines for Meeting Scribe.

**Estimated Time:** 5-6 days

**Prerequisites:**
- All previous documents completed (00-10)
- Application fully functional on all platforms
- Code signing certificates (for production)
- GitHub repository set up

## Table of Contents

1. [Build Optimization](#build-optimization)
2. [Windows Distribution](#windows-distribution)
3. [macOS Distribution](#macos-distribution)
4. [Linux Distribution](#linux-distribution)
5. [Auto-Update System](#auto-update-system)
6. [CI/CD Pipeline](#cicd-pipeline)
7. [Release Management](#release-management)
8. [Model Distribution](#model-distribution)
9. [Troubleshooting](#troubleshooting)

---

## Build Optimization

### References

- [Tauri Build Configuration](https://tauri.app/v2/reference/config/)
- [Rust Release Profile](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [min-sized-rust](https://github.com/nicholasbeadle/min-sized-rust) - Size optimization guide

### Cargo Release Profile

Configure optimized release builds in `src-tauri/Cargo.toml`:

```toml
# src-tauri/Cargo.toml

[profile.release]
# Enable Link-Time Optimization for smaller, faster binaries
lto = true
# Use single codegen unit for maximum optimization
codegen-units = 1
# Optimize for size while maintaining performance
opt-level = "z"
# Strip debug symbols
strip = "symbols"
# Enable panic abort for smaller binaries
panic = "abort"

[profile.release-debug]
# Profile for debugging release builds
inherits = "release"
debug = true
strip = "none"

[profile.dev]
# Faster dev builds
opt-level = 0
debug = true
incremental = true

[profile.dev.package."*"]
# Optimize dependencies even in dev
opt-level = 2
```

### Tauri Build Configuration

Update `src-tauri/tauri.conf.json` for production:

```json
{
  "$schema": "https://schema.tauri.app/config/2",
  "productName": "Meeting Scribe",
  "version": "1.0.0",
  "identifier": "com.meetingscribe.app",
  "build": {
    "beforeDevCommand": "pnpm dev",
    "devUrl": "http://localhost:5173",
    "beforeBuildCommand": "pnpm build",
    "frontendDist": "../dist"
  },
  "app": {
    "windows": [
      {
        "title": "Meeting Scribe",
        "width": 1200,
        "height": 800,
        "minWidth": 800,
        "minHeight": 600,
        "resizable": true,
        "fullscreen": false,
        "decorations": false,
        "transparent": false,
        "center": true
      }
    ],
    "security": {
      "csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
    },
    "trayIcon": {
      "iconPath": "icons/icon.png",
      "iconAsTemplate": true
    }
  },
  "bundle": {
    "active": true,
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "resources": [
      "resources/*"
    ],
    "copyright": "© 2025 Meeting Scribe",
    "category": "Productivity",
    "shortDescription": "Local-first meeting transcription and AI assistant",
    "longDescription": "Meeting Scribe captures, transcribes, and summarizes your meetings locally with AI. Features include real-time transcription, intelligent summaries, and a RAG-powered chat interface.",
    "targets": "all",
    "createUpdaterArtifacts": true,
    "windows": {
      "certificateThumbprint": null,
      "digestAlgorithm": "sha256",
      "timestampUrl": "http://timestamp.digicert.com",
      "nsis": {
        "installerIcon": "icons/icon.ico",
        "headerImage": "icons/nsis-header.bmp",
        "sidebarImage": "icons/nsis-sidebar.bmp",
        "license": "../LICENSE",
        "installMode": "currentUser"
      },
      "wix": null
    },
    "macOS": {
      "entitlements": "entitlements.plist",
      "exceptionDomain": "",
      "frameworks": [],
      "minimumSystemVersion": "10.15",
      "signingIdentity": null,
      "providerShortName": null
    },
    "linux": {
      "appimage": {
        "bundleMediaFramework": true
      },
      "deb": {
        "depends": [
          "libwebkit2gtk-4.1-0",
          "libgtk-3-0",
          "libayatana-appindicator3-1",
          "pipewire",
          "libpipewire-0.3-0"
        ],
        "section": "sound",
        "priority": "optional"
      },
      "rpm": {
        "depends": [
          "webkit2gtk4.1",
          "gtk3",
          "pipewire",
          "pipewire-libs"
        ]
      }
    }
  },
  "plugins": {
    "updater": {
      "pubkey": "YOUR_PUBLIC_KEY_HERE",
      "endpoints": [
        "https://releases.meetingscribe.app/{{target}}/{{arch}}/{{current_version}}"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

### Frontend Build Optimization

Configure Vite for production in `vite.config.ts`:

```typescript
// vite.config.ts
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';
import { visualizer } from 'rollup-plugin-visualizer';

export default defineConfig({
  plugins: [
    react(),
    // Analyze bundle size (run with ANALYZE=true)
    process.env.ANALYZE && visualizer({
      open: true,
      gzipSize: true,
      brotliSize: true,
    }),
  ].filter(Boolean),
  
  build: {
    // Target modern browsers for smaller bundles
    target: 'es2021',
    // Minification
    minify: 'terser',
    terserOptions: {
      compress: {
        drop_console: true,
        drop_debugger: true,
      },
    },
    // Code splitting
    rollupOptions: {
      output: {
        manualChunks: {
          // Vendor chunks
          'react-vendor': ['react', 'react-dom', 'react-router-dom'],
          'ui-vendor': ['framer-motion', 'lucide-react'],
          'data-vendor': ['zustand', '@tanstack/react-query'],
        },
      },
    },
    // Source maps for production debugging (optional)
    sourcemap: false,
    // Asset optimization
    assetsInlineLimit: 4096,
    chunkSizeWarningLimit: 1000,
  },
  
  // Optimize dependencies
  optimizeDeps: {
    include: ['react', 'react-dom', 'zustand', '@tanstack/react-query'],
  },
});
```

### Binary Size Reduction

```rust
// src-tauri/build.rs
fn main() {
    // Compress resources during build
    #[cfg(feature = "compress-resources")]
    {
        use std::process::Command;
        
        // Compress large model files if bundled
        let resources_dir = std::path::Path::new("resources");
        if resources_dir.exists() {
            println!("cargo:rerun-if-changed=resources/");
        }
    }
    
    tauri_build::build()
}
```

### Build Size Targets

| Component | Development | Release | Notes |
|-----------|-------------|---------|-------|
| Rust Binary | ~150 MB | ~25 MB | With LTO + strip |
| Frontend | ~5 MB | ~800 KB | Minified + compressed |
| Total App | ~160 MB | ~30 MB | Without models |
| With Models | N/A | ~2.5 GB | Downloaded separately |

---

## Windows Distribution

### References

- [Tauri Windows Distribution](https://tauri.app/v2/distribute/windows/)
- [NSIS Documentation](https://nsis.sourceforge.io/Docs/)
- [Windows Code Signing](https://docs.microsoft.com/en-us/windows/win32/seccrypto/signtool)

### NSIS Installer Configuration

Create `src-tauri/nsis/installer.nsi` for custom NSIS configuration:

```nsis
; src-tauri/nsis/installer.nsi
; Custom NSIS installer script

!include "MUI2.nsh"
!include "FileFunc.nsh"

; Installer attributes
Name "Meeting Scribe"
OutFile "MeetingScribe-Setup.exe"
InstallDir "$LOCALAPPDATA\Meeting Scribe"
RequestExecutionLevel user

; Modern UI settings
!define MUI_ABORTWARNING
!define MUI_ICON "..\..\icons\icon.ico"
!define MUI_UNICON "..\..\icons\icon.ico"
!define MUI_HEADERIMAGE
!define MUI_HEADERIMAGE_BITMAP "..\..\icons\nsis-header.bmp"
!define MUI_WELCOMEFINISHPAGE_BITMAP "..\..\icons\nsis-sidebar.bmp"

; Pages
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_LICENSE "..\..\..\LICENSE"
!insertmacro MUI_PAGE_DIRECTORY
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH

!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES

!insertmacro MUI_LANGUAGE "English"

; Installation section
Section "Install"
    SetOutPath $INSTDIR
    
    ; Copy application files
    File /r "..\..\target\release\bundle\nsis\*.*"
    
    ; Create data directories
    CreateDirectory "$APPDATA\meeting-scribe"
    CreateDirectory "$APPDATA\meeting-scribe\models"
    CreateDirectory "$APPDATA\meeting-scribe\data"
    CreateDirectory "$APPDATA\meeting-scribe\audio"
    
    ; Create start menu shortcuts
    CreateDirectory "$SMPROGRAMS\Meeting Scribe"
    CreateShortcut "$SMPROGRAMS\Meeting Scribe\Meeting Scribe.lnk" "$INSTDIR\Meeting Scribe.exe"
    CreateShortcut "$SMPROGRAMS\Meeting Scribe\Uninstall.lnk" "$INSTDIR\uninstall.exe"
    
    ; Desktop shortcut (optional)
    CreateShortcut "$DESKTOP\Meeting Scribe.lnk" "$INSTDIR\Meeting Scribe.exe"
    
    ; Write uninstaller
    WriteUninstaller "$INSTDIR\uninstall.exe"
    
    ; Registry entries for Add/Remove Programs
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "DisplayName" "Meeting Scribe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "UninstallString" "$INSTDIR\uninstall.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "DisplayIcon" "$INSTDIR\Meeting Scribe.exe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "Publisher" "Meeting Scribe"
    WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "DisplayVersion" "${VERSION}"
    
    ; Calculate installed size
    ${GetSize} "$INSTDIR" "/S=0K" $0 $1 $2
    IntFmt $0 "0x%08X" $0
    WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe" \
        "EstimatedSize" "$0"
SectionEnd

; Uninstallation section
Section "Uninstall"
    ; Remove application files
    RMDir /r "$INSTDIR"
    
    ; Remove shortcuts
    RMDir /r "$SMPROGRAMS\Meeting Scribe"
    Delete "$DESKTOP\Meeting Scribe.lnk"
    
    ; Remove registry entries
    DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\MeetingScribe"
    
    ; Ask about user data
    MessageBox MB_YESNO "Remove user data (recordings, transcripts, settings)?" IDNO skip_data
    RMDir /r "$APPDATA\meeting-scribe"
    skip_data:
SectionEnd
```

### Windows Code Signing

Set up code signing for Windows builds:

```powershell
# scripts/sign-windows.ps1
param(
    [Parameter(Mandatory=$true)]
    [string]$CertificatePath,
    [Parameter(Mandatory=$true)]
    [string]$CertificatePassword,
    [string]$TimestampUrl = "http://timestamp.digicert.com"
)

$files = @(
    "src-tauri/target/release/Meeting Scribe.exe",
    "src-tauri/target/release/bundle/nsis/Meeting Scribe_*_x64-setup.exe"
)

foreach ($pattern in $files) {
    Get-ChildItem -Path $pattern -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "Signing: $($_.FullName)"
        & signtool sign /f $CertificatePath /p $CertificatePassword /tr $TimestampUrl /td sha256 /fd sha256 $_.FullName
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to sign $($_.FullName)"
        }
    }
}
```

### Windows Build Script

```powershell
# scripts/build-windows.ps1
param(
    [switch]$Release,
    [switch]$Sign
)

Write-Host "Building Meeting Scribe for Windows..." -ForegroundColor Cyan

# Install dependencies
Write-Host "Installing dependencies..."
pnpm install

# Build frontend
Write-Host "Building frontend..."
pnpm build

# Build Tauri app
Write-Host "Building Tauri application..."
if ($Release) {
    Set-Location src-tauri
    cargo build --release
    Set-Location ..
    pnpm tauri build
} else {
    pnpm tauri build --debug
}

# Sign if requested
if ($Sign -and $Release) {
    Write-Host "Signing binaries..."
    & "$PSScriptRoot\sign-windows.ps1" `
        -CertificatePath $env:WINDOWS_CERT_PATH `
        -CertificatePassword $env:WINDOWS_CERT_PASSWORD
}

Write-Host "Build complete!" -ForegroundColor Green
Write-Host "Output: src-tauri/target/release/bundle/nsis/"
```

### Portable Version

Create a portable ZIP version without installation:

```powershell
# scripts/create-portable-windows.ps1
$version = (Get-Content src-tauri/tauri.conf.json | ConvertFrom-Json).version
$outputDir = "dist/portable"
$zipName = "MeetingScribe-$version-win64-portable.zip"

# Create directory structure
New-Item -ItemType Directory -Force -Path $outputDir
New-Item -ItemType Directory -Force -Path "$outputDir/data"

# Copy files
Copy-Item "src-tauri/target/release/Meeting Scribe.exe" $outputDir
Copy-Item "src-tauri/target/release/*.dll" $outputDir -ErrorAction SilentlyContinue
Copy-Item "README.md" $outputDir
Copy-Item "LICENSE" $outputDir

# Create portable marker file
"" | Set-Content "$outputDir/portable.txt"

# Create ZIP
Compress-Archive -Path "$outputDir/*" -DestinationPath "dist/$zipName" -Force

Write-Host "Created: dist/$zipName"
```

---

## macOS Distribution

### References

- [Tauri macOS Distribution](https://tauri.app/v2/distribute/macos/)
- [Apple Developer - Notarization](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [Creating a DMG](https://github.com/create-dmg/create-dmg)

### Entitlements

Create `src-tauri/entitlements.plist`:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Allow JIT compilation for ONNX Runtime -->
    <key>com.apple.security.cs.allow-jit</key>
    <true/>
    
    <!-- Allow unsigned executable memory -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>
    
    <!-- Audio input for microphone capture -->
    <key>com.apple.security.device.audio-input</key>
    <true/>
    
    <!-- Screen capture for system audio -->
    <key>com.apple.security.device.screen-capture</key>
    <true/>
    
    <!-- File access for saving recordings -->
    <key>com.apple.security.files.user-selected.read-write</key>
    <true/>
    
    <!-- Network access for model downloads -->
    <key>com.apple.security.network.client</key>
    <true/>
    
    <!-- Hardened runtime -->
    <key>com.apple.security.cs.disable-library-validation</key>
    <true/>
</dict>
</plist>
```

### Info.plist Additions

Create `src-tauri/Info.plist` for additional metadata:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Microphone usage description -->
    <key>NSMicrophoneUsageDescription</key>
    <string>Meeting Scribe needs microphone access to record your voice during meetings.</string>
    
    <!-- Screen recording usage description (for system audio) -->
    <key>NSScreenCaptureUsageDescription</key>
    <string>Meeting Scribe needs screen recording permission to capture system audio from meeting applications.</string>
    
    <!-- Document types -->
    <key>CFBundleDocumentTypes</key>
    <array>
        <dict>
            <key>CFBundleTypeName</key>
            <string>Meeting Scribe Recording</string>
            <key>CFBundleTypeExtensions</key>
            <array>
                <string>mscribe</string>
            </array>
            <key>CFBundleTypeRole</key>
            <string>Editor</string>
        </dict>
    </array>
    
    <!-- URL schemes -->
    <key>CFBundleURLTypes</key>
    <array>
        <dict>
            <key>CFBundleURLName</key>
            <string>Meeting Scribe</string>
            <key>CFBundleURLSchemes</key>
            <array>
                <string>meetingscribe</string>
            </array>
        </dict>
    </array>
    
    <!-- Minimum system version -->
    <key>LSMinimumSystemVersion</key>
    <string>10.15</string>
    
    <!-- High resolution capable -->
    <key>NSHighResolutionCapable</key>
    <true/>
    
    <!-- Supports automatic graphics switching -->
    <key>NSSupportsAutomaticGraphicsSwitching</key>
    <true/>
</dict>
</plist>
```

### Code Signing and Notarization

```bash
#!/bin/bash
# scripts/sign-macos.sh

set -e

APP_PATH="src-tauri/target/release/bundle/macos/Meeting Scribe.app"
IDENTITY="${APPLE_SIGNING_IDENTITY:-}"
APPLE_ID="${APPLE_ID:-}"
APPLE_PASSWORD="${APPLE_APP_PASSWORD:-}"
TEAM_ID="${APPLE_TEAM_ID:-}"

if [ -z "$IDENTITY" ]; then
    echo "Warning: No signing identity set. Skipping code signing."
    exit 0
fi

echo "Signing application..."

# Sign all nested frameworks and binaries
find "$APP_PATH" -type f -perm +111 -exec codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist \
    --sign "$IDENTITY" {} \;

# Sign the main bundle
codesign --force --options runtime \
    --entitlements src-tauri/entitlements.plist \
    --sign "$IDENTITY" \
    "$APP_PATH"

# Verify signature
codesign --verify --deep --strict "$APP_PATH"
echo "Code signing complete."

# Notarize
if [ -n "$APPLE_ID" ] && [ -n "$APPLE_PASSWORD" ]; then
    echo "Creating ZIP for notarization..."
    ditto -c -k --keepParent "$APP_PATH" "/tmp/MeetingScribe.zip"
    
    echo "Submitting for notarization..."
    xcrun notarytool submit "/tmp/MeetingScribe.zip" \
        --apple-id "$APPLE_ID" \
        --password "$APPLE_PASSWORD" \
        --team-id "$TEAM_ID" \
        --wait
    
    echo "Stapling notarization ticket..."
    xcrun stapler staple "$APP_PATH"
    
    rm /tmp/MeetingScribe.zip
    echo "Notarization complete."
else
    echo "Warning: Apple credentials not set. Skipping notarization."
fi
```

### DMG Creation

```bash
#!/bin/bash
# scripts/create-dmg.sh

set -e

VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | cut -d'"' -f4)
APP_PATH="src-tauri/target/release/bundle/macos/Meeting Scribe.app"
DMG_NAME="MeetingScribe-${VERSION}-macos"
OUTPUT_DIR="dist"

mkdir -p "$OUTPUT_DIR"

# Check if create-dmg is installed
if ! command -v create-dmg &> /dev/null; then
    echo "Installing create-dmg..."
    brew install create-dmg
fi

echo "Creating DMG..."

create-dmg \
    --volname "Meeting Scribe" \
    --volicon "src-tauri/icons/icon.icns" \
    --window-pos 200 120 \
    --window-size 600 400 \
    --icon-size 100 \
    --icon "Meeting Scribe.app" 150 190 \
    --hide-extension "Meeting Scribe.app" \
    --app-drop-link 450 190 \
    --background "assets/dmg-background.png" \
    "${OUTPUT_DIR}/${DMG_NAME}.dmg" \
    "$APP_PATH"

echo "Created: ${OUTPUT_DIR}/${DMG_NAME}.dmg"

# Sign the DMG
if [ -n "$APPLE_SIGNING_IDENTITY" ]; then
    echo "Signing DMG..."
    codesign --sign "$APPLE_SIGNING_IDENTITY" "${OUTPUT_DIR}/${DMG_NAME}.dmg"
fi
```

### macOS Build Script

```bash
#!/bin/bash
# scripts/build-macos.sh

set -e

echo "Building Meeting Scribe for macOS..."

# Install dependencies
echo "Installing dependencies..."
pnpm install

# Build frontend
echo "Building frontend..."
pnpm build

# Build Tauri app
echo "Building Tauri application..."
pnpm tauri build

# Sign and notarize
if [ "$1" = "--release" ]; then
    ./scripts/sign-macos.sh
    ./scripts/create-dmg.sh
fi

echo "Build complete!"
echo "Output: src-tauri/target/release/bundle/macos/"
```

### Universal Binary (Intel + Apple Silicon)

```bash
#!/bin/bash
# scripts/build-universal-macos.sh

set -e

echo "Building universal binary..."

# Build for Intel
echo "Building x86_64..."
CARGO_BUILD_TARGET=x86_64-apple-darwin pnpm tauri build --target x86_64-apple-darwin

# Build for Apple Silicon
echo "Building aarch64..."
CARGO_BUILD_TARGET=aarch64-apple-darwin pnpm tauri build --target aarch64-apple-darwin

# Combine into universal binary
echo "Creating universal binary..."
mkdir -p "src-tauri/target/universal-apple-darwin/release/bundle/macos"

lipo -create \
    "src-tauri/target/x86_64-apple-darwin/release/Meeting Scribe" \
    "src-tauri/target/aarch64-apple-darwin/release/Meeting Scribe" \
    -output "src-tauri/target/universal-apple-darwin/release/Meeting Scribe"

# Copy app bundle and replace binary
cp -R "src-tauri/target/aarch64-apple-darwin/release/bundle/macos/Meeting Scribe.app" \
    "src-tauri/target/universal-apple-darwin/release/bundle/macos/"

cp "src-tauri/target/universal-apple-darwin/release/Meeting Scribe" \
    "src-tauri/target/universal-apple-darwin/release/bundle/macos/Meeting Scribe.app/Contents/MacOS/"

echo "Universal binary created!"
```

---

## Linux Distribution

### References

- [Tauri Linux Distribution](https://tauri.app/v2/distribute/linux/)
- [AppImage Documentation](https://docs.appimage.org/)
- [Flatpak Documentation](https://docs.flatpak.org/)
- [Debian Packaging](https://wiki.debian.org/Packaging)

### AppImage Configuration

AppImage is the recommended format for broad Linux distribution:

```yaml
# src-tauri/appimage/AppImageBuilder.yml
version: 1
AppDir:
  path: ./AppDir
  
  app_info:
    id: com.meetingscribe.app
    name: Meeting Scribe
    icon: meetingscribe
    version: !ENV ${VERSION}
    exec: Meeting Scribe
    exec_args: $@
  
  apt:
    arch: amd64
    sources:
      - sourceline: 'deb http://archive.ubuntu.com/ubuntu/ jammy main restricted universe'
    include:
      - libwebkit2gtk-4.1-0
      - libgtk-3-0
      - libayatana-appindicator3-1
      - pipewire
      - libpipewire-0.3-0
      - libspa-0.2-modules
    exclude:
      - humanity-icon-theme
      - hicolor-icon-theme
  
  files:
    include:
      - /usr/lib/x86_64-linux-gnu/pipewire-0.3/*
    exclude:
      - usr/share/doc
      - usr/share/man
  
  runtime:
    env:
      APPDIR_LIBRARY_PATH: $APPDIR/usr/lib/x86_64-linux-gnu

AppImage:
  arch: x86_64
  update-information: gh-releases-zsync|meetingscribe|meeting-scribe|latest|MeetingScribe-*-x86_64.AppImage.zsync
```

### Desktop Entry

Create `src-tauri/linux/meeting-scribe.desktop`:

```ini
[Desktop Entry]
Name=Meeting Scribe
Comment=Local-first meeting transcription and AI assistant
Exec=meeting-scribe %U
Icon=meeting-scribe
Terminal=false
Type=Application
Categories=AudioVideo;Audio;Utility;
Keywords=meeting;transcription;audio;notes;ai;
StartupNotify=true
StartupWMClass=meeting-scribe
MimeType=x-scheme-handler/meetingscribe;
Actions=new-recording;

[Desktop Action new-recording]
Name=New Recording
Exec=meeting-scribe --new-recording
```

### Flatpak Manifest

Create `flatpak/com.meetingscribe.app.yml`:

```yaml
app-id: com.meetingscribe.app
runtime: org.gnome.Platform
runtime-version: '45'
sdk: org.gnome.Sdk
sdk-extensions:
  - org.freedesktop.Sdk.Extension.rust-stable
  - org.freedesktop.Sdk.Extension.node18

command: meeting-scribe

finish-args:
  # Wayland and X11
  - --socket=wayland
  - --socket=fallback-x11
  - --share=ipc
  
  # Audio
  - --socket=pulseaudio
  - --device=all  # For PipeWire
  
  # File access
  - --filesystem=home
  - --filesystem=xdg-documents
  
  # Network for model downloads
  - --share=network
  
  # GPU access
  - --device=dri
  
  # D-Bus for system integration
  - --talk-name=org.freedesktop.Notifications
  - --talk-name=org.kde.StatusNotifierWatcher

modules:
  # PipeWire (for system audio capture)
  - name: pipewire
    buildsystem: meson
    config-opts:
      - -Dgstreamer=disabled
      - -Dalsa=disabled
      - -Djack=disabled
      - -Dsystemd=disabled
    sources:
      - type: git
        url: https://gitlab.freedesktop.org/pipewire/pipewire.git
        tag: 0.3.80
  
  # Main application
  - name: meeting-scribe
    buildsystem: simple
    build-options:
      append-path: /usr/lib/sdk/rust-stable/bin:/usr/lib/sdk/node18/bin
      env:
        CARGO_HOME: /run/build/meeting-scribe/cargo
        npm_config_nodedir: /usr/lib/sdk/node18
    build-commands:
      - npm install -g pnpm
      - pnpm install
      - pnpm build
      - cargo build --release --manifest-path src-tauri/Cargo.toml
      - install -Dm755 src-tauri/target/release/meeting-scribe ${FLATPAK_DEST}/bin/meeting-scribe
      - install -Dm644 src-tauri/linux/meeting-scribe.desktop ${FLATPAK_DEST}/share/applications/com.meetingscribe.app.desktop
      - install -Dm644 src-tauri/icons/128x128.png ${FLATPAK_DEST}/share/icons/hicolor/128x128/apps/meeting-scribe.png
    sources:
      - type: dir
        path: ..
```

### Debian Package Script

```bash
#!/bin/bash
# scripts/build-deb.sh

set -e

VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | cut -d'"' -f4)
ARCH="amd64"
PKG_NAME="meeting-scribe_${VERSION}_${ARCH}"

echo "Building Debian package..."

# Build the application
pnpm tauri build --bundles deb

# The .deb file is automatically created by Tauri
DEB_PATH="src-tauri/target/release/bundle/deb/meeting-scribe_${VERSION}_${ARCH}.deb"

if [ -f "$DEB_PATH" ]; then
    echo "Debian package created: $DEB_PATH"
    
    # Verify the package
    dpkg-deb --info "$DEB_PATH"
    dpkg-deb --contents "$DEB_PATH"
else
    echo "Error: Debian package not found"
    exit 1
fi
```

### RPM Package Script

```bash
#!/bin/bash
# scripts/build-rpm.sh

set -e

VERSION=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | cut -d'"' -f4)

echo "Building RPM package..."

# Build the application
pnpm tauri build --bundles rpm

RPM_PATH="src-tauri/target/release/bundle/rpm/meeting-scribe-${VERSION}-1.x86_64.rpm"

if [ -f "$RPM_PATH" ]; then
    echo "RPM package created: $RPM_PATH"
    rpm -qip "$RPM_PATH"
else
    echo "Error: RPM package not found"
    exit 1
fi
```

### Linux Build Script

```bash
#!/bin/bash
# scripts/build-linux.sh

set -e

FORMAT="${1:-all}"  # appimage, deb, rpm, flatpak, all

echo "Building Meeting Scribe for Linux..."

# Install dependencies
pnpm install
pnpm build

case "$FORMAT" in
    appimage)
        pnpm tauri build --bundles appimage
        ;;
    deb)
        pnpm tauri build --bundles deb
        ;;
    rpm)
        pnpm tauri build --bundles rpm
        ;;
    flatpak)
        flatpak-builder --force-clean build-dir flatpak/com.meetingscribe.app.yml
        flatpak build-export repo build-dir
        flatpak build-bundle repo MeetingScribe.flatpak com.meetingscribe.app
        ;;
    all)
        pnpm tauri build
        ;;
    *)
        echo "Unknown format: $FORMAT"
        echo "Usage: $0 [appimage|deb|rpm|flatpak|all]"
        exit 1
        ;;
esac

echo "Build complete!"
echo "Output: src-tauri/target/release/bundle/"
```

---

## Auto-Update System

### References

- [Tauri Updater Plugin](https://tauri.app/v2/plugin/updater/)
- [Tauri Update Server](https://github.com/nicholasbeadle/tauri-update-server)

### Generate Update Keys

```bash
#!/bin/bash
# scripts/generate-update-keys.sh

echo "Generating Tauri update keys..."

# Generate key pair
pnpm tauri signer generate -w ~/.tauri/meeting-scribe.key

echo ""
echo "Keys generated!"
echo "Private key: ~/.tauri/meeting-scribe.key"
echo "Public key: ~/.tauri/meeting-scribe.key.pub"
echo ""
echo "Add the public key to tauri.conf.json plugins.updater.pubkey"
echo "Keep the private key secure for signing updates!"
```

### Update Configuration

Add to `src-tauri/tauri.conf.json`:

```json
{
  "plugins": {
    "updater": {
      "active": true,
      "pubkey": "dW50cnVzdGVkIGNvbW1lbnQ6IG1pbmlzaWduIHB1YmxpYyBrZXk6...",
      "endpoints": [
        "https://releases.meetingscribe.app/{{target}}/{{arch}}/{{current_version}}",
        "https://github.com/meetingscribe/meeting-scribe/releases/latest/download/latest.json"
      ],
      "windows": {
        "installMode": "passive"
      }
    }
  }
}
```

### Update Server Response Format

The update endpoint should return JSON in this format:

```json
{
  "version": "1.1.0",
  "notes": "Bug fixes and performance improvements",
  "pub_date": "2025-01-15T12:00:00Z",
  "platforms": {
    "windows-x86_64": {
      "signature": "dW50cnVzdGVkIGNvbW1lbnQ6IHNpZ25hdHVyZSBmcm9tIHRhdXJpIHNlY3JldCBrZXk...",
      "url": "https://releases.meetingscribe.app/download/1.1.0/MeetingScribe_1.1.0_x64-setup.nsis.zip"
    },
    "darwin-x86_64": {
      "signature": "...",
      "url": "https://releases.meetingscribe.app/download/1.1.0/MeetingScribe_1.1.0_x64.app.tar.gz"
    },
    "darwin-aarch64": {
      "signature": "...",
      "url": "https://releases.meetingscribe.app/download/1.1.0/MeetingScribe_1.1.0_aarch64.app.tar.gz"
    },
    "linux-x86_64": {
      "signature": "...",
      "url": "https://releases.meetingscribe.app/download/1.1.0/MeetingScribe_1.1.0_amd64.AppImage.tar.gz"
    }
  }
}
```

### Frontend Update Integration

```typescript
// src/components/settings/UpdateSettings.tsx
import { useState, useEffect } from 'react';
import { check, Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';
import { Button } from '../ui/Button';
import { Progress } from '../ui/Progress';
import { RefreshCw, Download, Check, AlertCircle } from 'lucide-react';

interface UpdateState {
  checking: boolean;
  available: boolean;
  downloading: boolean;
  progress: number;
  error: string | null;
  update: Update | null;
}

export function UpdateSettings() {
  const [state, setState] = useState<UpdateState>({
    checking: false,
    available: false,
    downloading: false,
    progress: 0,
    error: null,
    update: null,
  });
  
  const checkForUpdates = async () => {
    setState(s => ({ ...s, checking: true, error: null }));
    
    try {
      const update = await check();
      
      if (update?.available) {
        setState(s => ({
          ...s,
          checking: false,
          available: true,
          update,
        }));
      } else {
        setState(s => ({
          ...s,
          checking: false,
          available: false,
        }));
      }
    } catch (error) {
      setState(s => ({
        ...s,
        checking: false,
        error: error instanceof Error ? error.message : 'Failed to check for updates',
      }));
    }
  };
  
  const downloadAndInstall = async () => {
    if (!state.update) return;
    
    setState(s => ({ ...s, downloading: true, progress: 0 }));
    
    try {
      await state.update.downloadAndInstall((progress) => {
        if (progress.event === 'Started') {
          setState(s => ({ ...s, progress: 0 }));
        } else if (progress.event === 'Progress') {
          const percent = progress.data.chunkLength / progress.data.contentLength * 100;
          setState(s => ({ ...s, progress: Math.min(s.progress + percent, 100) }));
        } else if (progress.event === 'Finished') {
          setState(s => ({ ...s, progress: 100 }));
        }
      });
      
      // Prompt to restart
      const shouldRelaunch = window.confirm(
        'Update installed! Restart Meeting Scribe to apply the update?'
      );
      
      if (shouldRelaunch) {
        await relaunch();
      }
    } catch (error) {
      setState(s => ({
        ...s,
        downloading: false,
        error: error instanceof Error ? error.message : 'Failed to install update',
      }));
    }
  };
  
  // Check for updates on mount
  useEffect(() => {
    checkForUpdates();
  }, []);
  
  return (
    <div className="space-y-4">
      <div className="flex items-center justify-between">
        <div>
          <h3 className="font-medium">Application Updates</h3>
          <p className="text-sm text-gray-500">
            Current version: {__APP_VERSION__}
          </p>
        </div>
        
        <Button
          variant="outline"
          onClick={checkForUpdates}
          disabled={state.checking || state.downloading}
        >
          <RefreshCw className={`w-4 h-4 mr-2 ${state.checking ? 'animate-spin' : ''}`} />
          Check for Updates
        </Button>
      </div>
      
      {state.error && (
        <div className="flex items-center gap-2 text-red-600 text-sm">
          <AlertCircle className="w-4 h-4" />
          {state.error}
        </div>
      )}
      
      {state.available && state.update && (
        <div className="p-4 bg-blue-50 rounded-lg space-y-3">
          <div className="flex items-center gap-2">
            <Download className="w-5 h-5 text-blue-600" />
            <span className="font-medium">
              Update available: v{state.update.version}
            </span>
          </div>
          
          {state.update.body && (
            <p className="text-sm text-gray-600 whitespace-pre-wrap">
              {state.update.body}
            </p>
          )}
          
          {state.downloading ? (
            <div className="space-y-2">
              <Progress value={state.progress} />
              <p className="text-sm text-gray-500">
                Downloading... {Math.round(state.progress)}%
              </p>
            </div>
          ) : (
            <Button onClick={downloadAndInstall}>
              <Download className="w-4 h-4 mr-2" />
              Download and Install
            </Button>
          )}
        </div>
      )}
      
      {!state.available && !state.checking && !state.error && (
        <div className="flex items-center gap-2 text-green-600 text-sm">
          <Check className="w-4 h-4" />
          You're running the latest version
        </div>
      )}
    </div>
  );
}
```

### Sign Updates During Build

```bash
#!/bin/bash
# scripts/sign-update.sh

set -e

PRIVATE_KEY="${TAURI_SIGNING_PRIVATE_KEY:-$HOME/.tauri/meeting-scribe.key}"
PASSWORD="${TAURI_SIGNING_PRIVATE_KEY_PASSWORD:-}"

if [ ! -f "$PRIVATE_KEY" ]; then
    echo "Error: Private key not found at $PRIVATE_KEY"
    exit 1
fi

# Find update artifacts
find src-tauri/target/release/bundle -name "*.sig" -delete  # Remove old signatures

for artifact in src-tauri/target/release/bundle/**/*.{zip,tar.gz,AppImage}; do
    if [ -f "$artifact" ]; then
        echo "Signing: $artifact"
        if [ -n "$PASSWORD" ]; then
            pnpm tauri signer sign -k "$PRIVATE_KEY" -p "$PASSWORD" "$artifact"
        else
            pnpm tauri signer sign -k "$PRIVATE_KEY" "$artifact"
        fi
    fi
done

echo "All artifacts signed!"
```

---

## CI/CD Pipeline

### References

- [GitHub Actions](https://docs.github.com/en/actions)
- [Tauri GitHub Action](https://github.com/tauri-apps/tauri-action)
- [GitHub Releases](https://docs.github.com/en/repositories/releasing-projects-on-github)

### Main CI Workflow

Create `.github/workflows/ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [main, develop]
  pull_request:
    branches: [main]

concurrency:
  group: ${{ github.workflow }}-${{ github.ref }}
  cancel-in-progress: true

jobs:
  # Lint and type check
  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'pnpm'
      
      - name: Install dependencies
        run: pnpm install
      
      - name: Lint frontend
        run: pnpm lint
      
      - name: Type check frontend
        run: pnpm type-check
      
      - name: Check formatting
        run: pnpm format:check

  # Rust checks
  rust:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Install system dependencies
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libpipewire-0.3-dev \
            libasound2-dev
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: clippy, rustfmt
      
      - name: Cache Rust
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
      
      - name: Check formatting
        run: cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
      
      - name: Clippy
        run: cargo clippy --manifest-path src-tauri/Cargo.toml -- -D warnings
      
      - name: Run tests
        run: cargo test --manifest-path src-tauri/Cargo.toml

  # Build for all platforms
  build:
    needs: [lint, rust]
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            name: linux
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            name: windows
          - platform: macos-latest
            target: x86_64-apple-darwin
            name: macos-intel
          - platform: macos-latest
            target: aarch64-apple-darwin
            name: macos-arm
    
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'pnpm'
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Cache Rust
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
          key: ${{ matrix.name }}
      
      # Linux dependencies
      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libpipewire-0.3-dev \
            libasound2-dev
      
      - name: Install frontend dependencies
        run: pnpm install
      
      - name: Build
        run: pnpm tauri build --target ${{ matrix.target }}
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
      
      - name: Upload artifacts
        uses: actions/upload-artifact@v4
        with:
          name: build-${{ matrix.name }}
          path: |
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.exe
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.msi
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.dmg
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.app
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.AppImage
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.deb
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.rpm
```

### Release Workflow

Create `.github/workflows/release.yml`:

```yaml
name: Release

on:
  push:
    tags:
      - 'v*'

permissions:
  contents: write

jobs:
  create-release:
    runs-on: ubuntu-latest
    outputs:
      release_id: ${{ steps.create-release.outputs.id }}
      upload_url: ${{ steps.create-release.outputs.upload_url }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Get version from tag
        id: version
        run: echo "version=${GITHUB_REF#refs/tags/v}" >> $GITHUB_OUTPUT
      
      - name: Create Release
        id: create-release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ github.ref_name }}
          name: Meeting Scribe v${{ steps.version.outputs.version }}
          draft: true
          prerelease: ${{ contains(github.ref_name, '-') }}
          generate_release_notes: true

  build-release:
    needs: create-release
    strategy:
      fail-fast: false
      matrix:
        include:
          - platform: ubuntu-22.04
            target: x86_64-unknown-linux-gnu
            name: linux
          - platform: windows-latest
            target: x86_64-pc-windows-msvc
            name: windows
          - platform: macos-latest
            target: x86_64-apple-darwin
            name: macos-intel
          - platform: macos-latest
            target: aarch64-apple-darwin
            name: macos-arm
    
    runs-on: ${{ matrix.platform }}
    steps:
      - uses: actions/checkout@v4
      
      - name: Setup pnpm
        uses: pnpm/action-setup@v2
        with:
          version: 8
      
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          cache: 'pnpm'
      
      - name: Setup Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.target }}
      
      - name: Cache Rust
        uses: Swatinem/rust-cache@v2
        with:
          workspaces: src-tauri
          key: ${{ matrix.name }}-release
      
      # Platform-specific setup
      - name: Install Linux dependencies
        if: matrix.platform == 'ubuntu-22.04'
        run: |
          sudo apt-get update
          sudo apt-get install -y \
            libwebkit2gtk-4.1-dev \
            libgtk-3-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            libpipewire-0.3-dev \
            libasound2-dev
      
      # macOS code signing setup
      - name: Setup macOS signing
        if: startsWith(matrix.platform, 'macos')
        env:
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          KEYCHAIN_PASSWORD: ${{ secrets.KEYCHAIN_PASSWORD }}
        run: |
          echo $APPLE_CERTIFICATE | base64 --decode > certificate.p12
          security create-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security default-keychain -s build.keychain
          security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain
          security import certificate.p12 -k build.keychain -P "$APPLE_CERTIFICATE_PASSWORD" -T /usr/bin/codesign
          security set-key-partition-list -S apple-tool:,apple:,codesign: -s -k "$KEYCHAIN_PASSWORD" build.keychain
      
      - name: Install frontend dependencies
        run: pnpm install
      
      - name: Build release
        run: pnpm tauri build --target ${{ matrix.target }}
        env:
          TAURI_SIGNING_PRIVATE_KEY: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY }}
          TAURI_SIGNING_PRIVATE_KEY_PASSWORD: ${{ secrets.TAURI_SIGNING_PRIVATE_KEY_PASSWORD }}
          # macOS
          APPLE_SIGNING_IDENTITY: ${{ secrets.APPLE_SIGNING_IDENTITY }}
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
          # Windows
          TAURI_BUNDLE_WINDOWS_CERT: ${{ secrets.WINDOWS_CERTIFICATE }}
          TAURI_BUNDLE_WINDOWS_CERT_PASSWORD: ${{ secrets.WINDOWS_CERTIFICATE_PASSWORD }}
      
      # macOS notarization
      - name: Notarize macOS app
        if: startsWith(matrix.platform, 'macos')
        env:
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        run: |
          for app in src-tauri/target/${{ matrix.target }}/release/bundle/macos/*.app; do
            ditto -c -k --keepParent "$app" app.zip
            xcrun notarytool submit app.zip \
              --apple-id "$APPLE_ID" \
              --password "$APPLE_PASSWORD" \
              --team-id "$APPLE_TEAM_ID" \
              --wait
            xcrun stapler staple "$app"
            rm app.zip
          done
      
      - name: Upload release assets
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ github.ref_name }}
          files: |
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.exe
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.msi
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.dmg
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.AppImage
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.deb
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.rpm
            src-tauri/target/${{ matrix.target }}/release/bundle/**/*.sig

  # Generate and upload update manifest
  update-manifest:
    needs: build-release
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: artifacts
      
      - name: Generate update manifest
        run: |
          VERSION=${GITHUB_REF#refs/tags/v}
          node scripts/generate-update-manifest.js "$VERSION" > latest.json
      
      - name: Upload update manifest
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ github.ref_name }}
          files: latest.json

  # Publish release (after manual review)
  publish-release:
    needs: [build-release, update-manifest]
    runs-on: ubuntu-latest
    environment: production  # Requires manual approval
    steps:
      - name: Publish release
        uses: softprops/action-gh-release@v1
        with:
          tag_name: ${{ github.ref_name }}
          draft: false
```

### Update Manifest Generator

Create `scripts/generate-update-manifest.js`:

```javascript
#!/usr/bin/env node
// scripts/generate-update-manifest.js

const fs = require('fs');
const path = require('path');

const version = process.argv[2];
if (!version) {
  console.error('Usage: generate-update-manifest.js <version>');
  process.exit(1);
}

const baseUrl = `https://github.com/meetingscribe/meeting-scribe/releases/download/v${version}`;

// Read signatures from artifact files
function readSignature(platform) {
  const sigFiles = {
    'windows-x86_64': `MeetingScribe_${version}_x64-setup.nsis.zip.sig`,
    'darwin-x86_64': `MeetingScribe_${version}_x64.app.tar.gz.sig`,
    'darwin-aarch64': `MeetingScribe_${version}_aarch64.app.tar.gz.sig`,
    'linux-x86_64': `MeetingScribe_${version}_amd64.AppImage.tar.gz.sig`,
  };
  
  const sigPath = path.join('artifacts', sigFiles[platform]);
  if (fs.existsSync(sigPath)) {
    return fs.readFileSync(sigPath, 'utf-8').trim();
  }
  return '';
}

const manifest = {
  version,
  notes: `Release v${version}`,
  pub_date: new Date().toISOString(),
  platforms: {
    'windows-x86_64': {
      signature: readSignature('windows-x86_64'),
      url: `${baseUrl}/MeetingScribe_${version}_x64-setup.nsis.zip`,
    },
    'darwin-x86_64': {
      signature: readSignature('darwin-x86_64'),
      url: `${baseUrl}/MeetingScribe_${version}_x64.app.tar.gz`,
    },
    'darwin-aarch64': {
      signature: readSignature('darwin-aarch64'),
      url: `${baseUrl}/MeetingScribe_${version}_aarch64.app.tar.gz`,
    },
    'linux-x86_64': {
      signature: readSignature('linux-x86_64'),
      url: `${baseUrl}/MeetingScribe_${version}_amd64.AppImage.tar.gz`,
    },
  },
};

console.log(JSON.stringify(manifest, null, 2));
```

---

## Release Management

### Version Bumping

Create `scripts/bump-version.sh`:

```bash
#!/bin/bash
# scripts/bump-version.sh

set -e

TYPE="${1:-patch}"  # major, minor, patch

# Get current version from tauri.conf.json
CURRENT=$(grep '"version"' src-tauri/tauri.conf.json | head -1 | cut -d'"' -f4)

IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT"

case "$TYPE" in
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
    *)
        echo "Usage: $0 [major|minor|patch]"
        exit 1
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"

echo "Bumping version: $CURRENT → $NEW_VERSION"

# Update tauri.conf.json
sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW_VERSION\"/" src-tauri/tauri.conf.json
rm src-tauri/tauri.conf.json.bak

# Update Cargo.toml
sed -i.bak "s/^version = \"$CURRENT\"/version = \"$NEW_VERSION\"/" src-tauri/Cargo.toml
rm src-tauri/Cargo.toml.bak

# Update package.json
sed -i.bak "s/\"version\": \"$CURRENT\"/\"version\": \"$NEW_VERSION\"/" package.json
rm package.json.bak

# Update Cargo.lock
cargo update --manifest-path src-tauri/Cargo.toml --package meeting-scribe

echo "Updated to version $NEW_VERSION"
echo ""
echo "Next steps:"
echo "  1. Update CHANGELOG.md"
echo "  2. Commit: git commit -am 'Release v$NEW_VERSION'"
echo "  3. Tag: git tag v$NEW_VERSION"
echo "  4. Push: git push && git push --tags"
```

### Changelog Management

Create `CHANGELOG.md`:

```markdown
# Changelog

All notable changes to Meeting Scribe will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- 

### Changed
-

### Fixed
-

### Removed
-

## [1.0.0] - 2025-XX-XX

### Added
- Initial release
- Real-time audio capture (microphone + system audio)
- Voice activity detection with Silero VAD
- Audio denoising with RNNoise
- Speech-to-text with Parakeet (transcribe-rs)
- Meeting summaries with local LLM
- RAG-powered chat interface
- Full-text and vector search
- Cross-platform support (Windows, macOS, Linux)

[Unreleased]: https://github.com/meetingscribe/meeting-scribe/compare/v1.0.0...HEAD
[1.0.0]: https://github.com/meetingscribe/meeting-scribe/releases/tag/v1.0.0
```

---

## Model Distribution

### Model Download Strategy

Models are downloaded on first use rather than bundled with the installer:

```rust
// src-tauri/src/models/distribution.rs

use std::path::PathBuf;
use reqwest::Client;
use futures_util::StreamExt;
use tokio::io::AsyncWriteExt;
use tauri::Emitter;

/// Model registry with download URLs and checksums
pub struct ModelRegistry {
    models: Vec<ModelInfo>,
}

#[derive(Clone, serde::Serialize)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub size_bytes: u64,
    pub download_url: String,
    pub checksum_sha256: String,
    pub model_type: ModelType,
}

#[derive(Clone, serde::Serialize)]
pub enum ModelType {
    Transcription,
    Embedding,
    LLM,
}

impl ModelRegistry {
    pub fn new() -> Self {
        Self {
            models: vec![
                // Transcription models
                ModelInfo {
                    id: "parakeet-tdt-0.6b".into(),
                    name: "Parakeet TDT 0.6B".into(),
                    description: "Fast English transcription (recommended)".into(),
                    size_bytes: 450 * 1024 * 1024,
                    download_url: "https://huggingface.co/nvidia/parakeet-tdt-0.6b/resolve/main/model.onnx".into(),
                    checksum_sha256: "...".into(),
                    model_type: ModelType::Transcription,
                },
                ModelInfo {
                    id: "whisper-large-v3-turbo".into(),
                    name: "Whisper Large v3 Turbo".into(),
                    description: "Multilingual transcription (100+ languages)".into(),
                    size_bytes: 1600 * 1024 * 1024,
                    download_url: "https://huggingface.co/openai/whisper-large-v3-turbo/resolve/main/model.onnx".into(),
                    checksum_sha256: "...".into(),
                    model_type: ModelType::Transcription,
                },
                
                // Embedding model
                ModelInfo {
                    id: "embedding-gemma-300m".into(),
                    name: "EmbeddingGemma 300M".into(),
                    description: "Text embeddings for semantic search".into(),
                    size_bytes: 300 * 1024 * 1024,
                    download_url: "https://huggingface.co/onnx-community/EmbeddingGemma-300M-ONNX/resolve/main/model_q8.onnx".into(),
                    checksum_sha256: "...".into(),
                    model_type: ModelType::Embedding,
                },
                
                // LLM models
                ModelInfo {
                    id: "llama-3.2-3b-q4".into(),
                    name: "Llama 3.2 3B".into(),
                    description: "Fast local LLM for summaries and chat".into(),
                    size_bytes: 2000 * 1024 * 1024,
                    download_url: "https://huggingface.co/bartowski/Llama-3.2-3B-Instruct-GGUF/resolve/main/Llama-3.2-3B-Instruct-Q4_K_M.gguf".into(),
                    checksum_sha256: "...".into(),
                    model_type: ModelType::LLM,
                },
            ],
        }
    }
    
    pub fn get_model(&self, id: &str) -> Option<&ModelInfo> {
        self.models.iter().find(|m| m.id == id)
    }
    
    pub fn list_models(&self) -> &[ModelInfo] {
        &self.models
    }
}

/// Download a model with progress reporting
pub async fn download_model(
    model: &ModelInfo,
    target_dir: &PathBuf,
    app_handle: &tauri::AppHandle,
) -> anyhow::Result<PathBuf> {
    let client = Client::new();
    let response = client.get(&model.download_url).send().await?;
    
    let total_size = response.content_length().unwrap_or(model.size_bytes);
    
    let filename = model.download_url.split('/').last().unwrap_or("model");
    let target_path = target_dir.join(&model.id).join(filename);
    
    std::fs::create_dir_all(target_path.parent().unwrap())?;
    
    let mut file = tokio::fs::File::create(&target_path).await?;
    let mut stream = response.bytes_stream();
    let mut downloaded: u64 = 0;
    
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        file.write_all(&chunk).await?;
        downloaded += chunk.len() as u64;
        
        // Emit progress
        app_handle.emit("model-download-progress", serde_json::json!({
            "model_id": model.id,
            "downloaded": downloaded,
            "total": total_size,
            "percent": (downloaded as f64 / total_size as f64 * 100.0) as u32,
        }))?;
    }
    
    file.flush().await?;
    
    // Verify checksum
    let computed_hash = compute_sha256(&target_path).await?;
    if computed_hash != model.checksum_sha256 {
        std::fs::remove_file(&target_path)?;
        anyhow::bail!("Checksum mismatch for model {}", model.id);
    }
    
    Ok(target_path)
}

async fn compute_sha256(path: &PathBuf) -> anyhow::Result<String> {
    use sha2::{Sha256, Digest};
    
    let data = tokio::fs::read(path).await?;
    let mut hasher = Sha256::new();
    hasher.update(&data);
    let result = hasher.finalize();
    
    Ok(hex::encode(result))
}
```

### First-Run Setup UI

```typescript
// src/components/setup/FirstRunSetup.tsx
import { useState, useEffect } from 'react';
import { invoke } from '@tauri-apps/api/core';
import { listen } from '@tauri-apps/api/event';
import { Button } from '../ui/Button';
import { Progress } from '../ui/Progress';
import { Check, Download, Loader2 } from 'lucide-react';

interface ModelInfo {
  id: string;
  name: string;
  description: string;
  size_bytes: number;
  model_type: string;
}

interface DownloadProgress {
  model_id: string;
  downloaded: number;
  total: number;
  percent: number;
}

export function FirstRunSetup({ onComplete }: { onComplete: () => void }) {
  const [models, setModels] = useState<ModelInfo[]>([]);
  const [selectedModels, setSelectedModels] = useState<Set<string>>(new Set([
    'parakeet-tdt-0.6b',
    'embedding-gemma-300m',
    'llama-3.2-3b-q4',
  ]));
  const [downloading, setDownloading] = useState(false);
  const [progress, setProgress] = useState<Record<string, DownloadProgress>>({});
  const [completed, setCompleted] = useState<Set<string>>(new Set());
  
  useEffect(() => {
    // Load available models
    invoke<ModelInfo[]>('list_available_models').then(setModels);
    
    // Listen for download progress
    const unlisten = listen<DownloadProgress>('model-download-progress', (event) => {
      setProgress(p => ({
        ...p,
        [event.payload.model_id]: event.payload,
      }));
      
      if (event.payload.percent >= 100) {
        setCompleted(c => new Set([...c, event.payload.model_id]));
      }
    });
    
    return () => {
      unlisten.then(fn => fn());
    };
  }, []);
  
  const toggleModel = (id: string) => {
    setSelectedModels(s => {
      const next = new Set(s);
      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }
      return next;
    });
  };
  
  const startDownload = async () => {
    setDownloading(true);
    
    for (const id of selectedModels) {
      if (!completed.has(id)) {
        try {
          await invoke('download_model', { modelId: id });
        } catch (error) {
          console.error(`Failed to download ${id}:`, error);
        }
      }
    }
    
    setDownloading(false);
  };
  
  const allComplete = [...selectedModels].every(id => completed.has(id));
  
  const formatSize = (bytes: number) => {
    if (bytes >= 1024 * 1024 * 1024) {
      return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
    }
    return `${(bytes / 1024 / 1024).toFixed(0)} MB`;
  };
  
  const totalSize = models
    .filter(m => selectedModels.has(m.id))
    .reduce((sum, m) => sum + m.size_bytes, 0);
  
  return (
    <div className="min-h-screen flex items-center justify-center bg-gray-50 p-8">
      <div className="max-w-2xl w-full bg-white rounded-xl shadow-lg p-8">
        <h1 className="text-2xl font-bold mb-2">Welcome to Meeting Scribe</h1>
        <p className="text-gray-600 mb-8">
          Select the AI models to download. You can change these later in Settings.
        </p>
        
        <div className="space-y-4 mb-8">
          {models.map(model => {
            const isSelected = selectedModels.has(model.id);
            const isDownloading = downloading && progress[model.id] && !completed.has(model.id);
            const isComplete = completed.has(model.id);
            
            return (
              <div
                key={model.id}
                className={`p-4 border rounded-lg cursor-pointer transition-colors ${
                  isSelected ? 'border-blue-500 bg-blue-50' : 'border-gray-200 hover:border-gray-300'
                }`}
                onClick={() => !downloading && toggleModel(model.id)}
              >
                <div className="flex items-start justify-between">
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <h3 className="font-medium">{model.name}</h3>
                      {isComplete && <Check className="w-4 h-4 text-green-500" />}
                    </div>
                    <p className="text-sm text-gray-500">{model.description}</p>
                    <p className="text-xs text-gray-400 mt-1">{formatSize(model.size_bytes)}</p>
                  </div>
                  
                  <input
                    type="checkbox"
                    checked={isSelected}
                    onChange={() => {}}
                    className="mt-1"
                    disabled={downloading}
                  />
                </div>
                
                {isDownloading && (
                  <div className="mt-3">
                    <Progress value={progress[model.id].percent} />
                    <p className="text-xs text-gray-500 mt-1">
                      {formatSize(progress[model.id].downloaded)} / {formatSize(progress[model.id].total)}
                    </p>
                  </div>
                )}
              </div>
            );
          })}
        </div>
        
        <div className="flex items-center justify-between">
          <p className="text-sm text-gray-500">
            Total download: {formatSize(totalSize)}
          </p>
          
          {allComplete ? (
            <Button onClick={onComplete}>
              <Check className="w-4 h-4 mr-2" />
              Get Started
            </Button>
          ) : (
            <Button onClick={startDownload} disabled={downloading || selectedModels.size === 0}>
              {downloading ? (
                <>
                  <Loader2 className="w-4 h-4 mr-2 animate-spin" />
                  Downloading...
                </>
              ) : (
                <>
                  <Download className="w-4 h-4 mr-2" />
                  Download Models
                </>
              )}
            </Button>
          )}
        </div>
      </div>
    </div>
  );
}
```

---

## Troubleshooting

### Common Build Issues

#### Windows

```powershell
# Error: MSVC not found
# Solution: Install Visual Studio Build Tools
winget install Microsoft.VisualStudio.2022.BuildTools

# Error: WebView2 not found
# Solution: Install WebView2 runtime
winget install Microsoft.EdgeWebView2Runtime

# Error: signing failed
# Solution: Check certificate path and password
$env:WINDOWS_CERT_PATH = "path/to/certificate.pfx"
$env:WINDOWS_CERT_PASSWORD = "password"
```

#### macOS

```bash
# Error: codesign failed
# Solution: Unlock keychain
security unlock-keychain -p "$KEYCHAIN_PASSWORD" build.keychain

# Error: notarization failed - "The signature of the binary is invalid"
# Solution: Sign with hardened runtime
codesign --force --options runtime --entitlements entitlements.plist ...

# Error: "App is damaged and can't be opened"
# Solution: Clear quarantine attribute
xattr -cr "Meeting Scribe.app"
```

#### Linux

```bash
# Error: webkit2gtk not found
# Solution: Install development libraries
sudo apt install libwebkit2gtk-4.1-dev

# Error: AppImage won't start
# Solution: Make executable and install FUSE
chmod +x MeetingScribe.AppImage
sudo apt install fuse libfuse2

# Error: PipeWire not available
# Solution: Install PipeWire
sudo apt install pipewire pipewire-audio-client-libraries
```

### CI/CD Issues

```yaml
# Error: Rust build times out
# Solution: Use larger runner and caching
jobs:
  build:
    runs-on: ubuntu-latest-16-cores
    steps:
      - uses: Swatinem/rust-cache@v2
        with:
          cache-on-failure: true

# Error: macOS signing fails in CI
# Solution: Create temporary keychain
- run: |
    security create-keychain -p "" build.keychain
    security default-keychain -s build.keychain
    security unlock-keychain -p "" build.keychain
    security set-keychain-settings build.keychain
```

---

## Acceptance Criteria

### Build & Optimization
- [ ] Release builds complete in <15 minutes
- [ ] Windows installer <30 MB (without models)
- [ ] macOS DMG <35 MB
- [ ] Linux AppImage <40 MB
- [ ] All builds pass security scans

### Distribution
- [ ] Windows: NSIS installer works on Windows 10/11
- [ ] Windows: Portable version runs without installation
- [ ] macOS: Signed and notarized DMG
- [ ] macOS: Universal binary for Intel + Apple Silicon
- [ ] Linux: AppImage runs on Ubuntu 20.04+
- [ ] Linux: .deb installs on Debian/Ubuntu
- [ ] Linux: .rpm installs on Fedora/RHEL

### Auto-Updates
- [ ] Update check works on all platforms
- [ ] Downloads and installs updates correctly
- [ ] Signatures are verified
- [ ] Rollback works if update fails

### CI/CD
- [ ] CI runs on every PR
- [ ] Release workflow creates all artifacts
- [ ] Code signing works in CI
- [ ] Update manifest is generated automatically

---

## Summary

This document covered the complete deployment pipeline for Meeting Scribe:

1. **Build Optimization**: LTO, code stripping, frontend minification
2. **Windows**: NSIS installer, code signing, portable version
3. **macOS**: App bundle, DMG, code signing, notarization, universal binary
4. **Linux**: AppImage, .deb, .rpm, Flatpak
5. **Auto-Updates**: Tauri updater plugin, signed updates, update UI
6. **CI/CD**: GitHub Actions for testing, building, and releasing
7. **Model Distribution**: On-demand downloads with progress tracking

With all 12 documents complete, developers have a comprehensive guide to building Meeting Scribe from scratch through production deployment.

---

## Complete Document Index

| # | Document | Topics |
|---|----------|--------|
| 00 | [Overview](./00-overview.md) | Project structure, timeline, tech stack |
| 01 | [Project Setup](./01-project-setup.md) | Tauri scaffolding, dependencies |
| 02 | [Audio Capture](./02-audio-capture.md) | Microphone, waveform visualization |
| 03 | [Audio Preprocessing](./03-audio-preprocessing.md) | VAD, denoising |
| 04 | [Transcription Engine](./04-transcription-engine.md) | transcribe-rs, Parakeet |
| 05 | [Storage Layer](./05-storage-layer.md) | SQLite, LanceDB |
| 06 | [Embedding Engine](./06-embedding-engine.md) | ONNX, EmbeddingGemma |
| 07 | [LLM Engine](./07-llm-engine.md) | llama-cpp-2, summaries |
| 08 | [Frontend UI](./08-frontend-ui.md) | React, Tailwind, components |
| 09 | [RAG Implementation](./09-rag-implementation.md) | Vector search, chat |
| 10 | [Cross-Platform](./10-cross-platform.md) | macOS, Linux audio capture |
| 11 | [Deployment](./11-deployment.md) | Builds, installers, CI/CD |

---

## References

- [Tauri v2 Documentation](https://tauri.app/v2/)
- [Tauri GitHub Action](https://github.com/tauri-apps/tauri-action)
- [GitHub Actions](https://docs.github.com/en/actions)
- [Apple Notarization](https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution)
- [AppImage Specification](https://docs.appimage.org/)
- [Flatpak Documentation](https://docs.flatpak.org/)
