# GAME-DATA — Données gameplay du MVP (Phase 2)

Source de vérité machine : **`assets/manifest.json`** (JSON strict, 4 clés racine : `hero`, `monsters`, `level`, `sfx`).
Ce document est la version lisible : chaque valeur est citée depuis une **colonne réelle** des tables extraites
(`assets/patch_*.txt`, `assets/TreasureClassEx.txt`, `assets/MonLvl.txt`, `assets/levels_exp.txt`).

Archives : `data/d2char.mpq`, `data/d2data.mpq`, `engine/patch_d2.mpq` (release digitale fr-FR, v1.12).
Sondage par hash (`engine-rs/target/release/d2tool probe`) + extraction StormLib (`tools/stormlib_read.py`) ;
les 21 fichiers sprites/cof de ce manifest sont **identiques octet-pour-octet à l'archive (SHA-256)** — §7.

---

## 1. Héros — Barbare (`BA`)

Source : `assets/patch_charstats.txt`, ligne `Barbarian` (colonnes citées entre parenthèses).

| Stat | Valeur | Colonne charstats.txt |
|---|---|---|
| Force | **30** | `str` |
| Dextérité | **20** | `dex` |
| Vitalité | **25** | `vit` |
| Énergie | **10** | `int` |
| Endurance | **92** | `stamina` |
| Vie niveau 1 | **55** | `hpadd` (30) + `vit` (25) — formule niveau 1, §1.1 |
| Mana niveau 1 | **15** | `int` (10) + base moteur 5 — formule niveau 1, §1.1 |
| Vitesse marche | **6** yards/s | `WalkVelocity` |
| Vitesse course | **9** yards/s | `RunVelocity` |
| Drain d'endurance en course | **20** | `RunDrain` (valeur brute) |
| Facteur de toucher | **20** | `ToHitFactor` |
| Facteur de blocage | **25** | `BlockFactor` |
| Points de stats / niveau | **5** | `StatPerLevel` |

### 1.1 Gains par niveau (bloc « The following are in fourths » — valeurs stockées en quarts)

| Colonne | Brut | Réel (brut ÷ 4) |
|---|---|---|
| `LifePerLevel` | 8 | **+2 vie / niveau** |
| `LifePerVitality` | 16 | **+4 vie / point de vitalité** |
| `StaminaPerLevel` | 4 | +1 endurance / niveau |
| `StaminaPerVitality` | 4 | +1 endurance / vit |
| `ManaPerLevel` | 4 | +1 mana / niveau |
| `ManaPerMagic` | 4 | +1 mana / énergie |

Formules simplifiées retenues par le moteur (vérifiées contre les valeurs connues des 7 classes :
Barbare 55, Paladin/Druid 55, Amazon/Assassin 50, Necromancer 45, Sorceress 40) :
`vie(niv 1) = hpadd + vitalité` et `mana(niv 1) = énergie + 5`.

### 1.2 Équipement de départ (colonnes `item1..item3` de charstats)

| Colonne | Item | Table source | Valeurs citées |
|---|---|---|---|
| `item1` = `hax` | Hand Axe | `patch_weapons.txt` | `mindam`=3, `maxdam`=6, `wclass`=`1hs`, `cost`=170 |
| `item2` = `buc` | Buckler | `patch_armor.txt` | `minac`=4, `maxac`=6, `reqstr`=12, `cost`=68 |
| `item3` = `hp1` | Lesser Healing Potion | `patch_misc.txt` | `cost`=30 |
| (+ `tsc`/`isc`) | parchemins Identify/Town Portal | `patch_misc.txt` | — |

### 1.3 Animation WALK

`assets/dcc/BAWL1HS.COF` (source `data\global\chars\ba\cof\bawl1hs.cof`, d2char.mpq) déclare 9 couches
(`TR, LG, LA, RA, S1, S2, HD, SH, RH`). 7 couches sont livrées dans `assets/dcc/` (§7) ;
`SH` et `RH` : **FALLBACK** (aucun `baSH*/baRH*WL*.dcc` trouvé par hash dans d2char.mpq — la marche
sans bouclier n'en a pas ; l'ordre de dessin par frame reste dans le COF).

---

## 2. Monstres (Acte I, difficulté Normal)

Source : `assets/patch_monstats.txt` (valeurs de colonnes brutes, non scalées — voir §2.1).

