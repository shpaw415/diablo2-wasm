# DC6 — Spécification (Diablo 2 v1.12 digital, rétro-ingénierée 2026-10-22)

Source de vérité exécutable : `tools/dc6_mini.py` (Python) et `engine-rs/crates/d2dc6` (Rust, vecteurs SHA-256 par frame). Rendus de validation : `assets/skillicon-frames-v2.png`, `assets/invcap-render-v2.png`.

## En-tête de fichier (16 octets)
| Off | Taille | Champ | Notes |
|----|--------|-------|-------|
| 0x00 | u32 | version | 6 observé (7 = classique avec pointeurs de direction) |
| 0x04 | u32 | flags | 0x1 = variant « v1.12 » (compteur de frames explicite) |
| 0x08 | u32 | encoding | 0 ou 1 — la grammaire RLE réelle prime sur ce champ |
| 0x0C | u32 | filler | 0xCDCDCDCD (mémoire non initialisée chez Blizzard) |

## Deux layouts de pointeurs (validation A puis repli B dans d2dc6)
**Variant flags=0x1 (skillicon.dc6, invcap.dc6)** :
- 0x10 : u32 dirCount (=1 observé)
- 0x14 : u32 frameCount (24 skillicon, 1 invcap)
- 0x18 : u32[frameCount] pointeurs d'en-têtes de frame

**Classique** : 0x10 : u32[256] pointeurs (inutilisés = ordure), u32 0xEEEEEEEE après ; v7 : pointeurs de direction → {u32 frameCount ; u32[frameCount] pointeurs}.

## En-tête de frame (28 octets)
u32 flip, u32 width, u32 height, u32 offsetX, u32 offsetY, u32 alloc(=0), u32 nextFramePtr.
Frame data : u32 dataLen puis pixels (arrêter le décodage après dataLen/height — le padding 0xCD/0xEE n'est jamais décodé).

## Grammaire RLE (scanlines bas→haut, groupes de 4 ; dest_row = height-1-src_row)
**Variant (polarité inversée)** : `b <= 0x7F` → littéraux ; `b >= 0x80` → transparents (sans opérande).
**Classique** : `b <= 0x7F` → b transparents ; `0x80 n` → n>0 reculer de n, n==0 fin de ligne ; `0x81 n` → n littéraux ; `b > 0x81` → (b&0x7F) littéraux.
Runs pouvant déborder la largeur → clamper x à [0, width].

## Palette
Index 8 bits → `data\global\palette\act1\pal.dat` (767 octets réels, padder à 768) ; index 0 = transparent.

## Frames roses de skillicon (4 dernières)
Vraies frames de remplissage du DC6 original (index 132 = RGB 252,24,92) — identiques côté Python et Rust, pas un bug de décodage.
