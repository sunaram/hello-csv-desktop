<#
 Captures a real Hello CSV window to a PNG (Windows). Uses PrintWindow, so the window may be larger than the screen.
   .\tools\screenshot.ps1 -Csv samples\sales.csv -Out docs\screenshots\table.png [-Theme dark] [-Width 1920] [-Height 1080] [-Zoom 1]
       [-Steps 'click:300,260', 'keys:^f', 'keys:Seattle', 'wait:2']
 Steps run in order before the capture (click coordinates are screen pixels of the top-left part of the window;
 keys use SendKeys syntax). Runs with a throw-away APPDATA so your own settings are untouched.
#>
param(
    [Parameter(Mandatory)][string]$Csv,
    [Parameter(Mandatory)][string]$Out,
    [ValidateSet('', 'light', 'dark')][string]$Theme = '',
    [int]$Width = 1920,
    [int]$Height = 1080,
    [double]$Zoom = 1.0,
    [int]$WaitSeconds = 4,
    [string[]]$Steps = @(),
    [string]$Exe
)
$ErrorActionPreference = 'Stop'
if (-not $Exe) { $Exe = Join-Path $PSScriptRoot '../target/release/hellocsv.exe' }
Add-Type -AssemblyName System.Drawing, System.Windows.Forms
Add-Type @'
using System; using System.Runtime.InteropServices;
public class Win {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int hh, uint flags);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool PrintWindow(IntPtr h, IntPtr hdc, uint flags);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(int f, int dx, int dy, int d, int e);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
'@
[void][Win]::SetProcessDPIAware()

$tmp = Join-Path $env:TEMP ('hellocsv-shot-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Force "$tmp\hellocsv" | Out-Null
Set-Content "$tmp\hellocsv\settings.txt" "theme=$Theme`nzoom=$Zoom`nupdates=off`n"
$psi = New-Object System.Diagnostics.ProcessStartInfo
$psi.FileName = (Resolve-Path $Exe).Path
$psi.Arguments = '"' + (Resolve-Path $Csv).Path + '"'
$psi.UseShellExecute = $false
$psi.EnvironmentVariables['APPDATA'] = $tmp
$psi.EnvironmentVariables['HELLOCSV_LOCALE'] = 'en-US'   # 1,000,000 style number grouping in the images
$p = [System.Diagnostics.Process]::Start($psi)
try {
    for ($i = 0; $i -lt 50 -and $p.MainWindowHandle -eq 0; $i++) { Start-Sleep -Milliseconds 200; $p.Refresh() }
    $h = $p.MainWindowHandle
    if ($h -eq 0) { throw 'window did not appear' }
    [void][Win]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, ($Width + 22), ($Height + 13), 0x0044)   # grown by the invisible resize frame (10 px left/right/bottom) so the visible window is exactly Width x Height
      # SWP_NOZORDER | SWP_SHOWWINDOW
    Start-Sleep -Seconds 1
    [void][Win]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, ($Width + 21), ($Height + 12), 0x0044)   # nudge so the menu bar repaints at the final width
    [void][Win]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, ($Width + 22), ($Height + 13), 0x0044)
    Start-Sleep -Seconds $WaitSeconds
    [void][Win]::SetForegroundWindow($h)
    foreach ($s in $Steps) {
        $kind, $arg = $s -split ':', 2
        switch ($kind) {
            'click' { $x, $y = $arg -split ','; [void][Win]::SetCursorPos([int]$x, [int]$y); [Win]::mouse_event(2, 0, 0, 0, 0); [Win]::mouse_event(4, 0, 0, 0, 0); Start-Sleep -Milliseconds 600 }
            'dbl' { $x, $y = $arg -split ','; [void][Win]::SetCursorPos([int]$x, [int]$y); foreach ($k in 1..2) { [Win]::mouse_event(2, 0, 0, 0, 0); [Win]::mouse_event(4, 0, 0, 0, 0) }; Start-Sleep -Milliseconds 600 }
            'keys' { [System.Windows.Forms.SendKeys]::SendWait($arg); Start-Sleep -Milliseconds 500 }
            'wait' { Start-Sleep -Seconds ([double]$arg) }
        }
    }
    [void][Win]::SetCursorPos(1270, 710)   # park the pointer so no hover state shows
    Start-Sleep -Milliseconds 400
    $r = New-Object Win+RECT
    [void][Win]::GetWindowRect($h, [ref]$r)
    $w = $r.R - $r.L; $ht = $r.B - $r.T
    $bmp = New-Object System.Drawing.Bitmap $w, $ht
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $hdc = $g.GetHdc()
    $ok = [Win]::PrintWindow($h, $hdc, 2)                                            # PW_RENDERFULLCONTENT
    $g.ReleaseHdc($hdc); $g.Dispose()
    New-Item -ItemType Directory -Force (Split-Path -Parent $Out) | Out-Null
    $crop = $bmp.Clone((New-Object System.Drawing.Rectangle 11, 2, $Width, $Height), $bmp.PixelFormat)   # drop the invisible frame
    $bmp.Dispose(); $bmp = $crop
    # PrintWindow leaves the last few px of the menu bar unpainted: extend the neighbouring colour over them
    foreach ($y in 0..90) {
        if ($bmp.GetPixel($Width - 1, $y).ToArgb() -eq [System.Drawing.Color]::Black.ToArgb() -and $bmp.GetPixel($Width - 40, $y).ToArgb() -ne [System.Drawing.Color]::Black.ToArgb()) {
            $x = $Width - 1
            while ($x -gt 0 -and $bmp.GetPixel($x, $y).ToArgb() -eq [System.Drawing.Color]::Black.ToArgb()) { $x-- }
            $fill = $bmp.GetPixel($x, $y)
            foreach ($i in ($x + 1)..($Width - 1)) { $bmp.SetPixel($i, $y, $fill) }
        }
    }
    $bmp.Save((Join-Path (Resolve-Path (Split-Path -Parent $Out)) (Split-Path -Leaf $Out)), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
    "captured ${w}x${ht} printwindow=$ok -> $Out"
} finally {
    if (-not $p.HasExited) { $p.Kill() }
    $p.WaitForExit()
    Start-Sleep -Milliseconds 500
    Remove-Item $tmp -Recurse -Force -ErrorAction SilentlyContinue
}
