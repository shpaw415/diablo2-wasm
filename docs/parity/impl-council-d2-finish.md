# Council `d2-finish` — 5 chantiers intégrés (2026-10-23)

Council de 5 membres en arbres séparés (`d2w-*`), fusion séquentielle via
branches git, câblage front-end complété par l'intégrateur. **État final :
`cargo test --workspace` = 358 passed / 0 failed / 1 ignored** (baseline
305 → +53 tests), E2E navigateur 59 fps.

## 1. Arbre de compétences C (`d2w-skills`)

- `d2rules/src/skills_hero.rs` (nouveau) : points de skill par niveau,
  apprentissage validé (classe, prérequis `reqskill1-3`, `reqlevel`,
  plafond `maxlvl` 20) ; `skills_tree.rs` étendu (onglets + géométrie
  d'arbre depuis `patch_skills.txt`) ; `skill_tree_json` +
  `learn_skill` exposés WASM.
- UI câblée par l'intégrateur (le membre s'est arrêté avant) :
  `demo/skills-ui.js` (nouveau, touche **T** ou bouton « Skills », + par
  skill apprenable, groupé par onglet — arbre Barbare réel vérifié E2E).

## 2. Villes vivantes W14/W16 (`d2w-villes`)

- `d2rules/src/npc.rs` (nouveau) : roster Acte I réel (monstats + PATH de
  `TownN1.ds1`), patrouilles, non-attaquables, persistants (save).
- Rôles : **Akara** soin gratuit (vie/mana pleins), **Charsi** forge/réparation,
  **Gheed** gamble, **Kashya** mercenaires (API prête), **Warriv** caravane.
- Clic NPC → `NpcTalked` → **q3/q4 se débloquent de bout en bout**
  (preuve tests : bks → clic Akara → Tristram → clic Cain = Done + `rin` ;
  Smith → Malus → clic Charsi = Done + `charsi_imbue`).
- UI livrée : `demo/npc-ui.js` (marqueurs + dialogues) ; WASM `npcs`/`talkNpc`.

## 3. Mercenaires H (`d2w-mercos`)

- `hireling.txt` v1.12 extraite du MPQ (`assets/patch_hireling.txt`, BYO) :
  4 types, sous-types (DefaultCh), HP/Def/Str/Dex/AR par palier.
- `d2rules/src/mercenary.rs` (nouveau) : embauche (palier ≤ niveau héros),
  suit le héros, mêlée ou flèches (rogue via d2missiles), KO →
  résurrection chez le NPC, save additif ; un seul merc à la fois.
- **Fix intégration (IA)** : les monstres ciblent désormais le PLUS PROCHE
  de (héros, merc vivant) en poursuite ET en frappe (égalité → merc) —
  sinon le merc ne servait jamais d'encaisseur. Zéro RNG ajouté
  (déterminisme préservé).
- **Fix tests** : DefaultCh réelle v1.12 = 75 pour TOUS les sous-types
  Rogue (départage = premier du fichier, comme l'offre Desert) ; 2 tests
  blindaient un zombie que la flèche du héros tuait avant le scénario.
- UI câblée par l'intégrateur : `demo/merc-ui.js` (nouveau, carte
  nom/niveau/PV/état KO) ; WASM `mercInfo`.

## 4. Corps au sol J1 (`d2w-corps`)

- `d2rules/src/corpse.rs` (nouveau) : à la mort, le corps reste au sol
  avec l'équipement + l'or ; respawn au camp **torse nu** ; pénalité XP
  officielle (table réelle : Normal 0 %, NM 5 %, Hell 10 % — niveau
  jamais perdu) ; clic sur le corps (input `recoverCorpse`) = ré-équipement
  complet ; ancien corps droppé à la mort suivante. Save additif.
- UI livrée : `demo/corpse-ui.js` (overlay mort + flare + hint) ; WASM
  `corpseInfo`.

## 5. Musique par acte EXP-A1 (`d2w-audio`)

- `demo/music.js` (nouveau) : WebAudio, mapping zone→thème data-driven
  (thèmes v1.12 extraits : `town1`, `wild`, `caves`, `crypt`,
  `andarielaction`…), fondu doux aux changements de zone, autoplay au
  premier geste (politique navigateur), bouton « Musique : oui/non »,
  mute persistant ; aucune exception si fichier absent.
- Câblage livré : hooks `setLevel` sur les changements de zone (warp,
  waypoints, Charger, boot).

## Intégration

- Merges `member-*` résolus (champs save additifs unis : `npcs`, `quests`,
  `mercenary`, `corpse` ; events `NpcTalked` ∪ `Merc*` ∪ `Corpse*`).
- Deux régressions de fusion corrigées : champs `World` mercenaires
  perdus à l'union (restaurés) et marqueurs résiduels dans `wasm.rs`
  (endpoints `npcs`/`talkNpc`/`mercInfo`/`corpseInfo` unis).
- WASM reconstruit ; E2E : boot 59 fps, panneau compétences (arbre BA
  réel), bouton musique, carte mercenaire absente par défaut (aucun merc
  embauché), zéro erreur bloquante. Captures : `demo/council2-skills-e2e.png`,
  `demo/council2-jeu-e2e.png`.

## Limites connues

- Embauche mercenaire : l'API Rust est prête mais le bouton « Embaucher »
  du dialogue NPC reste à brancher (rôle Kashya/Charsi déjà listé).
- Icônes de skills : placeholder texte (sheets par classe hors MVP).
- Dialogue NPC : bulle minimale (libellés FR provisoires, TBL absentes W18).
