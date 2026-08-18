# faire-release.ps1 — build signe + latest.json d'une version (ADR 0013).
#
#   powershell scripts\faire-release.ps1 0.1.10
#
# Fait TOUTE la release, dans l'ordre : (1) verifie version + entree
# CHANGELOG, BUMPE la seule ligne de tauri.conf.json ; (2) `cargo tauri
# build` signe (cle definie plus bas, mot de passe demande a la main) ;
# (3) manifeste latest.json ; (4) APRES CONFIRMATION explicite (l'auto-
# update Latest est irreversible) : commit de release, push (la gate
# pre-push rejoue), tag VERSION NUE + Release GitHub avec les 3 assets,
# marquee Latest, notes tirees du CHANGELOG.
#
# Remplace l'ecriture a la main du latest.json, qui a paye deux pieges au
# terrain (validation ADR 0013) :
#   1. un BOM UTF-8 que l'updater (serde_json) refuse en silence ;
#   2. une URL pointant sur `v<version>` alors que le tag est la VERSION
#      NUE — d'ou un « 404 Not Found » au telechargement.

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"

# Cle de signature de l'updater (ADR 0013) : le fichier vit hors du depot,
# a C:\Keys\wind.key. `cargo tauri build` la lit dans cette variable.
# Le MOT DE PASSE n'est PAS pose ici, volontairement : Tauri le demande a
# la main a chaque build (TAURI_SIGNING_PRIVATE_KEY_PASSWORD laisse vide).
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\Keys\wind.key"
$repo = "smonchamps/wind"
$nsis = Join-Path $PSScriptRoot "..\target\release\bundle\nsis"
# arm64 natif depuis la 0.1.3 (PLAN-WIND E4) : le seul poste utilisateur
# est ARM64 et faisait tourner le canal x64 en emulation ; le canal x64
# reviendra quand la beta amenera des postes x64.
$exe = Join-Path $nsis "Wind_${Version}_arm64-setup.exe"
$sig = "$exe.sig"

# (1) Preparation, AVANT le long build (echec franc et rapide) : version bien
# formee, notes utilisateur ecrites, puis bump de tauri.conf.json.
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version « $Version » invalide — attendu MAJEUR.MINEUR.CORRECTIF (ex. 0.1.10), sans « v »."
}
# La publication finale passe par gh : le refuser tot, pas apres 4 min de build.
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh (GitHub CLI) introuvable — installe-le (winget install GitHub.cli) et « gh auth login », ou publie la Release a la main."
}
$changelog = Join-Path $PSScriptRoot "..\CHANGELOG.md"
if ((Get-Content -Raw $changelog) -notmatch [regex]::Escape("## [$Version]")) {
    throw "CHANGELOG.md n'a pas d'entree « ## [$Version] » — ecris d'abord les notes utilisateur."
}
# Bump de la SEULE ligne de version (regex ciblee : le reste du fichier, sa
# mise en forme et l'ordre des cles, ne bouge pas ; jamais de BOM que
# l'updater refuse). On exige exactement une cle « version ».
$conf = Join-Path $PSScriptRoot "..\apps\desktop\tauri.conf.json"
$json = Get-Content -Raw $conf
$pattern = '("version"\s*:\s*")[^"]*(")'
if (([regex]::Matches($json, $pattern)).Count -ne 1) {
    throw "tauri.conf.json : cle « version » introuvable ou multiple — bump automatique refuse, a faire a la main."
}
$json = [regex]::Replace($json, $pattern, "`${1}$Version`${2}")
[System.IO.File]::WriteAllText($conf, $json, (New-Object System.Text.UTF8Encoding $false))
Write-Host "tauri.conf.json bumpe a $Version."

# (2) Build signe. `cargo tauri build` lit la cle dans la variable posee en
# tete ; sans mot de passe en variable, Tauri le demande a la main ici.
# Depuis apps/desktop (ou vit tauri.conf.json) ; Pop-Location garanti.
$desktop = Join-Path $PSScriptRoot "..\apps\desktop"
Push-Location $desktop
try {
    cargo tauri build
    if ($LASTEXITCODE -ne 0) {
        throw "cargo tauri build a echoue (code $LASTEXITCODE) — release interrompue."
    }
}
finally {
    Pop-Location
}

