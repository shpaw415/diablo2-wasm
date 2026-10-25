# Interop export `.d2s` — layout « retail » (étape 19c, PLAN-FIDELITE phase 19)

## 1. Méthode — diff structurel contre l'oracle GPL

L'oracle `d2w-oracle/d2s-file-generator` (GPL-3.0, exécuté **hors arbre** —
ses octets sont des DONNÉES, aucun code copié dans l'arbre MIT) produit des
saves 0x60 dont le layout est **vérifié en jeu réel** (LoD v1.14d, cf. son
README : les cas niveau 1 sont des vraies saves). Trois saves ont été
générées proches de nos profils Barbare (SC, or 0, nom `Testba`,
`last_played`/`map_id` à 0 pour la reproductibilité) :

| Fichier (`d2d2s/tests/vectors/oracle/`) | Profil | Taille |
|---|---|---|
| `Bar19cFresh.d2s` | BA niveau 1, aucune difficulté terminée | 848 o |
| `Bar19cNight.d2s` | BA niveau 30, Normal terminé (actif = Cauchemar) | 857 o |
| `Bar19cHell.d2s` | BA niveau 99, Enfer terminé (actif = Enfer) | 857 o |

Chacune a été DIFFÉE octet par octet (python LSB-first, sections à offsets)
avec **nos exports équivalents** : save oracle → import → `World` miroir →
`save_d2s::export`. Les écarts de LAYOUT trouvés ont été corrigés dans
`save_d2s.rs` (+ defaults d'écriture `d2d2s`) ; les écarts de DONNÉES (ce
que le save v3 ne porte pas) sont documentés §4.

## 2. Règles de layout retail dérivées du diff

| # | Sujet | Avant (étape 6b) | Retail (oracle, vérifié jeu) | Fix 19c |
|---|---|---|---|---|
| 1 | **Stats : format** | flux 1.10+ nu à l'octet 765 | **`67 66` = « gf »** puis le flux 1.10+ standard (ID 9 bits + CSvBits, `0x1FF`, align) — présent sur TOUTES les saves oracle 0x60, y compris la ressource `Barbarian_1_HC.d2s` (vraie save 1.14d) | `save.stats.format = GfPrefixed110` (l'écriture `d2d2s` préfixait déjà « gf » pour ce format) |
| 2 | **Stats : zeros omis** | les 16 stats toujours écrites (0 compris) | retail n'écrit que les stats NON NULLES : niveau 1 → pas de statpts/newskills/xp/or/goldbank ; or 0 → stat 14 absente (`Nec-FreshG` : or stashé 123 456 → stat 15 présente) | `set_stat(id, v, force)` : attrs 0–3, bloc vie 6–11 et level 12 inconditionnels (jamais 0 en pratique), les autres seulement si ≠ 0 |
| 3 | **Statpts (stat 4)** | non écrit (commentaire « save v3 ne porte pas » — périmé) | écrit quand > 0 (150 / 505 sur les profils) | le save v3 porte `stat_points` depuis 19b : export **et** import mappent la stat 4 |
| 4 | **Squelette de listes d'items** | `JM+count / kf+0` (6 o, corpse et merc absents) | **quatre listes toujours écrites** : `JM+count / JM+0 (corpse) / jf NU (merc, sans « JM count ») / kf+flag` = 13 o (`4a 4d 00 00 4a 4d 00 00 6a 66 6b 66 00`) | export pose `corpse_items=Some(∅)`, `merc_items=Some(∅)`, `merc_bare=true` (l'écriture `jf` nue existait côté `d2d2s` depuis 19a) |
| 5 | **En-tête 0x29–0x2A** | `00 00` | **`10 1E`** (16, 30) sur toutes les saves oracle | `Header::default().unk_29 = [16, 30]` |
| 6 | **En-tête 0x34–0x37** | `00 00 00 00` | **`FF FF FF FF`** | `unk_34 = [0xFF; 4]` |
| 7 | **Apparence 0x88–0xA7** | 32 × `00` | **32 × `FF`** | `appearance = [0xFF; 32]` |
| 8 | **Hotkeys 0x38–0x77** | `FFFF` par u32 (= `FF FF 00 00`) | identique (`EMPTY_HOTKEYS` = `FF FF 00 00` ×16) | aucun changement (concordait déjà) |
| 9 | **Progression (0x25)** | 0 | **+5 par difficulté terminée** (0 / 5 / 10 / 15) | `progression = 5 × index difficulté courante` (approximation, cf. §4) |
| 10 | **Waypoints : octet final (633+80)** | 0 | **toujours 1** (même personnage frais) | `Waypoints::default()` pose `raw[80] = 1` |
| 11 | **Waypoints : bit 0** | posé seulement si le WP est actif | **Rogue Encampment (bit 0) actif dans les TROIS enregistrements** de difficulté, même à zéro ailleurs (`WAYPOINTS_EMPTY` = `02 01 01 00 00 00 00`) | export force le bit 0 des 3 difficultés |
| 12 | **Waypoints : bitfields** | 5 o LSB-first + inconnus `02 01` par enregistrement | identique (`FF FF FF FF 7F` = bits 0–39 quand tout est découvert) | aucun changement (concordait déjà) |
| 13 | **Quêtes / NPC** (layout) | `Woo! + 06 00 00 00 2A 01 + 3×96 o` ; `w4 + 50 o` | identiques en STRUCTURE (les blocs complétés diffèrent en DONNÉES, cf. §4) | aucun changement |
| 14 | **Items** | identifiés, qualité normale | les items de départ retail portent le bit **starter (33)** et sont identifiés | `starter: true` sur les items exportés (mission 19c) |

