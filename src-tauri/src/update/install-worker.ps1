param([Parameter(Mandatory=$true)][string]$PlanPath)
$ErrorActionPreference = 'Stop'
$plan = Get-Content -LiteralPath $PlanPath -Raw -Encoding UTF8 | ConvertFrom-Json
function Write-Receipt([string]$status, [string]$detail) {
    $receipt = @{ status=$status; version=$plan.version; detail=$detail; createdAt=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds() }
    [IO.File]::WriteAllText($plan.receipt, ($receipt | ConvertTo-Json -Compress), [Text.UTF8Encoding]::new($false))
}
function Test-ParentRunning {
    try {
        $process = [System.Diagnostics.Process]::GetProcessById([int]$plan.parentPid)
        try { return -not $process.HasExited }
        finally { $process.Dispose() }
    } catch [System.ArgumentException] {
        return $false
    } catch [System.InvalidOperationException] {
        # The process can exit between opening it and reading its state.
        return $false
    }
}
try {
    # Fail before closing the running application if the staged installer has changed.
    $hash = (Get-FileHash -LiteralPath $plan.installer -Algorithm SHA256).Hash
    if ($hash -ne $plan.sha256) { throw 'Installer checksum mismatch' }
    if (-not (Test-Path -LiteralPath $plan.executable -PathType Leaf)) { throw 'Installed executable is missing' }
    [IO.File]::WriteAllText($plan.ready, 'ready')
    $deadline = [DateTime]::UtcNow.AddSeconds(120)
    while (Test-ParentRunning) {
        if ([DateTime]::UtcNow -ge $deadline) { throw 'Application did not exit in time' }
        Start-Sleep -Milliseconds 100
    }
    # The parent has fully exited, so SQLite and the installed executable are no longer held open.
    $hash = (Get-FileHash -LiteralPath $plan.installer -Algorithm SHA256).Hash
    if ($hash -ne $plan.sha256) { throw 'Installer checksum changed before installation' }
    Write-Receipt 'installing' ''
    # NSIS /D must be last and unquoted, including when the directory contains spaces.
    $arguments = @('/P', '/UPDATE', "/D=$($plan.installDir)")
    $installer = Start-Process -FilePath $plan.installer -ArgumentList $arguments -PassThru -Wait
    if ($installer.ExitCode -ne 0) { throw "Installer returned exit code $($installer.ExitCode)" }
    $version = (Get-Item -LiteralPath $plan.executable).VersionInfo.ProductVersion
    if ($version -ne $plan.version) { throw "Installed version did not match $($plan.version)" }
    Write-Receipt 'success' ''
    Start-Process -FilePath $plan.executable -WorkingDirectory $plan.installDir -WindowStyle Hidden
} catch {
    Write-Receipt 'failed' ([string]$_)
    # Relaunch only after the original process has exited; otherwise it is still able to show errors.
    if (-not (Test-ParentRunning)) {
        if (Test-Path -LiteralPath $plan.executable -PathType Leaf) {
            Start-Process -FilePath $plan.executable -WorkingDirectory $plan.installDir -WindowStyle Hidden
        }
    }
    exit 1
}
