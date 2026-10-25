# RAPPORT-FIX — d2w-fix-ui : UI de jeu fidèle à Diablo 2 v1.12

Arbre : `/home/agent/workspace/d2w-fix-ui/` (copie du maître ; `extracted/`, `data/`,
`assets/` = liens symboliques lecture-seule vers le maître, jamais modifiés).
Statut : **livré** — jeu 100 % jouable, `window.__d2.errors = 0` sur toutes les sondes E2E.

## Cause

L'interface de gameplay (globes, barre du bas, inventaire, arbre de compétences)
était **redessinée à la main** (cercles canvas, boîtes CSS) au lieu d'utiliser les
sprites originaux. La mission impose la fidélité v1.12 : extraire les DC6 réels de
`data/global/ui/` (MPQ) et composer l'UI comme le jeu d'origine.

## Ce qui a été livré

1. **Barre du bas D2 réelle** (canvas, `demo/ui-d2.js`) : cadre `800ctrlpnl7.dc6`
   (7 frames, largeurs 117+48+128+128+128+86+48+117 = 800), orbes vie/mana
   `hlthmana.dc6` à remplissage **liquide** (tranche basse du sprite 80×80 selon
   la fraction de vie/mana — méthode RenderSection d'OpenDiablo2) + verre
   `overlap.dc6`, ceinture 4 alvéoles avec potions réelles (colonne 1-4, clic ou
   touches 1-4), barre d'XP (blanc, 120 px) et d'endurance (ocre/rouge, 102 px)
   posées sur les rigoles du panneau, icônes de compétence L/R 48×48 (feuilles
   `skillicon.dc6` / `<classe>skillicon.dc6` — attaque de base = cellule 2), boutons
   « + » (`level.dc6`) quand des points sont à dépenser, minipanel D2 repliable
   (`minipanel.dc6` + 8 boutons `minipanelbtn.dc6` + flèches `menubutton.dc6`).
2. **Panneau d'inventaire D2** (I) : `invchar6.dc6` recollé (4 quadrants,
   320×432 à (400,60)), grille 10×4 aux coordonnées **réelles** d'`inventory.txt`
   (origine (419,315), cellules 29×29), sprites d'objets extraits (potions hp/mp,
   parchemins, tomes, or), pièce + montant d'or dans la rigole, survol liseré doré
   + info-bulle, socle de fermeture cliquable. Utilisation des objets au clic :
   chemin historique inchangé (`useConsumable`/`useItem`).
3. **Arbre de compétences D2** (T, `demo/skills-ui.js` réécrit) : fond par classe
   `skltree_<classe>_back.dc6` (7 classes × 3 onglets recollés en
   `sktree_<clé>_tab{1,2,3}.png`, base groupe 0 + calque groupe 1-3), icônes réelles
   48×48 posées sur les alvéoles sculptées (règle IconCel : <24 = feuille générique,
   ≥24 = feuille de classe cellule IconCel−24), onglets I/II/III à la géométrie D2
   (canvas 170/277/385, 92×107, x=628), « Points : N », clic sur icône apprenable =
   `learnSkill` (validation d2rules inchangée). Icônes possédées/apprenables/verrouillées
   distinguées (plein / lueur au survol / assombri).
4. **Zones cliquables DOM** (`.d2-hit`) posées aux positions D2 exactes (orbes :
   info-bulles Vie/Mana ; L/R : popup de sélection hud.js inchangé ; ceinture :
   boire la colonne ; « + » : ouvrir l'arbre ; minipanel : Inventaire/Arbre/Automap
   câblés, écrans absents = statut « à venir » ; socle inventaire : fermer).
5. **Robustesse** : si `ui-d2.js` n'est pas chargé/pret, game.js retombe sur les
   globes et l'inventaire historiques (fallback JS intact). Objets sans sprite
   extrait (rvs/rvl/wms absents du digital release) = bloc biseauté teinté par kind.

## Modifs `demo/game.js` — 7 points, tous commentés `// FIX-UI :`

game.js est partagé avec d2w-fix-heros : modifications **append-only**, aucune ligne
existante supprimée (les anciens chemins restent le repli).

