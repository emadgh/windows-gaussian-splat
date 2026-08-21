param(
    [string]$Distro = "Ubuntu-24.04"
)

$ErrorActionPreference = "Stop"
$ProgressPreference = "SilentlyContinue"

function Test-Administrator {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = New-Object Security.Principal.WindowsPrincipal($identity)
    return $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
}

if (-not (Test-Administrator)) {
    $arguments = @(
        "-NoProfile",
        "-ExecutionPolicy", "Bypass",
        "-File", ('"{0}"' -f $PSCommandPath),
        "-Distro", $Distro
    )
    $process = Start-Process powershell.exe -Verb RunAs -Wait -PassThru -ArgumentList $arguments
    exit $process.ExitCode
}

Write-Host "[GSS] Checking WSL..."
$wslStatus = & wsl.exe --status 2>&1
if ($LASTEXITCODE -ne 0) {
    Write-Host "[GSS] Enabling WSL components..."
    & wsl.exe --install --no-distribution
    if ($LASTEXITCODE -ne 0) {
        throw "WSL installation failed. Windows may require a restart before setup can continue."
    }
}

& wsl.exe --set-default-version 2 | Out-Host

$distros = @(& wsl.exe --list --quiet 2>$null) | ForEach-Object { $_.Trim([char]0).Trim() }
if ($distros -notcontains $Distro) {
    Write-Host "[GSS] Installing $Distro..."
    & wsl.exe --install -d $Distro --no-launch
    if ($LASTEXITCODE -ne 0) {
        throw "Could not install $Distro. Restart Windows if WSL was enabled for the first time, then run setup again."
    }
}

& wsl.exe --set-version $Distro 2 | Out-Host

$bootstrapWindows = Join-Path $PSScriptRoot "bootstrap.sh"
if (-not (Test-Path $bootstrapWindows)) {
    throw "Missing embedded runtime file: $bootstrapWindows"
}

$bootstrapWsl = (& wsl.exe -d $Distro -u root -- wslpath -a -u $bootstrapWindows).Trim()
if ([string]::IsNullOrWhiteSpace($bootstrapWsl)) {
    throw "Could not translate the runtime bootstrap path into WSL."
}

Write-Host "[GSS] Installing Linux reconstruction runtime. This is resumable and safe to run again."
& wsl.exe -d $Distro -u root -- bash $bootstrapWsl
if ($LASTEXITCODE -ne 0) {
    throw "Linux runtime bootstrap failed with exit code $LASTEXITCODE."
}

Write-Host "[GSS] Runtime installation completed."
exit 0
