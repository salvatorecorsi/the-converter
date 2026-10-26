# Fail closed if a public application-only staging tree accidentally includes media engines.
[CmdletBinding()]
param([Parameter(Mandatory)][string]$SourceDirectory)
$ErrorActionPreference = 'Stop'
$source = (Resolve-Path -LiteralPath $SourceDirectory).Path
$required = @(
    'the-converter.exe',
    'the-converter-cli.exe',
    'the_converter_shell.dll',
    'THIRD-PARTY-NOTICES.md',
    'docs\release-licenses\README.md',
    'docs\release-licenses\RUST-THIRD-PARTY-LICENSES.txt',
    'docs\release-licenses\rust-components.json',
    'docs\release-licenses\option-ext-0.2.0\LICENSE.txt',
    'docs\release-licenses\option-ext-0.2.0\src\lib.rs',
    'docs\release-licenses\option-ext-0.2.0\src\impl.rs',
    'docs\release-licenses\rust-runtime\COPYRIGHT-library.html'
)
foreach ($relative in $required) {
    if (-not (Test-Path -LiteralPath (Join-Path $source $relative) -PathType Leaf)) {
        throw "Release file missing: $relative"
    }
}
$allow = @('the-converter.exe', 'the-converter-cli.exe', 'the_converter_shell.dll')
foreach ($file in Get-ChildItem -LiteralPath $source -File -Recurse) {
    if ($file.Extension -in @('.exe','.dll')) {
        $relative = $file.FullName.Substring($source.Length+1)
        if ($relative -notin $allow) { throw "Unexpected executable/library in thin release: $relative" }
    }
}
$components = Get-Content -LiteralPath (Join-Path $source 'docs\release-licenses\rust-components.json') -Raw | ConvertFrom-Json
if (@($components).Count -eq 0 -or @($components | Where-Object { $_.notice_count -lt 1 }).Count -ne 0) {
    throw 'Rust third-party notice inventory is incomplete.'
}
Write-Output "Thin release verified: app, CLI, Explorer DLL and $(@($components).Count) crate notices; no bundled media-engine executables or DLLs."
