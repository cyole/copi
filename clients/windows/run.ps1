param(
    [string]$Configuration = "Debug"
)

$ErrorActionPreference = "Stop"

if (-not $IsWindows -and $PSVersionTable.PSEdition -eq "Core") {
    throw "The WinUI 3 client must be run on Windows."
}

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path (Join-Path $ScriptDir "..\..")
$BuildDir = Join-Path $ScriptDir "build"
$CliPath = Join-Path $BuildDir "copi.exe"
$Project = Join-Path $ScriptDir "Copi.Windows\Copi.Windows.csproj"

New-Item -ItemType Directory -Force -Path $BuildDir | Out-Null
Push-Location $RepoRoot
try {
    go build -o $CliPath .\cmd\copi
    $env:COPI_CLI = $CliPath
    dotnet run --project $Project --configuration $Configuration
}
finally {
    Pop-Location
}
