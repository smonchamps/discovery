# Bascule REELLE du theme sombre Windows : registre + diffusion
# WM_SETTINGCHANGE « ImmersiveColorSet » — ce que font les Parametres.
# Set-ItemProperty seul ne previent personne (mesure aux sondes du
# constat terrain A42, 2026-08-16) : sans la diffusion, ni l'API Tauri
# ni aucune application ne voit la bascule. Utilise par le test
# « suivi OS » de refonte-ecran02.spec.js, Windows seulement.
param([Parameter(Mandatory)][int]$v)

Set-ItemProperty HKCU:\SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize -Name AppsUseLightTheme -Value $v -Type DWord
Add-Type -Namespace Wind -Name Diffuse -MemberDefinition '[DllImport("user32.dll", CharSet = CharSet.Auto)] public static extern IntPtr SendMessageTimeout(IntPtr hWnd, uint Msg, UIntPtr wParam, string lParam, uint fuFlags, uint uTimeout, out UIntPtr lpdwResult);'
[UIntPtr]$r = [UIntPtr]::Zero
# 0xffff = HWND_BROADCAST, 0x001A = WM_SETTINGCHANGE, 2 = SMTO_ABORTIFHUNG
[Wind.Diffuse]::SendMessageTimeout([IntPtr]0xffff, 0x001A, [UIntPtr]::Zero, "ImmersiveColorSet", 2, 5000, [ref]$r) | Out-Null
