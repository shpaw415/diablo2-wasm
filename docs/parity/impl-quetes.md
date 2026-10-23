# impl-quetes — W13 : moteur de quêtes + authoring Acte I (2026-10-23)

> Chantier XL W13 (finish-jeu P2, finish-monde §W13). Livré dans **deux fichiers
> nouveaux uniquement** — `d2rules/src/quests.rs` (moteur + authoring, Rust pur)
> et `demo/quests.js` (miroir JS + panneau journal) — conformément à la
> contrainte de chantier parallèle : **aucune touche** à world.rs, game.rs,
> game.js, save.rs. Le câblage des événements et la persistance sont Faits EN
> INTÉGRATION (contrat exact plus bas).
>
> État tests à la livraison : `cargo test -p d2rules --lib quests::` →
> **15 passed / 0 failed** ; lib complète 140 passed (le crate compte aussi un
> fichier `vendor.rs` en cours d'écriture par un chantier parallèle — ses
> échecs éventuels à l'instant T lui appartiennent, cf. §6).

## 1. Moteur (`d2rules/src/quests.rs`, ~900 lignes, 100 % pur)

Zéro dépendance à `World`/`GameEvent` : le moteur consomme des [`QuestEvent`]
typés et rend des [`QuestReward`] — testable `cargo test`, déterministe.

### Types

| Type | Rôle |
|---|---|
| `QuestState` | `NotStarted / InProgress / Done / Failed` (`snake_case` en JSON) |
| `QuestEvent` | `MonsterKilled{code}` · `ItemPickedUp{code}` · `ZoneEntered{level_id}` · `NpcTalked{npc}` (le brief portait « NpcTalken », typo corrigée) · `BossDead{code}` |
| `QuestTrigger` | Condition d'étape ; `MonsterKilled{code:"*"}` = joker tout monstre ; **`BossDead{X}` matche aussi `MonsterKilled{code:X}`** (l'intégration peut n'émettre que des kills tant que les boss scriptés W11 n'existent pas) |
| `QuestReward` | `Xp{amount}` · `Item{code}` · `Unlock{flag}` |
| `QuestStep` | `desc` + `trigger` + `count` requis (étapes **séquentielles**) |
| `QuestDef` | Authoring sérialisable : `id, act, name, summary, gates_act, steps, rewards` |

### Sémantique (`QuestLog::on_event(&mut self, event: &QuestEvent) -> Vec<QuestReward>`)

