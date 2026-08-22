# faire-release.ps1 — build signe BI-ARCH + latest.json d'une version
# (ADR 0013, bi-arch depuis PLAN-RETOURS-8).
#
#   powershell scripts\faire-release.ps1 0.6.0
#
# Fait TOUTE la release, dans l'ordre : (1) verifie version + entree
# CHANGELOG, BUMPE la seule ligne de tauri.conf.json ; (2) DEUX builds
# signes — arm64 natif puis x64 en cross-build (D6), cle definie plus
# bas, mot de passe demande par Tauri A CHAQUE build (deux fois —
# assume : il ne passe JAMAIS par une variable, l'environnement d'un
# build est herite par tous ses processus enfants, revue 2026-08-22) ;
# TOUT-OU-RIEN (D7) : un build en echec arrete tout, rien n'est
# publie ; (3) manifeste latest.json a
# DEUX cles de plateforme (un seul manifeste sert les deux canaux :
# l'updater lit la cle {os}-{arch} de SON binaire) ; (4) APRES
# CONFIRMATION explicite (l'auto-update Latest est irreversible) :
# commit de release, push (la gate pre-push rejoue), tag VERSION NUE +
# Release GitHub avec les 5 assets, marquee Latest, notes du CHANGELOG.
#
# Pieges payes et encodes ici (jamais laisses a la vigilance) :
#   1. un BOM UTF-8 que l'updater (serde_json) refuse en silence ;
#   2. une URL pointant sur `v<version>` alors que le tag est la VERSION
#      NUE — d'ou un « 404 Not Found » au telechargement ;
#   3. (bi-arch) une cle de plateforme manquante ou des signatures
#      CROISEES ne produisent AUCUNE erreur — l'updater du canal muet
#      conclut « pas de mise a jour ». Le manifeste est donc construit
#      par plateforme depuis le repertoire de SA cible, et les deux
#      signatures sont exigees distinctes.

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"

# Cle de signature de l'updater (ADR 0013) : le fichier vit hors du depot,
# a C:\Keys\wind.key. `cargo tauri build` la lit dans cette variable.
$env:TAURI_SIGNING_PRIVATE_KEY = "C:\Keys\wind.key"
$repo = "smonchamps/wind"

# Les DEUX canaux (D5/D6, PLAN-RETOURS-8) : arm64 natif (le poste), x64
# en cross-build local (toolset MSVC x64 + cible rustup, prouve a l'E1).
# Avec `--target`, Tauri ecrit sous target/<triple>/release/bundle/nsis —
# le chemin sans triple n'existe plus dans ce script.
$cibles = @(
    [ordered]@{
        triple    = "aarch64-pc-windows-msvc"
        plateforme = "windows-aarch64"
        exeNom    = "Wind_${Version}_arm64-setup.exe"
    },
    [ordered]@{
        triple    = "x86_64-pc-windows-msvc"
        plateforme = "windows-x86_64"
        exeNom    = "Wind_${Version}_x64-setup.exe"
    }
)
foreach ($c in $cibles) {
    $c.nsis = Join-Path $PSScriptRoot "..\target\$($c.triple)\release\bundle\nsis"
    $c.exe = Join-Path $c.nsis $c.exeNom
    $c.sig = "$($c.exe).sig"
}