# Controle de presence : le build vient de les produire. S'ils manquent, le
# nom du bundle a change, ou la version passee ne colle pas a tauri.conf.json.
foreach ($f in @($exe, $sig)) {
    if (-not (Test-Path $f)) {
        throw "Introuvable apres le build : $f`nLa version « $Version » colle-t-elle a celle de tauri.conf.json ?"
    }
}

$signature = (Get-Content -Raw $sig).Trim()
if ([string]::IsNullOrWhiteSpace($signature)) {
    throw "Signature vide dans $sig — l'updater refuserait le paquet."
}

# (3) Manifeste latest.json (sans BOM, URL au tag NU).
$manifest = [ordered]@{
    version   = $Version
    notes     = "Mise a jour signee (ADR 0013)"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = [ordered]@{
        "windows-aarch64" = [ordered]@{
            signature = $signature
            # Tag = VERSION NUE, jamais `v$Version` : c'est le piege du 404.
            url       = "https://github.com/$repo/releases/download/$Version/Wind_${Version}_arm64-setup.exe"
        }
    }
}

$out = Join-Path $nsis "latest.json"
# WriteAllText avec un encodeur sans BOM : Set-Content -Encoding utf8 en
# poserait un, et l'updater le refuse.
[System.IO.File]::WriteAllText($out, ($manifest | ConvertTo-Json -Depth 5), (New-Object System.Text.UTF8Encoding $false))

Write-Host "latest.json ecrit sans BOM : $out"

# (4) Publication. SORTANT et irreversible : une fois la Release marquee
# Latest avec son latest.json, les apps installees s'auto-updatent. D'ou la
# confirmation explicite (decision CE), APRES le build — jamais avant.
Write-Host ""
Write-Host "Pret a publier $Version : commit de release + push (gate) + tag NU + Release GitHub Latest."
$reponse = Read-Host "Publier maintenant ? Tape OUI en majuscules pour continuer"
if ($reponse -cne "OUI") {
    Write-Host "Publication ANNULEE. Les artefacts restent prets ; relance ou publie a la main."
    return
}

$racine = Join-Path $PSScriptRoot ".."
Push-Location $racine
try {
    # Commit de release : les fichiers du bump seulement (jamais `git add -A`
    # qui emporterait du travail voisin). Message SANS accents (PASSATION §2.8).
    git add apps/desktop/tauri.conf.json CHANGELOG.md scripts/faire-release.ps1
    git commit -m "release: version $Version" -m "Bump tauri.conf.json et entree CHANGELOG ; build signe + Release publiee par faire-release.ps1 (ADR 0013)."
    if ($LASTEXITCODE -ne 0) { throw "git commit a echoue (code $LASTEXITCODE)." }
    # Push : le hook pre-push rejoue la gate complete. Un rouge (parfois un
    # flake e2e local) l'arrete ici — le commit reste local, rejouable.
    git push
    if ($LASTEXITCODE -ne 0) { throw "git push a echoue (code $LASTEXITCODE) — gate pre-push rouge ? Le commit reste local." }
    $sha = (git rev-parse HEAD).Trim()
}
finally {
    Pop-Location
}

# Notes de release = la section CHANGELOG de la version (accents OK ici : ce
# n'est pas un message de commit). Repli sobre si l'extraction rate.
$clText = Get-Content -Raw $changelog
$rxSection = "(?sm)^## \[" + [regex]::Escape($Version) + "\].*?(?=^## \[|\z)"
$section = [regex]::Match($clText, $rxSection)
$notes = if ($section.Success) { $section.Value.Trim() } else { "Mise a jour signee (ADR 0013)." }
$notesFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($notesFile, $notes, (New-Object System.Text.UTF8Encoding $false))

# Release GitHub : tag = VERSION NUE (jamais v$Version, piege du 404), les 3
# assets, marquee Latest, ancree sur le commit de release qu'on vient de pousser.
try {
    gh release create $Version $exe $sig $out --title $Version --notes-file $notesFile --latest --target $sha
    if ($LASTEXITCODE -ne 0) { throw "gh release create a echoue (code $LASTEXITCODE)." }
}
finally {
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Release $Version publiee et marquee Latest."
Write-Host "Verifie sur GitHub : la Release est « Latest », les 3 fichiers attaches,"
Write-Host "latest.json a l'URL du tag NU. Puis confirme l'AUTO-UPDATE sur l'app"
Write-Host "installee — seule preuve vivante de la signature (ADR 0013)."
