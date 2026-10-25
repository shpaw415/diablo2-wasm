# impl-integration — consolidation des 3 chantiers XL (2026-10-22)

> Les 3 membres gros-morceaux (DRLG/niveaux, règles/skills, items/loot) ont
> livré en parallèle ; l'arbre final a été consolidé par l'intégrateur +
> le coordinateur. Résultat : **237 passed / 0 failed / 1 ignored**
> (`cargo test --workspace`, baseline 143 au démarrage des gros morceaux).

## Les 21 échecs initiaux → cause → résolution

Cause racine : hypothèses divergentes des 2 membres d2rules vs la vraie table
(5 vrais bugs de code), vérifiées contre les MPQ (`tools/stormlib_read.py`,
priorité `engine/patch_d2.mpq`). Convention retenue : **les tables `assets/`
font foi** ; goldens alignés sur le modèle typé (vide numérique ⇒ 0).

| # | Test(s) | Cause | Résolution |
|---|---|---|---|
| 1 | uniques_sets ×3 | compte sets 32 vs 33 (ligne marqueur « Expansion »), base Gnasher, raretés | table fait foi : 33 sets, Gnasher=hax, raretés réelles 1/3/7 |
| 2 | affixes ×3 | comptes + « of Health » = red-dmg 1-1 (pas hp), itype arme non couvert | 638/742 réels ; red-dmg ; filtre itype |
| 3 | calc ×1 | pas de pare-feu EOF (`"(5+3)x"` passait) | token résiduel = erreur ; seule tolérance Go = `)` finale en EOF |
| 4 | inventory ×3 | bras `StatProp::Ac` manquant (AC 6 vs 11) ; belt colonne-d'abord ; tailles hax/buc | bras rétabli ; rangée-d'abord (pot0→3) ; tailles réelles 1×3 / 2×2 ; **+ fix compile `*v as i64` (deref `&i32`, inventory.rs:468)** |
| 5 | item_quality ×2 | facteur TC 1024 ≠ garantie qualité (sature au plancher Min ≈ 2 %) — ancien test contredisait Diablerie ; nombres folklore 192/12 vs table réelle 12/8 | algorithme Diablerie respecté ; **ItemRatio v1.12 a une colonne `Function` en tête : aligné en-tête, HiQuality=12/8, Normal=2/2** (test + fallback corrigés, commentés) |
| 6 | loot ×3 | tailles de base inventées ; uniques sans clé gardaient quality=Unique | tailles réelles hax 1×3 / buc 2×2 / gld 1×1 ; dégradation → Rare |
| 7 | parity_tests ×4 | goldens générés avec hypothèses (vides, ELen brut, décalage XP ligne/niveau) | goldens régénérés (7210+2514+156 cellules) ; ELen = vraie formule ELevLen par paliers ; XP via `raw_row` (500 = ligne 1) |
| 8 | skills_tree ×2 | points non épuisés ; SkillPage firebolt | `grant_points(1)` réel ; Feu = page 1 |

## Bugs de compilation résiduels au handoff

- `inventory.rs:468` : `v as i64` sur `&i32` (E0606) → `*v as i64` (coordonateur).
- Re-run complet jamais exécuté par l'intégrateur (limite de steps atteinte
  après diagnostic 100 % + fixes) → workspace revérifié par le coordinateur :
  vert.

## Décisions consignées

1. Tables `assets/` = extraites de `patch_d2.mpq` = autorité absolue.
2. Goldens régénérés depuis les tables, jamais édités à la main vers le vert.
3. `elen` suit la vraie formule D2 (ELevLen 1-3 par paliers), pas le brut.
4. Facteur TC 1024 sature au plancher `Min` — ne garantit pas la qualité.
5. ItemRatio v1.12 : colonne `Function` ⇒ HiQuality=12/8, Normal=2/2.

## Rapports des chantiers

- `impl-drlg.md` (DRLG/niveaux, W01-W04) — livré par son membre.
- `impl-regles-skills.md` (A1-A2-C-D) — rédigé d'après le code + handoff.
- `impl-items-loot.md` (F1-F7) — idem.