# (1) Preparation, AVANT les longs builds (echec franc et rapide) : version
# bien formee, notes utilisateur ecrites, puis bump de tauri.conf.json.
if ($Version -notmatch '^\d+\.\d+\.\d+$') {
    throw "Version « $Version » invalide — attendu MAJEUR.MINEUR.CORRECTIF (ex. 0.1.10), sans « v »."
}
# La publication finale passe par gh : le refuser tot, pas apres 8 min de build.
if (-not (Get-Command gh -ErrorAction SilentlyContinue)) {
    throw "gh (GitHub CLI) introuvable — installe-le (winget install GitHub.cli) et « gh auth login », ou publie la Release a la main."
}
# La cible x64 doit etre installee (rustup) : echec franc avant le build.
$targets = rustup target list --installed
if ($targets -notcontains "x86_64-pc-windows-msvc") {
    throw "Cible rustup x86_64-pc-windows-msvc absente — « rustup target add x86_64-pc-windows-msvc » (PLAN-RETOURS-8 E1)."
}
$changelog = Join-Path $PSScriptRoot "..\CHANGELOG.md"
if ((Get-Content -Raw -Encoding UTF8 $changelog) -notmatch [regex]::Escape("## [$Version]")) {
    throw "CHANGELOG.md n'a pas d'entree « ## [$Version] » — ecris d'abord les notes utilisateur."
}
# Bump de la SEULE ligne de version (regex ciblee : le reste du fichier, sa
# mise en forme et l'ordre des cles, ne bouge pas ; jamais de BOM que
# l'updater refuse). On exige exactement une cle « version ».
$conf = Join-Path $PSScriptRoot "..\apps\desktop\tauri.conf.json"
$json = Get-Content -Raw -Encoding UTF8 $conf
$pattern = '("version"\s*:\s*")[^"]*(")'
if (([regex]::Matches($json, $pattern)).Count -ne 1) {
    throw "tauri.conf.json : cle « version » introuvable ou multiple — bump automatique refuse, a faire a la main."
}
$json = [regex]::Replace($json, $pattern, "`${1}$Version`${2}")
[System.IO.File]::WriteAllText($conf, $json, (New-Object System.Text.UTF8Encoding $false))
Write-Host "tauri.conf.json bumpe a $Version."

# (2) Les DEUX builds signes, arm64 puis x64. TOUT-OU-RIEN (D7) : le
# premier echec jette, rien n'est publie, jamais un canal decale. Le
# MOT DE PASSE n'est PAS pose en variable, volontairement (invariant
# ADR 0013 conserve) : Tauri le demande a la main a CHAQUE build —
# deux saisies, le prix pour qu'il n'apparaisse jamais dans un
# environnement herite par les processus enfants du build.
$desktop = Join-Path $PSScriptRoot "..\apps\desktop"
Push-Location $desktop
try {
    foreach ($c in $cibles) {
        Write-Host ""
        Write-Host "=== Build $($c.triple) ==="
        cargo tauri build --target $c.triple
        if ($LASTEXITCODE -ne 0) {
            throw "cargo tauri build --target $($c.triple) a echoue (code $LASTEXITCODE) — release interrompue, RIEN n'est publie (D7)."
        }
    }
}
finally {
    Pop-Location
}

# Controle de presence PAR CANAL : les deux exe et les deux signatures.
foreach ($c in $cibles) {
    foreach ($f in @($c.exe, $c.sig)) {
        if (-not (Test-Path $f)) {
            throw "Introuvable apres le build : $f`nLa version « $Version » colle-t-elle a celle de tauri.conf.json ?"
        }
    }
    $c.signature = (Get-Content -Raw $c.sig).Trim()
    if ([string]::IsNullOrWhiteSpace($c.signature)) {
        throw "Signature vide dans $($c.sig) — l'updater refuserait le paquet."
    }
}
# Garde anti-croisement (piege 3) : deux signatures identiques signent une
# copie accidentelle — un canal servirait le binaire de l'autre. Deux a
# deux distinctes, quel que soit le nombre de cibles (revue 2026-08-22 :
# jamais d'indexation en dur qui raterait une 3e cible).
$signaturesUniques = @($cibles | ForEach-Object { $_.signature } | Select-Object -Unique)
if ($signaturesUniques.Count -ne $cibles.Count) {
    throw "Des signatures de cibles differentes sont IDENTIQUES — croisement ou copie accidentelle, release interrompue."
}

# (3) Manifeste latest.json (sans BOM, URL au tag NU, UNE cle par canal —
# chaque entree est construite depuis le repertoire de SA cible).
$platforms = [ordered]@{}
foreach ($c in $cibles) {
    $platforms[$c.plateforme] = [ordered]@{
        signature = $c.signature
        # Tag = VERSION NUE, jamais `v$Version` : c'est le piege du 404.
        url       = "https://github.com/$repo/releases/download/$Version/$($c.exeNom)"
    }
}
$manifest = [ordered]@{
    version   = $Version
    notes     = "Mise a jour signee (ADR 0013)"
    pub_date  = (Get-Date).ToUniversalTime().ToString("yyyy-MM-ddTHH:mm:ssZ")
    platforms = $platforms
}

