<#
 Builds hellocsv.msix from target\release\hellocsv.exe.   Prerequisites: cargo build --release; Windows 10/11 SDK.

 Store submission (unsigned is fine - the Store signs it). Values can also come from the env vars
 MSIX_IDENTITY, MSIX_PUBLISHER, MSIX_PUBLISHER_DISPLAY, MSIX_DISPLAY_NAME (CI sets them from repo variables):
   .\build.ps1 -Identity "12345Name.HelloCSV" -Publisher "CN=XXXXXXXX-XXXX-..." -PublisherDisplayName "Your Name"
   (use the three values from Partner Center > Product management > Product identity)

 Local test install (self-signed):
   .\build.ps1 -Sign          creates/uses a self-signed cert "CN=hellocsv-dev", signs the package
   then, once, in an ADMIN PowerShell:  Import-Certificate -FilePath out\hellocsv-dev.cer -CertStoreLocation Cert:\LocalMachine\TrustedPeople
   then double-click out\hellocsv.msix (or Add-AppxPackage out\hellocsv.msix)
#>
param(
    [string]$Identity = $(if ($env:MSIX_IDENTITY) { $env:MSIX_IDENTITY } else { 'hellocsv.HelloCSV' }),
    [string]$Publisher = $(if ($env:MSIX_PUBLISHER) { $env:MSIX_PUBLISHER } else { 'CN=hellocsv-dev' }),
    [string]$PublisherDisplayName = $(if ($env:MSIX_PUBLISHER_DISPLAY) { $env:MSIX_PUBLISHER_DISPLAY } else { 'Hello CSV' }),
    [string]$DisplayName = $(if ($env:MSIX_DISPLAY_NAME) { $env:MSIX_DISPLAY_NAME } else { 'Hello CSV' }),
    [string]$Exe = "$PSScriptRoot\..\..\target\release\hellocsv.exe",
    [switch]$Sign
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path $Exe)) { throw "hellocsv.exe not found at $Exe - run cargo build --release" }
if ($Sign -and $Publisher -ne 'CN=hellocsv-dev') { Write-Warning 'Self-signing needs Publisher to equal the cert subject; use the default CN=hellocsv-dev for local tests.' }

$kit = Get-ChildItem "${env:ProgramFiles(x86)}\Windows Kits\10\bin\*\x64\makeappx.exe" | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $kit) { throw 'makeappx.exe not found - install the Windows 10/11 SDK.' }
$bin = $kit.DirectoryName

# MSIX versions are 4-part; the Store requires the last part to be 0
$cargoVer = (Select-String -Path "$PSScriptRoot\..\..\Cargo.toml" -Pattern '^version\s*=\s*"(\d+)\.(\d+)\.(\d+)').Matches[0].Groups
$version = "$($cargoVer[1].Value).$($cargoVer[2].Value).$($cargoVer[3].Value).0"

if (-not (Test-Path "$PSScriptRoot\Assets\StoreLogo.png")) { & "$PSScriptRoot\gen-icons.ps1" | Out-Null }

$stage = "$PSScriptRoot\stage"
$out = "$PSScriptRoot\out"
Remove-Item $stage -Recurse -Force -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force $stage, $out | Out-Null
Copy-Item $Exe "$stage\hellocsv.exe"
Copy-Item "$PSScriptRoot\Assets" "$stage\Assets" -Recurse

$m = Get-Content "$PSScriptRoot\AppxManifest.xml" -Raw
$map = @{ IDENTITY = $Identity; PUBLISHER = $Publisher; VERSION = $version; DISPLAYNAME = $DisplayName; PUBLISHERDISPLAY = $PublisherDisplayName }
foreach ($k in $map.Keys) { $m = $m.Replace("@$k@", [Security.SecurityElement]::Escape($map[$k])) }
[IO.File]::WriteAllText("$stage\AppxManifest.xml", $m, (New-Object Text.UTF8Encoding $false))

$pkg = "$out\hellocsv.msix"
& "$bin\makeappx.exe" pack /d $stage /p $pkg /o | Select-Object -Last 3
if ($LASTEXITCODE) { throw 'makeappx failed' }

if ($Sign) {
    $cert = Get-ChildItem Cert:\CurrentUser\My | Where-Object Subject -eq $Publisher | Select-Object -First 1
    if (-not $cert) {
        $cert = New-SelfSignedCertificate -Type Custom -Subject $Publisher -KeyUsage DigitalSignature -FriendlyName 'hellocsv dev' `
            -CertStoreLocation Cert:\CurrentUser\My -TextExtension @('2.5.29.37={text}1.3.6.1.5.5.7.3.3', '2.5.29.19={text}')
    }
    Export-Certificate -Cert $cert -FilePath "$out\hellocsv-dev.cer" | Out-Null
    & "$bin\signtool.exe" sign /fd SHA256 /sha1 $cert.Thumbprint $pkg | Select-Object -Last 2
    if ($LASTEXITCODE) { throw 'signtool failed' }
}
Remove-Item $stage -Recurse -Force
Write-Host "Built $pkg (version $version)"
