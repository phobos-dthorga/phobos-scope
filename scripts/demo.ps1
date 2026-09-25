#Requires -Version 7.0
[CmdletBinding()]
param([switch]$Benchmark)
. (Join-Path $PSScriptRoot 'support.ps1')
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $output = New-RunDirectory $root 'demo'
    Invoke-Checked cargo @('build', '--workspace', '--release', '--locked')
    Invoke-Checked dotnet @('run', '--project', 'samples/Phobos.Scope.Sample', '-c', 'Release', '--', (Join-Path $output 'capture.json'))
    Invoke-Checked (Get-ScopeExecutable $root) @('analyse', (Join-Path $output 'capture.json'), (Join-Path $output 'report'), '100')
    if ($Benchmark) {
        Invoke-Checked dotnet @('run', '--project', 'samples/Phobos.Scope.Sample', '-c', 'Release', '--no-build', '--', '--benchmark', (Join-Path $output 'overhead.json'))
    }
    Write-Host "Open $(Join-Path $output 'report/trace.json') in Perfetto. CSV files are beside it."
}
finally { Pop-Location }