| # | Ligne | Modif |
|---|-------|-------|
| 1 | ~285 (`drawGlobes`) | délègue à `D2UI.drawHud(ctx, stats)` (barre du bas D2) ; repli = anciens globes si UI pas prête |
| 2 | ~326 (objet `inv`) | géométrie D2 réelle : cellules 29, origine (419,315), 290×116 (au lieu de cellules 42 centrées) |
| 3 | ~346 (`drawInventory`) | délègue à `D2UI.drawInventory(ctx, invCache, null)` ; repli = dessin historique |
| 4 | ~602 (clic canvas) | zone active inventaire = panneau D2 entier via `D2UI.invHit(px,py)` ; repli = ancienne boîte de grille |
| 5 | ~893 (après `D2HUD.init`) | `D2UI.init(manifest)` — chargement des sprites + création des zones cliquables |
| 6 | ~910 (boucle frame) | `window.__d2.invHover = inv.hover` — survol lu par ui-d2.js (liseré + info-bulle) |
| 7 | ~917 (boucle frame) | `D2UI.poll(game, now)` ~1 Hz — ceinture (`beltInfo`), points, icônes (`skillTree`) |

Autres fichiers : `game.html` (+1 `<script src="./ui-d2.js">`, textes vie/mana
repositionnés sur les orbes (70/730, 556), `#xpwrap` masqué) ; `hud.css` (widgets DOM
redondants masqués, boutons L/R invisibles repositionnés (117,552)/(635,552), styles
`#d2-tree`/`.sktab`/`.d2-hit`) ; **`skills-ui.js` réécrit** ; **`hud.js` : NON modifié** ;
moteur Rust/WASM : **NON modifié** (aucun rebuild nécessaire, `demo/game-pkg` intact).

## Sprites extraits — sources et procédé

`tools/extract_ui_fix.py` (nouveau) : sonde StormLib priorité `patch_d2.mpq > d2exp > d2data >
d2char` (tomes du maître via `data/`+`engine/`), décodage `tools/dc6_mini.py`, palettes :
**`data\global\palette\sky\pal.dat`** pour toute l'UI (référence OpenDiablo2), **`units\pal.dat`**
pour les objets. Sortie : `demo/assets/ui/` + `ui-manifest.json` (tailles/frames).

| Sprite | Source MPQ | Usage |
|---|---|---|
| `panel_f0..f6` | `data\global\ui\PANEL\800ctrlpnl7.dc6` (d2data) | barre du bas (frames 0-5 utilisées) |
| `orbs_f0/f1` | `PANEL\hlthmana.dc6` | orbes mana (f0 bleu) / vie (f1 rouge) — 80×80 |
| `overlap_f0/f1` | `PANEL\overlap.dc6` (82×88) | verre/cerclage des orbes |
| `addskill_f*`, `runbutton_f*`, `menubutton_f*` | `PANEL\level.dc6`, `runbutton.dc6`, `menubutton.dc6` | boutons +, course (non câblé), flèches minipanel |
| `minipanel(_s).png`, `minipanelbtn_f0..f17` | `PANEL\minipanel*.dc6` | minipanel (9 boutons × 2 frames, 8 utilisés) |
| `goldcoinbtn_f*` | `panel\goldcoinbtn.dc6` | pièce d'or inventaire |
| `invchar6_f0..f7`, `invchar6tab_f*` | `PANEL\invchar6(.dc6|Tab.DC6)` | panneau inventaire (frames 4-7 = grille), dalles d'onglets |
| `inv_armor/belt/boots/helm/ring/weapons` | `PANEL\inv_*.DC6` | placeholders d'équipement (réserve) |
| `sktree_<ba,so,pa,am,as,dz,ne>_tab1..3.png` | `SPELLS\skltree_*_back.dc6` (16 frames = 4 groupes × 4 quadrants) | fonds d'arbre 320×432 |
| `sheet_generic.png` + `sheet_<clé>.png` | `SPELLS\skillicon.dc6` (24 cell.) + `<am,as,ba,dz,ne,pa,so>skillicon.dc6` (60 cell.) | icônes de compétences 48×48 (grille 8 colonnes) |
| `item_hp1..5, mp1..5, vps, yps, key, tsc, isc, tbk, gold` | `data\global\items\inv<invfile>.dc6` (d2data ; **invfile trouvés par hash** : tsc=`invpor`, isc=`invscr`, tome=`invbok`, or=`goldpile`) | objets inventaire/ceinture 28×28 (or 60×29) |
| `inventory.txt`, `belts.txt`, `items.txt` | `data\global\excel\*` (patch_d2) | coordonnées réelles de la grille |

