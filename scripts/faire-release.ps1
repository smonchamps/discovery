# faire-release.ps1 — assemble le latest.json d'une version signee (ADR 0013).
#
#   powershell scripts\faire-release.ps1 0.1.3
#
# Ne PUBLIE pas : il prepare le manifeste que la Release GitHub servira.
# La publication (attacher .exe, .sig, latest.json au tag) reste manuelle
# — web UI ou `gh release create` si le CLI est installe.
#
# Remplace l'ecriture a la main du latest.json, qui a paye deux pieges au
# terrain (validation ADR 0013) :
#   1. un BOM UTF-8 que l'updater (serde_json) refuse en silence ;
#   2. une URL pointant sur `v<version>` alors que le tag est la VERSION
#      NUE — d'ou un « 404 Not Found » au telechargement.

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"
$repo = "smonchamps/discovery"
$nsis = Join-Path $PSScriptRoot "..\target\release\bundle\nsis"
$exe = Join-Path $nsis "Wind_${Version}_x64-setup.exe"
$sig = "$exe.sig"

foreach ($f in @($exe, $sig)) {
    if (-not (Test-Path $f)) {
        throw "Introuvable : $f`nAs-tu lance `cargo tauri build` avec TAURI_SIGNING_PRIVATE_KEY defini ?"
    }
}

$signature = (Get-Content -Raw $sig).Trim()
if ([string]::IsNullOrWhiteSpace($signature)) {
    throw "Signature vide dans $sig — l'updater refuserait le paquet."
}

$manifest = [ordered]@{
    version   = $Version
    notes     = "Mise a jour signee (ADR 0013)"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-x86_64" = [ordered]@{
            signature = $signature
            # Tag = VERSION NUE, jamais `v$Version` : c'est le piege du 404.
            url       = "https://github.com/$repo/releases/download/$Version/Wind_${Version}_x64-setup.exe"
        }
    }
}

$out = Join-Path $nsis "latest.json"
# WriteAllText avec un encodeur sans BOM : Set-Content -Encoding utf8 en
# poserait un, et l'updater le refuse.
[System.IO.File]::WriteAllText($out, ($manifest | ConvertTo-Json -Depth 5), (New-Object System.Text.UTF8Encoding $false))

Write-Host "latest.json ecrit sans BOM : $out"
Write-Host ""
Write-Host "Publie la Release avec le tag « $Version » (PAS « v$Version ») et ces trois fichiers :"
Write-Host "  $exe"
Write-Host "  $sig"
Write-Host "  $out"
