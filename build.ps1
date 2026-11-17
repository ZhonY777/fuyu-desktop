param([string]$Destination = (Join-Path $PSScriptRoot 'dist/win-x64'))

$ErrorActionPreference = 'Stop'
$desktop = Join-Path $PSScriptRoot 'desktop'
$island = Join-Path $PSScriptRoot 'island'
$app = Join-Path $desktop 'src/DeskBox/DeskBox.csproj'
$updater = Join-Path $desktop 'src/DeskBox.Updater/DeskBox.Updater.csproj'
$output = [IO.Path]::GetFullPath($Destination)
$platform = @('-p:Platform=x64', '-r', 'win-x64')

Push-Location $island
try {
    $rustVersion = rustc -vV
    if ($LASTEXITCODE -ne 0 -or !($rustVersion -match '^host: x86_64-pc-windows-(msvc|gnu)$')) {
        throw 'An x64 Windows Rust toolchain is required.'
    }
    cargo rustc --locked --release --lib --features embedded --crate-type cdylib --target-dir (Join-Path $island 'target')
    if ($LASTEXITCODE -ne 0) { throw 'Failed to build the island module.' }
}
finally {
    Pop-Location
}

Push-Location $desktop
try {
    dotnet restore $updater @platform
    if ($LASTEXITCODE -ne 0) { throw 'Failed to restore the updater.' }
    dotnet publish $app -c Release @platform --self-contained true -p:WindowsAppSDKSelfContained=true -o $output
    if ($LASTEXITCODE -ne 0) { throw 'Failed to publish the Windows application.' }
}
finally {
    Pop-Location
}

& (Join-Path $desktop 'copy-winappsdk-resource.ps1') -Destination $output
Copy-Item -LiteralPath (Join-Path $island 'target/release/daozai.dll') -Destination (Join-Path $output 'FuyuIsland.dll') -Force
Copy-Item -LiteralPath (Join-Path $island 'LICENSE') -Destination (Join-Path $output 'DAOZAI-LICENSE.txt') -Force
Copy-Item -LiteralPath (Join-Path $island 'THIRD_PARTY_NOTICES.txt') -Destination $output -Force
Copy-Item -LiteralPath (Join-Path $desktop 'LICENSE') -Destination (Join-Path $output 'DESKBOX-LICENSE.txt') -Force
