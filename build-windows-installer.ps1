#requires -Version 5.1
[CmdletBinding()]
param(
    [ValidateSet('dev', 'release')]
    [string] $Profile = 'release',

    [switch] $Dev,
    [switch] $Release,
    [switch] $SkipBuild,
    [switch] $Compact,

    [string] $WixBin = $env:WIX,
    [string] $CargoTargetDir = (Join-Path $env:LOCALAPPDATA 'rio-custom-cargo-target')
)

Set-StrictMode -Version 2.0
$ErrorActionPreference = 'Stop'

$RootDir = Split-Path -Parent $MyInvocation.MyCommand.Path
Set-Location -LiteralPath $RootDir

if ($Dev -and $Release) {
    throw 'Use either -Dev or -Release, not both.'
}

if ($Dev) {
    $Profile = 'dev'
}

if ($Release) {
    $Profile = 'release'
}

if (-not [System.IO.Path]::IsPathRooted($CargoTargetDir)) {
    $CargoTargetDir = Join-Path $RootDir $CargoTargetDir
}

$PackageName = 'Rio'
$PackageId = 'rioterm'
$PackageDir = Join-Path $RootDir 'target\windows-package'
$UpgradeCode = '87c21c74-dbd5-4584-89d5-46d9cd0c40a8'
$PathComponentGuid = 'edf0b679-9eb6-46f7-a5d1-5160f30acb34'
$ShortcutComponentGuid = 'aa36e61a-23cd-4383-b744-2f78e912f0dc'
$ContextMenuComponentGuid = '449f9121-f7b9-41fe-82da-52349ea8ff91'

if ($Profile -eq 'release') {
    $TargetProfileDir = 'release'
    $MsiNameSuffix = 'x86_64'
} else {
    $TargetProfileDir = 'debug'
    $MsiNameSuffix = 'x86_64-fast'
}

function Require-Command {
    param([Parameter(Mandatory = $true)][string] $Name)

    if (-not (Get-Command $Name -ErrorAction SilentlyContinue)) {
        throw "Missing required command: $Name"
    }
}

