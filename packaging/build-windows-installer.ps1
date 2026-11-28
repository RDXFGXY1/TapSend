param(
    [ValidatePattern('^\d+\.\d+\.\d+(-[A-Za-z0-9.-]+)?$')]
    [string]$ReleaseVersion = '1.0.2',
    [ValidatePattern('^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$')]
    [string]$Repository = 'RDXFGXY1/TapSend',
    [string]$Stage = 'dist/windows',
    [string]$Compiler
)

$ErrorActionPreference = 'Stop'
$root = Split-Path $PSScriptRoot -Parent
$stagePath = (Resolve-Path -LiteralPath $Stage).Path
$dist = Join-Path $root 'dist'
$name = "TapSend-v$ReleaseVersion-windows-x64"
$archive = Join-Path $dist "$name.zip"

foreach ($required in @('TapSend-Desktop.exe', 'tapsend.exe', 'tapsend-server.exe',
                        'tapsend-tui.exe', 'TapSend.CameraScanner.exe', 'TapSend.CameraScanner.dll',
                        'TapSend.CameraScanner.runtimeconfig.json', 'OpenCvSharp.dll')) {
    if (-not (Test-Path -LiteralPath (Join-Path $stagePath $required) -PathType Leaf)) {
        throw "Missing Windows bundle file: $required"
    }
}
if (-not (Get-ChildItem -LiteralPath $stagePath -Recurse -File -Filter 'OpenCvSharpExtern.dll')) {
    throw 'Missing the native OpenCV runtime from the Windows bundle.'
}

if (-not $Compiler) {
    $Compiler = (Get-Command ISCC.exe -ErrorAction SilentlyContinue).Source
}
if (-not $Compiler) {
    foreach ($directory in @(
        (Join-Path ${env:ProgramFiles(x86)} 'Inno Setup 6'),
        (Join-Path $env:LOCALAPPDATA 'Programs/Inno Setup 6')
    )) {
        $candidate = Join-Path $directory 'ISCC.exe'
        if (Test-Path -LiteralPath $candidate) { $Compiler = $candidate; break }
    }
}
if (-not $Compiler) { throw 'Inno Setup 6.6 or later is required to compile the installer.' }

$icon = Join-Path $PSScriptRoot 'tapsend.ico'
if (Test-Path -LiteralPath $icon -PathType Leaf) {
    Copy-Item -LiteralPath $icon -Destination (Join-Path $stagePath 'TapSend.ico') -Force
}
Compress-Archive -Path (Join-Path $stagePath '*') -DestinationPath $archive -Force
$hash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash
& $Compiler "/DTapSendVersion=$ReleaseVersion" "/DTapSendRepo=$Repository" "/DBundleSHA256=$hash" (Join-Path $PSScriptRoot 'tapsend.iss')
if ($LASTEXITCODE -ne 0) { throw "Inno Setup failed with exit code $LASTEXITCODE." }

$installer = Join-Path $dist "$name-setup.exe"
$installerHash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash
Set-Content -LiteralPath (Join-Path $dist "$name-SHA256SUMS.txt") -Encoding ascii -Value @(
    "$($hash.ToLowerInvariant())  $name.zip"
    "$($installerHash.ToLowerInvariant())  $name-setup.exe"
)
