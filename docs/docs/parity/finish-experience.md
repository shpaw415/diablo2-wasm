# Audit EXPÉRIENCE & FINITIONS — « finir le jeu » (diablo-2-wasm, 2026-10-22)

- **Périmètre** : tout ce qui sépare l'état actuel de la **définition de « finir le jeu »** :
  boucle PvE solo complète **5 actes × 3 difficultés, 7 classes, création → mort de Baal**.
  Multi et cinématiques vidéo hors scope (crédits/interludes **texte** inclus ci-dessous).
- **Méthode** : lecture du code (`demo/`, `engine-rs/crates/`), du manifest
  (`assets/manifest.json`), des assets (`assets/`), et des 3 audits de parité
  (`SYNTHESE.md`, `od2-features-moteur.md`, `od2-features-gameplay.md` — ne pas
  recopier : cet audit couvre le domaine *expérience*, les autres couvrent formats/moteur/gameplay).
- **Audit seul : zéro ligne de code.** Chaque item cite ses preuves fichier:ligne.

**État de départ (vérifié ce jour)** : 2 écrans (menu + jeu), 3 classes jouables sur 7,
2 niveaux (town, bloodmoor) reliés par 1 warp physique, 6 monstres, **0 NPC**, 13 sfx,
0 musique, 0 font/0 TBL, 1 slot de sauvegarde, `cargo test` 143 passed (SYNTHESE.md §Phase 2),
60 fps E2E sur les 2 cartes actuelles (engine-rs/REPORT.md §7).

**Échelle de taille** : S ≤ ½ j · M = 1–3 j · L = 3–10 j · XL = chantier (>10 j).
**Phases** : **P0** = correctif immédiat sans dépendance · **PH1** = finitions à 1 acte ·
**PH2** = infrastructure 5 actes × 3 difficultés · **PH3** = contenu complet jusqu'à Baal.

---

## 1. Écrans

