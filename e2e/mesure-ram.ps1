# Somme des working sets PRIVES : wind-desktop.exe + ses processus
# WebView2 (identifies par leur ligne de commande dev.elements.wind).
#
# -AppPid et -Profil RESTREIGNENT la mesure a UNE instance. Sans eux, le
# script sommait toutes les instances de la machine : une fenetre ouverte
# par l'utilisateur pendant la mesure s'ajoutait a celle du banc, et le
# total ne voulait plus rien dire. Constate au gate 3 -- 14 processus au
# lieu de 7, 202 Mo pour deux applications.
param(
    [int] $AppPid = 0,
    [string] $Profil = ''
)

$ids = @()
if ($AppPid -gt 0) {
    $ids += $AppPid
} else {
    $ids += (Get-CimInstance Win32_Process -Filter "Name='wind-desktop.exe'").ProcessId
}

$webview = Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
    Where-Object { $_.CommandLine -match 'dev\.elements\.wind' }
if ($Profil -ne '') {
    # Le dossier de donnees utilisateur identifie l'instance de facon
    # sure : WebView2 le passe a chacun de ses processus enfants.
    $motif = [regex]::Escape($Profil)
    $webview = $webview | Where-Object { $_.CommandLine -match $motif }
}
$ids += $webview.ProcessId

$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains $_.IDProcess }
$sum = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum
"{0:N1} Mo (working set prive, {1} processus)" -f ($sum / 1MB), @($ids).Count
