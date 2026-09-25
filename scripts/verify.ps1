#Requires -Version 7.0
[CmdletBinding()]
param([string]$TraceProcessor)
. (Join-Path $PSScriptRoot 'support.ps1')
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    $output = New-RunDirectory $root 'verify'
    Invoke-Checked cargo @('fmt', '--all', '--check')
    Invoke-Checked cargo @('clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings')
    Invoke-Checked cargo @('test', '--workspace', '--locked')
    Invoke-Checked cargo @('build', '--workspace', '--release', '--locked')
    Invoke-Checked dotnet @('run', '--project', 'tests/Phobos.Scope.Recording.Tests', '-c', 'Release', '--', (Join-Path $output 'recorder'))
    Invoke-Checked dotnet @('run', '--project', 'samples/Phobos.Scope.Sample', '-c', 'Release', '--', (Join-Path $output 'sample.json'))
    $cli = Get-ScopeExecutable $root
    $captures = @(Get-ChildItem -LiteralPath (Join-Path $output 'recorder') -Filter '*.json')
    foreach ($capture in $captures) {
        Invoke-Checked $cli @('analyse', $capture.FullName, (Join-Path $output "reports/$($capture.BaseName)"), '4')
    }
    Invoke-Checked $cli @('analyse', 'fixtures/known-detailed.json', (Join-Path $output 'fixture-report'), '4')
    Invoke-Checked $cli @('analyse', (Join-Path $output 'sample.json'), (Join-Path $output 'sample-report'), '100')
    $known = Get-Content -LiteralPath (Join-Path $output 'reports/known-detailed/report.json') -Raw | ConvertFrom-Json
    if ($known.statistics[0].total_ms -ne 10 -or $known.statistics[1].total_ms -ne 3 -or $known.statistics[0].calls_per_second -ne 50) {
        throw 'Known C# capture did not produce expected Rust statistics.'
    }
    if (Test-Path -LiteralPath (Join-Path $output 'reports/known-summary/trace.json')) { throw 'Summary capture produced a false timeline.' }
    $dropped = Get-Content -LiteralPath (Join-Path $output 'reports/dropped/report.json') -Raw | ConvertFrom-Json
    if ($dropped.dropped_records -ne 2 -or $dropped.statistics[0].total_ms -ne 5 -or $dropped.statistics[0].retained_calls -ne 1) {
        throw 'Dropped records corrupted complete aggregates.'
    }
    $escaped = Import-Csv -LiteralPath (Join-Path $output 'reports/escaped/summary.csv')
    if ($escaped.name -ne "'=formula,`"quoted`"`nline") { throw 'CSV escaping/formula neutralisation failed.' }
    $sample = Get-Content -LiteralPath (Join-Path $output 'sample-report/report.json') -Raw | ConvertFrom-Json
    if ($sample.statistics[0].calls -ne 120 -or $sample.statistics[1].calls -ne 120 -or $sample.dropped_records -ne 0) {
        throw 'Standalone sample did not retain its full workload.'
    }
    # CLI failure paths must leave previously produced output and source captures intact.
    $summaryPath = Join-Path $output 'sample-report/summary.csv'
    $originalHash = (Get-FileHash -LiteralPath $summaryPath).Hash
    $expectedError = & $cli analyse (Join-Path $output 'sample.json') (Join-Path $output 'sample-report') 2>&1
    if ($LASTEXITCODE -eq 0 -or (Get-FileHash -LiteralPath $summaryPath).Hash -ne $originalHash) { throw 'CLI overwrote an existing report.' }
    $invalid = Join-Path $output 'truncated.json'
    Set-Content -LiteralPath $invalid -Value '{"format_version":1,"events":[' -NoNewline
    $expectedError = & $cli validate $invalid 2>&1
    if ($LASTEXITCODE -eq 0 -or "$expectedError" -notmatch 'capture.json') { throw 'CLI did not explain truncated JSON.' }
    # Both expected native failures passed their assertions; report overall success to CI.
    $global:LASTEXITCODE = 0
    if ($TraceProcessor) {
        & (Join-Path $PSScriptRoot 'verify-perfetto.ps1') -TraceProcessor $TraceProcessor -ReportDirectory (Join-Path $output 'sample-report')
    }
    Write-Host "Verified Rust behaviour, C# lifecycle, $($captures.Count) cross-language captures, sample exports and CLI failure paths."
    Write-Host "Verification artifacts: $output"
}
finally { Pop-Location }
