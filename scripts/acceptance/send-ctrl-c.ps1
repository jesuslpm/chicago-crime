<#
.SYNOPSIS
    Presses Ctrl+C on a generator run for AC-G7. Started by run-acceptance.ps1 in a console of
    its own; never run it from your terminal, because it sends Ctrl+C to every process of its
    console.

.DESCRIPTION
    Reads a JSON request (Exe, Arguments, DelaySeconds, TempDir, ResultFile), starts the generator
    in this same console, waits, and sends a real CTRL_C_EVENT with GenerateConsoleCtrlEvent, as a
    keyboard Ctrl+C would. Then writes what happened to ResultFile as JSON.
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$RequestFile
)

$ErrorActionPreference = 'Stop'
$request = Get-Content -LiteralPath $RequestFile -Raw | ConvertFrom-Json

Add-Type -TypeDefinition @"
using System;
using System.Runtime.InteropServices;
public static class AcceptanceConsole {
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool GenerateConsoleCtrlEvent(uint ctrlEvent, uint processGroupId);
    [DllImport("kernel32.dll", SetLastError = true)]
    public static extern bool SetConsoleCtrlHandler(IntPtr handler, bool add);
}
"@

$arguments = ($request.Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
# Whoever started this console may have disabled Ctrl+C processing, and children inherit that
# attribute: enable it again, as in an ordinary terminal, before starting the generator.
$enabled = [AcceptanceConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $false)
$process = Start-Process -FilePath $request.Exe -ArgumentList $arguments -NoNewWindow -PassThru
$null = $process.Handle  # keeps the exit code readable once the process ends
Start-Sleep -Seconds $request.DelaySeconds

$runningWhenSent = -not $process.HasExited
$tempDuring = @(Get-ChildItem -LiteralPath $request.TempDir -Force | ForEach-Object { $_.Name })
# This script must survive the event: ignore Ctrl+C here, then send it to the whole console.
$ignoredHere = [AcceptanceConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)
$sent = [AcceptanceConsole]::GenerateConsoleCtrlEvent(0, 0)
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$process.WaitForExit()
$watch.Stop()

[pscustomobject]@{
    CtrlCEnabledForChild = $enabled
    RunningWhenSent      = $runningWhenSent
    IgnoredHere          = $ignoredHere
    Sent                 = $sent
    TempDuring           = $tempDuring
    SecondsToExit        = [math]::Round($watch.Elapsed.TotalSeconds, 2)
    ExitCode             = $process.ExitCode
} | ConvertTo-Json | Set-Content -LiteralPath $request.ResultFile -Encoding UTF8
