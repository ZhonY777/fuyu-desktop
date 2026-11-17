param([Parameter(Mandatory)][string]$Destination)

$ErrorActionPreference = 'Stop'
$projectAssets = Join-Path $PSScriptRoot 'src/DeskBox/obj/project.assets.json'
$assets = Get-Content -LiteralPath $projectAssets -Raw | ConvertFrom-Json
$runtime = @($assets.libraries.PSObject.Properties | Where-Object Name -Like 'Microsoft.WindowsAppSDK.Runtime/*')
if ($runtime.Count -ne 1) {
    throw 'Expected one Windows App SDK runtime in the restored project assets.'
}
$packagePath = $runtime[0].Value.path
$frameworkPath = 'tools/MSIX/win10-x64/Microsoft.WindowsAppRuntime.2.msix'
if ($frameworkPath -notin $runtime[0].Value.files) {
    throw 'The restored runtime does not declare the x64 Windows App SDK framework.'
}
$archivePath = $assets.packageFolders.PSObject.Properties.Name | ForEach-Object {
    Join-Path (Join-Path $_ $packagePath) $frameworkPath
} | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (!$archivePath) {
    throw 'The restored x64 Windows App SDK framework was not found.'
}

# The self-contained component omits this resource used by native notifications.
# Match the same restored runtime used to compile the host.
Add-Type -AssemblyName System.IO.Compression.FileSystem
$fileName = 'Microsoft.WindowsAppRuntime.Insights.Resource.dll'
$archive = [IO.Compression.ZipFile]::OpenRead($archivePath)
try {
    $entry = $archive.GetEntry($fileName)
    if (!$entry) {
        throw "The runtime framework is missing $fileName."
    }
    [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, (Join-Path $Destination $fileName), $true)
}
finally {
    $archive.Dispose()
}
