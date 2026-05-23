$ErrorActionPreference = 'Stop'

$Repo = if ($env:AX_EVAL_REPO) { $env:AX_EVAL_REPO } else { 'mwaldstein/ax-eval' }
$InstallDir = if ($env:INSTALL_DIR) { $env:INSTALL_DIR } else { Join-Path $HOME '.local\bin' }
$Version = if ($env:AX_EVAL_VERSION) { $env:AX_EVAL_VERSION } else { 'latest' }
$IncludePrereleases = $env:AX_EVAL_INCLUDE_PRERELEASES -in @('1', 'true')
$BinName = 'ax-eval'

function Resolve-Version {
    param([string]$RequestedVersion)

    if ($RequestedVersion -ne 'latest') {
        return $RequestedVersion.TrimStart('v')
    }

    if ($IncludePrereleases) {
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases" | Select-Object -First 1
    }
    else {
        # GitHub's latest endpoint ignores prereleases.
        $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$Repo/releases/latest"
    }
    if (-not $release.tag_name) {
        throw "Unable to resolve latest release for $Repo"
    }

    return $release.tag_name.TrimStart('v')
}

function Resolve-Target {
    $arch = [System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture
    switch ($arch) {
        'X64' { return 'x86_64-pc-windows-msvc' }
        default { throw "Unsupported Windows architecture: $arch" }
    }
}

$ResolvedVersion = Resolve-Version -RequestedVersion $Version
$Target = Resolve-Target
$Asset = "$BinName-$ResolvedVersion-$Target.zip"
$BaseUrl = "https://github.com/$Repo/releases/download/v$ResolvedVersion"
$TempDir = Join-Path ([System.IO.Path]::GetTempPath()) ([System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $TempDir | Out-Null

try {
    $ArchivePath = Join-Path $TempDir $Asset
    $SumsPath = Join-Path $TempDir 'SHA256SUMS'
    Invoke-WebRequest -Uri "$BaseUrl/$Asset" -OutFile $ArchivePath
    Invoke-WebRequest -Uri "$BaseUrl/SHA256SUMS" -OutFile $SumsPath

    $checksumLine = Get-Content $SumsPath | Where-Object { $_ -match "\s$([regex]::Escape($Asset))$" } | Select-Object -First 1
    if (-not $checksumLine) {
        throw "Checksum for $Asset not found"
    }

    $expected = ($checksumLine -split '\s+')[0].ToLowerInvariant()
    $actual = (Get-FileHash -Algorithm SHA256 $ArchivePath).Hash.ToLowerInvariant()
    if ($expected -ne $actual) {
        throw "Checksum mismatch for $Asset"
    }

    Expand-Archive -Path $ArchivePath -DestinationPath $TempDir -Force
    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null
    Copy-Item (Join-Path $TempDir "$BinName.exe") (Join-Path $InstallDir "$BinName.exe") -Force

    Write-Host "Installed $BinName $ResolvedVersion to $InstallDir\$BinName.exe"
    $pathEntries = $env:PATH -split ';'
    if ($pathEntries -notcontains $InstallDir) {
        Write-Host "Add $InstallDir to PATH to run $BinName from any shell."
    }
}
finally {
    Remove-Item -Recurse -Force $TempDir -ErrorAction SilentlyContinue
}
