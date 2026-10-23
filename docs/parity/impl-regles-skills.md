# Implémentation — socles règles + skills (gros-morceaux, A1-A2-C1-C4-D1-D5)

> 2026-10-22. Chantier XL de `finish-systemes.md`. Modules dans
> `engine-rs/crates/d2rules/src/` — API pures, câblage `world.rs` réservé à
> l'intégration. Données réelles v1.12 embarquées (`include_str!` depuis
> `assets/`).

## Livré

### `stats.rs` — Stat/StatList génériques (socle A1)
Port de `opendiablo2/d2core/d2stats/diablo2stats/{stat,statlist,stat_value}.go`.
`StatList` typée par id, alimentée par `ItemStatCost.txt` comme dictionnaire
(id ↔ nom, bits, champs de description). API `add/get/agrégation` — consommée
par les propriétés d'items (`item_props.rs` → stats).

### `calc.rs` — parseur/évaluateur CalcString (socle A2)
Port direct de `d2common/d2calculation` + `d2lexer` + `d2parser` Go. Lexer,
parser, évaluateur des colonnes `*Calc` de skills.txt (synergies, ToHitCalc,
passives, auras). Pare-feu EOF ajouté à l'intégration : `CalcString::parse`
refuse un token résiduel (`"5+3)x"`, `"(5+3))"` = erreur) ; seule tolérance
Go conservée = parenthèse finale non fermée en EOF. ~23 formules réelles de
skills.txt en golden tests.

### `skills_tree.rs` — arbre complet (socle C1-C4)
210 skills de classe (+ Attack/Kick + skills monstres) depuis le vrai
`skills.txt` v1.12 : prérequis (`reqskill1-3`), `reqlevel`, plafond `maxlvl`
(20 pour les skills de classe), missiles, synergies évaluées via `calc.rs`.
API `SkillTree::learn/can_learn/synergy_factor` + points (1/niveau de perso,
`grant_points(1)`, branche NoPoints testée). SkillPage réel (Feu = page 1).

### `xp.rs` — XP 1→99 × 7 classes (socle D1/D5)
`patch_experience.txt` complet : 99 niveaux × 7 colonnes + MaxLvl + ExpRatio.
Sémantique réelle : ligne N = XP cumulés pour atteindre N+1 (ligne 1 = 500).
Golden record-à-record via `raw_row(lvl)` (l'accesseur documenté ; le seuil
500 est la ligne 1 — xp.rs avait raison, le golden initial s'était trompé).

## Tests

+27 nouveaux (stats 5, calc 8, skills_tree 9, xp 5), goldens régénérés depuis
les tables réelles (`golden_skills.txt` 7210 cellules vides→"0",
`golden_isc.txt` 2514, `golden_dmg_curve.txt` 156 cellules `elen` recalculées
avec la vraie formule ELevLen par paliers). Arbre final : 237 passed /
0 failed (workspace complet).

## Écarts / restant

- Libellés fr des skills (skilldesc → TBL fr-FR) — dépend EXP-T1.
- Câblage `world.rs` : le hero doit lire ses stats dérivées via `StatList`
  (intégration), dégâts firebolt via `synergy_factor`.
- Passifs/auras évalués par tick : à brancher quand le MVP est câblé.