> **Correction de codes** : dans cette table v1.12, Zombie = `ZM` et Quill Rat = `SI`
> (le brief mentionnait `Z`/`B`, codes qui n'existent pas dans `monstats.txt` — vérifié par lecture de colonne `Code`).

| Colonne monstats | Fallen `fallen1` | Zombie `zombie1` | QuillRat `quillrat1` |
|---|---|---|---|
| `Level` | 1 | 1 | 1 |
| `minHP` | 21 | 101 | 21 |
| `maxHP` | 61 | 181 | 81 |
| `AC` | 84 | 84 | 84 |
| `A1MinD` | 51 | 51 | 51 |
| `A1MaxD` | 101 | 151 | 101 |
| `A1TH` (toucher) | 101 | 101 | 101 |
| `Exp` | 61 | 111 | 71 |
| `Velocity` (marche) | 5 | 1 | 3 |
| `Run` | 5 | 3 | 3 |
| `TreasureClass1` | Act 1 H2H A | Act 1 H2H A | Quill 1 |
| `MonSound` | fallen | zombie | quillrat |

`manifest.json` → `monsters[].stats` = ces colonnes brutes (`hp`=`maxHP`, `hpMin`=`minHP`,
`dmgMin/Max`=`A1MinD/MaxD`, `th`=`A1TH`, `speed`=`Velocity`, `runSpeed`=`Run`).

### 2.1 Scaling par niveau — `assets/MonLvl.txt` (extrait de `patch_d2.mpq`, `data\global\excel\monlvl.txt`)

En 1.10+, l'AC/TH effectifs des monstres viennent de MonLvl (les colonnes AC/TH de monstats sont
vestigiales — d'où le 84/101 identique sur les 3 monstres). Ligne **level 1 (Normal)** :

| Colonne MonLvl | Valeur | Interprétation |
|---|---|---|
| `AC` | 6 | AC effectif d'un monstre niveau 1 |
| `TH` | 8 | toucher effectif |
| `HP` | 7 | facteur ×7/100 appliqué à `minHP..maxHP` |
| `DM` | 2 | facteur ×2/100 appliqué aux dégâts |
| `XP` | 30 | facteur ×30/100 appliqué à `Exp` |

Exemple : Fallen niveau 1 ⇒ HP ≈ `21..61 × 0.07` = **2..4 PV**, dégâts ≈ `51..101 × 0.02` = **1..2**.
Phase 2 peut consommer les valeurs brutes du manifest puis appliquer ces facteurs (`level` = 1).

### 2.2 Animations WALK (sondées par hash, extraites, SHA vérifiés)

| Monstre | COF (couches déclarées) | Couches livrées | FALLBACK |
|---|---|---|---|
| FA | `FAWLHTH.COF` (4 : TR S1 SH RH) | TR, S1 | SH, RH |
| ZM | `ZMWLHTH.COF` (8 : HD LA LG RA TR S3 S1 S2) | HD, TR, LG, RA, LA, S1, S2 | S3 |
| SI | `SIWLHTH.COF` (1 : TR) | TR | — |

Le champ `walk.missingLayers` du manifest liste les couches COF sans DCC trouvé (ne pas inventer) ;
l'engine les ignore et dessine les présentes selon la priorité par frame du COF.

---

## 3. Formule de dégâts simplifiée (colonnes citées)

**Toucher** — attaquant (colonne `A1TH` monstre / `ToHitFactor`+dex héros) vs défenseur (`AC` MonLvl/monstre, `dex/4`+armure héros) :

```
chance_toucher_% = 200 × TH_att / (TH_att + AC_def)      bornée [5, 95]
```

**Dégâts** :

```
dégâts = rand(dmgMin, dmgMax)          # monstre : A1MinD/A1MaxD × MonLvl.DM/100
dégâts héros = floor(rand(mindam, maxdam) × (1 + force/100))    # arme hax : rand(3..6) × 1.30 à 30 force
```

**Bloc/Bouclier** (colonne `BlockFactor` héros = 25, `block` buckler) : à implémenter Phase 2+
(formule complète : `bloc% = Block × (dex − 15) / 150`).

Défense du héros niveau 1 : `dex/4` (≈5) + rand(`minac`..`maxac` des pièces, buckler 4..6).

---

## 4. XP par niveau — `assets/patch_experience.txt` (colonne `Barbarian`, 5 premières lignes)

| Niveau | XP cumulés requis |
|---:|---:|
| 1 | 500 |
| 2 | 1 500 |
| 3 | 3 750 |
| 4 | 7 875 |
| 5 | 14 175 |

(`MaxLvl`=99 ; `ExpRatio`=1024 — identique pour les 7 classes aux premiers niveaux.)

## 5. Drops — `assets/TreasureClassEx.txt` (chaîne réelle, probabilités citées)

**Fallen et Zombie → `Act 1 H2H A`** (Picks=1, NoDrop=100) :

| Entrée | Probabilité |
|---|---:|
| (rien — NoDrop) | 100 |
| `gld` (or) | 21 |
| `Act 1 Equip A` | 16 |
| `Act 1 Junk` | 21 |
| `Act 1 Good` | 2 |

⇒ P(rien) = 100/160 = **62,5 %** ; P(or) = 21/160 = 13,1 % ; P(objet) = 39/160 = 24,4 %.

**Quill Rat → `Quill 1`** (Picks=1) : NoDrop=125, `gld`=19, `Act 1 Junk`=19 ⇒ P(rien) = 125/163 ≈ **76,7 %**.

**`Act 1 Junk`** (Picks=1) : `Potion`=8, `Misc 0`=4, `Ammo`=4.
**`Potion 1`** (Picks=1) : `hp1`=12, `hp2`=4, `mp1`=5, `mp2`=4, `rvs`=1, `vps`=4.

### 3 items possibles (colonnes `patch_misc.txt`)

| Code | Nom (colonne `name`) | `cost` | `rarity` |
|---|---|---:|---:|
| `hp1` | Lesser Healing Potion | 30 | 1 |
| `mp1` | Lesser Mana Potion | 60 | 1 |
| `gld` | gold | 0 | 1 |

---

## 6. Niveau — Camp des Rousses (Town)

Source : `assets/levels_exp.txt` (levels.txt), ligne `Act 1 - Town` (`Id`=1) ; nom fr-FR officiel :
**Camp des Rousses** (Rogue Encampment). Fichiers (voir `assets/map/MANIFEST.json`) :

- `assets/map/TownN1.ds1` — le DS1 référence **8 tuiles .tg1** ; les 8 DT1 livrés leur correspondent
  une à une (vérifié par re-lecture de la liste interne du DS1) : `town_floor`, `town_fence`,
  `town_objects`, `out_treegroups`, `out_stonewall`, `out_river`, `out_objects`, `warp`.
- `assets/map/pal_act1.dat` — palette Acte I, **767 octets** (à padder à 768 au chargement).

Grille : `SizeX`=56 × `SizeY`=40 tuiles, `LevelType`=1, `Portal`=1.

## 7. Inventaire des assets du manifest — VERIFIÉ vs FALLBACK

**VERIFIÉ = 21 sprites/COF + 10 map + 13 sfx + 16 tables, FALLBACK = 5 couches DCC.**

Sprites/COF (21) — chacun : SHA-256 local == SHA-256 réextrait de l'archive ce jour :

| Fichier local | Taille | Archive source | Chemin MPQ |
|---|---:|---|---|
| BAWL1HS.COF | 1 269 | d2char.mpq | `data\global\chars\ba\cof\bawl1hs.cof` |
| BAWLHD/TR/LG/LA/RA/S1/S2.dcc | 9 921 / 30 895 / 52 294 / 20 813 / 20 303 / 5 846 / 7 081 | d2char.mpq | `data\global\chars\ba\<L>\ba<L>LITWLHTH.dcc` |
| FAWLHTH.COF | 394 | d2data.mpq | `data\global\monsters\FA\COF\FAWLHTH.COF` |
| FAWLTR.dcc / FAWLS1.dcc | 37 037 / 3 516 | d2data.mpq | `data\global\monsters\FA\TR\FATRLITWLHTH.dcc` / `FA\S1\FAS1LITWLHTH.dcc` |
| ZMWLHTH.COF | 880 | d2data.mpq | `data\global\monsters\ZM\COF\ZMWLHTH.COF` |
| ZMWLHD/TR/LG/RA/LA/S1/S2.dcc | 3 377 / 12 149 / 24 224 / 6 117 / 6 226 / 3 454 / 3 620 | d2data.mpq | `data\global\monsters\ZM\<L>\ZM<L>LITWLHTH.dcc` |
| SIWLHTH.COF | 118 | d2data.mpq | `data\global\monsters\SI\COF\SIWLHTH.COF` |
| SIWLTR.dcc | 31 893 | d2data.mpq | `data\global\monsters\SI\TR\SITRLITWLHTH.dcc` |

Nouveaux ce passage : `BAWLLG.dcc` (couches du COF barbare complétées) + tout le ZM (8 fichiers) + le SI (2 fichiers) ;
FA ré-extrait et re-vérifié (2 DCC + COF identiques).

**FALLBACK (5)** — couches déclarées par un COF sans DCC trouvé par hash (formes `<C><L>{LIT,MED,HVY}WLHTH` testées) :
`FA: SH, RH` · `ZM: S3` · `BA: SH, RH`.

Map (10) : TownN1.ds1 + 8 DT1 + pal_act1.dat — déjà documentés `assets/map/MANIFEST.json`, référencés tels quels.
SFX (13) : `assets/sfx/*.wav` — inchangés (mission précédente) ; le manifest les mappe
(`die` → `fallen_die.wav` ; variants par monstre sous `sfx.monsters`).
Tables : 16 citées ci-dessus (14 `patch_*.txt` + `TreasureClassEx.txt` + `levels_exp.txt`) + `assets/MonLvl.txt`
(extrait ce jour de `patch_d2.mpq` — `data\global\excel\monlvl.txt`, 14 707 o).

### Méthodo

- Sondage par hash : `engine-rs/target/release/d2tool probe <mpq> <chemin>` (exit 0 = hit).
- Extraction : `tools/stormlib_read.py` (StormLib — les DCC monstre sont en compression `0x74`
  que le mini-décompresseur Python/Rust DCL ne couvre pas encore).
- Layout découvert : monstres = `data\global\monsters\<CODE>\COF\<CODE><MODE><WC>.COF` et
  `data\global\monsters\<CODE>\<COUCHE>\<CODE><COUCHE><ARMURE><MODE><WC>.dcc` (token armure `LIT`) ;
  héros = `data\global\chars\<CL>\cof\<cl><mode><wc>.cof` et `...\chars\<CL>\<COUCHE>\<cl><COUCHE>LIT<MODE><WC>.dcc`.
- Le `(listfile)` des MPQ est tronqué (102 o — piège connu) : tous les noms ont été résolus par hash.
