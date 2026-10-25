# Notes OpenDiablo2 → diablo-2-wasm

Source : https://github.com/OpenDiablo2/OpenDiablo2 (Go, archivé par les auteurs) — cloné et étudié le 2026-10-22.
**Rôle de ce document : digest d'architecture et de savoir-faire.** Nous nous inspirons de leurs algorithmes et de
leurs choix, nous n'importons pas leur code (licence iD-Tech-ish OSL-3.0 : citer les auteurs, réécrire en Rust).
Auteurs à créditer dans notre README si on s'inspire directement : l'équipe OpenDiablo2 (essenciaL, LordTocs,
Necropotame, etc. — liste complète dans le fichier `CONTRIBUTORS` du repo).

⚠️ Tout ce qui suit est issu de LECTURE DE CODE (donnée d'étude), pas un copier-coller.

---

## Section 1 — Cheminement EXACT des assets DCC/COF (LE gap bloquant)

Toute la logique est dans `d2core/d2asset/composite.go` (fonctions `createMode`, `loadCompositeLayer`,
`baseString`) + `d2common/d2enum/{hero,composite_type,player_animation_mode,weapon_class}.go`.

### 1.1 La formule maîtresse (identique joueurs / monstres / objets)

```
COF : <base>/<TOKEN>/COF/<TOKEN><MODE><WC>.COF
DCC : <base>/<TOKEN>/<LAYER>/<TOKEN><LAYER><ARMOR><MODE><WCLAYER>.dcc
DC6 : idem avec .dc6 (ils essaient DCC d'abord, puis DC6)
```

avec `<base>` selon le type d'entité (`baseString`, composite.go:374) :

| Type d'entité | `<base>` | Token (`<TOKEN>`) |
|---|---|---|
| Joueur (`ObjectTypePlayer`) | `data/global/chars` | 2 lettres, cf. 1.2 |
| Monstre/NPC (`ObjectTypeCharacter`) | `data/global/monsters` | colonne **`Code`** de `monstats.txt` (2-3 lettres : `FA`, `ZM`, `SI`, `SK`…) |
| Objet décor (`ObjectTypeItem`) | `data/global/objects` | token du type d'objet (table objtypes) |

**Tout en MAJUSCULES dans leur code** (tokens, LAYER, MODE) sauf **ARMOR/WC en minuscules** — et ça marche
parce que la hash table MPQ est **insensible à la casse** (leur loader MPQ met le nom en minuscule avant lookup,
`d2fileformats/d2mpq/mpq.go:102`). Sur un système de fichiers local il faut donc **normaliser la casse
nous-mêmes** (chercher insensible à la casse) — c'est exactement le piège v1.12 qu'on a rencontré.

### 1.2 Tokens joueurs (d2enum/hero.go `GetToken`)

| Classe | Token 2 lettres (dossiers + fichiers) | Token 3 lettres (UI, pas les chemins d'anim) |
|---|---|---|
| Amazon | `AM` | AMA |
| Assassin | **`AI`** (pas AS !) | ASS |
| Barbarian | `BA` | BAR |
| Druid | **`DZ`** (pas DR !) | DRU |
| Necromancer | `NE` | NEC |
| Paladin | `PA` | PAL |
| Sorceress | `SO` | SOR |

### 1.3 Les variables de la formule

- **LAYER** = code du membre, `CompositeType` (composite_type.go) — 16 couches :
  `HD` (tête), `TR` (torse), `LG` (jambes), `RA`/`LA` (bras D/G), `RH`/`LH` (mains D/G), `SH` (bouclier),
  `S1`…`S8` (spécial : ailes, aura, flèches de carquois…).
- **MODE** = mode d'animation (player_animation_mode.go) :
  `DT` mort, `NU` idle, `WL` marche, `RN` course, `GH` touché, `TN`/`TW` ville idle/marche,
  `A1`/`A2` attaque 1/2, `BL` blocage, `SC` cast, `TH` jet, `KK` coup de pied, `S1`…`S4` skills, `DD` cadavre.
  (monstres : mêmes codes, cf. monster_animation_mode.go).
- **ARMOR** (couche vêtement) : `lit` (léger) / `med` / `hvy` — la valeur par défaut est **`lit`**
  quand rien n'est équipé (composite.go:287 `layerValue = "lit"`). Pour `RH`/`LH`/`SH`, l'« armure » est le
  **code item 3 lettres** de weapons.txt/armor.txt (ex : `hax`, `buc`).
- **WCLAYER** = weapon class **de la couche, lu dans le COF** (voir 1.4 — c'est LE point qu'on avait raté).
  Codes WC possibles (weapon_class.go) : `hth, bow, 1hs, 1ht, stf, 2hs, 2ht, xbw, 1js, 1jt, 1ss, 1st, ht1, ht2`.

### 1.4 ⚡ Le détail qui débloque nos FALLBACK : le WC est PAR COUCHE, déclaré dans le COF

Dans `createMode` (composite.go:284-309) : ils parcourent les **CofLayers du COF chargé**, et pour chaque
couche ils construisent le nom DCC avec **`cofLayer.WeaponClass`** — le champ weapon-class du **bloc de cette
couche dans le COF lui-même** — PAS le WC global du mode. Le WC global ne sert qu'à trouver le COF.

Conséquence pratique : **un COF `<TOKEN><MODE><WC>.COF` liste exactement quelles couches existent, avec
quelle armure-valeur (notre équipement) et quel WC par couche.** Il ne faut donc JAMAIS deviner les noms de
DCC : il faut 1) charger le COF, 2) pour chaque CofLayer, générer
`<TOKEN><LAYER><ARMOR><MODE><WCLAYER_DE_CETTE_COUCHE>.dcc`. Nos fallbacks `FA: SH, RH` / `ZM: S3` / `BA: SH, RH`
sont probablement soit des couches absentes du COF (pas à dessiner), soit des variantes WC qu'on n'a pas essayées.

