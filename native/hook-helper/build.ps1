# Run from an x64 Visual Studio Developer PowerShell (MSVC + Windows SDK).
$ErrorActionPreference = 'Stop'
if (-not (Get-Command cl.exe -ErrorAction SilentlyContinue)) {
    throw 'cl.exe not found. Use an x64 Visual Studio Developer PowerShell.'
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
