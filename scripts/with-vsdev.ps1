param(
  [Parameter(ValueFromRemainingArguments = $true)]
  [string[]] $Command
)

$programFilesX86 = [Environment]::GetFolderPath("ProgramFilesX86")
$vsWhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"
$vsDevCmd = $null

if (Test-Path $vsWhere) {
  $vsDevCmd = & $vsWhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -find Common7\Tools\VsDevCmd.bat |
    Select-Object -First 1
}

if (-not $vsDevCmd) {
  foreach ($edition in @("BuildTools", "Community", "Professional", "Enterprise")) {
    $candidate = Join-Path $programFilesX86 "Microsoft Visual Studio\2022\$edition\Common7\Tools\VsDevCmd.bat"
    if (Test-Path $candidate) {
      $vsDevCmd = $candidate
      break
    }
  }
}

if (-not $vsDevCmd) {
  Write-Error "VsDevCmd.bat not found. Install Visual Studio Build Tools with the C++ workload."
  exit 1
}

if (-not $Command -or $Command.Count -eq 0) {
  Write-Error "Usage: powershell -File scripts/with-vsdev.ps1 <command> [args...]"
  exit 1
}

$staleApp = Get-Process -Name "yinghuoji_desktop" -ErrorAction SilentlyContinue
if ($staleApp) {
  $staleApp | Stop-Process -Force
}

function ConvertTo-CmdArgument {
  param([string] $Value)

  if ($null -eq $Value -or $Value.Length -eq 0) {
    return '""'
  }

  if ($Value -notmatch '[\s&()^|<>"]') {
    return $Value
  }

  return '"' + ($Value -replace '"', '\"') + '"'
}

$commandLine = ($Command | ForEach-Object { ConvertTo-CmdArgument $_ }) -join ' '
$cmdLine = 'call "' + $vsDevCmd + '" -arch=x64 -host_arch=x64 >nul && ' + $commandLine
cmd.exe /d /s /c $cmdLine
exit $LASTEXITCODE
