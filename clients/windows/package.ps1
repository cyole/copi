param(
    [string]$Configuration = "Release",
    [string]$Runtime = "win-x64",
    [string]$Version = "dev"
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
    throw "The WinUI 3 client must be packaged on Windows."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")
$BuildDir = Join-Path $ScriptDir "build"
$DistDir = Join-Path $ScriptDir "dist"
$PublishDir = Join-Path $BuildDir "publish-$Runtime"
$CliPath = Join-Path $BuildDir "copi.exe"
$Project = Join-Path $ScriptDir "Copi.Windows\Copi.Windows.csproj"
$ZipPath = Join-Path $DistDir "copi-windows-$Version-$Runtime.zip"

New-Item -ItemType Directory -Force -Path $BuildDir, $DistDir | Out-Null
if (Test-Path $PublishDir) {
    Remove-Item -Recurse -Force $PublishDir
}
if (Test-Path $ZipPath) {
    Remove-Item -Force $ZipPath
}

Push-Location $RepoRoot
try {
    go build -trimpath -ldflags="-s -w" -o $CliPath .\cmd\copi
    dotnet publish $Project `
        --configuration $Configuration `
        --runtime $Runtime `
        --self-contained true `
        --output $PublishDir `
        -p:WindowsAppSDKSelfContained=true `
        -p:PublishSingleFile=false
    Copy-Item $CliPath (Join-Path $PublishDir "copi.exe") -Force
    Compress-Archive -Path (Join-Path $PublishDir "*") -DestinationPath $ZipPath
    Write-Host "Wrote $ZipPath"
}
finally {
    Pop-Location
}
