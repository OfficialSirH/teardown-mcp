# Manual test helper: send tool calls to the server over stdio and print the answers.
#   .\tests\call.ps1 '{"name":"ping","arguments":{}}' '{"name":"run_lua","arguments":{"code":"GetVersion()"}}'
# An image result is saved to -ImageOut instead of printed.
[CmdletBinding(PositionalBinding = $false)]
param(
    [Parameter(ValueFromRemainingArguments = $true)][string[]]$Calls,
    [string]$ImageOut = "$env:TEMP\teardown-mcp-shot.jpg"
)
$exe = Join-Path $PSScriptRoot "..\target\release\teardown-mcp.exe"
$lines = @('{"jsonrpc":"2.0","id":0,"method":"initialize","params":{"protocolVersion":"2025-06-18","capabilities":{},"clientInfo":{"name":"test","version":"0"}}}')
$i = 0
foreach ($c in $Calls) { $i++; $lines += "{`"jsonrpc`":`"2.0`",`"id`":$i,`"method`":`"tools/call`",`"params`":$c}" }
$sw = [Diagnostics.Stopwatch]::StartNew()
$lines | & $exe | ForEach-Object {
    $j = $_ | ConvertFrom-Json
    if ($j.id -eq 0) { return }
    $tag = if ($j.result.isError) { "ERROR" } else { "ok" }
    "--- [$($j.id)] $tag  (t+$([math]::Round($sw.Elapsed.TotalSeconds,1))s)"
    foreach ($part in $j.result.content) {
        if ($part.type -eq "image") {
            [IO.File]::WriteAllBytes($ImageOut, [Convert]::FromBase64String($part.data)); "image saved to $ImageOut"
        } else { $part.text }
    }
    if ($j.error) { $j.error | ConvertTo-Json -Compress }
}
