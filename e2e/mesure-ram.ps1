# Somme des working sets PRIVES : discovery-desktop.exe + ses processus
# WebView2 (identifies par leur ligne de commande dev.discovery.app).
$ids = @()
$ids += (Get-CimInstance Win32_Process -Filter "Name='discovery-desktop.exe'").ProcessId
$ids += (Get-CimInstance Win32_Process -Filter "Name='msedgewebview2.exe'" |
    Where-Object { $_.CommandLine -match 'discovery' }).ProcessId
$perf = Get-CimInstance Win32_PerfFormattedData_PerfProc_Process |
    Where-Object { $ids -contains $_.IDProcess }
$sum = ($perf | Measure-Object -Property WorkingSetPrivate -Sum).Sum
"{0:N1} Mo (working set prive, {1} processus)" -f ($sum / 1MB), @($ids).Count