Algorithme complet d'un `Composite` OOD2 (à porter en Rust) :
1. Résoudre `cofPath` = `<base>/<TOKEN>/COF/<TOKEN><MODE><WC>.COF` ; échouer proprement si absent.
2. Lire le COF : nombre de directions, blocs par couche, table **Priority[dir][frame]** (ordre de dessin).
3. Récupérer frames + vitesse dans **`animdata.d2`** (chemin `data/global/animdata.d2`) — clé =
   **`TOKEN+MODE+WC` en majuscules** (ex `BAWL1HS`). Le COF ne donne PAS le nombre de frames :
   `FramesPerDirection` et `Speed` viennent d'animdata.d2 (composite.go:270-281). Plusieurs records
   homonymes possibles → ils prennent le **dernier** (`GetRecord`). `Speed` est en 256ièmes de 25 fps
   (speedUnit = 25/256).
4. Pour chaque CofLayer : valeur d'équipement (`lit` si rien), drawEffect si transparent, shadow flag,
   puis charger le DCC/DC6 de la couche (formule 1.1).

### 1.5 Exemples concrets (chemins complets)

Barbare en marche une main (`WL`, `1hs`), couches du COF `BAWL1HS.COF` :

```
data/global/chars/BA/COF/BAWL1HS.COF
data/global/chars/BA/TR/BATRLITWL1HS.dcc     ← WC de la couche TR dans le COF = 1hs
data/global/chars/BA/LG/BALGLITWL1HS.dcc
data/global/chars/BA/HD/BAHDLITWL1HS.dcc
data/global/chars/BA/RA/BARALITWL1HS.dcc
data/global/chars/BA/LA/BALALITWL1HS.dcc
data/global/chars/BA/RH/BARHHAXWL1HS.dcc     ← main droite équipée de la hache `hax` : ARMOR = code item !
data/global/chars/BA/SH/BASHBU CWL1HS.dcc    ← (exemple) bouclier `buc` → BASHBUCWL1HS.dcc
```

Équivalent de notre « BAWLHD » complet : **`data/global/chars/BA/HD/BAHDLITWL1HS.dcc`** (sans arme équipée).

Monstre Fallen (`FA`) en marche :

```
data/global/monsters/FA/COF/FAWLHTH.COF
data/global/monsters/FA/TR/FATRLITWLHTH.dcc
data/global/monsters/FA/S1/FAS1LITWLHTH.dcc
data/global/monsters/FA/COF/palshift.dat    ← variantes de couleur (palette index = colonne TransLvl de
                                               monstats.txt + 2 ; certains tokens l'ignorent, ex `FC`)
```

Palette des unités : `data/global/palette/units/pal.dat` (`PaletteUnits`, utilisée pour TOUS joueurs/monstres).