| Id | Manque | Pourquoi (réf. « finir le jeu ») | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------------------------------|---------------|--------|-------------|-------|
| EXP-E1 | Sélection de perso multi-saves (liste, scroll, suppression, confirmation) | 7 classes × saves multiples exigent un vrai sélecteur | MISSING côté liste ; côté « Continuer » **BUG P0** : `menu.js:36` envoie `?load=1` alors que `game.js:65` lit `?continue=1` ; `menu.js:56` grise sur la clé `d2save` jamais écrite (la vraie clé est `d2wasm.save.v1`, `game.js:56`) → **le bouton Continuer est mort en l'état** | S (fix) + M (liste) | EXP-S2 | P0 |
| EXP-E2 | Création de héros 7 classes **animée** (composite NU animé, description, nom, sons de classe) | « 7 classes » dans la définition de fin | PARTIAL : 3 cartes statiques PNG (`menu.html:22-27`, composites figés `assets/dcc/composite.png`, `sor-composite.png`, `pal-composite.png`) ; pas de champ nom, pas de hardcore, pas d'anim NU jouée | L | extraction DCC des 4 classes manquantes (shortcut `shortcut-sprites-perso-d2`), EXP-S2 | PH3 (4 classes en PH2) |
| EXP-E3 | Écran de chargement inter-niveaux/inter-actes avec progression réelle | 5 actes = chargements longs, sinon écran noir | PARTIAL : barre de boot uniquement (`game.html:79-83`, progression fetch `game.js:636-648`) ; les warps internes sont masqués par un fondu 260 ms (`game.js:407-413`) | S | EXP-K2 | PH2 |
| EXP-E4 | Pause/options réelles (Esc = menu pause ; volumes musique/sfx/ambiance ; rebind ; quit vers menu) | finitions exigées ; aujourd'hui Esc ferme seulement l'inventaire | MISSING : `game.js:537-540` ; seul réglage = toggle son (`game.js:553-557`) ; le `tick` ne s'arrête jamais | M | — | PH1 |
| EXP-E5 | Help overlay (toutes les touches, écran dédié H) | raccourcis only en pied de page statique | MISSING : `game.html:93` (une ligne fixe) ; l'audit gameplay L11 le confirme | S | EXP-C1 | PH1 |
| EXP-E6 | **Automap complet (Tab)** : overlay carte + entités + warps, transparence, toggle | panneau D2 emblématique ; navigation 5 actes impossible confortablement sans lui | MISSING total : aucun handler Tab, aucune mention `automap` dans demo/engine-rs ; la grille marchable existe (`compose.rs:72-77`) et `screen_to_sub` (`compose.rs:88`) donne la projection → dessinable en canvas | M | — | PH1 |
| EXP-E7 | Journal de quêtes (onglets par acte, objectifs, état) | boucle PvE 5 actes = quêtes | MISSING (UI **et** modèle) ; gameplay F3/L8 ; aucune mention `quest` dans le code | XL (avec le modèle de quêtes) | quêtes d2rules (hors ce domaine), EXP-E3 | PH3 |
| EXP-E8 | Skill tree UI (3 onglets, icônes, points) + **sélection L/R** | compétences exigées ; l'état existe déjà mais est **inaccessible au navigateur** | MISSING côté UI : `select_skill` existe (`d2rules/src/world.rs:557`) et `hero_ranged_attack` consomme le skill actif (`world.rs:906`) mais **aucun endpoint wasm ni input JS** ne l'expose (`wasm.rs` : 24 méthodes, aucune `skill` ; `PlayerInput` sans variante skill) ; `demo/main.js:42-55` décode `skillicon.dc6` en démo seulement | M (MVP) | A6 étendu (points/arbres, gameplay) | PH1 (MVP 1 skill) → PH3 (arbres) |
| EXP-E9 | Panneau stats avec **dépense de points** (str/dex/vit/ene, +5/niveau) | leveling déjà livré (`apply_xp` world.rs) mais sans choix joueur | MISSING : `HeroStats` sans vit/ene/stamina/points (`d2rules/src/lib.rs:212-245`) alors que le manifest porte déjà les données (`manifest.json:8,14-20` stamina 92, vitality, energy, lifePerVitality…) ; gameplay A2/L7 | M | — | PH1 |
| EXP-E10 | Vendor / gamble UI (Gheed, Akara, Charsi : buy/sell/repair/gamble) | économie D2, or a un usage | MISSING total : **aucun NPC** (town `monsters: []` `manifest.json:1361` ; aucune entité NPC dans `d2rules/src/world.rs`), aucune mention vendor/stash/gamble dans le code | XL | entités NPC (gameplay), génération d'items D1 (gameplay) | PH2 (vendeur minimal) → PH3 (gamble/repair) |
| EXP-E11 | Stash + Horadric Cube UI | économie/inventaire fin d'acte | MISSING total ; `assets/patch_cubemain.txt` déjà extrait (recettes disponibles) | L (stash) / XL (cube) | inventaire multi-cases C3 (gameplay) | PH3 |
| EXP-E12 | Waypoint UI (panneau, réseau inter-actes) | déplacements 5 actes | MISSING : seuls des warps physiques existent (`manifest.json:1362-1404`, zones walk-in + cooldown `game.rs:715-737`) ; aucune mention waypoint | M | multi-actes | PH2 |
| EXP-E13 | Crédits défilants (texte) + interludes texte entre actes | demandé explicitement (cinématiques vidéo hors scope) | MISSING : pas de bouton Crédits au menu (`menu.html:16-21` : Solo/Continuer/GitHub/Quitter) ; aucun interlude | S (crédits) / M (interludes) | — | PH1 (crédits) / PH3 (interludes) |

