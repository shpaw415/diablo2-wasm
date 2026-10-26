# assets/menu/ — écran-titre D2 (extraction 2026-10-22)

Source : `data/d2data.mpq` (le (listfile) est corrompu/PKWARE — sonde par hash via `tools/mpq_mini.py`,
voir `tools/probe_menu.py` / `probe_menu2.py`).

| Fichier | Origine MPQ | Rôle |
|---|---|---|
| `titlescreen.dc6` | `data\global\ui\frontend\titlescreen.dc6` (488 892 o) | Fond du titre : 12 tuiles 256×256 (+colonne 32) → 800×600 |
| `diablo2.dc6` | `data\global\ui\frontend\diablo2.dc6` (25 399 o) | Logo DIABLO II : 2 frames 256×151 + 64×151 (collées côte à côte) |
| `fire.dc6` | `data\global\ui\frontend\fire.dc6` (163 229 o) | Feu de camp animé : 30 frames ~110 px, offsetX=-34, offsetY=132 (base commune) |
| `title.png` | rendu | Fond 800×600 (tuiles 4×3, crop 768→600) |
| `logo.png` | rendu | Logo 320×151 |
| `fire-sheet.png` | rendu | Sprite sheet 30 cellules 180×140, base alignée à y=132, anim CSS `steps(30)` |
| `pal_loading.pl2` | `data\global\palette\loading\pal.pl2` (442 916 o) | Palette des DC6 frontend |
| `btn.dc6`, `menubutton.dc6`, `menupanel.dc6` | `data\global\ui\panel\*.dc6` | Extraits, non utilisés (btn 14×14, menubutton = flèches scrollbar 15×24 ×4 frames, menupanel 120×96) |

## Palette frontend (VERIFIÉ)
`pal_loading.pl2 @0` : 256 entrées RGBA **6 bits** (stride 4, alpha=0) → 8 bits par `v*255//252`.
C'est la palette `loading` (pas `act1`) qui rend le feu ORANGE et le ciel nocturne sans corruption ;
`act1/pal.dat` = feu vert, `act1/pal.pl2 @0` = ciel magenta.

## Boutons (FALLBACK CSS)
`broadmain.dc6` et tout bouton large 3 frames introuvables dans d2data/d2exp/d2char/patch_d2
(26 000+ noms sondés par hash). Le menu utilise des boutons texte CSS style D2
(or serif, rouge braise au survol/enfoncé) — le look du menu principal original.

Rendu : `python3 -c "..."` avec `tools/dc6_mini.py` (decode_dc6) + palette ci-dessus.
