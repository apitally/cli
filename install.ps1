# Apitally CLI installer
# https://github.com/apitally/cli
#
# Usage:
#   irm https://apitally.io/cli/install.ps1 | iex
#
# Environment variables:
#   APITALLY_VERSION     - Install a specific version (e.g. "v0.1.0") instead of latest
#   APITALLY_INSTALL_DIR - Override the install directory (default: ~/.local/bin)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$REPO = "apitally/cli"
$BINARY = "apitally"
$TMP_DIR = $null

# --- Helper functions --------------------------------------------------------

function Say {
    param([string]$Message)

    Write-Host $Message
}

function Err {
    param([string]$Message)

    throw "error: $Message"
}

function Normalize-Path {
    param([string]$PathValue)

    if ([string]::IsNullOrWhiteSpace($PathValue)) {
        return ""
    }

    return $PathValue.Trim().TrimEnd("\")
}

function Path-Contains {
    param(
        [string]$PathValue,
        [string]$Directory
    )

    $needle = Normalize-Path $Directory
    foreach ($entry in ($PathValue -split ";")) {
        if ((Normalize-Path $entry) -ieq $needle) {
            return $true
        }
    }

    return $false
}

function New-WebClient {
    $client = New-Object Net.WebClient
    $client.Headers["User-Agent"] = "apitally-installer"
    return $client
}

# Download a URL to a file
function Download-File {
    param(
        [string]$Url,
        [string]$Destination
    )

    $client = New-WebClient
    try {
        $client.DownloadFile($Url, $Destination)
    } finally {
        $client.Dispose()
    }
}

# Fetch a URL and parse it as JSON
function Fetch-Json {
    param([string]$Url)

    $client = New-WebClient
    try {
        return $client.DownloadString($Url) | ConvertFrom-Json
    } finally {
        $client.Dispose()
    }
}

function Get-Arch {
    try {
        $assembly = [System.Reflection.Assembly]::LoadWithPartialName("System.Runtime.InteropServices.RuntimeInformation")
        if ($assembly) {
            $runtimeInformationType = $assembly.GetType("System.Runtime.InteropServices.RuntimeInformation")
            $osArchitectureProperty = $runtimeInformationType.GetProperty("OSArchitecture")

            switch ($osArchitectureProperty.GetValue($null).ToString()) {
                "X64" { return "x86_64" }
                "Arm64" { return "aarch64" }
                "X86" { Err "32-bit Windows is not supported" }
            }
        }
    } catch {
    }

    if ([System.Environment]::Is64BitOperatingSystem) {
        return "x86_64"
    }

    Err "unsupported architecture"
}

function Add-Path {
    param([string]$Directory)

    $registryPath = "registry::HKEY_CURRENT_USER\Environment"
    $currentPath = (Get-Item -LiteralPath $registryPath).GetValue("Path", "", "DoNotExpandEnvironmentNames")

    if (Path-Contains $currentPath $Directory) {
        return $false
    }

    $entries = @()
    foreach ($entry in ($currentPath -split ";")) {
        if (-not [string]::IsNullOrWhiteSpace($entry)) {
            $entries += $entry
        }
    }

    $newPath = (@($Directory) + $entries) -join ";"
    Set-ItemProperty -LiteralPath $registryPath -Name Path -Value $newPath -Type ExpandString

    # Trigger a lightweight environment refresh for future shells.
    $dummyName = "apitally-installer-" + [guid]::NewGuid().ToString()
    [Environment]::SetEnvironmentVariable($dummyName, "1", "User")
    [Environment]::SetEnvironmentVariable($dummyName, $null, "User")

    return $true
}

function Add-Session-Path {
    param([string]$Directory)

    if (Path-Contains $env:Path $Directory) {
        return $false
    }

    if ([string]::IsNullOrEmpty($env:Path)) {
        $env:Path = $Directory
    } else {
        $env:Path = "$Directory;$env:Path"
    }

    return $true
}

function New-Temp-Dir {
    $path = Join-Path ([System.IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Path $path | Out-Null
    return $path
}

# --- Main --------------------------------------------------------------------

function Main {
    if ($PSVersionTable.PSVersion.Major -lt 5) {
        Err "PowerShell 5 or later is required to install $BINARY"
    }

    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12

    $arch = Get-Arch
    $target = "$arch-pc-windows-msvc"
    $archive = "$BINARY-$target.zip"

    if ($env:APITALLY_VERSION) {
        $version = $env:APITALLY_VERSION
    } else {
        try {
            $release = Fetch-Json "https://api.github.com/repos/$REPO/releases/latest"
        } catch {
            Err "could not fetch latest release from GitHub (check your internet connection)"
        }

        if (-not $release.tag_name) {
            Err "could not determine latest version"
        }

        $version = $release.tag_name
    }

    if ($env:APITALLY_INSTALL_DIR) {
        $installDir = $env:APITALLY_INSTALL_DIR
    } else {
        $installDir = Join-Path $HOME ".local\bin"
    }

    Say "downloading $BINARY $version..."

    $script:TMP_DIR = New-Temp-Dir
    $archivePath = Join-Path $TMP_DIR $archive
    $binaryPath = Join-Path $TMP_DIR "$BINARY.exe"
    $installPath = Join-Path $installDir "$BINARY.exe"
    $url = "https://github.com/$REPO/releases/download/$version/$archive"

    try {
        Download-File $url $archivePath
    } catch {
        Err "failed to download $url"
    }

    Expand-Archive -LiteralPath $archivePath -DestinationPath $TMP_DIR -Force

    if (-not (Test-Path -LiteralPath $binaryPath)) {
        Err "downloaded archive did not contain $BINARY.exe"
    }

    New-Item -ItemType Directory -Path $installDir -Force | Out-Null
    Copy-Item -LiteralPath $binaryPath -Destination $installPath -Force

    Say "installed to $installPath"

    $pathAdded = $false
    try {
        $pathAdded = Add-Path $installDir
    } catch {
        Say "warning: could not add $installDir to PATH automatically"
    }

    if ($pathAdded) {
        $null = Add-Session-Path $installDir
        Say "added $installDir to PATH"
    }

    if (-not (Path-Contains $env:Path $installDir)) {
        Say "restart PowerShell or run: `$env:Path = `"$installDir;`$env:Path`""
    }
}

try {
    Main
} catch {
    Write-Host $_.Exception.Message -ForegroundColor Red
    exit 1
} finally {
    if ($TMP_DIR -and (Test-Path -LiteralPath $TMP_DIR)) {
        Remove-Item -LiteralPath $TMP_DIR -Recurse -Force -ErrorAction SilentlyContinue
    }
}