$out = Join-Path $cibles[0].nsis "latest.json"
# WriteAllText avec un encodeur sans BOM : Set-Content -Encoding utf8 en
# poserait un, et l'updater le refuse.
[System.IO.File]::WriteAllText($out, ($manifest | ConvertTo-Json -Depth 5), (New-Object System.Text.UTF8Encoding $false))

Write-Host "latest.json ecrit sans BOM ($($cibles.Count) plateformes) : $out"

# (4) Publication. SORTANT et irreversible : une fois la Release marquee
# Latest avec son latest.json, les apps installees s'auto-updatent. D'ou la
# confirmation explicite (decision CE), APRES les builds — jamais avant.
Write-Host ""
Write-Host "Pret a publier $Version : commit de release + push (gate) + tag NU + Release GitHub Latest (5 assets : 2 exe, 2 sig, latest.json)."
$reponse = Read-Host "Publier maintenant ? Tape OUI en majuscules pour continuer"
if ($reponse -cne "OUI") {
    Write-Host "Publication ANNULEE. Les artefacts restent prets ; relance ou publie a la main."
    return
}

$racine = Join-Path $PSScriptRoot ".."
Push-Location $racine
try {
    # Commit de release : les fichiers du bump seulement (jamais `git add -A`
    # qui emporterait du travail voisin). Message SANS accents (STANDARD §2.8).
    git add apps/desktop/tauri.conf.json CHANGELOG.md scripts/faire-release.ps1
    git commit -m "release: version $Version" -m "Bump tauri.conf.json et entree CHANGELOG ; builds signes arm64 + x64 et Release publiee par faire-release.ps1 (ADR 0013, bi-arch PLAN-RETOURS-8)."
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
# -Encoding UTF8 IMPERATIF : sans lui, Windows PowerShell 5.1 (invoque par
# `powershell scripts\faire-release.ps1`) lit le CHANGELOG UTF-8 en cp1252,
# puis WriteAllText le ré-encode en UTF-8 — double encodage, les accents
# partent en mojibake (« Ã© » pour « é ») dans les notes de la Release.
# (Constat terrain 2026-08-22 : 0.1.10 a 0.6.0 reparees a la main.)
$clText = Get-Content -Raw -Encoding UTF8 $changelog
$rxSection = "(?sm)^## \[" + [regex]::Escape($Version) + "\].*?(?=^## \[|\z)"
$section = [regex]::Match($clText, $rxSection)
$notes = if ($section.Success) { $section.Value.Trim() } else { "Mise a jour signee (ADR 0013)." }
$notesFile = [System.IO.Path]::GetTempFileName()
[System.IO.File]::WriteAllText($notesFile, $notes, (New-Object System.Text.UTF8Encoding $false))

# Release GitHub : tag = VERSION NUE (jamais v$Version, piege du 404), les
# assets DERIVES de $cibles (revue 2026-08-22 : une cible ajoutee au tableau
# est publiee par construction, jamais oubliee), marquee Latest, ancree sur
# le commit de release qu'on vient de pousser.
$assets = @($cibles | ForEach-Object { $_.exe; $_.sig }) + $out
try {
    gh release create $Version @assets --title $Version --notes-file $notesFile --latest --target $sha
    if ($LASTEXITCODE -ne 0) { throw "gh release create a echoue (code $LASTEXITCODE)." }
}
finally {
    Remove-Item $notesFile -ErrorAction SilentlyContinue
}

Write-Host ""
Write-Host "Release $Version publiee et marquee Latest."
Write-Host "Verifie-la : powershell scripts\verifier-release.ps1 $Version (STANDARD 2.10,"
Write-Host "les DEUX plateformes). Puis confirme l'AUTO-UPDATE sur l'app installee"
Write-Host "(arm64 : ce poste ; x64 : le second poste, decision D5) — seule preuve"
Write-Host "vivante de la signature (ADR 0013)."
