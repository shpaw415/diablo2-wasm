# Council `d2-parite` — 4 chantiers intégrés (2026-10-23)

Council de 4 membres en arbres séparés (`d2w-*`), fusion séquentielle dans le
miroir git (branches `member-*` → merges, tests à chaque étape), câblage
front-end complété par l'intégrateur. **État final : `cargo test --workspace`
= 305 passed / 0 failed / 1 ignored** (baseline 254 → +51 tests).

## 1. Save P0 — clé de niveau + multi-slots (`d2w-p0-save`)

- `d2rules/src/save.rs` : format **v3** avec la clé du niveau courant
  (cohérente avec les warps multi-niveaux Town↔Blood Moor) ; rétro-compat
  v1/v2 testée (défauts sensés).
- `demo/slots.js` (nouveau) : **5 slots nommés** identités par héros dans
  `localStorage["d2wasm.slots.v1"]`, miroir de la clé historique
  `d2wasm.save.v1` (les liens `?continue=1` survivent), `?slot=<id>` pour
  reprendre un slot précis, `D2Slots.upsertFromSave` à chaque sauvegarde.
- `demo/menu.html/css/js` : écran « Parties » (reprendre, supprimer,
  **export/import .json**), « Continuer » reprend le slot le plus récent.
- Tests : round-trip v3, chargement v1/v2, clé de niveau préservée
  (`d2rules/src/tests.rs` +78 lignes).

## 2. HUD parité D2 — EXP-H* (`d2w-hud`)

- `demo/hud.js` + `demo/hud.css` (nouveaux) : boutons skills **L/R**
  (clic droit = compétence main droite), **survol monstre** (nom + vie),
  **labels d'objets au sol** (Alt), **dégâts flottants**, **minipanel
  8 boutons** (Menu Inv Stats Skills Quêtes Carte Aide Son), routage des
  inputs via `window.__d2.queue`.
- `d2rules/src/stamina.rs` (nouveau) : endurance (drain/régén) + champ
  save additif ; barre verte à côté du globe de vie (E2E captures).
- Câblage minimal commenté dans `game.js` (`D2HUD.events/update/frame`)
  et `game.html` (script tag).

## 3. Waypoints W05/W19 (`d2w-waypoints`)

- `d2rules/src/waypoints.rs` (nouveau) : table réelle v1.12 (colonne
  `Waypoint` de `patch_levels.txt` + positions DS1/DRLG), activation au
  contact (event `WaypointActivated`), voyage direct seedé ;
  `d2game/src/waypoint_travel.rs` (nouveau) : `travelWaypoint(slot)` avec
  régénération seedée du niveau cible.
- **41 waypoints A1→A5** réels listés dans le panneau (`demo/waypoints.js`,
  nouveau ; Tab ou bouton « Waypoints »).
- Save : champ additif `waypoints` (rétro-compat, pas de bump de version).
- Outil : `tools/ds1_objects.py` + table `objracts.txt` extraite
  (`assets/objects.txt`, BYO non publié).

## 4. NM/Enfer W08/K (`d2w-difficultes`)

- `d2rules/src/difficulty.rs` (nouveau) : `Difficulty {Normal, Nightmare,
  Hell}` + colonnes **MonLvl (N)/(H)** (`patch_monstats.txt`, `MonLvl.txt`),
  multiplicateurs AC/TH/HP/DM/XP par difficulté, résists (N)/(H) avec
  immunités, pénalités. Chaque valeur tracée vers sa colonne source.
- Spawns data-driven par difficulté (`d2game` : levels/monsters/world) ;
  plan Normal = plan historique EXACT (même seed).
- UI : `?diff=nm|hell` → badge « Difficulté : Hell » + `difficultyInfo()`.
- Save : champ additif `difficulty`.
- Note d'intégration : le test initial supposait « aucune résist en
  Normal » — faux : zombie1 a **ResPo(N)=50** réel v1.12 (colonne ResPo).
  Test corrigé pour refléter la table (le parseur était juste).

## Intégration (fusion des 4 arbres)

- Merges git : `member-p0-save` → `member-hud` → `member-waypoints` →
  `member-difficultes` ; conflits résolus en union (game.html scripts,
  doc `save_world` v3 + bloc API HUD, champs save `waypoints` +
  `difficulty`).
- Câblage complété par l'intégrateur (membres interrompus avant la fin) :
  `game.html` (script `waypoints.js`, bouton, styles du panneau, badge
  difficulté), `game.js` (`?diff` → `setDifficulty` avant `finalize`,
  badge via `difficultyInfo()`, `D2Waypoints.init/refresh` dans la boucle).
- WASM reconstruit depuis l'arbre fusionné (305 tests verts) →
  `demo/game-pkg/*` (d2game.d.ts +2,4 Ko : `setDifficulty`,
  `difficultyInfo`, `waypoints`, `travelWaypoint`).
- E2E navigateur (port 8090, zéro erreur bloquante) :
  `demo/council-waypoints-e2e.png` (panneau 41 waypoints, 57 fps),
  `demo/council-hell-e2e.png` (badge Hell + minipanel + stamina),
  `demo/council-slots-e2e.png` (écran Parties + import/export).

## Limites connues

- Le voyage waypoint exige une pierre découverte (contact) — le panneau
  liste les non-découverts en grisé (comportement D2).
- Libellés FR des waypoints provisoires (TBL FR absentes — W18).
- Sons de découverte/voyage : uiClick générique.
