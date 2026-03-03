$tmp = (New-TemporaryFile).FullName
& (Join-Path $PSScriptRoot 'twf.exe') $args -cwd "$tmp"
$cwd = Get-Content -Path $tmp -Encoding UTF8
if (-not [String]::IsNullOrEmpty($cwd) -and $cwd -ne $PWD.Path){
    Set-Location -Literalpath (Resolve-Path -LiteralPath $cwd).Path
}
Remove-Item -Path $tmp
