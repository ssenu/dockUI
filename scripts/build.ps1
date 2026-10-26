<#
.SYNOPSIS
  cargo로 릴리스 exe를 만들고 Inno Setup으로 설치 프로그램과 SHA256SUMS.txt를 만든다.
.EXAMPLE
  ./scripts/build.ps1
  ./scripts/build.ps1 -Version 0.2.0
#>
param(
    [string]$Version
)
$ErrorActionPreference = "Stop"
$root = Split-Path $PSScriptRoot -Parent
Set-Location $root

$cargoToml = Join-Path $root "Cargo.toml"
$current = ((Get-Content $cargoToml -Raw) | Select-String '(?m)^version = "([^"]+)"').Matches[0].Groups[1].Value
if (-not $Version) { $Version = $current }
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw "잘못된 버전 형식: $Version" }
if ($Version -ne $current) {
    throw "Cargo.toml 버전($current)과 -Version($Version)이 다릅니다. Cargo.toml을 먼저 고치세요."
}
Write-Host "Building YT Launcher $Version"

# 태그로 버전을 바꾸면 Cargo.lock의 자기 버전 줄도 바뀌므로 --locked는 쓰지 않는다
cargo build --release
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

# 설치 프로그램이 묶는 폴더: exe 하나 (글꼴·아이콘은 exe 안에 들어 있다)
$stage = Join-Path $root "dist\YTLauncher"
Remove-Item -Recurse -Force $stage -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage | Out-Null
Copy-Item (Join-Path $root "target\release\YTLauncher.exe") $stage

$iscc = $null
$command = Get-Command iscc.exe -ErrorAction SilentlyContinue
if ($command) { $iscc = $command.Source }
if (-not $iscc) {
    $iscc = @(
        "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
        "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
        "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
    ) | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $iscc) { throw "ISCC.exe를 찾을 수 없습니다. Inno Setup 6을 설치하세요." }

& $iscc "/DAppVersion=$Version" "installer\setup.iss"
if ($LASTEXITCODE -ne 0) { throw "ISCC failed" }

$setupName = "YT-Launcher-Setup-$Version.exe"
$setupPath = Join-Path $root "dist\installer\$setupName"
if (-not (Test-Path $setupPath)) { throw "설치 프로그램이 없습니다: $setupPath" }
$hash = (Get-FileHash $setupPath -Algorithm SHA256).Hash.ToLower()
"$hash  $setupName" | Set-Content (Join-Path $root "dist\installer\SHA256SUMS.txt") -Encoding ascii
Write-Host "Done: $setupPath"
