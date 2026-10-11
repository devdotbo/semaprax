param([Parameter(Mandatory = $true)][string]$Version)
$ErrorActionPreference = 'Stop'
. "$PSScriptRoot/installed-starter-journey.ps1"
$links = [System.IO.Path]::Combine($env:LOCALAPPDATA, 'Microsoft', 'WinGet', 'Links')
$cli = [System.IO.Path]::Combine($links, 'semaprax.exe')
$daemon = [System.IO.Path]::Combine($links, 'semapraxd.exe')
$scratch = [System.IO.Path]::Combine([System.IO.Path]::GetTempPath(), 'winget journey ' + [Guid]::NewGuid().ToString('N'))
try {
    foreach ($file in @($cli, $daemon)) {
        if (-not (Test-Path -LiteralPath $file)) { throw "Missing portable alias: $file" }
    }
    $cliTarget = (Get-Item -LiteralPath $cli).Target
    $daemonTarget = (Get-Item -LiteralPath $daemon).Target
    if ([System.IO.Path]::GetDirectoryName($cliTarget) -cne [System.IO.Path]::GetDirectoryName($daemonTarget)) {
        throw 'Portable aliases resolve to different package directories'
    }
    if ($cliTarget -notlike "*semaprax-v$Version-x86_64-pc-windows-msvc*") { throw 'Portable alias names a different release' }
    Invoke-SemapraxStarterJourney -Cli $cli -Daemon $daemon -Version $Version -Directory $scratch
} finally { Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue }
