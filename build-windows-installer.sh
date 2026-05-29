#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
cd "$ROOT_DIR"

PACKAGE_NAME=Rio
PACKAGE_ID=rioterm
PACKAGE_DIR=target/windows-package
UPGRADE_CODE=87c21c74-dbd5-4584-89d5-46d9cd0c40a8
PATH_COMPONENT_GUID=edf0b679-9eb6-46f7-a5d1-5160f30acb34
SHORTCUT_COMPONENT_GUID=aa36e61a-23cd-4383-b744-2f78e912f0dc
CONTEXT_MENU_COMPONENT_GUID=449f9121-f7b9-41fe-82da-52349ea8ff91
PROFILE=dev
COMPACT_TARGET=0
SKIP_BUILD=0

usage() {
    cat <<'EOF'
Usage: ./build-windows-installer.sh [--release] [--compact] [--skip-build]

Default mode is fastest:
  dev build, keeps Cargo cache under Windows %LOCALAPPDATA%,
  packages rio.exe into target/windows-package.

Options:
  --release     Build and package target/release/rio.exe.
  --compact     After a successful package, keep only target/windows-package
                inside this project. The Windows Cargo cache is kept.
  --skip-build  Package the existing binary for the selected profile.
  -h, --help    Show this help.
EOF
}

while [ "$#" -gt 0 ]; do
    case "$1" in
        --fast | --dev)
            PROFILE=dev
            ;;
        --release)
            PROFILE=release
            ;;
        --compact)
            COMPACT_TARGET=1
            ;;
        --skip-build)
            SKIP_BUILD=1
            ;;
        -h | --help)
            usage
            exit 0
            ;;
        *)
            printf 'unknown option: %s\n' "$1" >&2
            usage >&2
            exit 1
            ;;
    esac
    shift
done

if [ "$PROFILE" = release ]; then
    TARGET_PROFILE_DIR=release
    MSI_NAME_SUFFIX=x86_64
else
    TARGET_PROFILE_DIR=debug
    MSI_NAME_SUFFIX=x86_64-fast
fi

require_command() {
    if ! command -v "$1" >/dev/null 2>&1; then
        printf 'missing required command: %s\n' "$1" >&2
        exit 1
    fi
}

find_wix_bin() {
    if [ "${WIX:-}" != "" ] &&
        [ -x "$WIX/candle.exe" ] &&
        [ -x "$WIX/light.exe" ]; then
        printf '%s\n' "$WIX"
        return
    fi

    for candidate in \
        "/mnt/c/Tools/wix314" \
        "/mnt/c/Program Files (x86)/WiX Toolset v3.14/bin" \
        "/mnt/c/Program Files (x86)/WiX Toolset v3.11/bin"; do
        if [ -x "$candidate/candle.exe" ] && [ -x "$candidate/light.exe" ]; then
            printf '%s\n' "$candidate"
            return
        fi
    done

    printf 'missing WiX 3 candle.exe/light.exe; set WIX to the WiX bin directory\n' >&2
    exit 1
}

require_command cargo
require_command powershell.exe
require_command wslpath
require_command od

is_windows_x64_exe() {
    local path=$1
    local mz
    local pe_offset
    local pe_header_and_machine

    [ -f "$path" ] || return 1

    mz=$(od -An -tx1 -N2 "$path" | tr -d ' \n') || return 1
    [ "$mz" = "4d5a" ] || return 1

    pe_offset=$(od -An -tu4 -j60 -N4 "$path" | tr -d ' \n') || return 1
    case "$pe_offset" in
        '' | *[!0-9]*)
            return 1
            ;;
    esac

    pe_header_and_machine=$(od -An -tx1 -j "$pe_offset" -N6 "$path" | tr -d ' \n') || return 1
    [ "$pe_header_and_machine" = "504500006486" ]
}

is_msi_installer() {
    local path=$1
    local header

    [ -f "$path" ] || return 1

    header=$(od -An -tx1 -N8 "$path" | tr -d ' \n') || return 1
    [ "$header" = "d0cf11e0a1b11ae1" ]
}

WIX_BIN=$(find_wix_bin)
VERSION=$(cargo pkgid -p "$PACKAGE_ID" | sed 's/.*#//')
MSI_NAME="${PACKAGE_NAME}-${VERSION}-${MSI_NAME_SUFFIX}.msi"
PROJECT_WIN=$(wslpath -w "$ROOT_DIR")
WINDOWS_TEMP_WIN=$(powershell.exe -NoProfile -Command '[System.IO.Path]::GetTempPath()' | tr -d '\r')
WINDOWS_TEMP_WSL=$(wslpath -u "$WINDOWS_TEMP_WIN")
CARGO_TARGET_DIR_WIN=$(powershell.exe -NoProfile -Command "[System.IO.Path]::Combine([Environment]::GetEnvironmentVariable('LOCALAPPDATA'), 'rio-custom-cargo-target')" | tr -d '\r')
CARGO_TARGET_DIR_WSL=$(wslpath -u "$CARGO_TARGET_DIR_WIN")
TARGET_EXE="$CARGO_TARGET_DIR_WSL/$TARGET_PROFILE_DIR/rio.exe"
STAGE_DIR="$WINDOWS_TEMP_WSL/rio-installer-${VERSION}-${PROFILE}-$$"

