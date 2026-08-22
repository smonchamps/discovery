# verifier-release.ps1 -- la verification STANDARD 2.10 d'une release
# publiee, scriptee (PLAN-RETOURS-8 : avec 5 assets et 2 plateformes,
# les controles manuels doublent -- la friction est encodee une fois).
#
#   powershell scripts\verifier-release.ps1 0.6.0
#
# Verifie, pour la version donnee : Release marquee Latest au tag NU ;
# les 5 assets nommes (2 exe, 2 sig, latest.json) ; latest.json sans
# BOM, version concordante, les DEUX cles de plateforme ; par
# plateforme : signature du manifeste == fichier .sig, URL au tag nu
# qui resout (200, Content-Length == taille de l'asset) ; signatures
# distinctes (garde anti-croisement). Ce que ce script NE PROUVE PAS
# (STANDARD 2.10) : la crypto minisign -- la preuve definitive reste
# l'auto-update <n-1> -> <n> constate au terrain, PAR CANAL (arm64 : ce
# poste ; x64 : le second poste, D5).

param([Parameter(Mandatory = $true)][string]$Version)

$ErrorActionPreference = "Stop"
$repo = "smonchamps/wind"
$echecs = 0
function Dire($ok, $texte) {
    if ($ok) { Write-Host "PASS  $texte" }
    else { Write-Host "ECHEC $texte"; $script:echecs += 1 }
}

# 1. La Release Latest est celle de la version, au tag NU.
$latest = gh api "repos/$repo/releases/latest" | ConvertFrom-Json
Dire ($latest.tag_name -eq $Version) "Latest au tag nu '$Version' (vu : '$($latest.tag_name)')"

# Les controles d'assets visent la release DE LA VERSION, jamais celle
# de Latest (revue 2026-08-22 : re-verifier une n-1 apres une n
# comparait les assets d'une autre release).
$release = $null
try {
    $release = gh api "repos/$repo/releases/tags/$Version" 2>$null | ConvertFrom-Json
}
catch { }
if ($null -eq $release -or $null -eq $release.assets) {
    Dire $false "release au tag '$Version' introuvable sur GitHub"
    Write-Host ""
    Write-Host "$echecs controle(s) en echec -- la release ne se declare PAS verifiee."
    exit 1
}

# 2. Les 5 assets, nommes exactement.
$attendus = @(
    "Wind_${Version}_arm64-setup.exe",
    "Wind_${Version}_arm64-setup.exe.sig",
    "Wind_${Version}_x64-setup.exe",
    "Wind_${Version}_x64-setup.exe.sig",
    "latest.json"
)
$noms = @($release.assets | ForEach-Object { $_.name })
Dire ($noms.Count -eq 5) "5 assets ($($noms.Count) vus)"
foreach ($n in $attendus) {
    Dire ($noms -contains $n) "asset '$n' present"
}

# 3. latest.json : sans BOM, version concordante, les deux plateformes.
$tmp = Join-Path ([System.IO.Path]::GetTempPath()) "wind-verif-$Version"
New-Item -ItemType Directory -Force $tmp | Out-Null
try {
    # Un telechargement qui echoue (asset manquant, reseau) est un
    # ECHEC de verdict, jamais une exception qui avale le rapport
    # (revue 2026-08-22) — c'est le scenario que ce script existe a
    # attraper.
    gh release download $Version --repo $repo --pattern "latest.json" --pattern "*.sig" --dir $tmp --clobber
    if ($LASTEXITCODE -ne 0 -or -not (Test-Path (Join-Path $tmp "latest.json"))) {
        Dire $false "telechargement des assets (latest.json + .sig) -- gh release download code $LASTEXITCODE"
        Write-Host ""
        Write-Host "$echecs controle(s) en echec -- la release ne se declare PAS verifiee."
        exit 1
    }

    $octets = [System.IO.File]::ReadAllBytes((Join-Path $tmp "latest.json"))
    $bom = ($octets.Length -ge 3 -and $octets[0] -eq 0xEF -and $octets[1] -eq 0xBB -and $octets[2] -eq 0xBF)
    Dire (-not $bom) "latest.json sans BOM ($($octets.Length) octets)"
    $manifest = [System.Text.Encoding]::UTF8.GetString($octets) | ConvertFrom-Json
    Dire ($manifest.version -eq $Version) "manifeste version '$($manifest.version)'"

    $plateformes = @(
        @{ cle = "windows-aarch64"; exe = "Wind_${Version}_arm64-setup.exe" },
        @{ cle = "windows-x86_64"; exe = "Wind_${Version}_x64-setup.exe" }
    )
    $signatures = @()
    foreach ($p in $plateformes) {
        $entree = $manifest.platforms.($p.cle)
        Dire ($null -ne $entree) "plateforme '$($p.cle)' presente au manifeste"
        if ($null -eq $entree) { continue }

        # Signature du manifeste == fichier .sig de la MEME architecture.
        $sigFichier = Join-Path $tmp "$($p.exe).sig"
        if (Test-Path $sigFichier) {
            $sig = (Get-Content -Raw $sigFichier).Trim()
            Dire ($entree.signature -eq $sig) "$($p.cle) : signature == $($p.exe).sig"
        }
        else {
            Dire $false "$($p.cle) : fichier $($p.exe).sig absent de la release"
        }
        $signatures += $entree.signature

        # URL au tag NU, nom de la bonne architecture.
        $urlAttendue = "https://github.com/$repo/releases/download/$Version/$($p.exe)"
        Dire ($entree.url -eq $urlAttendue) "$($p.cle) : URL au tag nu vers $($p.exe)"

        # L'URL resout (302 puis 200), Content-Length == taille de l'asset.
        $asset = $release.assets | Where-Object { $_.name -eq $p.exe }
        try {
            # -UseBasicParsing : PowerShell 5.1 passerait sinon par IE.
            # Headers['Content-Length'] est un tableau sous pwsh, une
            # chaine sous 5.1 -- les deux formes sont acceptees.
            $reponse = Invoke-WebRequest -Uri $entree.url -Method Head -MaximumRedirection 5 -UseBasicParsing
            $cl = $reponse.Headers['Content-Length']
            if ($cl -is [array]) { $cl = $cl[0] }
            $taille = [int64]$cl
            Dire ($reponse.StatusCode -eq 200 -and $taille -eq $asset.size) "$($p.cle) : l'exe resout 200 / $taille octets (asset : $($asset.size))"
        }
        catch {
            Dire $false "$($p.cle) : l'URL ne resout pas -- $($_.Exception.Message)"
        }
    }
    # Garde anti-croisement : deux canaux, deux signatures DISTINCTES.
    Dire ($signatures.Count -eq 2 -and $signatures[0] -ne $signatures[1]) "signatures arm64 et x64 distinctes"
}
finally {
    Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
if ($echecs -eq 0) {
    Write-Host "Verification 2.10 : tout passe. Reste la preuve terrain -- l'auto-update"
    Write-Host "constate PAR CANAL (arm64 : ce poste ; x64 : le second poste, D5)."
}
else {
    Write-Host "$echecs controle(s) en echec -- la release ne se declare PAS verifiee."
}
# Pas de ternaire : le script doit tourner sous Windows PowerShell 5.1.
if ($echecs -eq 0) { exit 0 } else { exit 1 }
