# finish-jeu — La liste de ce qu'il manque pour FINIR LE JEU (2026-10-22)

Council `finish-d2` : 3 analystes (monde/monstres, systèmes héros, UX/finitions).
**Cible** : boucle PvE solo complète — 5 actes × 3 difficultés, 7 classes, création
du perso → mort de Baal, waypoints, quêtes actables, loot D2 fidèle, en navigateur.
**Hors-scope flaggé** : multi-joueurs, cinématiques Bink, vaches secrètes
(hirelings : données dispo, flaggé optionnel).

Rapports détaillés (preuves fichier:ligne, tailles, dépendances) :
- `finish-monde.md` — 20 items (W01-W20)
- `finish-systemes.md` — 20 items (A1-K20)
- `finish-experience.md` — 49 items (EXP-*, dont **1 BUG P0**)

**Total : ~89 items** — tailles S ≤ 1 j · M 2-4 j · L 1-2 sem · XL > 2 sem (solo).

## La liste condensée (chemin critique consolidé)

**P0 — bloquant immédiat**
1. **BUG Continuer** : la save est quasi inutilisable depuis le menu (clés/paramètres,
   clé de niveau absente du save `detectSaveLevel`) + index multi-slots (EXP-S1/S3/E1).

**P1 — socles data-driven (débloquent tout)**
2. Lecteur runtime des tables de niveaux + graphe Vis/Warp générique (W01, W04).
3. `Stat/StatList` générique + parseur `CalcString` (A1, A2) — socle affixes,
   synergies, uniques.
4. Extraction + lecture TSV runtime des tables restantes : magicprefix/suffix,
   ItemRatio, ItemStatCost, skilldesc, hireling, Sets, inventory, gamble,
   DifficultyLevels (A3, A4).
5. Texte fr-FR : STRING/TBL extraites consommées, zéro chaîne codée en dur ;
   décision tranchée : DOM restera le moteur de rendu texte (EXP-T1, EXP-T2).

**P2 — Acte I complet jouable**
6. Contenu monstre data-driven : planches mon1-10 par niveau, densités, packs,
   champions bleus / uniques dorés (MonUMod), super-uniques nommés (W06-W10).
7. Levelling 1→99 ×7 classes : 5 stats + 1 skill/niveau, vit/ene/stamina,
   formules PerVit exactes (C1-C4, D1, D5) + panneau stats + ceinture 1-4 (EXP-H1/H2).
8. Arbre de compétences 210 skills : prérequis, synergies CalcString, plafond 20,
   icônes DC6 (C).
9. Équipement dérivé : objets X×Y, 8 slots, AC/AR/dmg des objets réels, toucher
   v1.12 avec HitShift (F1, F6, F7, E1, E2).
10. Loot D2 réel : ItemRatio → qualités, affixes ilvl, propriétés → stats (F2-F5).

**P3 — profondeur & monde étendu**
11. DRLG des 3 types : maze/preset-stamps/wilderness seedés en Rust (W02, W03) —
    LE plus gros bloc moteur (XL, par acte).
12. Quêtes : moteur d'états + authoring (~6/acte, pas de table dans les MPQ) (W13).
13. Chaîne de boss Andariel → Duriel → Mephisto → Diablo → Baal : records, sprites,
    arènes, mort de Baal = écran de fin (W11) — XL.
14. Villes vivantes : 5 villes, NPC + rôles (soin/vente/réparation/waypoint) (W14, W16).
15. Vendors/gamble/cube horadrim/sockets/runewords (G, F8-F10) — XL.
16. Waypoints : 38-39 activables + UI + persistance (W05, W19).

**P4 — finitions & difficultés**
17. Mercenaires (4 types, hireling.txt, résurrection) (H).
18. Mort avec corps au sol, perte or, pénalité XP (J) — aujourd'hui respawn auto 2 s.
19. NM/Enfer : MonLvl (N)/(H), densités, résists négatifs, pénalités (W08, K) → Baal Hell.
20. HUD parité D2 : L/R skill, survol monstre (nom+vie), labels flippy+Alt, dégâts
    flottants, minipanel 8 boutons, stamina (EXP-H*).
21. Audio complet : musique par acte, ambiance jour/nuit, panning, 3 volumes (EXP-A*).
22. Perf/packaging : ≥50 fps grandes cartes, <500 Mo WASM, PWA offline, manifests
    par acte, écrans de chargement + interludes, crédits (EXP-P*, EXP-Q*).
23. Qualité : E2E sur le parcours complet, golden tests tables v1.12, save
    multi-slots + export/import + migration versionnée (EXP-S*, EXP-Q*).

## Definition of done globale

« Un joueur crée n'importe quelle des 7 classes, joue les 5 actes en
Normal/Nightmare/Hell avec quêtes, waypoints, loot complet (magique/rare/unique/set),
marchands, cube et mercenaire, meurt (corps au sol), reprend sa partie, tue Baal —
le tout seed-déterministe, en fr-FR issu des tables, sans éditer le code pour
ajouter du contenu. »

## Les 6 blocs XL (à piloter en premier)

DRLG (W02) · Quêtes (W13) · Boss/Baal (W11) · Skills/auras passifs (C) ·
Vendors (G) · Pack contenu/audio grand volume.
