# Generates the Microsoft Store display images into docs\store\ from assets\icon.png:
#   box-art-1080x1080.png     1:1  box art
#   box-art-300x300.png       1:1  small version (Store logo / title-card minimum size)
#   poster-720x1080.png       9:16 poster art
#   hero-1920x1080.png        16:9 super hero art (optional promo image)
Add-Type -AssemblyName System.Drawing
$root = Join-Path $PSScriptRoot '..\..'
$out = Join-Path $root 'docs\store'
New-Item -ItemType Directory -Force $out | Out-Null
$out = (Resolve-Path $out).Path
$icon = [System.Drawing.Image]::FromFile((Resolve-Path (Join-Path $root 'assets\icon.png')).Path)

function New-Canvas([int]$w, [int]$h) {
    $bmp = New-Object System.Drawing.Bitmap $w, $h
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.TextRenderingHint = 'AntiAliasGridFit'
    $g.InterpolationMode = 'HighQualityBicubic'
    $rect = New-Object System.Drawing.Rectangle 0, 0, $w, $h
    $grad = New-Object System.Drawing.Drawing2D.LinearGradientBrush $rect, ([System.Drawing.Color]::FromArgb(255, 15, 23, 42)), ([System.Drawing.Color]::FromArgb(255, 14, 116, 144)), 45.0
    $g.FillRectangle($grad, $rect)
    # soft highlight in the corner for depth
    $glow = New-Object System.Drawing.Drawing2D.GraphicsPath
    $glow.AddEllipse([int](-0.3 * $w), [int](-0.4 * $h), [int](0.9 * $w), [int](0.9 * $w))
    $pg = New-Object System.Drawing.Drawing2D.PathGradientBrush $glow
    $pg.CenterColor = [System.Drawing.Color]::FromArgb(55, 255, 255, 255)
    $pg.SurroundColors = [System.Drawing.Color[]]@([System.Drawing.Color]::FromArgb(0, 255, 255, 255))
    $g.FillPath($pg, $glow)
    [pscustomobject]@{ Bmp = $bmp; G = $g }
}

# the app icon with a soft shadow, $size px square, centred on ($cx,$cy)
function Draw-Icon($g, [double]$cx, [double]$cy, [double]$size) {
    $shadow = New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb(70, 0, 0, 0))
    $g.FillEllipse($shadow, [single]($cx - $size * 0.45), [single]($cy + $size * 0.42), [single]($size * 0.9), [single]($size * 0.12))
    $g.DrawImage($icon, [single]($cx - $size / 2), [single]($cy - $size / 2), [single]$size, [single]$size)
}

function Draw-Text($g, [string]$text, [double]$cx, [double]$y, [double]$size, [bool]$bold, [int]$alpha, [string]$align = 'Center') {
    $style = if ($bold) { [System.Drawing.FontStyle]::Bold } else { [System.Drawing.FontStyle]::Regular }
    $font = New-Object System.Drawing.Font('Segoe UI', [single]$size, $style, [System.Drawing.GraphicsUnit]::Pixel)
    $sf = New-Object System.Drawing.StringFormat
    $sf.Alignment = $align; $sf.FormatFlags = 'NoWrap'
    $g.DrawString($text, $font, (New-Object System.Drawing.SolidBrush ([System.Drawing.Color]::FromArgb($alpha, 255, 255, 255))), [single]$cx, [single]$y, $sf)
}

function Save($c, $name) {
    $c.G.Dispose()
    $c.Bmp.Save((Join-Path $out $name), [System.Drawing.Imaging.ImageFormat]::Png)
    $c.Bmp.Dispose()
}
$dot = [char]0x00B7

# 1:1 box art
$c = New-Canvas 1080 1080
Draw-Icon $c.G 540 540 700
Save $c 'box-art-1080x1080.png'

# 1:1 small
$c = New-Canvas 300 300
Draw-Icon $c.G 150 150 190
Save $c 'box-art-300x300.png'

# 9:16 poster
$c = New-Canvas 720 1080
Draw-Icon $c.G 360 380 380
Draw-Text $c.G 'Hello CSV' 360 650 100 $true 255
Draw-Text $c.G 'View and edit CSV & TSV' 360 790 40 $false 235
Draw-Text $c.G 'files of any size' 360 840 40 $false 235
Draw-Text $c.G "Fast  $dot  Private  $dot  Tiny" 360 960 34 $true 215
Save $c 'poster-720x1080.png'

# 16:9 hero
$c = New-Canvas 1920 1080
Draw-Icon $c.G 400 540 460
Draw-Text $c.G 'Hello CSV' 720 360 120 $true 255 'Near'
Draw-Text $c.G 'View and edit CSV & TSV files of any size.' 726 520 50 $false 235 'Near'
Draw-Text $c.G "Millions of rows  $dot  Instant search  $dot  Sort  $dot  Edit in place" 726 605 36 $false 215 'Near'
Draw-Text $c.G 'Tiny. Fast. Private.' 726 690 42 $true 255 'Near'
Save $c 'hero-1920x1080.png'
$icon.Dispose()

Get-ChildItem $out | Select-Object Name, Length
