<#
 Per-user installer for hellocsv (no admin rights, no extra tools).
   .\install.ps1                 install  (copies hellocsv.exe, registers .csv/.tsv association, Start menu, uninstall entry)
   .\install.ps1 -Uninstall      remove everything again
 -Exe      path to hellocsv.exe        (default: hellocsv.exe next to this script, or ..\..\target\release\hellocsv.exe)
 -Prefix   install directory      (default: %LOCALAPPDATA%\Programs\hellocsv)
 -TestRoot HKCU:\Software\x       write registry keys under this key instead of the real ones (for testing)
#>
param(
    [switch]$Uninstall,
    [string]$Exe,
    [string]$Prefix = "$env:LOCALAPPDATA\Programs\hellocsv",
    [string]$TestRoot
)
$ErrorActionPreference = 'Stop'
$ProgId = 'hellocsv.Csv'
$Exts = '.csv', '.tsv', '.tab'
$Classes = if ($TestRoot) { "$TestRoot\Classes" } else { 'HKCU:\Software\Classes' }
$UninstKey = if ($TestRoot) { "$TestRoot\Uninstall\hellocsv" } else { 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\hellocsv' }
$Lnk = Join-Path ([Environment]::GetFolderPath('Programs')) 'Hello CSV.lnk'
$Target = Join-Path $Prefix 'hellocsv.exe'

function Notify-Shell {
    if ($TestRoot) { return }
    Add-Type -Namespace W -Name S -MemberDefinition '[DllImport("shell32.dll")] public static extern void SHChangeNotify(int e,int f,IntPtr a,IntPtr b);'
    [W.S]::SHChangeNotify(0x08000000, 0, [IntPtr]::Zero, [IntPtr]::Zero)   # SHCNE_ASSOCCHANGED
}
function Set-Default($key, $value) {
    New-Item -Path $key -Force | Out-Null
    Set-ItemProperty -LiteralPath $key -Name '(default)' -Value $value
}

if ($Uninstall) {
    foreach ($e in $Exts) {
        Remove-ItemProperty -LiteralPath "$Classes\$e\OpenWithProgids" -Name $ProgId -ErrorAction SilentlyContinue
        # only clear the default if it is still ours
        if ((Get-ItemProperty -LiteralPath "$Classes\$e" -ErrorAction SilentlyContinue).'(default)' -eq $ProgId) {
            Set-ItemProperty -LiteralPath "$Classes\$e" -Name '(default)' -Value ''
        }
    }
    Remove-Item -LiteralPath "$Classes\$ProgId", "$Classes\Applications\hellocsv.exe", $UninstKey -Recurse -Force -ErrorAction SilentlyContinue
    if (-not $TestRoot) {
        Remove-Item -LiteralPath $Lnk -Force -ErrorAction SilentlyContinue
        # this script lives in $Prefix, so delete the folder after it exits
        Start-Process cmd -WindowStyle Hidden -ArgumentList '/c', "ping -n 3 127.0.0.1 >nul & rmdir /s /q `"$Prefix`""
    }
    Notify-Shell
    Write-Host 'hellocsv uninstalled.'
    return
}

if (-not $Exe) {
    $Exe = @("$PSScriptRoot\hellocsv.exe", "$PSScriptRoot\..\..\target\release\hellocsv.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
}
if (-not $Exe -or -not (Test-Path $Exe)) { throw 'hellocsv.exe not found - build it first (cargo build --release) or pass -Exe.' }

$version = '0.1.0'
$m = Select-String -Path "$PSScriptRoot\..\..\Cargo.toml" -Pattern '^version\s*=\s*"(.+)"' -ErrorAction SilentlyContinue | Select-Object -First 1
if ($m) { $version = $m.Matches[0].Groups[1].Value }

if (-not $TestRoot) {
    New-Item -ItemType Directory -Force $Prefix | Out-Null
    Copy-Item $Exe $Target -Force
    Copy-Item $PSCommandPath (Join-Path $Prefix 'install.ps1') -Force
}

$open = "`"$Target`" `"%1`""
Set-Default "$Classes\$ProgId" 'CSV / TSV Document'
Set-Default "$Classes\$ProgId\DefaultIcon" "`"$Target`",0"
Set-Default "$Classes\$ProgId\shell\open\command" $open
Set-Default "$Classes\Applications\hellocsv.exe\shell\open\command" $open
Set-ItemProperty -LiteralPath "$Classes\Applications\hellocsv.exe" -Name 'FriendlyAppName' -Value 'Hello CSV'
foreach ($e in $Exts) {
    New-Item -Path "$Classes\$e\OpenWithProgids" -Force | Out-Null
    New-ItemProperty -LiteralPath "$Classes\$e\OpenWithProgids" -Name $ProgId -Value '' -PropertyType String -Force | Out-Null
    # become the default handler only when nothing else is registered
    $cur = (Get-ItemProperty -LiteralPath "$Classes\$e" -ErrorAction SilentlyContinue).'(default)'
    if (-not $cur) { Set-ItemProperty -LiteralPath "$Classes\$e" -Name '(default)' -Value $ProgId }
}

New-Item -Path $UninstKey -Force | Out-Null
$u = @{
    DisplayName     = 'Hello CSV'
    DisplayVersion  = $version
    Publisher       = 'Hello CSV'
    InstallLocation = $Prefix
    DisplayIcon     = $Target
    UninstallString = "powershell -NoProfile -ExecutionPolicy Bypass -File `"$Prefix\install.ps1`" -Uninstall"
}
foreach ($k in $u.Keys) { Set-ItemProperty -LiteralPath $UninstKey -Name $k -Value $u[$k] }
Set-ItemProperty -LiteralPath $UninstKey -Name NoModify -Value 1 -Type DWord
Set-ItemProperty -LiteralPath $UninstKey -Name NoRepair -Value 1 -Type DWord

if (-not $TestRoot) {
    $s = (New-Object -ComObject WScript.Shell).CreateShortcut($Lnk)
    $s.TargetPath = $Target
    $s.Save()
}
Notify-Shell
Write-Host "hellocsv installed to $Prefix."
Write-Host 'If .csv files do not open with it: right-click > Open with > Choose another app > Hello CSV > Always.'