cleanup() {
    rm -rf "$STAGE_DIR"
}
trap cleanup EXIT

mkdir -p "$STAGE_DIR"

BUILD_PS1="$STAGE_DIR/build-rio.ps1"
cat > "$BUILD_PS1" <<'PS1'
param(
    [Parameter(Mandatory = $true)]
    [string] $ProjectPath,

    [Parameter(Mandatory = $true)]
    [ValidateSet('dev', 'release')]
    [string] $Profile,

    [Parameter(Mandatory = $true)]
    [string] $TargetDir
)

$ErrorActionPreference = 'Stop'

function Find-VcVars64 {
    $candidates = @()

    if ($env:VCVARS64) {
        $candidates += $env:VCVARS64
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere) {
        $installPath = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($LASTEXITCODE -eq 0 -and $installPath) {
            $candidates += (Join-Path $installPath 'VC\Auxiliary\Build\vcvars64.bat')
        }
    }

    $candidates += @(
        'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat',
        'C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Auxiliary\Build\vcvars64.bat'
    )

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return $candidate
        }
    }

    throw 'vcvars64.bat was not found; install Visual Studio Build Tools with MSVC x64 tools'
}

function Import-VcVars64 {
    param([Parameter(Mandatory = $true)][string] $VcVars64)

    $originalLocation = Get-Location
    Set-Location -LiteralPath $env:SystemRoot
    try {
        $lines = cmd.exe /d /s /c "call `"$VcVars64`" >nul && set"
        if ($LASTEXITCODE -ne 0) {
            throw "failed to import MSVC environment from $VcVars64"
        }

        foreach ($line in $lines) {
            if ($line -match '^(.*?)=(.*)$') {
                [Environment]::SetEnvironmentVariable($matches[1], $matches[2], 'Process')
            }
        }
    } finally {
        Set-Location -LiteralPath $originalLocation
    }
}

Set-Location -LiteralPath $ProjectPath
$env:CARGO_TARGET_DIR = $TargetDir
$vcvars64 = Find-VcVars64
Write-Host "MSVC environment: $vcvars64"
Import-VcVars64 -VcVars64 $vcvars64
Write-Host "Cargo target dir: $env:CARGO_TARGET_DIR"

Write-Host "Windows cargo:"
cargo --version
Write-Host "Windows rustc:"
rustc -vV

$cargoArgs = @('build', '-p', 'rioterm')
if ($Profile -eq 'release') {
    $cargoArgs += '--release'
}

& cargo @cargoArgs
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}
PS1

if [ "$SKIP_BUILD" -eq 0 ]; then
    BUILD_PS1_WIN=$(wslpath -w "$BUILD_PS1")
    powershell.exe -NoProfile -ExecutionPolicy Bypass -File "$BUILD_PS1_WIN" "$PROJECT_WIN" "$PROFILE" "$CARGO_TARGET_DIR_WIN"
else
    printf 'Skipping build; packaging existing %s\n' "$TARGET_EXE"
    LEGACY_TARGET_EXE="target/$TARGET_PROFILE_DIR/rio.exe"
    if [ ! -f "$TARGET_EXE" ] && [ -f "$LEGACY_TARGET_EXE" ]; then
        TARGET_EXE="$LEGACY_TARGET_EXE"
        printf 'Using existing project target binary: %s\n' "$TARGET_EXE"
    fi
fi

if [ ! -f "$TARGET_EXE" ]; then
    printf 'expected build output does not exist: %s\n' "$TARGET_EXE" >&2
    exit 1
fi

if ! is_windows_x64_exe "$TARGET_EXE"; then
    printf 'build output is not a Windows x86_64 executable: %s\n' "$TARGET_EXE" >&2
    od -An -tx1 -N64 "$TARGET_EXE" >&2 || true
    exit 1
fi

cp "$TARGET_EXE" "$STAGE_DIR/rio.exe"
cp misc/windows/rio.ico "$STAGE_DIR/rio.ico"
cp misc/windows/License.rtf "$STAGE_DIR/License.rtf"

WXS_FILE="$STAGE_DIR/rio.wxs"
cat > "$WXS_FILE" <<WXS
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Rio" UpgradeCode="$UPGRADE_CODE" Language="1033" Codepage="1252" Version="\$(var.Version)" Manufacturer="Raphael Amorim">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" Description="Rio terminal is a hardware-accelerated GPU terminal emulator, focusing to run in desktops and browsers." />
    <MajorUpgrade AllowSameVersionUpgrades="yes" DowngradeErrorMessage="A newer version of [ProductName] is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <WixVariable Id="WixUILicenseRtf" Value="\$(var.LicenseRtf)" />
    <Icon Id="RioIcon" SourceFile="\$(var.RioIcon)" />
    <Property Id="ARPPRODUCTICON" Value="RioIcon" />
    <UIRef Id="WixUI_Minimal" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="RioProgramFiles" Name="Rio">
          <Component Id="RioExe" Guid="*">
            <File Id="RioExeFile" Source="\$(var.RioExe)" Name="rio.exe" KeyPath="yes" />
          </Component>
          <Component Id="ModifyPathEnv" Guid="$PATH_COMPONENT_GUID" KeyPath="yes">
            <Environment Id="PathEnv" Value="[RioProgramFiles]" Name="PATH" Permanent="no" Part="first" Action="set" System="yes" />
          </Component>
          <Component Id="ContextMenu" Guid="$CONTEXT_MENU_COMPONENT_GUID">
            <RegistryKey Root="HKCU" Key="Software\\Classes\\Directory\\Background\\shell\\Open Rio here\\command">
              <RegistryValue Type="string" Value='[RioProgramFiles]rio.exe --working-dir "%v"' KeyPath="yes" />
            </RegistryKey>
            <RegistryKey Root="HKCU" Key="Software\\Classes\\Directory\\Background\\shell\\Open Rio here">
              <RegistryValue Type="string" Name="Icon" Value="[RioProgramFiles]rio.exe" />
            </RegistryKey>
          </Component>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="RioProgramMenu" Name="Rio">
          <Component Id="RioShortcut" Guid="$SHORTCUT_COMPONENT_GUID">
            <Shortcut Id="RioShortcutFile" Name="Rio" Description="A hardware-accelerated GPU terminal emulator" Target="[RioProgramFiles]rio.exe" Icon="RioIcon" WorkingDirectory="RioProgramFiles" />
            <RemoveFolder Id="RioProgramMenu" On="uninstall" />
            <RegistryValue Root="HKCU" Key="Software\\Microsoft\\Rio" Name="installed" Type="integer" Value="1" KeyPath="yes" />
          </Component>
        </Directory>
      </Directory>
    </Directory>
    <Feature Id="ProductFeature" Title="ConsoleApp" Level="1">
      <ComponentRef Id="RioExe" />
      <ComponentRef Id="RioShortcut" />
      <ComponentRef Id="ModifyPathEnv" />
      <ComponentRef Id="ContextMenu" />
    </Feature>
  </Product>
</Wix>
WXS

WXS_WIN=$(wslpath -w "$WXS_FILE")
WIXOBJ_WIN=$(wslpath -w "$STAGE_DIR/rio.wixobj")
STAGE_MSI_WIN=$(wslpath -w "$STAGE_DIR/$MSI_NAME")
STAGE_EXE_WIN=$(wslpath -w "$STAGE_DIR/rio.exe")
STAGE_ICON_WIN=$(wslpath -w "$STAGE_DIR/rio.ico")
STAGE_LICENSE_WIN=$(wslpath -w "$STAGE_DIR/License.rtf")

"$WIX_BIN/candle.exe" \
    -nologo \
    -arch x64 \
    -ext WixUIExtension \
    -dVersion="$VERSION" \
    -dRioExe="$STAGE_EXE_WIN" \
    -dRioIcon="$STAGE_ICON_WIN" \
    -dLicenseRtf="$STAGE_LICENSE_WIN" \
    -out "$WIXOBJ_WIN" \
    "$WXS_WIN"

"$WIX_BIN/light.exe" \
    -nologo \
    -ext WixUIExtension \
    -out "$STAGE_MSI_WIN" \
    "$WIXOBJ_WIN"

if ! is_msi_installer "$STAGE_DIR/$MSI_NAME"; then
    printf 'WiX output is not an MSI installer: %s\n' "$STAGE_DIR/$MSI_NAME" >&2
    od -An -tx1 -N64 "$STAGE_DIR/$MSI_NAME" >&2 || true
    exit 1
fi

rm -rf "$PACKAGE_DIR"
mkdir -p "$PACKAGE_DIR"
cp "$STAGE_DIR/$MSI_NAME" "$PACKAGE_DIR/$MSI_NAME"
cp "$STAGE_DIR/rio.exe" "$PACKAGE_DIR/rio.exe"

if [ "$COMPACT_TARGET" -eq 1 ]; then
    find target -mindepth 1 -maxdepth 1 \
        ! -name "$(basename "$PACKAGE_DIR")" \
        -exec rm -rf {} +
fi

printf 'Windows installer: %s\n' "$PACKAGE_DIR/$MSI_NAME"
printf 'Portable executable: %s\n' "$PACKAGE_DIR/rio.exe"
printf 'Cargo cache: %s\n' "$CARGO_TARGET_DIR_WSL"
