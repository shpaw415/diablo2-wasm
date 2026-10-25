# Synthèse parité OpenDiablo2 → diablo-2-wasm (2026-10-22)

Council `od2-parity` : 3 auditeurs (formats, moteur, gameplay) + 4 implémenteurs.
Référence : OpenDiablo2 (réimplémentation Go) clonée dans `~/workspace/opendiablo2`.

## Résultat de l'audit (avant phase 2)

122 fonctionnalités comparées : **20 FULL · 54 PARTIAL · 36 MISSING · 12 N-A**.

| Domaine | Rapport | FULL | PARTIAL | MISSING | N-A |
|---|---|---:|---:|---:|---:|
| Formats | `od2-features-formats.md` | 11 | 16 | 7 | 2 |
| Moteur/core | `od2-features-moteur.md` | 4 | 15 | 7 | 2 |
| Gameplay/réseau/app | `od2-features-gameplay.md` | 5 | 23 | 22 | 8 |

Le port dépasse OpenDiablo2 sur : A* (leur pathfinding = raycast), save
déterministe bit-exact, bzip2 MPQ, collision missiles, IA monstres, warps
multi-niveaux. N-A assumés (hors scope navigateur) : réseau TCP/UDP
multi-joueurs, scripting embarqué (le navigateur EST le moteur JS), layout
engine (CSS), streaming MPQ.

## Phase 2 — manques P1 comblés (4 chantiers, tous testés)

1. **AnimData.d2 → crate `d2animdata`** (`impl-animdata.md`) : port byte-faithful
   du parseur Go ; vrai fichier v1.12 extrait (3 558 records, parité record à
   record avec un dump Python indépendant ET le testdata du repo Go) ; branché
   sur les sprites avec repli COF garanti. Résout formats #19 (MISSING n°1).
2. **Loot TreasureClassEx réel** (`impl-loot.md`) : `d2rules/src/loot.rs`, chaîne
   complète TC → Picks/NoDrop → récursion → codes dynamiques (weap33…), or
   `gld,mul`, déterminisme au seed, repli MVP conservé. 853 TCs réels. Résout
   gameplay D1 au niveau MVP (qualités/affixes restent en phase 3).
3. **Rendu : occlusion + variation** (`impl-rendu.md`) : murs hauts en 2e couche
   (un mur masque le héros — équivalent simplifié des passes 2/3 Go, prouvé E2E
   navigateur, captures `demo/occlusion-*.png`) + variation seedée des tuiles de
   sol (fenêtre p1..p1+15, garde flags, déterministe par niveau).
4. **Compétences** (`impl-skills.md`) : `d2rules/src/skills.rs` — SOR démarre
   avec Fire Bolt (tire `firebolt`, 3–8 feu via la table `d2missiles`), BA/PA
   mêlée ; save v2 rétrocompatible v1 ; API `select_skill`. Résout gameplay A6
   au niveau départ de classe.

**État final : `cargo test --workspace` = 143 passed / 0 failed / 1 ignored.**
La baseline d'audit (109) reste verte ; +34 tests nouveaux.

## Parité après phase 2 (approx.)

122 features : ~24 FULL · ~56 PARTIAL · ~31 MISSING · 12 N-A.
Les 4 chantiers transforment des MISSING/PARTIAL en FULL-partiel (détails dans
chaque `impl-*.md`).

## Reste (roadmap priorisée par les audits)

1. Système de stats génériques `Stat/StatList` + parseur `CalcString` (E1/B1-B3)
   — socle des affixes, uniques, synergies.
2. Panneau stats + points à répartir (A2/L7), ceinture potions 1-4 (L2).
3. DT1 : SubTileFlags par sous-tuile (collision/LOS fines) ; DS1 multi-versions
   + objets/substitutions/PATH (prérequis stamps + mapgen).
4. PL2 transforms + palshift (couleurs champions/uniques), fonts/TBL in-engine.
5. Génération d'items complète (ItemRatio, qualités, affixes), équipement 8
   slots, multi-sauvegardes, arbre de compétences, crédits, cinématiques.
6. Vitesse d'animation : consommer `d2animdata::fps()` dans la boucle d2rules
   (`FRAME_MS` constant aujourd'hui), events SFX.

Pas de commit git (non demandé) — arbre de travail propre et testé.