## 2. HUD

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-H1 | Ceinture potions 4 slots + touches 1-4 | réflexe D2 n°1 | MISSING : potions via P (première potion, `game.js:542-548`) ou clic inventaire ; gameplay L2 ; données `patch_misc.txt` extraites | S-M | — | PH1 |
| EXP-H2 | Barre d'endurance (stamina) | HUD D2 complet | MISSING : valeur présente au manifest (`manifest.json:8` stamina 92) mais **non consommée** par `HeroStats` (`lib.rs:212-245`) ; cours = gratuit aujourd'hui | S (après A2) | A2 (gameplay) | PH1 |
| EXP-H3 | Minipanel 8 boutons D2 (inventaire/stats/skills/quêtes/automap/menu…) | équivalent Go `mini_panel.go` | PARTIAL réduit : 3 boutons HTML (save/load/son, `game.html:85-89`) ; I/P/E en clavier seul | S-M | EXP-E4/E6/E8/E9 | PH1 |
| EXP-H4 | Boutons L/R skill + hotkeys F1-F8 | base des compétences D2 | MISSING : pas de notion de main gauche/droite nulle part (gameplay L4) | M | EXP-E8 | PH1 |
| EXP-H5 | Tooltip XP au survol de la barre | gameplay L3 signalait déjà l'écart | MISSING : `#xpwrap` sans tooltip (`game.html:17-19`) | S | — | PH1 |
| EXP-H6 | Noms de monstres + barre de vie au survol | lisibilité combat | MISSING : aucun hover entité (`game.js:498-506` ne gère que l'inventaire) ; les monstres n'ont ni nom ni vie affichés en jeu | M | EXP-T2 (texte projeté monde) | PH1 |
| EXP-H7 | Dégâts flottants | retour visuel D2 | MISSING ; faisable sans font in-engine (DOM/canvas overlay) | S-M | — | PH1 |
| EXP-H8 | Items au sol : DC6 **flippy** + labels (toggle Alt) | état actuel = rectangles colorés | MISSING : `item_marker` (`game.rs:854-855`) dessine un marqueur, pas le sprite DC6 ; aucun label ; moteur P1 #5 l'a déjà listé | M | extraction flippy DC6 | PH1 |

## 3. Texte fr-FR dynamique (TRANCHEMENT fonts/TBL)

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-T1 | Tables de chaînes fr-FR extraites et consommées (patchstring/string.tbl) | « portage fr-FR » : les textes doivent venir des données, pas du code | MISSING : **aucun `.tbl`** dans `assets/` (ni fonts) ; textes codés en dur : zone `name`/`nameDisplay` (`manifest.json:1337,1371-1372`, « Blood Moor » non traduit), noms d'items anglais (`manifest.json:23` « Hand Axe »), messages FR en dur dans `game.js:426-467` | M (extraction + index i18n manifest) | pipeline d'extraction MPQ (existe : `tools/mpq_mini.py`, MPQs complets dans `extracted/`) | PH1 |
| EXP-T2 | Décision **fonts DC6 in-engine vs DOM** — tranchée ici : **DOM/canvas fillText pour toute l'UI, overlay DOM projeté pour le texte « dans le monde », PAS de port d2font/d2tbl in-engine au stade 1 acte** | l'UI actuelle est DOM/canvas 2D (`game.js:331,384` fillText system-ui) ; porter `d2font`+`d2tbl` Go (~1 500-2 000 l.) pour un rendu identique au fillText n'apporte rien ; `screen_to_world` existe déjà (`game.rs:573`) pour projeter labels/dégâts ; seul cas futur justifiant le DC6 font : dégâts massifs haute cadence (atlas pré-render) ou palette-locked style D2 | DÉCISION (état : fonts MISSING, moteur #6) ; voir justification ci-dessous | S (décision appliquée) ; L si atlas DC6 (rejeté en PH1) | EXP-T1 | PH1 |
| EXP-T3 | Textes gameplay absents à écrire (quêtes, NPC, noms uniques/affixes) | 5 actes de contenu | MISSING (dépend des systèmes gameplay D1-D5/F3) | XL | gameplay quêtes/affixes | PH3 |

**Justification EXP-T2 (à conserver telle quelle dans le repo)** : le moteur ne rend que des
indices palette dans un framebuffer (`game.rs:795-866`) ; ajouter le texte in-engine imposerait
un rasteriseur DC6 font + un lecteur TBL + le patchstring, pour produire le même pixels que
`ctx.fillText` avec une webfont libre proche d'Exocet. L'UI reste DOM (accessibilité, i18n,
reword facile), le monde reçoit un **overlay canvas 2D** positionné par `screen_to_world`
(projection existante `compose.rs:88`), alimenté par les chaînes TBL de EXP-T1. Coût
réel : quasi nul au moteur ; bénéfice immédiat : labels items, dégâts, noms monstres.

## 4. Audio

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-A1 | Musique par acte (boucles menu + actes) | identité D2 | MISSING : `assets/sfx/` = 13 WAV SFX uniquement (attack, hit, ui_click, walk, 3 monstres ×3, fallen_die) ; aucune piste musicale extraite | M (extraction + loop seam) | droits : données © Blizzard (même statut que le reste du port) | PH2 |
| EXP-A2 | Ambiance jour/nuit par région (SoundEnvironment) + **panning positionnel** + volume par distance | moteur #26 PARTIAL ; Go `sound_environment.go:32` | MISSING : WebAudio sans `StereoPannerNode`/`PannerNode` (`game.js:190-198` = BufferSource→Gain direct) ; pas d'ambiance | M | — | PH2 |
| EXP-A3 | Mixage : 3 volumes (musique/sfx/ambiance) persistés | options D2 de base | MISSING : toggle binaire seul (`game.js:553-557`), volume fixé par appel | S | localStorage prefs | PH1 |
| EXP-A4 | Pas / events sonores manquants : `walk` déclaré **jamais déclenché** | finitions | BUG léger : `manifest.sfx.walk` (`manifest.json:1314`) mais aucun event footstep côté d2rules ; `game.js` n'écoute pas de `HeroStep` | S | event moteur | PH1 |
| EXP-A5 | Speech NPC (voix d'accueil vendeurs/quêtes) | expérience D2 | MISSING : aucun NPC ; `monsounds.txt` extrait mais seuls attack/hit/die câblés (`manifest.json:1317-1333`) | L | NPC (EXP-E10) | PH3 |

## 5. Contrôles

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-C1 | Hotkeys D2 complètes : 1-4 ceinture, F1-F8 skills, Tab automap, Q stats, T skills, Q/S/T panneaux, H help, Esc pause + **rebind** | contrôles D2 ; audit L11 MISSING | MISSING : seuls I/E/P/Maj/Échap-inventaire existent (`game.js:525-552`) ; aucune keymap, aucun handler priorisé (moteur #23) | M | EXP-E4/E6/E8/H1 | PH1 |
| EXP-C2 | Curseurs D2 (DC6 curseurs : normale, attaque, porte, talk) | feel D2 | MISSING : `cursor: crosshair/pointer` CSS uniquement (`game.js:503`, `game.html:13`) | S | — | PH1 |
| EXP-C3 | File d'inputs : **1 seule entrée traitée par frame** | pertes de clics en combat dense | BUG léger : `loop.queue.shift()` une par tick (`game.js:706`) ; un double-clic 16 ms perd le premier ordre | S | — | PH1 |

## 6. Économie & perf navigateur

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-P1 | Mémoire des grandes cartes : framebuffer **idx + overlay = 2 o/px**, carte entière résidente | actes 2-3 ont des DS1 plus grands qu'il faut pouvoir afficher | MESURE : town (56×40 tuiles) → 7 680×4 340 px ×2 buffers = **66,7 Mo** ; bloodmoor (71×71) → 11 360×6 180 ×2 = **140,4 Mo** (formules `compose.rs:59-62`, buffers `compose.rs:67-71`) ; une seule carte résidente à la fois (`load_level` remplace, `game.rs:291-322`) ; 3 difficultés × 5 actes en DS1 extraits → risque WASM memory ; mitigations : composition viewport-only ou cache de tuiles (Go `tile_cache.go`) | XL | — | PH2 |
| EXP-P2 | Cache de frames de sprites | chaque entité est **recomposée à chaque frame** | PREUVE : `blit_entity` → `set.compose(...)` par entité par frame (`game.rs:927`, composition complète COF dans `sprite.rs:88-124`) ; 20 monstres = 20 recompositions/60 Hz ; 60 fps tient encore (REPORT.md §7) mais pas avec des foules d'acte 2-3 | M | — | PH2 |
| EXP-P3 | Copie wasm→JS du framebuffer : 800×600×4 = **1,92 Mo/frame ≈ 115 Mo/s** | budget CPU navigateur | PREUVE : `render()` retourne le viewport RGBA (`game.rs:795`, copie `wasm.rs:154`) + `putImageData` (`game.js:715-716`) ; passer par ImageData réutilisée est déjà fait, mais un blit WebGL ou un canvas partagé diviserait le coût | M | — | PH2 |
| EXP-P4 | Instrumentation perf permanente (budget fps + profiler long) | cible 60 fps à tenir sur 15 cartes | PARTIAL : compteur fps barre de statut (`game.js:577-587`), stats compose (`game.rs:606`) ; pas de budget automatisé en test | S | EXP-Q1 | PH2 |

## 7. Packaging & distribution

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-K1 | PWA/offline (service worker + webmanifest) | « jeu navigateur » doit tourner hors tunnel | MISSING : aucun `sw.js`/`*.webmanifest` dans `demo/` ; distribution actuelle = serveur statique + tunnel éphémère port 8077 (cf. mémoire projet) | M | — | PH2 |
| EXP-K2 | Chargement par acte (manifest par acte / lazy) | boot actuel charge **tous** les niveaux | PREUVE : `collectPaths` charge les assets de TOUS les blocs `levels` au boot (`game.js:111-152`, commentaire lignes 110-112) ; à 10+ cartes, boot en minutes ; wasm = 1,0 Mo (`demo/game-pkg/d2game_bg.wasm`), menu PNG = **5,6 Mo** (`demo/assets/menu`, 88 fichiers dont 60 frames de logo) | M-L | EXP-E3 | PH2 |
| EXP-K3 | Script de build/serve documenté (README) | reproductibilité | MISSING : README sans instructions serve/build (aucune mention `http.server`/8077) ; build wasm à la main | S | — | PH1 |

## 8. Sauvegarde

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-S1 | Fix « Continuer » menu↔jeu (clés + paramètre désalignés) | la persistance est la fonctionnalité n°1 d'un RPG | **BUG P0** : `menu.js:56` teste `d2save` (jamais écrit) ; `menu.js:36` passe `?load=1` (ignoré, `game.js:65` lit `?continue=1`) ; `menu.js:52` stocke `d2hero` (non utilisé au retour) | S | — | P0 |
| EXP-S2 | Multi-slots + export/import fichier + suppression | 7 classes × reprises | MISSING : 1 clé unique `d2wasm.save.v1` (`game.js:56,558-574`) ; gameplay A7 | M | EXP-E1 | PH1 |
| EXP-S3 | Migration/robustesse : `detectSaveLevel` par **largeur de grille** | 2 niveaux de même largeur = mauvaise carte au Continue | RISQUE prouvé : détection par `grid[0]*5` (`game.js:79-89`) — town 285, bloodmoor 360 ; collision garantie dès 2 DS1 de même taille ; la version existe côté Rust (v2 rétro v1, `save.rs:181`) mais pas de clé niveau dans le save | S | — | P0 |
| EXP-S4 | État de partie complet : **acte, difficulté, nom, skills L/R** dans le save | reprise 5 actes × 3 diff | MISSING : gameplay A5 (ni act, ni difficulté, ni nom) ; `active_skill` déjà sauvé (`save.rs:48,146`) | M | multi-actes | PH2 |

## 9. Qualité

| Id | Manque | Pourquoi | État / preuve | Taille | Dépendances | Phase |
|----|--------|----------|---------------|--------|-------------|-------|
| EXP-Q1 | Suite E2E **rejouable** (création→jeu→save→load→warp→mort→respawn) | 15+ cartes × 3 diff impossibles à valider à la main | PARTIAL : sondes `window.__d2` (`game.js:18-46`, `goto:678`, `?run=1:670`) + captures PNG manuelles (`demo/*-e2e.png`) ; cargo 143 tests moteur ; aucune suite navigateur automatisée dans le repo | M | — | PH1 |
| EXP-Q2 | Accessibilité clavier (focus panneaux, aria-live messages, prefers-reduced-motion) | exigée par le brief | MISSING : jeu canvas sans alternative clavier ; bannière/animations CSS sans `prefers-reduced-motion` (`game.html:27-40`) ; menus DOM focusables (OK par défaut) | S-M | — | PH1 |
| EXP-Q3 | Données dupliquées front/back (XP_THR, résolution héros) | dette de cohérence | PREUVE : seuils XP dupliqués `game.js:100` vs `XP_TABLE` `world.rs:59` ; résolution héros dupliquée JS/Rust avec garde d'alignement `game.js:91-98,628-631` ; un endpoint `levelInfo`-like suffirait | S | — | PH1 |

---

## 10. Compte

49 items : 1 **BUG P0** (EXP-S1, + 2 légers EXP-A4/EXP-C3/EXP-S3 en risque),
0 FULL hors existant, le reste MISSING/PARTIAL par rapport à la cible « finir le jeu ».
Tailles : S×18 · M×19 · L×6 · XL×6.

## 11. Definition of done — domaine EXPÉRIENCE & FINITIONS

1. **Boucle complète** : menu → création (7 classes animées) → jeu → mort de Baal, sans
   recharger la page ; chaque changement d'acte passe par un écran de chargement à progression
   et un interlude texte ; crédits accessibles depuis le menu.
2. **HUD = parité D2 minimale** : globes, ceinture 1-4, stamina, XP+tooltip, minipanel 8
   boutons, L/R skill, survol monstre (nom + vie), labels d'items au sol (flippy DC6 + Alt),
   dégâts flottants.
3. **Écrans** : pause/options (3 volumes + rebind), help overlay, automap Tab (entités + warps),
   journal de quêtes, skill tree (3 onglets), panneau stats avec dépense, sélection multi-saves
   (liste/suppression), vendor buy/sell (1 vendeur par camp) — tous en fr-FR issu des tables.
4. **Texte** : zéro chaîne de jeu codée en dur dans `game.js`/manifest ; tout passe par les
   tables fr-FR extraites (EXP-T1) ; décision EXP-T2 appliquée (pas de font in-engine).
5. **Audio** : musique d'acte en boucle + ambiance jour/nuit + panning positionnel + 3 volumes
   persistés ; pas de `walk.wav` mort.
6. **Save** : multi-slots, export/import fichier, clé de niveau dans le save (fini
   `detectSaveLevel`), acte/difficulté/nom sauvés, migration versionnée v2→vN testée.
7. **Perf** : ≥ 50 fps sur les 3 plus grandes cartes (référence portables intégrés), mémoire
   WASM < 500 Mo pour la plus grande carte, cache de frames sprites en place.
8. **Packaging** : PWA installable offline complète ; boot < 15 s sur connexion 10 Mb/s
   (manifests par acte) ; build+serve documentés dans le README.
9. **Qualité** : suite E2E rejouable verte sur le parcours complet ; navigation clavier des
   panneaux ; `prefers-reduced-motion` respecté.

## 12. Top 5 ordonné

1. **EXP-S1 + EXP-S3 + EXP-E1 (P0)** — réparer « Continuer » (clés/paramètres), stocker la clé
   du niveau dans le save, poser l'index multi-slots. Zéro dépendance, débloque toute la
   reprise de partie ; aujourd'hui la seule sauvegarde est quasi inutilisable depuis le menu.
2. **EXP-E9 + EXP-H1 + EXP-H2 (PH1)** — panneau stats avec dépense (vit/ene/stamina/points,
   données déjà au manifest) + ceinture 1-4. Le leveling livré (`apply_xp`) devient enfin un
   choix de joueur.
3. **EXP-E8 + EXP-H4 (PH1)** — exposer l'état skill au navigateur (endpoint wasm + input
   `SelectSkill`, le socle `select_skill`/`hero_ranged_attack` existe : `world.rs:557,906`) +
   skill tree MVP + L/R. Un seul nouveau panneau pour rendre les 3 classes réellement différentes.
4. **EXP-T1 + EXP-T2 + EXP-H6/H7/H8 (PH1)** — extraction patchstring fr-FR + décision fonts
   (DOM/overlay, pas d'in-engine) puis labels d'items, dégâts flottants, survols monstres :
   le « feel » D2 au coût moteur quasi nul.
5. **EXP-P1 + EXP-P2 + EXP-K2 (PH2)** — cache de frames, composition économe (viewport/overlay)
   et manifest par acte : le prérequis matériel des 5 actes × 3 difficultés ; sans lui, chaque
   carte d'acte 2-3 coûte > 140 Mo de RAM et un boot de minutes.

---

*Audit statique (lecture de code + assets + tests existants), aucune écriture de code.
Preuves citées `chemin:ligne` depuis `/home/agent/workspace/diablo-2-wasm/`.
Domaines voisins non recopiés : formats/moteur/gameplay → `od2-features-*.md`.*
