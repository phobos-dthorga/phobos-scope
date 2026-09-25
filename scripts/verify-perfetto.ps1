#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$TraceProcessor,
    [Parameter(Mandatory)][string]$ReportDirectory
)
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
$tracePath = Join-Path $ReportDirectory 'trace.json'
$trace = Get-Content -LiteralPath $tracePath -Raw | ConvertFrom-Json
$expectedDurations = @($trace.traceEvents | Where-Object ph -eq 'X')
$expectedCounters = @($trace.traceEvents | Where-Object ph -eq 'C')

function Query-Trace([string]$Sql) {
    $lines = & $TraceProcessor $tracePath -Q $Sql
    if ($LASTEXITCODE -ne 0) { throw 'Perfetto could not import/query the trace.' }
    return @($lines | ConvertFrom-Csv)
}

$slices = Query-Trace 'SELECT name, count(*) AS n, sum(dur) AS total_ns, max(depth) AS max_depth FROM slice GROUP BY name;'
foreach ($group in ($expectedDurations | Group-Object name)) {
    $row = @($slices | Where-Object name -eq $group.Name)
    if ($row.Count -ne 1 -or [long]$row[0].n -ne $group.Count) { throw "Perfetto lost duration slices for $($group.Name)." }
    $expectedNs = ($group.Group | Measure-Object -Property dur -Sum).Sum * 1000
    # Chrome JSON timestamps are floating-point microseconds; allow <=1 ns rounding per slice.
    if ([Math]::Abs([double]$row[0].total_ns - $expectedNs) -gt $group.Count) { throw "Perfetto changed durations for $($group.Name)." }
}
$counterCount = Query-Trace 'SELECT count(*) AS n FROM counter;'
if ([long]$counterCount[0].n -ne $expectedCounters.Count) { throw 'Perfetto lost counter observations.' }
$errors = @(Query-Trace "SELECT name, value FROM stats WHERE severity = 'error' AND value > 0;")
if ($errors.Count -ne 0) { throw "Perfetto import errors: $($errors | ConvertTo-Json -Compress)" }
if (@($expectedDurations | Where-Object name -eq 'sample.scan').Count -gt 0) {
    $scan = @($slices | Where-Object name -eq 'sample.scan')
    if ([long]$scan[0].max_depth -ne 1) { throw 'Nested sample scan did not appear inside its update slice.' }
}
Write-Host "Perfetto imported $($expectedDurations.Count) duration slices and $($expectedCounters.Count) counter observations with matching timings and no parser errors."
