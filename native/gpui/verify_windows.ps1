# Run desktop checks with the logged-in user's interactive token. No password,
# privilege elevation, notification-policy changes or test bypass is involved.
param(
    [Parameter(Mandatory)][ValidateSet('x86_64', 'aarch64')][string]$Architecture,
    [switch]$DesktopOnly
)
$ErrorActionPreference = 'Stop'
$workspace = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$python = (Get-Command python.exe).Source
$taskName = 'lolping-verification-' + [Guid]::NewGuid().ToString('N')
$scratch = Join-Path $env:RUNNER_TEMP $taskName
New-Item -ItemType Directory -Force $scratch | Out-Null
$logs = Join-Path $workspace 'release/native/verification-logs'
New-Item -ItemType Directory -Force $logs | Out-Null
$log = Join-Path $logs "$taskName.log"
$result = Join-Path $scratch 'result.json'
$contextFile = Join-Path $scratch 'context.json'
$context = @{
    architecture = $Architecture
    desktop_only = [bool]$DesktopOnly
    log = $log
    result = $result
    environment = @{
        PATH = $env:PATH
        GITHUB_ACTIONS = $env:GITHUB_ACTIONS
        GITHUB_SHA = $env:GITHUB_SHA
        PYTHONUTF8 = '1'
    }
}
[IO.File]::WriteAllText($contextFile, ($context | ConvertTo-Json -Depth 4), [Text.UTF8Encoding]::new($false))
$shell = Get-CimInstance Win32_Process -Filter "Name='explorer.exe'" | Select-Object -First 1
if (-not $shell) { throw 'No logged-in Explorer session available for GUI verification' }
$owner = Invoke-CimMethod -InputObject $shell -MethodName GetOwner
$user = "$($owner.Domain)\$($owner.User)"
Write-Host "Running desktop verification as $user in the interactive logon session"
$script = Join-Path $PSScriptRoot 'verify_windows.py'
$action = New-ScheduledTaskAction -Execute $python -Argument ('"{0}" --ci-context "{1}"' -f $script, $contextFile) -WorkingDirectory $workspace
$principal = New-ScheduledTaskPrincipal -UserId $user -LogonType Interactive -RunLevel Limited
$settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 4) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
$registered = $false
try {
    Register-ScheduledTask -TaskName $taskName -Action $action -Principal $principal -Settings $settings -Force | Out-Null
    $registered = $true
    Start-ScheduledTask -TaskName $taskName
    $deadline = (Get-Date).AddMinutes(4)
    while (-not (Test-Path $result)) {
        if ((Get-Date) -gt $deadline) {
            $info = Get-ScheduledTaskInfo -TaskName $taskName
            throw "Desktop task timed out (last result $($info.LastTaskResult))"
        }
        Start-Sleep -Seconds 1
    }
    Get-Content $log -Encoding utf8
    $outcome = Get-Content $result -Raw | ConvertFrom-Json
    if ($outcome.exit_code -ne 0) { throw "Interactive verification failed: $($outcome.exit_code)" }
} finally {
    if ($registered) {
        Stop-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue
        Unregister-ScheduledTask -TaskName $taskName -Confirm:$false -ErrorAction SilentlyContinue
    }
    Remove-Item $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
