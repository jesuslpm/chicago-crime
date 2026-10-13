<#
.SYNOPSIS
    Presses Ctrl+C on a run for AC-G7 (generator) and AC-C10 (query app). Started by the
    acceptance scripts in a console of its own; never run it from your terminal, because it sends
    Ctrl+C to every process of its console.

.DESCRIPTION
    Reads a JSON request, starts the program in this same console, waits, and sends a real
    CTRL_C_EVENT with GenerateConsoleCtrlEvent, as a keyboard Ctrl+C would. Then writes what
    happened to ResultFile as JSON.

    Request fields:
      Exe, Arguments, ResultFile   always;
      DelaySeconds                 press Ctrl+C after this many seconds, unless TriggerDir is set;
      TriggerDir, TriggerMinBytes  press Ctrl+C as soon as a file in TriggerDir holds at least
                                   TriggerMinBytes bytes (a state, not the clock), or once
      TimeoutSeconds               have passed without it (Observed is then null);
      TempDir                      a folder listed when Ctrl+C is pressed (optional);
      StdoutFile, StderrFile       capture the program's output as UTF-8 (optional).
#>
param(
    [Parameter(Mandatory = $true)]
    [string]$RequestFile
)

$ErrorActionPreference = 'Stop'
$request = Get-Content -LiteralPath $RequestFile -Raw | ConvertFrom-Json

function Get-RequestField([string]$Name) {
    if ($request.PSObject.Properties[$Name]) { $request.$Name } else { $null }
}

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

# The first file in $Dir that holds at least $MinBytes bytes, in words, or $null. The size is read
# through a handle, because the folder entry of a file still being written may lag behind.
function Get-TriggerState([string]$Dir, [long]$MinBytes) {
    foreach ($file in @(Get-ChildItem -LiteralPath $Dir -File -Force)) {
        $length = try {
            $stream = [System.IO.File]::Open($file.FullName, 'Open', 'Read', 'ReadWrite, Delete')
            try { $stream.Length } finally { $stream.Dispose() }
        }
        catch { $file.Length }
        if ($length -ge $MinBytes) { return ('file {0} held {1} bytes' -f $file.Name, $length) }
    }
    $null
}

$arguments = ($request.Arguments | ForEach-Object { '"' + $_ + '"' }) -join ' '
$stdoutFile = Get-RequestField 'StdoutFile'
$stderrFile = Get-RequestField 'StderrFile'
$captured = $stdoutFile -or $stderrFile
# Whoever started this console may have disabled Ctrl+C processing, and children inherit that
# attribute: enable it again, as in an ordinary terminal, before starting the program.
$enabled = [AcceptanceConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $false)
if ($captured) {
    # Redirected output, but no window of its own: the program shares this console and so
    # receives the Ctrl+C.
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $request.Exe
    $psi.Arguments = $arguments
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdoutTask = $process.StandardOutput.ReadToEndAsync()
    $stderrTask = $process.StandardError.ReadToEndAsync()
}
else {
    $process = Start-Process -FilePath $request.Exe -ArgumentList $arguments -NoNewWindow -PassThru
}
$null = $process.Handle  # keeps the exit code readable once the process ends
$clock = [System.Diagnostics.Stopwatch]::StartNew()

$triggerDir = Get-RequestField 'TriggerDir'
$observed = $null
if ($triggerDir) {
    $minBytes = [long](Get-RequestField 'TriggerMinBytes')
    $timeout = [double](Get-RequestField 'TimeoutSeconds')
    while (-not $process.HasExited -and $clock.Elapsed.TotalSeconds -lt $timeout) {
        # A read that fails while the program changes its files only means "not yet".
        $observed = try { Get-TriggerState $triggerDir $minBytes } catch { $null }
        if ($observed) { break }
        Start-Sleep -Milliseconds 50
    }
}
else {
    Start-Sleep -Seconds $request.DelaySeconds
}

$sentAtSeconds = $clock.Elapsed.TotalSeconds
$runningWhenSent = -not $process.HasExited
$tempDir = Get-RequestField 'TempDir'
$tempDuring = if ($tempDir) { @(Get-ChildItem -LiteralPath $tempDir -Force | ForEach-Object { $_.Name }) } else { @() }
# This script must survive the event: ignore Ctrl+C here, then send it to the whole console.
$ignoredHere = [AcceptanceConsole]::SetConsoleCtrlHandler([IntPtr]::Zero, $true)
$sent = [AcceptanceConsole]::GenerateConsoleCtrlEvent(0, 0)
$watch = [System.Diagnostics.Stopwatch]::StartNew()
$process.WaitForExit()
$watch.Stop()

if ($captured) {
    $utf8 = New-Object System.Text.UTF8Encoding $false
    if ($stdoutFile) { [System.IO.File]::WriteAllText($stdoutFile, $stdoutTask.Result, $utf8) }
    if ($stderrFile) { [System.IO.File]::WriteAllText($stderrFile, $stderrTask.Result, $utf8) }
}

[pscustomobject]@{
    CtrlCEnabledForChild = $enabled
    RunningWhenSent      = $runningWhenSent
    IgnoredHere          = $ignoredHere
    Sent                 = $sent
    SentAtSeconds        = [math]::Round($sentAtSeconds, 2)
    Observed             = $observed
    TempDuring           = $tempDuring
    SecondsToExit        = [math]::Round($watch.Elapsed.TotalSeconds, 2)
    ExitCode             = $process.ExitCode
} | ConvertTo-Json | Set-Content -LiteralPath $request.ResultFile -Encoding UTF8
