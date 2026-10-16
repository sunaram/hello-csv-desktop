# Resizes assets\icon.png (256 px) into the MSIX tile / logo set in packaging\msix\Assets.
Add-Type -AssemblyName System.Drawing
$out = Join-Path $PSScriptRoot 'Assets'
$src = [System.Drawing.Image]::FromFile((Resolve-Path (Join-Path $PSScriptRoot '..\..\assets\icon.png')))
New-Item -ItemType Directory -Force $out | Out-Null

function Save-Icon([string]$name, [int]$w, [int]$h) {
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = 'HighQualityBicubic'
    $g.SmoothingMode = 'AntiAlias'
    $g.Clear([System.Drawing.Color]::Transparent)
    $side = [Math]::Min($w, $h)
    $g.DrawImage($src, [int](($w - $side) / 2), [int](($h - $side) / 2), $side, $side)
    $bmp.Save((Join-Path $out $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}

Save-Icon 'StoreLogo.png' 50 50
Save-Icon 'Square44x44Logo.png' 44 44
foreach ($s in 16, 24, 32, 48, 256) {
    Save-Icon "Square44x44Logo.targetsize-$s.png" $s $s
    Save-Icon "Square44x44Logo.targetsize-${s}_altform-unplated.png" $s $s
}
Save-Icon 'Square71x71Logo.png' 71 71
Save-Icon 'Square150x150Logo.png' 150 150
Save-Icon 'Square310x310Logo.png' 310 310
Save-Icon 'Wide310x150Logo.png' 310 150
$src.Dispose()
