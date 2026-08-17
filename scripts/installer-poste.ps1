#Requires -Version 5.1
<#
  installer-poste.ps1 -- prepare un poste Windows pour developper Wind.

  Idempotent : re-jouable sans casse (chaque outil est verifie avant pose).
  Ne touche a AUCUN secret : jetons OAuth et cle de signature restent a la
  main de l'utilisateur (rappeles en fin de script).

  Ecrit SANS ACCENTS -- convention des .ps1 du depot (cf. faire-release.ps1),
  qui evite le piege d'encodage de PowerShell 5.1 sur les sources UTF-8.

  Sur une machine neuve (rien d'installe) :
     winget install -e --id Git.Git
     git clone https://github.com/smonchamps/wind.git C:\dev\wind
     cd C:\dev\wind
     powershell -ExecutionPolicy Bypass -File scripts\installer-poste.ps1

  (Cloner HORS OneDrive : il perturbe les mesures et fait flaker les e2e
   -- PASSATION section 7.3.)

  Options :
     -SkipBuildTools    ne touche pas a Visual Studio Build Tools
     -AvecCargoAudit    installe aussi cargo-audit (joue par la CI, pas le gate local)
     -CrossArm64Check   compile wind-desktop pour arm64 (preuve du cross-build, ~minutes)

  Ce poste est x64. On installe EN PLUS la cible arm64 (cross-compilation)
  pour pouvoir produire les deux canaux de release. La release bi-arch
  elle-meme (faire-release.ps1 + latest.json) est un chantier a part.
#>
param(
    [switch]$SkipBuildTools,
    [switch]$AvecCargoAudit,
    [switch]$CrossArm64Check
)

$ErrorActionPreference = "Stop"

function Section($t) { Write-Host ""; Write-Host "=== $t ===" -ForegroundColor Cyan }
function Info($t) { Write-Host "  $t" }
function Ok($t) { Write-Host "  OK   $t" -ForegroundColor Green }
function Warn($t) { Write-Host "  !!   $t" -ForegroundColor Yellow }

function Update-SessionPath {
    # winget/rustup ecrivent le PATH persistant ; on le recharge dans CETTE
    # session pour que node/cargo/git servent sans rouvrir le shell.
    $m = [Environment]::GetEnvironmentVariable("Path", "Machine")
    $u = [Environment]::GetEnvironmentVariable("Path", "User")
    $env:Path = ($m, $u -join ";")
    $cargoBin = Join-Path $env:USERPROFILE ".cargo\bin"
    if ((Test-Path $cargoBin) -and ($env:Path -notmatch [regex]::Escape($cargoBin))) {
        $env:Path += ";$cargoBin"
    }
}

function Winget-Present($id) {
    winget list --id $id -e *> $null
    return ($LASTEXITCODE -eq 0)
}

function Ensure-Winget($id, $override) {
    if (Winget-Present $id) { Ok "$id deja installe"; return }
    Info "Installation de $id ..."
    $a = @("install", "-e", "--id", $id, "--accept-source-agreements", "--accept-package-agreements")
    if ($override) { $a += @("--override", $override) }
    winget @a
    if ($LASTEXITCODE -ne 0) { throw "winget a echoue pour $id (code $LASTEXITCODE)" }
    Ok "$id installe"
}

function Npm-Bootstrap($dir) {
    Push-Location $dir
    try {
        if (Test-Path "package-lock.json") { npm ci } else { npm install }
        if ($LASTEXITCODE -ne 0) { throw "npm a echoue dans $dir (code $LASTEXITCODE)" }
    }
    finally { Pop-Location }
}

# --------------------------------------------------------------------------

Section "Prerequis"
if (-not (Get-Command winget -ErrorAction SilentlyContinue)) {
    throw "winget introuvable. Installe 'App Installer' depuis le Microsoft Store, puis relance."
}
Ok "winget present"

Section "Outils systeme (winget)"
Ensure-Winget "Git.Git"
Ensure-Winget "Rustlang.Rustup"
Ensure-Winget "Microsoft.EdgeWebView2Runtime"   # webview de l'app + fenetre pilotee par les e2e
Ensure-Winget "OpenJS.NodeJS.LTS"               # CI epingle Node 24 (LTS)
Ensure-Winget "Python.Python.3.13"              # e2e/sonde-gel.py
Ensure-Winget "GitHub.cli"                      # gh run list / gh release

Section "Visual Studio Build Tools (C++ x64 + cross arm64)"
if ($SkipBuildTools) {
    Warn "Ignore (-SkipBuildTools)."
}
elseif (Winget-Present "Microsoft.VisualStudio.2022.BuildTools") {
    Warn "Build Tools deja present : je ne modifie pas une install existante."
    Warn "Pour LINKER l'arm64, ajoute le composant 'MSVC v143 - VS 2022 C++ ARM64"
    Warn "build tools' via l'app 'Visual Studio Installer' (Modifier > Composants"
    Warn "individuels). Sans lui, la cible arm64 se telecharge mais ne LIE pas."
}
else {
    # VCTools = compilateur MSVC x64/x86 + (avec includeRecommended) Windows SDK.
    # VC.Tools.ARM64 = cross-compilateur arm64, requis pour le second canal.
    $ovr = "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools " +
    "--add Microsoft.VisualStudio.Component.VC.Tools.ARM64 --includeRecommended"
    Ensure-Winget "Microsoft.VisualStudio.2022.BuildTools" $ovr
}

Update-SessionPath

Section "Rust (rustup)"
if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    throw "rustup injoignable apres installation. Rouvre PowerShell et relance le script (idempotent)."
}
# La VERSION est epinglee dans rust-toolchain.toml (1.97.1) : rustup bascule
# tout seul dessus au premier cargo dans le depot. On garantit juste un
# defaut stable pour les commandes hors depot.
rustup default stable | Out-Null
Ok "rustup: defaut stable (le depot forcera 1.97.1 via rust-toolchain.toml)"