### 1.6 Directions

Le moteur interne est en **64 directions** ; les COF n'en ont que 4/8/16/32/64. Table de conversion
`Dir64ToCof` (d2cof/cof_dir_lookup.go) : correspondances exactes par pas (ex 8 dirs : 8 pas de 8 sur 64,
le 0 se répète en fin de table). À porter telle quelle (c'est une table de données, pas du code créatif).

### 1.7 Checklist résolution MPQ v1.12 digital (nos archives)

- Nos MPQ : `d2char.mpq` (héros), `d2data.mpq` (monstres/objets), `patch_d2.mpq` (tables + surcharges v1.12).
  **Priorité de lecture : patch_d2.mpq > d2exp.mpq > d2data.mpq/d2char.mpq** (c'est l'ordre MPQ standard).
- `(listfile)` tronqué (102 o) → générer les noms candidats par la formule 1.1 et les tester par hash
  (notre `d2tool probe`), MAIS dériver les candidats des **CofLayers** (1.4) au lieu de formes devinées.
- Casse insensible (1.1) : c'est normal que `data\global\chars\<CLASS>` « ne réponde pas » si on cherche
  `BA` avec un filesystem sensible à la casse — les entrées réelles sont en minuscule dans les MPQ digitaux
  (vérifié chez nous : `data\global\chars\ba\cof\bawl1hs.cof`).

---

## Section 2 — Architecture moteur à adopter

Packages OOD2 de référence : `d2app` (boucle), `d2core/d2screen` (états), `d2core/d2map/{d2mapengine,
d2maprenderer,d2mapstamp,d2mapentity}` (monde + rendu iso), `d2core/d2asset` (composites/animations),
`d2core/d2audio`, `d2core/d2input`, `d2core/d2ui` + `d2gui`, `d2game/d2gamescreen` (écrans),
`d2game/d2player` (contrôles/HUD/panneaux), `d2core/d2records` (toutes les tables txt), `d2networking`
(client/serveur optionnel). Constat d'ensemble : découpage propre en couches sans dépendances inversées —
**mais** beaucoup de singletons injectés à la main (AssetManager partout). En Rust on fera des modules +
passage de contextes explicites (déjà notre style dans engine-rs).

### 2.1 Boucle et états

- Boucle unique : `update(dt)` / `render(surface)` à chaque tick (48-62 ebiten, nous : rAF du navigateur) ;
  `dt` en secondes propagé à Advance(elapsed) partout (interpolation par accumulation de temps fractionnaire
  dans les animations — cf. composite Advance, frameIndex += elapsed/speed).
- **ScreenManager** (d2screen) : pile à UN état courant + `NextState` avec écran de chargement asynchrone
  (LoadingState : la grosse init se fait pendant l'écran "loading", puis swap). États concrets :
  MainMenu, CharacterSelect, SelectHeroClass, Game, Credits, Cinematics. À porter tel quel en machine à
  états dans **d2game** (on a déjà brief-menu / brief-herosel qui suivent cette forme).
- Leçon OOD2 : le temps de jeu est *scaled* (`timeScale`) — gratuit chez nous, skip.

### 2.2 Rendu isométrique (LE cœur à reprendre)

Espaces de coordonnées (viewport.go) — trois espaces, conversions exactes :

```
world  : 1 unité = 1 tuile (x+y axes diag)          — physique/logique
ortho  : pixels écran sans caméra                    orthoX=(x-y)*80  orthoY=(x+y)*40
screen : ortho - offset caméra                       (entier, floor)
world ← ortho : x=(ox/80+oy/40)/2  y=(oy/40-ox/80)/2
```

- Tuile = **80×40 px**, découpée en **5×5 sous-tuiles de 16×8 px**. Les flags DT1 (blockwalk etc.) sont
  PAR SOUS-TUILE — collision ET tri d'entités à cette granularité. (Nous avions déjà cette info dans
  MAP-SPEC.md, OOD2 la confirme et montre l'usage.)
- **4 passes de rendu** (renderer.go:231-358), ordre strict, culling par rect visible :
  1. floors + lower walls + ombres de tuiles (du DT1),
  2. entités *derrière* les murs (entity layer == 1),
  3. upper walls + entités *devant* (layer != 1),
  4. roofs.
- **Pas de tri global par y** : entités groupées par tuile, puis boucle subtile (subY, subX 0..4) et on
  dessine celles dont l'offset subtile correspond → tri implicite stable et O(1) par entité. À reprendre
  tel quel en Rust (Buckets: HashMap<(tx,ty), Vec<EntityIdx>> + 25 itérations de balayage).
- **Cache d'images de tuiles** (tile_cache.go) : chaque (style,sequence,type,randomIndex) DT1 est
  composé en une image une fois pour toutes (generateFloorCache/WallCache/ShadowCache), invalidée si la
  palette change. En WASM : encore plus important (les blits JS coûtent) → générer des atlas au chargement,
  dessiner avec drawImage tranché. 
- Culling : `IsTileVisible` avec marge de ±3 tuiles en X (worldToOrthoOffsetX=3) pour les murs hauts.
- Palette : une palette PAR ACTE pour le terrain (`data/global/palette/actN/pal.dat`), palette `units/pal.dat`
  pour les sprites d'unités — c'est pour ça que les couleurs des persos ne changent pas d'acte en jeu.

### 2.3 Monde : DS1/DT1 → MapEngine (d2mapengine + d2mapstamp)

- MapEngine = tableau plat de tuiles (width×height), liste d'entités (map uuid→entity), seed RNG,
  `TileAt/SubTileAt`, `Advance(dt)` qui avance les entités.
- Chargement : `AddDS1` (layout : pour chaque tuile, styles/séquences → quel DT1 tile picker) + `AddDT1`
  (bibliothèque de tuiles). **LevelType** (lvltypes.txt) liste quels DT1 forment une région ;
  **LevelPreset** (lvlprest.txt) référence le DS1. `GenerateMap(regionType, preset, fileIndex)` fait :
  load DS1 via preset → stamps → PlaceStamp à l'offset.
- **Stamp** = DS1 préfabriqué positionnable (avec ses entités : objects/monstres du DS1 converties en
  entités à la pose). C'est leur unité de génération : le d2mapgen/act1_overworld est un petit générateur
  maison qui pose des stamps wilderness autour de la ville.
- Variations aléatoires : `getRandomTile(tiles, x, y, seed)` — même seed ⇒ même carte (seedable, replayable).
  À garder : un seul RNG seedé pour toute la map, exposé à l'UI (rejouabilité, partage de seed).
- Layers d'entité : `GetLayer()` 1 = derrière murs, autre = devant — porté par le type d'entité.

### 2.4 Entités et animation

- Hiérarchie : MapEntity (position world + velocity + target + Step) ← AnimatedEntity (Composite + direction)
  ← Player / NPC / Object / Missile / CastOverlay / Item.
- Position : float world ; déplacement = cible + vitesse par tick (`Step`) ; direction = 64 dirs calculée
  depuis le vecteur vitesse (atan2 → index).
- Composite : cf. §1.4 (COF = plan des couches + priority par frame ; animdata.d2 = frames + speed).
- Rendu d'un frame composite : pour la direction COF résolue, dessiner les couches dans l'ordre
  `COF.Priority[dir][frame]` — le COF donne L'ORDRE DE DESSIN par frame, on ne réinvente rien.
- Missile : entité animée qui vole vers une cible (range + angle), collision non résolue chez eux (§4).

### 2.5 Audio, input, UI

- Audio : SoundEngine (SFX par id, panning selon position) + SoundEnvironment (ambiance par région) ;
  sons déclarés dans `data/global/excel/sounds.txt` (+ MusicDefs pour les mp3). Chez nous : wavs déjà
  extraits ; panning stéréo simple = bonus facile en WebAudio.
- Input : InputManager à handlers (bind par écran), keymap par défaut rebindable (binding_layout.go) ;
  GameControls : click-to-move (clic droit) + casts (clic gauche) + HUD (globe, skill bar, inventory
  grid 10×10, stash…). Port direct dans **d2game** (brief-menu déjà dans cette voie).
- UI : widgets DC6 (boutons/panneaux DC6 d'UI sous `data/global/ui/`), layout flow vertical/horizontal
  (d2gui). Le HUD complet (hud.go, globes, minipanel) est une bonne carte routière pour l'UX.

### 2.6 Mapping vers nos crates

| OOD2 (Go) | Chez nous (Rust) | État |
|---|---|---|
| d2fileformats/d2mpq | `d2mpq` | ✅ fait (équivalent, validé SHA-256) |
| d2fileformats d2dcc/d2dc6 | `d2dcc`, `d2dc6` | ✅ fait |
| d2core/d2asset Composite + animdata | `d2game::anim` (à créer) | 🔜 clé §1.4 |
| d2core/d2map/d2mapengine (+stamp) | `d2map` (à créer) | 🔜 |
| d2core/d2map/d2maprenderer | `d2wasm` (canvas/JS glue) | 🔜 4 passes + cache |
| d2core/d2records | `d2rules` | partiel (16 tables déjà citées dans GAME-DATA.md) |
| d2core/d2hero + d2game/d2player | `d2rules` (stats/skills) + `d2game` (HUD) | partiel |
| d2networking | — (hors scope MVP, single player) | n/a |
| d2script (Lua terminal) | — | n/a |

---

## Section 3 — Gameplay à porté (après le MVP)

- **Tables** (d2core/d2records — 60+ tables parsées, commentaires = documentation des colonnes excellente,
  à consulter avant chaque port) : monstats/monstats2, monlvl (scaling par difficulté — déjà extrait),
  levels/lvlprest/lvltypes/lvlwarp, missiles, skills/skilldetails/skilldesc, magicprefix/magicsuffix +
  rareprefix/rareprefix (groupes, fréquence, conditions levelType/maxlevel), treasureclassex, items
  (weapons/armor/misc + automagic), overlays, states.
- **Missiles** (missiles_record.go : parsing 100 %, moteur ~10 %) : record = mouvement (Velocity/Range/Accel
  + fonctions serveur/client), collision (types 0/1/3/6/8, friendly fire, timer), dégâts (min/max + paliers
  par niveau [2-8|9-16|17-22|23-28|29+] + synergies calc), éléments (durée en frames, 25 = 1 s), sous-missiles
  (spawn au mouvement / à l'impact), explosion, sons, lumières. Sprites : `data/global/missiles/*.dc6`.
  ⚠️ colonne `HitShift` : dégâts ×2^hitshift (unité 256ièmes) — piège classique.
  Plan de port : d'abord missile droit (flèche/firebolt) avec collision subtile + sous-missile explosion ;
  les fonctions de mouvement exotiques (spirales, forks) plus tard.
- **Affixes** : commun prefix/suffix avec groupes d'exclusion, fréquence pondérée, `levelType` (0/1/2),
  `maxlevel`, div floor. Roll = filtre par ilvl → poids → RNG seedé. Chez nous : table déjà citée dans
  GAME-DATA, port simple dans d2rules.
- **Monstres** : NewNPC = monstats (Code→token, missiles A1/S1…, treasureclass, TransLvl) + monstats2
  (equipment options par couche + BaseWeaponClass → composite). IA : OOD2 n'en a PAS (paths de patrouille
  DS1 seulement) — notre IA aggro/poursuite d2rules est déjà en avance.
- **Items au sol** : entité Item avec DC6 flippy (`data/global/items/*.dc6`, colonne `flippyfile`),
  pickup par clic. Simple et très visible en démo.
- **UI gameplay** : inventory 10×10 grid + equipment slots (d2player/inventory.go), hero stats panel,
  skill tree (skilltree.go) — gros morceaux, après le HUD de base.

---

## Section 4 — Leurs limitations / erreurs à éviter (issues citées dans leur code + status.md)

1. **Jamais jouable au combat** (status.md, mars 2021) : ville Act1 + panneaux seulement ; l'équipe a
   abandonné le repo pour AbyssEngine. Ne pas sur-estimer leur moteur : les décodages (MPQ/DCC/DC6/COF/DS1)
   sont solides, le gameplay est squelettique.
2. **Pathfinding = simple raycast LOS** (pathfind.go) : pas d'A* — les monstres/NPC restent coincés.
   Notre d2rules doit garder un vrai A* subtile (déjà prévu dans brief-rules).
3. **Perf** : pas de culling des entités (issue 821), régénération manuelle du tile cache, lectures MPQ
   synchrones bloquantes (loader), goroutines par SetDirection (composite.go:165) = overhead inutile.
   En WASM : extraire/décoder les atlas OFFLINE (notre pipeline d'extraction) au lieu de decoder DCC à
   chaque lancement ; garder le cache de tuiles.
4. **Palettes monstres non implémentées** : `palshift.dat` documenté mais pas chargé — variantes de couleur
   (champions/uniques) impossibles chez eux. Opportunité pour nous : remappage de palette DCC (on a déjà
   le décodeur + les palettes), ça donnera les variantes uniques en démo.
5. **Course aux singletons/injection** (AssetManager passé partout, loggers globaux) : en Rust, garder des
   structs clairs + un seul contexte asset. Leur `composite.go` mélange chargement et état d'animation —
   séparer loader (d2dcc) et runtime (d2game::anim).
6. **Issues à lire avant de porter un bout** : 789 (rendu pendant chargement), 803 (paths NPC), 819
   (Advance missile incomplet), 821 (culling). Leurs commentaires de code citent l'issue : bonne source
   de pièges.

---

## Section 5 — Recommandations de découpage (3 membres)

**Membre « Animations complètes » (brief-anims/dcc)**
1. Porter l'algorithme §1.4 : loader COF → générer TOUS les chemins DCC par couche (WC par couche) →
   probe → extract. Ça supprime les 5 FALLBACK du manifest ou les explique.
2. Extraire animdata.d2 (`data/global/animdata.d2`, patch_d2.mpq prioritaire) : records
   TOKEN+MODE+WC → frames/direction, speed, events (frames trigger pour SFX de pas/coups).
3. Livrer : BA (7 modes NU/WL/RN/A1/BL/GH/DT minimum), FA, ZM, SI complets + un 4e monstre (SK squelette).
4. Bonus : palshift.dat (variantes couleur) — on serait les seuls à l'avoir.

**Membre « Niveaux supplémentaires » (brief-map/levels)**
1. Port du rendu 4 passes + cache d'atlas tuiles (§2.2) — structure identique à notre démo actuelle.
2. Support lvltypes/lvlprest/DS1 multiples (stamps §2.3) : ajouter 2-3 régions Act1 (Blood Moor, Den of Evil)
   avec portes/warps (lvlwarp) plutôt qu'une grande carte unique.
3. Flags subtile blockwalk branchés à d2rules (collision) + LOS.

**Membre « Moteur/UX » (brief-moteur/engine)**
1. Machine à états d2game (menu → select → game) propre avec écran de chargement (§2.1).
2. HUD minimal type OOD2 : globe vie/mana, barre de skills, label nom/niveau au survol, click-to-move.
3. Missiles : flèche + firebolt (§3) avec collision subtile ; items flippy au sol.
4. Audio : brancher les 13 wavs aux events animdata (pas + coups) + panning.

---

## Section 6 — Rapport final analyste

**Ce qu'OpenDiablo2 fait mieux / à reprendre** : la RIGUEUR des chemins d'assets (§1 — ils ont le schéma
complet et surtout le principe « le COF est la source de vérité des couches ») ; les 3 espaces de
coordonnées + 4 passes de rendu (§2.2) ; le tri par tuile/subtile (pas de z-sort global) ; le cache de
tuiles ; les stamps DS1 comme unité de génération ; la documentation des colonnes txt (leurs commentaires
monstats/missiles = spéc de référence).

**Ce qu'on fait déjà aussi bien (ou mieux)** : lecteur MPQ complet validé octet-pour-octet (leur loader
est plus lent, synchrone) ; décodeurs DCC/DC6 déjà fonctionnels (les leurs ont eu des bugs de RLE connus) ;
pathfinding réel prévu (eux : raycast) ; pipeline d'extraction offline (eux : tout au runtime) ; le gate
v1.12 digital (hash probing) qu'ils n'ont jamais eu à résoudre — et §1 le résout définitivement.

**Les 5 prochaines actions priorisées**
1. **Porter l'algorithme §1.4 dans d2game::anim** (COF → couches → chemins DCC) et régénérer le
   manifest : débloque toutes les animations (priorité absolue, c'était LE gap).
2. Extraire et parser `animdata.d2` (frames/speed/events) — nécessaire pour animer correctement TOUT
   (vitesses et déclencheurs sonores exacts).
3. Mode A1 + DT du barbare et du Fallen avec priority du COF par frame → première attaque complète en jeu.
4. Rendu 4 passes + cache d'atlas dans d2wasm (structure §2.2) et collision blockwalk subtile → d2rules.
5. Deuxième région (Blood Moor via stamp DS1) + un missile (flèche) → boucle de gameplay fermée :
   marcher, tirer, tuer, ramasser.

