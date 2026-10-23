# Implémentation rendu — variation seedée (#9) + occlusion 2 couches (#16)

Date : 2026-10-22 · Périmètre : `engine-rs/crates/d2game/src` uniquement
(`compose.rs`, `game.rs`, `wasm.rs`) · Référence : `od2-features-moteur.md`
lignes #9 (`map_tile.go:85 getRandomTile`) et #16 (`renderer.go:231-358`,
passes 2/3).

## 1. Variation seedée des tuiles (feature 9)

* `compose::compose_seeded(ds1, tilesets, seed)` — `compose(ds1, tilesets)`
  est conservé (graine 0) pour `tests/proof.rs`, dont la signature ne change
  pas. `game.rs::load_level` passe `fnv1a(clé du niveau)` : même niveau ⇒
  même carte, indépendant de l'historique de combat.
* Le LCG historique du spawn (`LevelRng`, constantes inchangées) est déplacé
  dans `compose.rs` (`compose::Lcg`) et partagé ; le stream du spawn reste
  bit-identique (instance séparée pour les tuiles).
* Règle de tirage : **tuiles SOL uniquement**, parmi la fenêtre famille
  `p1..=p1+15` (l'espace adressable par la variante `p2 & 0x0F`) — gardes :
  même nature (`is_floor`) et **même signature de flags de sous-blocs** que
  la tuile résolue du DS1, afin que la grille de collision (calculée sur la
  tuile déterministe) ne soit jamais contredite par la variante peinte.
  Les murs ne sont jamais re-tirés (structurels : chaque index est une pièce
  gauche/droite/coin). Un seul tirage par cellule peinte, ordre de parcours
  fixe ⇒ déterministe.

## 2. Occlusion — murs hauts en 2e couche (feature 16)

* `Dt1Tile` « haut » = au moins un bloc avec `b < 0` portant des pixels réels
  (contenu au-dessus du losange de la cellule : arbres, clôtures, falaises).
  Test **structurel** — les octets d'orientation DS1/DT1 sont non fiables en
  v1.12 (les arbres du Camp portent `orient == 0`), cf. en-tête `dt1.rs`.
* `compose.rs` remplit un 2e tampon d'indices `ComposedMap::overlay` (même
  géométrie que `idx`) : murs hauts routés là, sols et murs bas dans la base.
  Ordre de peinture interne conservé (rangées `cy`, puis `cx`).
* `game.rs::render` : 1) base (LUT) → 2) ombres + entités triées par `y` →
  **3) overlay blitté par-dessus** (`blit_index_layer`, index 0 transparent).
  Un mur masque désormais le héros qui passe derrière — équivalent simplifié
  des passes 2/3 de Go (pas de buckets par tuile : une entité juste devant
  un mur haut reste sous la canopée qui la surplombe, arbitrée par le y-sort
  seul ; suffisant pour le MVP, cf. limites).
* `ComposeStats.tall` compte les tuiles routées ; `composeStats` WASM
  ajoute un 5e élément **append-only** (le destructuring JS
  `[drawn, hidden, , oob]` de `demo/game.js` reste compatible).

## 3. Tests

`cargo test --workspace` : **144 passed / 0 failed** (109 à l'audit ; les
autres chantiers ont ajouté les leurs entre-temps). d2game lib : 38, dont :

* `compose::tall_wall_overlays_entity_cell` — le test demandé : une cellule
  derrière un mur haut reste peinte en couche 1 (visible à l'ancienne) mais
  est **recouverte en couche 2** (`overlay` au même pixel) ;
* `compose::short_wall_stays_in_base`, `compose::walls_are_never_rerolled` ;
* `compose::variation_is_deterministic_per_seed` (même graine ⇒ même carte,
  graines différentes ⇒ cartes différentes),
  `variation_never_picks_out_of_window_or_kind` (garde nature + flags),
  `explicit_variant_still_respected_without_candidates` (candidat unique ⇒
  la variante du DS1 fait foi, sans RNG) ;
* `game::overlay_blit_covers_entities_only_where_walls_are` (le blit
  n'écrase que les pixels du mur) ; `full_pipeline_smoke` étendu (overlay du
  Town peint, `tall > 300`).

## 4. Vérification navigateur

Build WASM release régénéré vers `demo/game-pkg` (wasm-bindgen, cible web).
`game.html` servi en local : 59-60 fps, zéro erreur (`window.__d2.errors`
vide, titre sans ⚠). Preuves :

* `demo/occlusion-spawn-e2e.png` — au spawn (cœur de la forêt) le héros est
  **masqué** par la canopée au sud de lui (avant : dessiné par-dessus tout) ;
* `demo/occlusion-e2e.png` — héros en lisière : ses jambes sont couvertes
  par les arbres en avant, le buste reste visible au-dessus du feuillage.

## 5. Note hors périmètre initial

En cours de route, les tests de transfert de héros (`switch_to_bloodmoor`,
`level_switch_carries`) cassaient : le chantier parallèle d2rules vient de
passer le format de save à `version: 2` (`d2rules/src/save.rs`). Correction
dans d2game (`patch_save_for_level`) : accepter v1 **et** v2 (les champs
patchés existent dans les deux ; d2rules charge les deux).

## 6. Limites connues / suite

* Pas de buckets par (tuile, sous-tuile) comme Go : une entité devant un mur
  haut mais sous sa canopée est couverte — l'approximation 2 couches choisie.
* L'overlay double la RAM du tampon d'indices (≈ 34 Mo/carte) — acceptable
  aux tailles actuelles, à revoir pour de grandes cartes.
* La variation ne s'applique pas encore aux arbres (volontaire : structurels
  ; Go l'obtient par style/séquence exacts, métadonnées non décodées ici).