Section "Outils cargo"
$cargoList = (cargo install --list) 2>$null
if ($cargoList -match "tauri-cli") {
    Ok "tauri-cli deja installe"
}
else {
    Info "Installation de tauri-cli (compile depuis les sources, plusieurs minutes) ..."
    cargo install tauri-cli --version "^2.0" --locked
    Ok "tauri-cli installe (cargo tauri build)"
}
if ($AvecCargoAudit) {
    if ($cargoList -match "cargo-audit") { Ok "cargo-audit deja installe" }
    else { cargo install cargo-audit --locked; Ok "cargo-audit installe" }
}

Section "Depot Wind"
$root = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $root
if (-not (Test-Path (Join-Path $root "Cargo.toml"))) {
    throw "Ce script doit vivre dans scripts\ du depot Wind (Cargo.toml introuvable a la racine)."
}
Ok "Racine du depot : $root"

# Gate pre-push : un clone neuf ne l'active pas tout seul.
git config core.hooksPath .githooks
Ok "core.hooksPath = .githooks (gate pre-push arme)"

# Identite git locale : JAMAIS devinee (donnee personnelle). On avertit si absente.
$mail = ((git config --local user.email) 2>$null)
if ([string]::IsNullOrWhiteSpace($mail)) {
    Warn "Identite git non definie sur ce clone. Reprends-la de la machine actuelle :"
    Warn "  git config user.name  ""smonchamps"""
    Warn "  git config user.email ""<ton-adresse-noreply@users.noreply.github.com>"""
}
else {
    Ok "Identite git locale : $mail"
}

Section "Dependances JS + premiere compilation"
Info "ui-v2 : npm ci + build (DOIT preceder cargo -- generate_context! exige ui-v2/dist)"
Npm-Bootstrap (Join-Path $root "apps\desktop\ui-v2")
Push-Location (Join-Path $root "apps\desktop\ui-v2")
try { npm run build; if ($LASTEXITCODE -ne 0) { throw "build ui-v2 echoue" } } finally { Pop-Location }
Ok "ui-v2 construit"

Info "e2e : npm ci"
Npm-Bootstrap (Join-Path $root "e2e")
Ok "e2e pret"

Info "cargo build --workspace (materialise la toolchain 1.97.1 du depot)"
cargo build --workspace
if ($LASTEXITCODE -ne 0) { throw "cargo build a echoue" }
Ok "workspace compile (x64 natif)"

Section "Cible arm64 (cross-build depuis ce poste x64)"
# Ajoutee A la toolchain du depot : on est dans $root, l'override 1.97.1 est actif.
if (((rustup target list --installed) -join "`n") -match "aarch64-pc-windows-msvc") {
    Ok "aarch64-pc-windows-msvc deja presente"
}
else {
    rustup target add aarch64-pc-windows-msvc
    Ok "cible arm64 ajoutee a la toolchain du depot"
}
if ($CrossArm64Check) {
    Info "Preuve du cross-build arm64 (cargo build -p wind-desktop --target aarch64-pc-windows-msvc) ..."
    cargo build -p wind-desktop --target aarch64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw "cross-build arm64 echoue (composant C++ ARM64 manquant ?)" }
    Ok "cross-build arm64 OK"
}
else {
    Info "Preuve du cross-build non jouee (option -CrossArm64Check)."
    Info "Le LIEN arm64 exige le composant VS 'MSVC v143 C++ ARM64 build tools'."
}

Section "Verification"
Update-SessionPath
function Show($label, $cmd) {
    try {
        $v = (& ([scriptblock]::Create($cmd)) 2>$null | Select-Object -First 1)
        Ok ("{0,-8} {1}" -f $label, $v)
    }
    catch { Warn "$label introuvable" }
}
Show "git" "git --version"
Show "rustc" "rustc --version"
Show "cargo" "cargo --version"
Show "node" "node --version"
Show "npm" "npm --version"
Show "python" "python --version"
Show "gh" "gh --version"
Show "tauri" "cargo tauri --version"

$node = ((node --version) 2>$null)
if ($node -notmatch "^v24\.") {
    Warn "Node = $node (la CI epingle v24). Si un parcours e2e diverge, installe le 24."
}

Section "Termine"
Write-Host "Poste pret pour le cycle cargo + e2e." -ForegroundColor Green
Write-Host ""
Write-Host "A ta main (le script ne touche aucun secret) :"
Write-Host "  1. Secrets OAuth au niveau utilisateur -- valeurs a reprendre de la machine actuelle :"
Write-Host "       setx GOOGLE_CLIENT_ID     ..."
Write-Host "       setx GOOGLE_CLIENT_SECRET ..."
Write-Host "       setx MICROSOFT_CLIENT_ID  ..."
Write-Host "  2. Rejouer le gate complet pour valider la toolchain :"
Write-Host "       cargo fmt --all -- --check"
Write-Host "       cargo clippy --workspace --all-targets -- -D warnings"
Write-Host "       cargo test --workspace --all-targets ; cargo test --workspace --doc"
Write-Host "       cd e2e ; npm test ; cd .."
Write-Host ""
Write-Host "Release bi-arch (x64 + arm64) : CHANTIER A PART. faire-release.ps1 et le"
Write-Host "manifeste latest.json ne portent aujourd'hui que windows-aarch64 (ADR 0013)."