1. Événement **hors ordre = ignoré** (seule l'étape COURANTE est testée) —
   comme le retail, pas de rattrapage.
2. Une quête `NotStarted` démarre (`InProgress`) quand son **étape 0** matche ;
   le même événement compte déjà pour l'étape 0.
3. `count += 1` par événement correspondant ; à `count == required` :
   étape suivante (compteur remis à 0) ou, dernière étape, quête `Done` —
   état **figé** (`step = dernier`, `count = requis`) et récompenses rendues
   dans l'ordre d'authoring.
4. `Done`/`Failed` ignorent tout (replay sans double récompense — testé).
5. `gates_act` : quand toutes les quêtes porteuses d'un acte sont `Done`, le
   moteur émet `Unlock{flag:"act{N}_done"}` (dédié, dédupliqué). Acte I : q5
   seule porteuse ⇒ **tuer Andariel suffit à finir l'acte**, quêtes
   secondaires non bloquantes (fidèle au retail).
6. `QuestLog::fail(id)` : échec manuel `InProgress → Failed` (aucun échec
   automatique au MVP ; rien dans l'Acte I retail n'échoue).
7. Lecture UI : `states()`, `state(id)`, `progress(id) -> QuestProgress`
   `{state, step, desc, count, required}`, `act_done(act)`, `defs()`.

### Persistance (contrat pour save.rs — champ NON ajouté, cf. §4)

- `to_json()` : **état seul**, versionné —
  `{"version":1,"quests":[{"id","state","step","count"},…]}`. Les définitions
  ne sont jamais dupliquées (authoring statique = code).
- `QuestLog::from_json(&str) -> Result<QuestLog, ()>` : reconstruit l'authoring
  Acte I et valide TOUT — version, ids connus et dans l'ordre, `step/count`
  dans les bornes de l'étape, états figés `Done` cohérents — tout écart ⇒
  `Err(())` (l'appelant garde son état précédent). Round-trips testés :
  frais, partiel, chaîne complète, `Failed`.
- **Contrat JSON Rust == JS vérifié byte-à-byte** (même progression → même
  chaîne exacte, script de comparaison exécuté).

## 2. Authoring Acte I (`quest_defs_act1()` — 5 quêtes)

Aucune table de quêtes n'existe dans les MPQ (prouvé finish-monde §1) :
source wiki/D2, **codes vérifiés dans `assets/`** (autorité absolue).

| # | id | Nom (EN canonique) | Chaîne MVP | Récompenses |
|---|---|---|---|---|
| q1 | `a1q1` | Den of Evil | `ZoneEntered(8)` → `MonsterKilled("*")×6` | `Unlock{skill_point}` (Akara, retail +1 point) |
| q2 | `a1q2` | Sisters' Burial Grounds | `BossDead{bloodraven}` | `Unlock{rogue_scout}` (recrue Kashya) |
| q3 | `a1q3` | The Search for Cain | `ItemPickedUp{bks}` → `NpcTalked{akara}` → `ZoneEntered(38)` → `NpcTalked{cain}` | `Item{rin}` + `Unlock{cain_free_id}` |
| q4 | `a1q4` | Tools of the Trade | `BossDead{smith}` → `ItemPickedUp{hamm}` → `NpcTalked{charsi}` | `Unlock{charsi_imbue}` |
| q5 | `a1q5` | Sisters to the Slaughter (**gates_act**) | `BossDead{andariel}` | — (l'unlock `act1_done` vient du moteur) |

Codes réels utilisés : boss `bloodraven` (monstats 267), `smith` (402,
super-unique « The Smith »), `andariel` (156) ; objets `bks` (Bark Scroll,
patch_misc), `hamm` (Horadric Malus, patch_weapons), `rin` (convention bague) ;
niveaux `patch_levels.txt` : **8** = Act 1 - Cave 1 (Den of Evil), **38** =
Tristram. Notes d'authoring : le compteur q1 (6) est aligné à la densité de
spawn actuelle du port (5-8 monstres/niveau) — le retail = « tous les
monstres de la grotte » (~40), à recaler quand les planches W06 existent ;
les libellés FR sont **provisoires** (TBL fr = W18), noms EN canoniques.

## 3. `demo/quests.js` — miroir JS + panneau journal (self-contenu)

- **Miroir exact du moteur** : mêmes DEFS, mêmes sémantiques (étapes
  séquentielles, auto-start, joker `*`, `BossDead`⇔`MonsterKilled`, figeage,
  unlock d'acte), **même format d'état JSON v1** — `QuestLog.from_json` y
  applique les mêmes validations.
- **Panneau journal** : touche **Q** (ouvre/ferme), liste les 5 quêtes avec
  état FR (`Non commencée / En cours / Terminée ✔ / Échouée`), objectif
  courant + progression `(3/6)`, résumé si non commencée ; style de la page
  `game.html` (or/pierre, panneau fixe en haut à droite) ; CSS injecté par le
  script, aucune dépendance.
- **Intégration = 3 lignes** (fait en intégration, game.js est interdit ici) :
  ```html
  <script src="./quests.js"></script>
  ```
  ```js
  D2Quests.init();                     // après DOM ready
  D2Quests.feed(ev);                   // pour chaque GameEvent JSON du moteur
  D2Quests.importLog(json);            // au chargement d'une save (localStorage)
  const json = D2Quests.exportLog();   // à la sauvegarde
  ```
- API complète : `init(dom?)`, `feed(ev)`, `show/hide/toggle/isOpen`, `reset`,
  `exportLog/importLog`, `log` (instance), classes `QuestLog`/`DEFS` exportées
  (tests node). Formes d'événements acceptées : le JSON tagged du moteur
  (`{"MonsterKilled":{"code":"FA"}}`…) et le raccourci
  `{type:"MonsterKilled", code:"FA"}`.
- Smoke test node exécuté : chaîne A1 complète, ordre, boss-via-kill,
  round-trip, rejet corrompu → **ALL OK**.

## 4. CONTRAT D'INTÉGRATION EXACT (à faire dans world.rs/game.rs/save.rs)

**Règle d'or : la source des `QuestEvent`, c'est la boucle d2game (game.js →
CoreEvent), PAS world.rs** — le moteur ne lit jamais le monde. Le mappeur
minimal, à câbler côté d2game là où les `CoreEvent` sont produits :

| Source (existant, prouvé) | QuestEvent émis |
|---|---|
| `CoreEvent::World(GameEvent::MonsterDied{index,..})` | `MonsterKilled{code: mon_codes[index].clone()}` — **lire le code AVANT le retrait** (game.rs entretient `mon_codes` parallèle à `monsters()`, game.rs:129-132 ; il pop au même événement, game.rs:923-928 : capturer le code poppé). Les codes boss (`bloodraven`, `smith`, `andariel`) passent tels quels : le trigger `BossDead` accepte un kill banal. |
| `CoreEvent::World(GameEvent::HeroPickedUp{slot})` | `ItemPickedUp{code: world.inventory()[slot].code.clone()}` |
| `CoreEvent::LevelSwitch(LevelSwitchInfo{key,name})` | `ZoneEntered{level_id}` = id `levelsId` du niveau résolu (manifest `levels[i].levelsId` ; le moteur compare aux ids de table 8/38…). Si l'id n'est pas porté par `LevelSwitchInfo`, l'ajouter (2 champs, additif). |
| `NpcTalked` | **Aucune source avant W16** (NPC de ville) : synthétiser l'événement à l'interaction NPC quand elle existera. Les étapes NPC de q3/q4 restent donc inactives en jeu sans W16 — pas de blocage (moteur testé par événements directs). |
| `BossDead` | Optionnel : les kills suffisent (cf. §1-3). Un `BossDead{code}` dédié devient propre quand W11 (boss scriptés) existera. |

**Persistance (save.rs, W19)** — champ additif rétro-compatible, aucun break
v1/v2 :

```rust
// struct Save : #[serde(default)] pub quests: Option<serde_json::Value>,
// à la sauvegarde :  quests: Some(serde_json::from_str(&quest_log.to_json()).unwrap()),
// au chargement  :   match s.quests { Some(v) => QuestLog::from_json(v.get()).map… (Err ⇒ état neuf, jamais de refus de save),
//                                     None   => QuestLog::act1() }
```

Côté page : `localStorage` (ou le flux save existant de game.js) embarque la
chaîne `D2Quests.exportLog()` / restitue par `importLog` (throw ⇒ log neuf).
**Où vit le `QuestLog` ?** Recommandation : champ du `GameCore` d2game (pas du
`World` d2rules), alimenté par le mappeur ci-dessus — aucun changement de
contrat `World::save/load` nécessaire, le JSON de quêtes voyage à côté.

**UI (game.js)** : `<script src="./quests.js"></script>` dans game.html +
`D2Quests.init()` au boot + `D2Quests.feed(...)` sur chaque événement moteur +
`Q` déjà géré par quests.js lui-même. Aucune autre modification requise.

## 5. Tests

`d2rules/src/quests.rs` — 15 tests, tous verts :

- états : journal frais tout `NotStarted`, ids inconnus `None` ;
- chaînes : q1 (zone→6 kills, joker, ordre), q2 (BossDead ET kill banal,
  mauvais boss ignoré), q3 (ordre strict 4 étapes, mauvais objet ignoré),
  q4 (smith→malus→charsi), q5 (Andariel seul ⇒ `act1_done`, actes vides
  jamais débloqués, pas de double unlock) ;
- **replay complet A1** (16 événements narratifs : den→bloodraven→cain→
  smith→charsi→andariel) : 5×`Done`, `act_done(1)`, récompenses exactes dans
  l'ordre `[skill_point, rogue_scout, rin, cain_free_id, charsi_imbue,
  act1_done]`, replay ⇒ plus rien ;
- `fail()` manuel + quêtes échouées muettes ; récompenses `Xp`/`Item` via
  authoring arbitraire (`with_defs`) ;
- sérialisation : forme JSON exacte, round-trips frais/partiel/complet/`Failed`
  (identité byte), 7 corruptions rejetées (version, JSON, nombre, id, compteur
  hors bornes, `Done` non figé, étape inexistante) + contrôle positif.

`demo/quests.js` — smoke node : chaîne complète, ordre, boss-via-kill,
round-trip, rejet corrompu, **égalité byte du JSON avec la sortie Rust**.

## 6. Restant (hors chantier, signalé pour la suite)

1. **Câblage** du §4 (game.rs/game.js/save.rs) — intentionnellement non fait
   (fichiers communs interdits ce chantier) ; ~1 h d'intégration.
2. **Quêtes A2-A5** (~21 quêtes) : mêmes structs, `quest_defs_act2()`… ;
   q3-A2 (Arcane/Duriel) dépendra de W11/W14 (orbe, tunas).
3. **Journal fr TBL** (W18) : libellés FR réels des quêtes — les chaînes FR
   actuelles sont provisoires et centralisées dans `quest_defs_act1()`/`DEFS`.
4. Recaler le compteur q1 sur les planches réelles (W06) et ajouter les
   récompenses à dialogue (Den of Evil se clôt en parlant à Akara au retail).
5. Note d'exécution : le crate a été compilé/validé pendant qu'un chantier
   parallèle éditait `vendor.rs` (fichier lui appartenant) ; validation
   finale isolée de quests.rs effectuée hors arbre (15/15) + lib complète
   140 passed au moment du handoff.
