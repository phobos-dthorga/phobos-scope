#Requires -Version 7.0
# Optional visual review in a separate headless browser profile; never uses an open browser session.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$HtmlFile,
    [Parameter(Mandatory)][string]$BrowserPath
)
. (Join-Path $PSScriptRoot 'support.ps1')
if (-not $IsWindows) { throw 'This optional renderer uses Windows Chromium/Edge. Open the HTML directly on other systems.' }
$source = (Resolve-Path -LiteralPath $HtmlFile).Path
$browser = (Resolve-Path -LiteralPath $BrowserPath).Path
if ([IO.Path]::GetExtension($source) -ne '.html') { throw 'Select a generated HTML report.' }
$output = New-RunDirectory (Split-Path -Parent $PSScriptRoot) 'html-review'
$url = [uri]::new($source).AbsoluteUri
foreach ($view in @(@{ Name = 'desktop'; Size = '1440,1800' }, @{ Name = 'narrow'; Size = '600,1800' })) {
    $image = Join-Path $output ($view.Name + '.png')
    $profile = Join-Path $output ('profile-' + $view.Name)
    $arguments = @('--headless', '--disable-gpu', '--no-first-run', '--no-default-browser-check', '--hide-scrollbars',
        ('--user-data-dir="' + $profile + '"'), ('--screenshot="' + $image + '"'), ('--window-size=' + $view.Size), ('"' + $url + '"'))
    $process = Start-Process -FilePath $browser -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if (-not $process.WaitForExit(30000)) { throw "Report renderer is still running with PID $($process.Id)." }
    if ($process.ExitCode -ne 0 -or -not (Test-Path -LiteralPath $image)) { throw 'The browser did not produce a screenshot.' }
    Write-Output $image
}
