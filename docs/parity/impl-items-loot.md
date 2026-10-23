# Implémentation — items & loot complets (gros-morceaux, F1-F7)

> 2026-10-22. Chantier XL de `finish-systemes.md` (items F). Suite directe de
> `impl-loot.md` (phase 2 : chaîne TreasureClassEx). Modules dans
> `engine-rs/crates/d2rules/src/`, données réelles v1.12 embarquées.
> Réf. algorithme qualité : Diablerie `Engine/ItemDrop.cs`
> `TestQuality`/`SelectItemQuality`, aligné sur od2 `item_factory.go:163-330`.

## Livré

### `item_quality.rs` — qualités via ItemRatio.txt v1.12 (F2)
Tirage de qualité d'un drop : unique/set/rare/magic/hi-quality/normal avec
facteurs /8-/2-, `qlvl`/`ilvl`, Magic Find, plafond colonne `Min`. IMPORTANT
(table fait foi) : le fichier v1.12 extrait a une colonne `Function` en tête ;
aligné sur l'en-tête, HiQuality = **12/8** et Normal = **2/2** (les valeurs
« folklore » 192/12 et 8/2 décrivent un autre arrangement de colonnes —
voir `impl-integration.md`).

### `affixes.rs` — affixes MagicPrefix/Suffix v1.12 (F3)
**638 prefixes / 742 suffixes réels** (exact v1.12). Filtres ilvl/spawnable/
version, max 1 prefix + 1 suffix, choix pondéré, exclusivité des groupes,
« of Health » = red-dmg 1-1 (la table fait foi). Rare = les deux + flag.

### `item_props.rs` — descripteurs de propriétés (F4)
Port de `property_descriptor.go` : chaque `propN/parN/minN/maxN` des tables
devient un descriptor typé `(prop, params, plage roulée)` — consommé par la
`StatList` du socle règles (`impl-regles-skills.md`).

### `uniques_sets.rs` — uniques & sets identifiés (F5)
**33 sets** (le fichier réel contient la ligne marqueur « Expansion »),
uniques résolus vers leur base (The Gnasher = hax/Hand Axe), raretés réelles
SetItems (1/3/7) vérifiées par test de distribution (part ≈ 7/8).

### `inventory.rs` — item riche + grille + équipement (F1/F6/F7)
`Item` complet (code, qualité, affixes, uniques/sets, props roulées, taille
X×Y réelle : hax 1×3, buc 2×2, or 1×1), `canFit` grille 10×4, **8 slots
d'équipement**, ceinture 4×4 (remplissage rangée-d'abord, D2 réel :
pot0→pot3 puis colonnes), stats dérivées AC/dmg des objets portés
(`apply()` — le bras `StatProp::Ac` manquant rétabli, AC 6→11).

### Branchement loot
`loot.rs` : roll TCE → qualité (ItemRatio) → affixes → descriptors ;
dégradation unique/set sans clé → `Quality::Rare` (jamais d'unique sans
propriétés). Or 1×1. Toujours déterministe au seed (`Rng` du World).

## Tests

+25 nouveaux (8+8+4+5+6), non-régression totale : **237 passed / 0 failed**
(workspace). Goldens sur nombres réels ItemRatio, paliers affixes ilvl 1 vs
30, distribution raretés, AC dérivée.

## Écarts / restant

- Noms rares (rareprefix/suffix) et libellés fr : TBL non extraites (EXP-T1).
- Colonne class-specific : ligne de classe non sélectionnée (écart documenté).
- LeftArm od2 vs gants D2 : mapping slots à harmoniser avec l'équipement HUD.
- Une seule difficulté (facteurs NM/Enfer d'ItemRatio à brancher en P4).
- Cube/runewords/sockets : F8-F10, prochaine passe.