function Find-WixBin {
    param([string] $RequestedPath)

    $candidates = @()
    if ($RequestedPath) {
        $candidates += $RequestedPath
    }

    $candleOnPath = Get-Command candle.exe -ErrorAction SilentlyContinue
    if ($candleOnPath) {
        $candidates += (Split-Path -Parent $candleOnPath.Source)
    }

    $candidates += @(
        'C:\Tools\wix314',
        (Join-Path ${env:ProgramFiles(x86)} 'WiX Toolset v3.14\bin'),
        (Join-Path ${env:ProgramFiles(x86)} 'WiX Toolset v3.11\bin')
    )

    foreach ($candidate in $candidates) {
        if (-not $candidate) {
            continue
        }

        $expanded = [Environment]::ExpandEnvironmentVariables($candidate)
        $candle = Join-Path $expanded 'candle.exe'
        $light = Join-Path $expanded 'light.exe'
        if ((Test-Path -LiteralPath $candle) -and (Test-Path -LiteralPath $light)) {
            return (Resolve-Path -LiteralPath $expanded).Path
        }
    }

    throw 'WiX 3 candle.exe/light.exe was not found. Install WiX Toolset 3.x or pass -WixBin C:\path\to\wix\bin.'
}

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

    foreach ($edition in @('BuildTools', 'Community', 'Professional', 'Enterprise')) {
        $candidates += (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\2022\$edition\VC\Auxiliary\Build\vcvars64.bat")
        $candidates += (Join-Path $env:ProgramFiles "Microsoft Visual Studio\2022\$edition\VC\Auxiliary\Build\vcvars64.bat")
        $candidates += (Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\2019\$edition\VC\Auxiliary\Build\vcvars64.bat")
        $candidates += (Join-Path $env:ProgramFiles "Microsoft Visual Studio\2019\$edition\VC\Auxiliary\Build\vcvars64.bat")
    }

    foreach ($candidate in $candidates) {
        if ($candidate -and (Test-Path -LiteralPath $candidate)) {
            return (Resolve-Path -LiteralPath $candidate).Path
        }
    }

    throw 'vcvars64.bat was not found. Install Visual Studio Build Tools with the MSVC x64 tools.'
}

function Import-VcVars64 {
    param([Parameter(Mandatory = $true)][string] $VcVars64)

    $originalLocation = Get-Location
    Set-Location -LiteralPath $env:SystemRoot
    try {
        $lines = cmd.exe /d /s /c "call `"$VcVars64`" >nul && set"
        if ($LASTEXITCODE -ne 0) {
            throw "Failed to import MSVC environment from $VcVars64"
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

function Test-WindowsX64Exe {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    $stream = [System.IO.File]::OpenRead($Path)
    try {
        $reader = New-Object System.IO.BinaryReader($stream)
        if ($reader.ReadByte() -ne 0x4d -or $reader.ReadByte() -ne 0x5a) {
            return $false
        }

        [void] $stream.Seek(0x3c, [System.IO.SeekOrigin]::Begin)
        $peOffset = $reader.ReadInt32()
        if ($peOffset -lt 0 -or $peOffset -gt ($stream.Length - 6)) {
            return $false
        }

        [void] $stream.Seek($peOffset, [System.IO.SeekOrigin]::Begin)
        $signature = $reader.ReadBytes(4)
        if ($signature.Length -ne 4 -or $signature[0] -ne 0x50 -or $signature[1] -ne 0x45 -or $signature[2] -ne 0x00 -or $signature[3] -ne 0x00) {
            return $false
        }

        $machine = $reader.ReadUInt16()
        return $machine -eq 0x8664
    } finally {
        $stream.Dispose()
    }
}

function Test-MsiInstaller {
    param([Parameter(Mandatory = $true)][string] $Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $false
    }

    $expected = [byte[]] (0xd0, 0xcf, 0x11, 0xe0, 0xa1, 0xb1, 0x1a, 0xe1)
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        if ($stream.Length -lt $expected.Length) {
            return $false
        }

        $actual = New-Object byte[] $expected.Length
        [void] $stream.Read($actual, 0, $actual.Length)
        for ($i = 0; $i -lt $expected.Length; $i++) {
            if ($actual[$i] -ne $expected[$i]) {
                return $false
            }
        }

        return $true
    } finally {
        $stream.Dispose()
    }
}

Require-Command cargo
Require-Command rustc

$WixBin = Find-WixBin -RequestedPath $WixBin
$Candle = Join-Path $WixBin 'candle.exe'
$Light = Join-Path $WixBin 'light.exe'

$pkgid = & cargo pkgid -p $PackageId
if ($LASTEXITCODE -ne 0) {
    exit $LASTEXITCODE
}

$Version = $pkgid -replace '^.*#', ''
$MsiName = "$PackageName-$Version-$MsiNameSuffix.msi"
$TargetExe = Join-Path $CargoTargetDir "$TargetProfileDir\rio.exe"
$StageDir = Join-Path ([System.IO.Path]::GetTempPath()) ("rio-installer-{0}-{1}-{2}" -f $Version, $Profile, [Guid]::NewGuid().ToString('N'))

New-Item -ItemType Directory -Force -Path $StageDir | Out-Null

try {
    if (-not $SkipBuild) {
        $vcvars64 = Find-VcVars64
        Write-Host "MSVC environment: $vcvars64"
        Import-VcVars64 -VcVars64 $vcvars64

        New-Item -ItemType Directory -Force -Path $CargoTargetDir | Out-Null
        $env:CARGO_TARGET_DIR = $CargoTargetDir

        Write-Host "WiX bin: $WixBin"
        Write-Host "Cargo target dir: $env:CARGO_TARGET_DIR"
        Write-Host 'Cargo:'
        cargo --version
        Write-Host 'Rustc:'
        rustc -vV

        $cargoArgs = @('build', '-p', $PackageId)
        if ($Profile -eq 'release') {
            $cargoArgs += '--release'
        }

        & cargo @cargoArgs
        if ($LASTEXITCODE -ne 0) {
            exit $LASTEXITCODE
        }
    } else {
        Write-Host "Skipping build; packaging existing binary for profile: $Profile"
        $legacyTargetExe = Join-Path $RootDir "target\$TargetProfileDir\rio.exe"
        if (-not (Test-Path -LiteralPath $TargetExe) -and (Test-Path -LiteralPath $legacyTargetExe)) {
            $TargetExe = $legacyTargetExe
            Write-Host "Using project target binary: $TargetExe"
        }
    }

    if (-not (Test-Path -LiteralPath $TargetExe)) {
        throw "Expected build output does not exist: $TargetExe"
    }

    if (-not (Test-WindowsX64Exe -Path $TargetExe)) {
        throw "Build output is not a Windows x86_64 executable: $TargetExe"
    }

    $StageExe = Join-Path $StageDir 'rio.exe'
    $StageIcon = Join-Path $StageDir 'rio.ico'
    $StageLicense = Join-Path $StageDir 'License.rtf'
    $WxsFile = Join-Path $StageDir 'rio.wxs'
    $WixObj = Join-Path $StageDir 'rio.wixobj'
    $StageMsi = Join-Path $StageDir $MsiName

    Copy-Item -LiteralPath $TargetExe -Destination $StageExe -Force
    Copy-Item -LiteralPath (Join-Path $RootDir 'misc\windows\rio.ico') -Destination $StageIcon -Force
    Copy-Item -LiteralPath (Join-Path $RootDir 'misc\windows\License.rtf') -Destination $StageLicense -Force

    $wxsTemplate = @'
<?xml version="1.0" encoding="UTF-8"?>
<Wix xmlns="http://schemas.microsoft.com/wix/2006/wi">
  <Product Id="*" Name="Rio" UpgradeCode="__UPGRADE_CODE__" Language="1033" Codepage="1252" Version="$(var.Version)" Manufacturer="Raphael Amorim">
    <Package InstallerVersion="200" Compressed="yes" InstallScope="perMachine" Description="Rio terminal is a hardware-accelerated GPU terminal emulator, focusing to run in desktops and browsers." />
    <MajorUpgrade AllowSameVersionUpgrades="yes" DowngradeErrorMessage="A newer version of [ProductName] is already installed." />
    <MediaTemplate EmbedCab="yes" />
    <WixVariable Id="WixUILicenseRtf" Value="$(var.LicenseRtf)" />
    <Icon Id="RioIcon" SourceFile="$(var.RioIcon)" />
    <Property Id="ARPPRODUCTICON" Value="RioIcon" />
    <UIRef Id="WixUI_Minimal" />
    <Directory Id="TARGETDIR" Name="SourceDir">
      <Directory Id="ProgramFiles64Folder">
        <Directory Id="RioProgramFiles" Name="Rio">
          <Component Id="RioExe" Guid="*">
            <File Id="RioExeFile" Source="$(var.RioExe)" Name="rio.exe" KeyPath="yes" />
          </Component>
          <Component Id="ModifyPathEnv" Guid="__PATH_COMPONENT_GUID__" KeyPath="yes">
            <Environment Id="PathEnv" Value="[RioProgramFiles]" Name="PATH" Permanent="no" Part="first" Action="set" System="yes" />
          </Component>
          <Component Id="ContextMenu" Guid="__CONTEXT_MENU_COMPONENT_GUID__">
            <RegistryKey Root="HKCU" Key="Software\Classes\Directory\Background\shell\Open Rio here\command">
              <RegistryValue Type="string" Value='[RioProgramFiles]rio.exe --working-dir "%v"' KeyPath="yes" />
            </RegistryKey>
            <RegistryKey Root="HKCU" Key="Software\Classes\Directory\Background\shell\Open Rio here">
              <RegistryValue Type="string" Name="Icon" Value="[RioProgramFiles]rio.exe" />
            </RegistryKey>
          </Component>
        </Directory>
      </Directory>
      <Directory Id="ProgramMenuFolder">
        <Directory Id="RioProgramMenu" Name="Rio">
          <Component Id="RioShortcut" Guid="__SHORTCUT_COMPONENT_GUID__">
            <Shortcut Id="RioShortcutFile" Name="Rio" Description="A hardware-accelerated GPU terminal emulator" Target="[RioProgramFiles]rio.exe" Icon="RioIcon" WorkingDirectory="RioProgramFiles" />
            <RemoveFolder Id="RioProgramMenu" On="uninstall" />
            <RegistryValue Root="HKCU" Key="Software\Microsoft\Rio" Name="installed" Type="integer" Value="1" KeyPath="yes" />
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
'@

    $wxsContent = $wxsTemplate.
        Replace('__UPGRADE_CODE__', $UpgradeCode).
        Replace('__PATH_COMPONENT_GUID__', $PathComponentGuid).
        Replace('__SHORTCUT_COMPONENT_GUID__', $ShortcutComponentGuid).
        Replace('__CONTEXT_MENU_COMPONENT_GUID__', $ContextMenuComponentGuid)
    Set-Content -LiteralPath $WxsFile -Value $wxsContent -Encoding UTF8

    & $Candle `
        -nologo `
        -arch x64 `
        -ext WixUIExtension `
        "-dVersion=$Version" `
        "-dRioExe=$StageExe" `
        "-dRioIcon=$StageIcon" `
        "-dLicenseRtf=$StageLicense" `
        -out $WixObj `
        $WxsFile
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    & $Light `
        -nologo `
        -ext WixUIExtension `
        -out $StageMsi `
        $WixObj
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }

    if (-not (Test-MsiInstaller -Path $StageMsi)) {
        throw "WiX output is not an MSI installer: $StageMsi"
    }

    if (Test-Path -LiteralPath $PackageDir) {
        Remove-Item -LiteralPath $PackageDir -Recurse -Force
    }
    New-Item -ItemType Directory -Force -Path $PackageDir | Out-Null

    $PackageMsi = Join-Path $PackageDir $MsiName
    $PackageExe = Join-Path $PackageDir 'rio.exe'
    Copy-Item -LiteralPath $StageMsi -Destination $PackageMsi -Force
    Copy-Item -LiteralPath $StageExe -Destination $PackageExe -Force

    if ($Compact) {
        $targetRoot = [System.IO.Path]::GetFullPath((Join-Path $RootDir 'target'))
        $packageRoot = [System.IO.Path]::GetFullPath($PackageDir)
        if ((Test-Path -LiteralPath $targetRoot) -and $packageRoot.StartsWith($targetRoot, [System.StringComparison]::OrdinalIgnoreCase)) {
            Get-ChildItem -LiteralPath $targetRoot -Force |
                Where-Object { $_.Name -ne (Split-Path -Leaf $PackageDir) } |
                Remove-Item -Recurse -Force
        }
    }

    Write-Host "Windows installer: $PackageMsi"
    Write-Host "Portable executable: $PackageExe"
    Write-Host "Cargo cache: $CargoTargetDir"
} finally {
    if (Test-Path -LiteralPath $StageDir) {
        Remove-Item -LiteralPath $StageDir -Recurse -Force -ErrorAction SilentlyContinue
    }
}
