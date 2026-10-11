# Shared checkout-free installed CLI/daemon acceptance for script and WinGet
# routes. Callers provide exact executable paths; nothing is built or fetched.
function Invoke-SemapraxStarterProcess {
    param([string]$File, [string[]]$Arguments, [string]$Directory, [string]$InputText = '')
    $start = New-Object System.Diagnostics.ProcessStartInfo
    $start.FileName = $File
    # Paths occur as individual quoted arguments; Windows escaping retains
    # trailing backslashes and literal quotes rather than invoking a shell.
    $start.Arguments = (($Arguments | ForEach-Object {
        '"' + (($_ -replace '(\\*)"', '$1$1\"') -replace '(\\+)$', '$1$1') + '"'
    }) -join ' ')
    $start.WorkingDirectory = $Directory
    $start.UseShellExecute = $false
    $start.RedirectStandardOutput = $true
    $start.RedirectStandardError = $true
    $start.RedirectStandardInput = $true
    $process = [System.Diagnostics.Process]::Start($start)
    try {
        $output = $process.StandardOutput.ReadToEndAsync()
        $errors = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.Write($InputText)
        $process.StandardInput.Close()
        if (-not $process.WaitForExit(30000)) {
            $process.Kill()
            throw "Installed process timed out: $File"
        }
        $process.WaitForExit()
        if ($process.ExitCode -ne 0) { throw "Installed process failed: $File ($($process.ExitCode)): $($errors.Result)" }
        return $output.Result
    } finally { $process.Dispose() }
}

function Invoke-SemapraxStarterJourney {
    param([string]$Cli, [string]$Daemon, [string]$Version, [string]$Commit, [string]$Directory)
    [void][System.IO.Directory]::CreateDirectory($Directory)
    $identity = (Invoke-SemapraxStarterProcess -File $Cli -Arguments @('version', '--json') -Directory $Directory) | ConvertFrom-Json
    if ($identity.version -cne $Version -or ($Commit -and $identity.commit -cne $Commit)) {
        throw 'Installed CLI version/commit does not match the selected release'
    }
    foreach ($arguments in @(
        @('--version'), @('new', 'first-semaprax'),
        @('check', 'first-semaprax/semaprax.toml'),
        @('test', 'first-semaprax/semaprax.toml'),
        @('run', 'first-semaprax/semaprax.toml'))) {
        $result = Invoke-SemapraxStarterProcess -File $Cli -Arguments $arguments -Directory $Directory
    }
    if ($result.Trim().Split("`n")[-1].Trim() -cne '42') { throw 'Installed starter did not print 42' }
    $requests = '{"jsonrpc":"2.0","id":1,"method":"protocol"}' + "`n" + '{"jsonrpc":"2.0","id":2,"method":"shutdown"}' + "`n"
    $output = Invoke-SemapraxStarterProcess -File $Daemon -Directory $Directory -InputText $requests -Arguments @(
        '--stdio', '--manifest-path', ([System.IO.Path]::Combine($Directory, 'first-semaprax', 'semaprax.toml')),
        '--max-request-bytes', '65536', '--max-response-bytes', '1048576')
    $responses = @($output.Trim().Split("`n") | ForEach-Object { $_ | ConvertFrom-Json })
    if ($responses.Count -ne 2 -or $responses[0].id -ne 1 -or $responses[1].id -ne 2 -or
        $responses[0].result.version -cne $Version -or $responses[0].result.protocol -notlike 'semaprax.agent-transport.v*' -or
        $null -ne $responses[0].error -or $null -ne $responses[1].error) {
        throw 'Installed daemon protocol/shutdown disagrees with the release'
    }
    Write-Host "Installed starter printed 42; daemon protocol/shutdown passed ($Version)."
}
