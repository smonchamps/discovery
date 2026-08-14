# Régénère apps/desktop/icons/icon.ico depuis la géométrie de la marque
# (Système, section « Marque » ; assets/marque/wind-tuile.svg ;
# docs/PLAN-WIND.md E2). GDI+ trace les mêmes formes que le SVG — le
# « W » en traits ronds, jamais en fonte.
#
# Tailles : 256 et 48 avec la pastille « W » ; 32 et 16 sans (en
# dessous de 48 px la pastille est une bouillie). Aux petites tailles
# l'enveloppe s'élargit et le trait plancher évite le sous-pixel.
#
#   pwsh scripts/faire-icone.ps1                  # écrit icon.ico
#   pwsh scripts/faire-icone.ps1 -Apercu dossier  # + PNG de contrôle

param([string]$Apercu = "")

Add-Type -AssemblyName System.Drawing

$fondTuile = [System.Drawing.Color]::FromArgb(0xFF, 0xE2, 0xEB, 0xE8)
$vertWind  = [System.Drawing.Color]::FromArgb(0xFF, 0x36, 0x5A, 0x4F)

function New-CheminArrondi([single]$x, [single]$y, [single]$w, [single]$h, [single]$r) {
    $p = New-Object System.Drawing.Drawing2D.GraphicsPath
    $d = 2 * $r
    $p.AddArc($x, $y, $d, $d, 180, 90)
    $p.AddArc($x + $w - $d, $y, $d, $d, 270, 90)
    $p.AddArc($x + $w - $d, $y + $h - $d, $d, $d, 0, 90)
    $p.AddArc($x, $y + $h - $d, $d, $d, 90, 90)
    $p.CloseFigure()
    return $p
}

function New-Rendu([int]$taille, [bool]$pastille, [single]$fractionGlyphe, [single]$traitPlancher) {
    $bmp = New-Object System.Drawing.Bitmap($taille, $taille, [System.Drawing.Imaging.PixelFormat]::Format32bppArgb)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.SmoothingMode = 'AntiAlias'
    $g.PixelOffsetMode = 'HighQuality'
    $g.Clear([System.Drawing.Color]::Transparent)

    # Avec pastille, le canevas porte les 70 unités de la marque (tuile
    # 64 en haut-gauche, pastille 25 débordant de 6) ; sans, la tuile
    # emplit tout.
    $u = if ($pastille) { $taille / 70.0 } else { $taille / 64.0 }
    $tuile = 64.0 * $u

    $pinceau = New-Object System.Drawing.SolidBrush($fondTuile)
    $g.FillPath($pinceau, (New-CheminArrondi 0 0 $tuile $tuile (15.0 * $u)))

    $boite = $tuile * $fractionGlyphe
    $orig = ($tuile - $boite) / 2.0
    $su = $boite / 48.0
    $trait = [Math]::Max(3.0 * $su, $traitPlancher)
    $plume = New-Object System.Drawing.Pen($vertWind, $trait)
    $plume.StartCap = 'Round'; $plume.EndCap = 'Round'; $plume.LineJoin = 'Round'
    $g.DrawPath($plume, (New-CheminArrondi ($orig + 7 * $su) ($orig + 13 * $su) (34 * $su) (23 * $su) (3 * $su)))
    $g.DrawLines($plume, [System.Drawing.PointF[]]@(
        (New-Object System.Drawing.PointF(($orig + 9 * $su),  ($orig + 16 * $su))),
        (New-Object System.Drawing.PointF(($orig + 24 * $su), ($orig + 27 * $su))),
        (New-Object System.Drawing.PointF(($orig + 39 * $su), ($orig + 16 * $su)))))

    if ($pastille) {
        $b = 25.0 * $u; $bx = 45.0 * $u; $by = 45.0 * $u
        $pinceauB = New-Object System.Drawing.SolidBrush($vertWind)
        $g.FillPath($pinceauB, (New-CheminArrondi $bx $by $b $b (8.0 / 25.0 * $b)))
        $bu = $b / 25.0
        $plumeW = New-Object System.Drawing.Pen([System.Drawing.Color]::White, (2.5 * $bu))
        $plumeW.StartCap = 'Round'; $plumeW.EndCap = 'Round'; $plumeW.LineJoin = 'Round'
        $w = foreach ($pt in @(@(5.8, 7.5), @(9.4, 17.5), @(12.5, 10.5), @(15.6, 17.5), @(19.2, 7.5))) {
            New-Object System.Drawing.PointF(($bx + $pt[0] * $bu), ($by + $pt[1] * $bu))
        }
        $g.DrawLines($plumeW, [System.Drawing.PointF[]]$w)
    }
    $g.Dispose()
    return $bmp
}

$rendus = @(
    @{ taille = 256; pastille = $true;  fraction = (34.0 / 64.0); plancher = 0 },
    @{ taille = 48;  pastille = $true;  fraction = (34.0 / 64.0); plancher = 0 },
    @{ taille = 32;  pastille = $false; fraction = (34.0 / 64.0); plancher = 1.4 },
    @{ taille = 16;  pastille = $false; fraction = (42.0 / 64.0); plancher = 1.5 }
)

$pngs = foreach ($r in $rendus) {
    $bmp = New-Rendu $r.taille $r.pastille $r.fraction $r.plancher
    $ms = New-Object System.IO.MemoryStream
    $bmp.Save($ms, [System.Drawing.Imaging.ImageFormat]::Png)
    if ($Apercu) {
        New-Item -ItemType Directory -Force $Apercu | Out-Null
        [System.IO.File]::WriteAllBytes((Join-Path $Apercu "apercu-$($r.taille).png"), $ms.ToArray())
    }
    $bmp.Dispose()
    @{ taille = $r.taille; octets = $ms.ToArray() }
}

$sortie = Join-Path $PSScriptRoot "..\apps\desktop\icons\icon.ico"
$flux = [System.IO.File]::Create($sortie)
$ecrivain = New-Object System.IO.BinaryWriter($flux)
$ecrivain.Write([uint16]0)               # réservé
$ecrivain.Write([uint16]1)               # type icône
$ecrivain.Write([uint16]$pngs.Count)
$decalage = 6 + 16 * $pngs.Count
foreach ($p in $pngs) {
    $dim = if ($p.taille -ge 256) { 0 } else { $p.taille }
    $ecrivain.Write([byte]$dim); $ecrivain.Write([byte]$dim)
    $ecrivain.Write([byte]0); $ecrivain.Write([byte]0)   # palette, réservé
    $ecrivain.Write([uint16]1); $ecrivain.Write([uint16]32)
    $ecrivain.Write([uint32]$p.octets.Length)
    $ecrivain.Write([uint32]$decalage)
    $decalage += $p.octets.Length
}
foreach ($p in $pngs) { $ecrivain.Write($p.octets) }
$ecrivain.Dispose()

Write-Host "icon.ico écrit ($($pngs.Count) tailles : $(($rendus | ForEach-Object { $_.taille }) -join ', '))."
