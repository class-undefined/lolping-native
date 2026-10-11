# Run from a Visual Studio Developer PowerShell targeting x64 or ARM64.
param([ValidateSet('x64', 'arm64')][string]$Architecture = $env:VSCMD_ARG_TGT_ARCH)
$ErrorActionPreference = 'Stop'
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    throw 'cl.exe not found. Use a Visual Studio Developer PowerShell.'
}
if ($Architecture -and $env:VSCMD_ARG_TGT_ARCH -ne $Architecture) {
    throw "Expected MSVC target $Architecture; developer environment targets $env:VSCMD_ARG_TGT_ARCH"
}
Push-Location $PSScriptRoot
try {
    New-Item -ItemType Directory -Force build | Out-Null
    $flags = @('/nologo', '/std:c++20', '/Zc:preprocessor', '/EHsc', '/utf-8', '/W4', '/MT', '/Fobuild\')
    & cl.exe @flags /O2 src/main_win.cpp src/hooks_win.cpp src/simulate.cpp src/commands.cpp `
        src/output.cpp src/json.cpp src/decision.cpp /Febuild/hook-helper.exe /link user32.lib
    if ($LASTEXITCODE -ne 0) { throw 'Input helper build failed' }
    & cl.exe @flags /Od tests/helper_tests.cpp src/decision.cpp src/commands.cpp src/macinput.cpp `
        src/json.cpp src/output.cpp /Febuild/helper_tests.exe
    if ($LASTEXITCODE -ne 0) { throw 'Input helper tests build failed' }
} finally {
    Pop-Location
}