Astuces éprouvées : le `(listfile)` de d2data est piégé → sonde par nom via hash
(`mpq_mini.block_info`) ; `mpq_list.py` (SFileFindFirstFile) fait « stack smashing » ;
le listfile de **d2exp** est lisible (10 095 entrées, sauvegardé dans /tmp lors de la session).

## Coordonnées mesurées (référence OpenDiablo2 `d2game/d2player/{hud,globeWidget,game_controls,mini_panel,inventory,skilltree}.go` + tables v1.12)

- Panel frames posées à x = 0, 165, 293, 421, 549, 683 (bas ancré y=600) ; trous 48 px pour L/R.
- Orbe vie : sprite 80×80 à (30,507), overlap à (28,507) ; orbe mana : (690,508), overlap (691,504) ;
  remplissage = `drawImage` de la tranche basse `h = frac*80`.
- Ceinture : alvéoles de `panel_f3` (bevaux mesurés) → potions 28×28 à x = 424/455/486/517, y = 561.
- XP (256,562,120×2 blanc) ; endurance (273,573,102×19, rgba(175,136,72,.78), rouge <25 %).
- Icônes L/R : (117,552) et (635,552), 48×48. Boutons « + » : (206,561) et (563,561).
- Minipanel : barre 176×26 à (325,525), 8 boutons 20×20 à (328+i×22, 528), flèches (392,538).
- Inventaire : panneau (400,60) 320×432 ; grille origine (419,315), pas 29 (`inventory.txt` Barbarian2,
  identique 7 classes) ; or : bouton (485,455), montant (510,458) ; fermer (419,449).
- Arbre : panneau (400,60) 320×432 ; centres d'alvéoles `x=50+(col-1)*60, y=381-(row-1)*61`
  (panel-local ; validé au pixel : bash p1 r1 c2 → centre écran (510,441)) ;
  onglets x=628 (canvas), y=170/277/385, 92×107 (haut=page 3, bas=page 1).

## Vérifications

- `node --check` : game.js, hud.js, ui-d2.js, skills-ui.js — OK.
- E2E headless (`demo/shot.html` + firefox-esr, sonde `window.__d2`) : **erreurs=0**,
  `mode=wasm`, touches 1-4 ceinture sans erreur, « Inventaire ouvert. »
- Screenshots (dans `docs/`) : `fix-ui-avant.png` (maître, avant), `fix-ui-apres.png`,
  `fix-ui-inventaire.png`, `fix-ui-arbre.png`.

## Points d'intégration pour d2w-fix-heros

- Mes 7 modifs game.js sont des **délégations** (`window.D2UI.*`) avec repli : si tu
  touches `drawGlobes`/`drawInventory`/la boucle, garde les appels D2UI en tête de fonction.
- Le HUD dessine dans la bande y≥496 (panneau) et pose des div `.d2-hit` (z-index 4)
  dans `#stage` ; `#d2-tree` (z-index 40) vit dans `#stage` à (400,60). L'échelle caméra
  ne les affecte pas (espace canvas 800×600 fixe).
- `game.html` a gagné une ligne `<script src="./ui-d2.js">` entre hud.js et waypoints.js :
  à préserver en cas de fusion.
- Aucun endpoint wasm ajouté ; j'utilise `beltInfo`, `skillTree`, `useBeltKey` (existants).

## Restant (hors périmètre, proposé pour une vague UI suivante)

- Bouton course `runbutton.dc6` extrait mais non câblé (le moteur gère Maj = course).
- Placeholders d'équipement (`inv_*.dc6`) non dessinés (le moteur n'expose pas encore
  l'équipement porté dans `inventory()`).
- Bouton or (dépôt/retrait) non câblé (pas d'écran de saisie de montant).
- Feuilles d'icônes par classe : les frames ≥60 cellules (ba=156) non tronquées —
  la planche `sheet_ba.png` couvre 156 cellules (8 colonnes), suffisant pour IconCel−24.
