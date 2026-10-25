# Implémentation loot — branchement TreasureClassEx (phase 2, feature #34 / D1)

Date : 2026-10-22 · Portée : `engine-rs/crates/d2rules` uniquement · Réf. Go :
`opendiablo2/d2core/d2item/diablo2item/item_factory.go:163-330`,
`d2records/treasure_class_loader.go`, `item_ratio_loader.go`.

## Ce qui est branché

* **Nouveau module `d2rules/src/loot.rs`** (pur, zéro IO) : parse TSV de
  `assets/TreasureClassEx.txt` (853 TCs réels v1.12) + tables d'items
  `patch_weapons.txt` / `patch_armor.txt` / `patch_misc.txt`, embarquées à la
  compilation (`include_str!`, même pattern que le manifest dans les tests).
* **Chaîne complète** (port `ItemsFromTreasureClass`) :
  TC (`stats.treasureClass` du manifest, ex. "Act 1 H2H A") → `Picks`/`NoDrop`
  (tirage pondéré ; `Picks < 0` = comptes par trésor) → sous-TC récursif
  (garde profondeur 16) → code d'item final. Colonnes `ItemN`/`ProbN`
  découvertes dynamiquement depuis l'en-tête (pas codées en dur).
* **Codes génériques** (`resolveDynamicTreasureCode`) : `weap33`/`mele3`/
  `bow3`/`armo3` → item concret `spawnable` de la famille avec
  `level ∈ [L, L+3)` (fenêtre od2 `dynamicItemLevelRange`), choix uniforme ;
  familles triées (level, code) ⇒ déterminisme sans dépendre de `HashMap`.
* **Or** : `gld` + variantes `gld,mul=X` (multiplicateur /1024, convention D2)
  sur base `[5×mlvl, 15×mlvl]` (héritage MVP ; le D2 réel lit `monstats`).
* **`world.rs::roll_drop`** : utilise la TC du monstre ; repli automatique sur
  les constantes MVP 60 % rien / 25 % or / 15 % `hp1` si la table est absente
  ou si la TC est vide/inconnue (manifest sans `treasureClass`).
* **`data.rs`** : `MonTpl.treasure_class` lu dans le manifest.
* **`ItemKind::Misc`** ajouté (gemmes, runes, clés, parchemins…) ; `hp1` garde
  la définition potion exacte du MVP. `save.rs` : aucun changement de format
  (tables statiques, non persistées) — contrat bit-exact inchangé.

## Déterminisme

Tirages exclusivement via `Rng::below` (SplitMix64 seedé du `World`, état déjà
sauvegardé) : même graine ⇒ même butin, vérifié par test sur "Act 1 H2H A".

## Écarts documentés (prochaines phases)

* **Qualité (unique/set/rare/magic)** : colonnes `Unique/Set/Rare/Magic` des
  TCs parsées mais non exploitées — nécessite `ItemRatio.txt` (non extrait)
  + affixes/noms (D2-D5, gameplay D1-D5 toujours MISSING). `ItemDef` ne porte
  pas encore de qualité.
* **NoDrop multi-joueurs** : NoDrop brut (= solo, parité od2) ; le scaling
  joueur classique n'est pas implémenté (pas de multi au MVP).
* **Picks uniformes** dans les équivalences (parité od2) ; le poids `rarity`
  des tables D2 n'est pas appliqué.
* `ItemDef.name` = code brut pour les items hors `hp1` (noms D2 dans les
  `.tbl`, pas encore extraits).

## Tests

`cargo test -p d2rules` : **52/52 ok** (44 préexistants + 8 nouveaux dans
`loot::tests` : parse ~853 TCs, champs "Act 1 H2H A", déterminisme au seed,
résolution concrète 200 graines, fenêtres `armo3`/`mele3`/`weap33`/`bow3`,
repli MVP, `gld,mul=2048` = ×2, picks négatifs). 1 test MVP ajusté
(`fallen_drops_potion_pickup_and_use_heals` : le drop réel peut lâcher
hp1..hp5/rejuv — l'assertion code=="hp1" est devenue kind==Potion).
Workspace : d2missiles 12, d2mpq 7, d2dc6 8 ok ; **d2game : 34/38** — les 4
échecs (level-switch, sprite, compose) sont le chantier concurrent en cours
dans d2game, hors périmètre de cette passe.