Résultat du diff APRÈS fix :

* **Niveau 1 : notre export est identique octet pour octet à l'oracle**
  (848 o — en-tête, quêtes, waypoints, NPC, stats, skills, squelette).
* **Niveau 30 : même taille (857 o), mêmes offsets** — ne restent que les
  écarts de données §4 (quêtes complétées, waypoints des autres
  difficultés, intros NPC).
* **Niveau 99 : idem** + progression 10 vs 15 (cf. §4).

## 3. Garanties (tests `d2rules/src/save_d2s_interop.rs`)

1. `oracle_saves_have_the_retail_layout` — les 3 saves oracle : parse
   STRICT (checksum), `gf` à 765, sections aux offsets canoniques
   (335/633/714), squelette 13 o, octet final waypoints, bit 0 ×3.
2. `our_export_adopts_the_retail_layout` — nos exports miroirs : même
   TAILLE que l'oracle, mêmes offsets, rélecture STRICTE (checksum),
   zeros omis, unks retail, progression ×5.
3. `export_import_export_cycle_is_byte_stable_per_profile` —
   export→import→export **octet pour octet**, cycle de 3 inclus, état
   mappé conforme au profil (niveau/xp plafonnés aux tables XP réelles).
4. `imported_oracle_save_round_trips_through_our_pipeline` — une save
   ORACLE traverse notre chaîne import→export et reste strict-lisible.
5. `exported_items_are_starter_gear_identified` + `zero_stats_are_omitted…`.

Suite complète : `cargo test --workspace` = **1015 passed / 0 failed /
1 ignored** (les 14 saves oracle du coéquipier, `d2d2s/tests/
oracle_saves.rs`, restent vertes).

## 4. Écarts de DONNÉES restants (non-layout, assumés)

Le save JSON v3 du port ne porte pas ces états — l'export ne peut pas les
inventer (ils restent aux valeurs « retail fraîches » que le jeu remplit
lui-même en jouant) :

* **Quêtes complétées** : l'oracle écrit les blocs 96 o `QUESTS_COMPLETED`
  (bits `Woo!` des quêtes + récompenses) par difficulté terminée ; nous
  écrivons les blocs nuls. Le jeu ré-affiche les introductions de quêtes.
* **Waypoints par difficulté** : retail garde, PAR difficulté, les WP
  découverts (Enfer complet = `FF FF FF FF 7F` sur les 3 enregistrements) ;
  nous n'exportons que les bits de la difficulté COURANTE (+ bit 0).
* **Intros/salutations NPC** (section `w4`) : retail pose les bits 40
  introductions (`FF FF FF FF FF`) des difficultés franchies ; nous
  écrivons 0 (les PNJ re-présentent — cosmétique).
* **Progression 15** : « Enfer terminé » n'est pas exprimable depuis la
  seule difficulté courante (10 posé ; 15 nécessiterait l'état de quêtes).
* **Horodatage et map_id à 0** : écart VOLONTAIRE — l'export est
  déterministe (même état ⇒ mêmes octets, contrat save_d2s étape 6b).
  Retail écrit l'heure réelle et une seed de map.
* **Équipement porté** : le héros MVP n'a pas de modèle « équipé » séparé
  (l'arme est implicite) — seuls les 40 slots d'inventaire sont exportés.

## 5. Limite assumée (inchangée)

La preuve finale d'interop = ouvrir nos exports dans le vrai jeu chez
Justin (hors portée session). Tout ci-dessus est dérivé d'un générateur
dont les SORTIES sont vérifiées jeu réel + de la vraie save D2R
`Javazon.d2s` (lecture) — pas du jeu lui-même.
