# PLAN — Diablo II jouable dans le navigateur (MVP « Prairies Sanglantes »)

Objectif final : un MVP **jouable** en WASM — héros animé contrôlable, une carte réelle d'acte I
(monstre vivant, combat, loot, inventaire, sauvegarde .d2s). Tout en Rust→WASM, données du jeu
chargées côté navigateur (MPQ déjà décodables via d2mpq).

## Définition de « jouable » (critère de sortie)
- [ ] Héros (barbare) animé (marche/attaque/meurt) rendu avec les DCC du jeu, palette correcte
- [ ] Carte Prairies Sanglantes (ou town acte I) rendue en isométrique depuis DT1/DS1 réels
- [ ] Clic pour se déplacer (A* sur flags DT1), animations directionnelles
- [ ] ≥ 3 espèces de monstres animées (DCC), IA basique (errance, poursuite, attaque)
- [ ] Combat : formules tables (monstats/skills/missiles), dégâts, mort, XP, niveau
- [ ] Loot : drops TreasureClassEx, items au sol, ramassage, grille d'inventaire, équipement
- [ ] Sauvegarde/chargement .d2s
- [ ] Page demo jouable dans Chromium, zéro erreur JS, capture d'écran

## Phases et membres
**Phase 1 (parallèle)** :
- Ingénieur DCC : crates d2dcc + d2cof (animations, couches COF) + rendus de validation
- Ingénieur cartes : crate d2map (DT1/DS1, collision) + rendu PNG d'un vrai niveau
- Analyste données : GAME-DATA.md + manifest.json (chemins exacts de chaque asset MVP) + sfx

**Phase 2 (après validation phase 1)** : moteur de jeu (boucle, A*, caméra), gameplay
(combat/IA/drops/inventaire/.d2s), intégration demo jouable + E2E navigateur.

**Phase 3** : polissage (sons, UI, palettes PL2), livraison.

## Contrats techniques (tous les crates)
- Workspace engine-rs/, edition 2021, pas d'IO fichier dans les libs, erreurs style D2Error
- Ne pas casser d2mpq/d2dc6/d2d2tool existants (cargo test vert = 21 tests)
- Chaque crate : tests unitaires + validation sur fichiers réels (jamais de mock seulement)
- Preuves : toute découverte format → evidence/*.md avec offsets

## Statut
- [x] Fondation : d2mpq (7/7 SHA-256), d2dc6 (byte-identique), demo WASM, CLI d2tool
- [x] Phase 1 : DCC/COF (barbare composé validé visuellement) · Town DT1/DS1 rendu isométrique validé · manifest.json + GAME-DATA.md + 13 sfx WAV · crate d2dcc
- [x] Phase 2 : crates d2game (moteur 60 FPS) + d2rules (28 tests, save bit-exact) + collision.json + page jouable
- [x] Phase 3 : assemblage (d2rules branché dans d2game), palette corrigée, HUD + sons + save/load, E2E prouvé (demo/game-e2e.png : combat, +18/+21 xp, HUD, 60 fps)

## 🎮 Jouable : demo/game.html — servi sur :8077 (tunnel cloudflared éphémère)
