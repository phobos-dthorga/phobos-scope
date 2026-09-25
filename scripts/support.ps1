Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Invoke-Checked {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "$Program failed with exit code $LASTEXITCODE." }
}

function New-RunDirectory {
    param([string]$Root, [string]$Prefix)
    $name = '{0}-{1}-{2}' -f $Prefix, (Get-Date -Format 'yyyyMMdd-HHmmss'), ([guid]::NewGuid().ToString('N').Substring(0, 8))
    $path = Join-Path $Root "artifacts/$name"
    New-Item -ItemType Directory -Path $path | Out-Null
    return $path
}

function Get-ScopeExecutable {
    param([string]$Root)
    $name = if ($IsWindows) { 'phobos-scope.exe' } else { 'phobos-scope' }
    return Join-Path $Root "target/release/$name"
}
