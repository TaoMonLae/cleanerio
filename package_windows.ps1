# Builds Cleanerio for Windows and packages it in dist\:
#   Cleanerio-<version>-windows-x64.zip          the program, ready to run from any folder
#   Cleanerio-<version>-windows-x64-setup.exe    an installer (made when Inno Setup is installed)
# Run it in PowerShell on Windows:   .\package_windows.ps1
# If PowerShell refuses to run scripts:   powershell -ExecutionPolicy Bypass -File .\package_windows.ps1
#
# You need:   Rust (https://rustup.rs) and the Visual Studio Build Tools with
#             "Desktop development with C++" (rustup offers to install them).
# Installer:  install Inno Setup from https://jrsoftware.org/isdl.php and run this again.
# Signing:    set $env:SIGN_THUMBPRINT to the thumbprint of a code-signing certificate in
#             your certificate store to sign the program and the installer with signtool.
$ErrorActionPreference = "Stop"
Set-Location $PSScriptRoot

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "Rust isn't installed. Install it from https://rustup.rs and run this again."
    exit 1
}

$version = (Select-String -Path Cargo.toml -Pattern '^version\s*=\s*"(.+)"' | Select-Object -First 1).Matches[0].Groups[1].Value
Write-Host "Building Cleanerio $version for Windows..."
cargo build --release
if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }

$stage = "dist\windows"
if (Test-Path $stage) { Remove-Item $stage -Recurse -Force }
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item "target\release\cleanerio.exe" "$stage\Cleanerio.exe"
Copy-Item "LICENSE" "$stage\LICENSE.txt"
Copy-Item "THIRD-PARTY-NOTICES.md" "$stage\THIRD-PARTY-NOTICES.md"

function Sign($file) {
    if (-not $env:SIGN_THUMBPRINT) { return }
    $signtool = Get-Command signtool -ErrorAction SilentlyContinue
    if (-not $signtool) {
        Write-Host "signtool wasn't found, so $file was not signed. It comes with the Windows SDK."
        return
    }
    & signtool sign /sha1 $env:SIGN_THUMBPRINT /fd SHA256 /tr http://timestamp.digicert.com /td SHA256 $file
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}
Sign "$stage\Cleanerio.exe"

$zip = "dist\Cleanerio-$version-windows-x64.zip"
if (Test-Path $zip) { Remove-Item $zip -Force }
Compress-Archive -Path "$stage\*" -DestinationPath $zip
Write-Host "Made $zip"

$iscc = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $iscc) {
    $candidate = "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe"
    if (Test-Path $candidate) { $iscc = $candidate }
}
if ($iscc) {
    $isccPath = if ($iscc -is [string]) { $iscc } else { $iscc.Source }
    & $isccPath "/DAppVersion=$version" "assets\windows\cleanerio.iss" | Out-Null
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $setup = "dist\Cleanerio-$version-windows-x64-setup.exe"
    Sign $setup
    Write-Host "Made $setup"
} else {
    Write-Host "Inno Setup isn't installed, so no installer was made."
    Write-Host "Get it from https://jrsoftware.org/isdl.php and run this again."
}

Write-Host ""
Write-Host "Checksums:"
Get-ChildItem dist -File | Where-Object { $_.Name -like "Cleanerio-$version-windows-*" } | ForEach-Object {
    "{0}  {1}" -f (Get-FileHash $_.FullName -Algorithm SHA256).Hash.ToLower(), $_.Name
}
