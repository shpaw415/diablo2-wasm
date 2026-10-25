# evidence.md — Fonds d'arbre de talents DC6 par classe (Diablo 2 v1.12)

Projet : diablo-2-wasm / étape 18a (UI fidèle). MPQ LECTURE SEULE
(`/home/agent/workspace/diablo-2-wasm/data|engine`). Extraction : `d2tool extract`.

## Sources MPQ

Dossier interne : `data\global\ui\SPELLS\`. Un fichier PAR classe
(convention vérifiée via OpenDiablo2 `d2resource` : a/b/d/i/n/p/s) ;
les 5 classiques trouvées par sonde de nom direct dans d2data.mpq, les 2
LoD (druide/assassin) dans d2exp.mpq (listfile).

| Classe | Fichier livré | MPQ réel | Taille | SHA-256 |
|---|---|---|---|---|
| Amazone | `skltree_a_back.dc6` | data/d2data.mpq | 375 192 o | `b82177145b20fde1d26a05709b10fd6ef5d604c9ff587b4da992d217dac24ef7` |
| Barbare | `skltree_b_back.dc6` | data/d2data.mpq | 375 192 o | `d2de75cc86ec5ec1f939ec7e9fec7c0728a1a30679884b2c6f4e24dfafd179b4` |
| Druide | `skltree_d_back.dc6` | data/d2exp.mpq | 375 192 o | `e8daa4235d98f395bff5f28320fe7c31366eec3258cde19ea3c2fca7305456fa` |
| Assassin | `skltree_i_back.dc6` | data/d2exp.mpq | 375 192 o | `a8328034fd6ecb5f6915e72027cea260747c01fc67694de2c67162343c257fce` |
| Nécromancien | `skltree_n_back.dc6` | data/d2data.mpq | 375 192 o | `7efac0cc5bc5350a7ecaade0ee151a0391e93009bbeb31f43adf0ae59f61178f` |
| Paladin | `skltree_p_back.dc6` | data/d2data.mpq | 375 192 o | `7bf08d4cea1506f6d0495e8b3a53c0b7350f84bd42546ef01ea96b96c8c0edd3` |
| Sorcière | `skltree_s_back.dc6` | data/d2data.mpq | 375 192 o | `c9141651ec59097facca72a12102bf83648782e2374731e200922bd3581149f0` |

## Structure observée (v1.12, identique sur les 7 fichiers)

```
u32 version 6, flags 1, encoding 0, champ4 variable, directions 1,
frames_per_direction 16, puis u32 pointers[16]
```

16 frames = 4 tuiles communes (256×256, 64×256, 256×176, 64×176 → panneau
320×432 posé en (0,0)/(256,0)/(0,256)/(256,256)) + par onglet t∈0..3 :
overlay large (frames 4+4t @ (0,0) et 6+4t @ (0,256)) + colonne droite
« pile d'onglets » (5+4t @ (256,0), 7+4t @ (256,256), l'onglet sélectionné
y est dessiné allumé ; frames 5 et 15 entièrement transparentes).
Composition vérifiée contre OpenDiablo2 `d2game/d2player/skilltree.go`
(frameCommonTab* 0-3, topFrame 4+4·tab, bottomFrame 6+4·tab) et rendue en
PNG par `d2game::game::skill_tree_bg_png18a` (tests 18a).

Palette : `assets/pal_act1.dat` (convention démo — acte courant).

## Icônes (copies vérifiées)

`../skillicons/*.dc6` : copies de `/home/agent/workspace/d2w-re/sprites/
skillicons/` (SHA-256 d'origine conservés, cf. evidence.md de ce dossier) :
am/as/ba/dr/ne/pa/so skillicon.dc6 — 60 cellules 48×48 (156 barbare).
