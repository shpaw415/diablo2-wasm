# Validation du format .d2s — campagne d2w-integ

## 1. État avant le 2026-10-24 (vecteurs synthétiques)

Le crate `d2d2s` et le parseur `d2w-re/tools/d2s_parse.py` étaient fondés sur
l'evidence `d2w-re/evidence-formats/d2s/evidence.md` (spec 0x60 = v1.10-1.13)
et testés **exclusivement** contre 7 vecteurs synthétiques fabriqués par
`make_vectors.py` — donc valides seulement par construction. Limite assumée,
documentée dans `d2d2s/src/lib.rs` : « Aucun vrai .d2s retail n'a encore
validé ce crate ».

## 2. VALIDATION RÉELLE — 2026-10-24

### 2.1 Le fichier

Justin a fourni un vrai fichier de sauvegarde :

| | |
|---|---|
| Chemin source | `d2w-re/evidence-formats/d2s/real/Javazon.d2s` (lecture seule) |
| Vecteur doré | `engine-rs/crates/d2d2s/tests/vectors/real/Javazon.d2s` |
| Taille | 2 838 octets |
| SHA-256 | `0361b4ad6cac44e1bfb31f7d6c26edab44c2dc3db216b33362b3bbd9bdc5dfc2` |

Décodage intégral (crate Rust `d2d2s`, test `tests/real_javazon.rs`, 11 tests) :

* **Héros** : `LightningAmazon` — Amazon (classe 0), niveau 99, expansion
  (status 0x20), progression 0. Difficulté Normal active (acte 0), NM/Hell
  inactives. Waypoint Rogue Encampment posé.
* **Version : 97 (0x61) = Diablo II Resurrected** — PAS 0x60.
* **Stats** (« gf » + liste 1.10+) : force 855, énergie 959, dextérité 904,
  vitalité 949, hitpoints 3 512, maxhp 2 837, mana 1 698, maxmana 1 430,
  stamina 1 063, maxstamina 1 013, **niveau 99, expérience 3 520 485 254**
  (seuil EXACT du niveau 99), or 990 000, banque 2 500 000 ; `statpts` et
  `newskills` (= 0) **absents du flux**. Caractéristiques éditées : le
  fichier est un mule de test (toutes les compétences Amazon à 20/20).
* **Skills** : marqueur `if`, 30 slots × 20 points.
* **Items** : 78 items réels (le champ count du fichier dit 50 — périmé,
  cf. §2.4), dont l'équipement complet d'une Javazon fin de jeu :
  **Enigma** (Ghost Armor sup, indestructible, Jah+Ith+Ber sertis),
  **Call to Arms** (Crystal Sword, Amn+Ral+Mal+Ist+Ohm), **Fortitude**
  (Archon Plate sup éthérée 4 sockets, El+Sol+Dol+Lo), **Infinity**
  (Cryptic Axe sup éthérée, Ber+Mal+Ber+Ist), **Titan's Revenge**
  (éthérée, pile de 140), **Griffon's Eye**, **Highlord's Wrath**,
  **Raven Frost**, **Stormshield**, **War Traveler**, **Thundergod's
  Vigor**, **Arachnid Mesh**, **Andariel's Visage** (avec rubis « of
  Fervor » sertí), **Razortail**, **Guillaume's Face**, **Aldur's
  Advance**, **Dwarf Star**, **Hellfire Torch**, **Annihilus**,
  **Gheed's Fortune**, 9 charmes « Shimmering » +8 charmes « Harpoonist's
  of Vita » (+45 vie, +1 tab javazon), gants rares +2 tab javazon, etc.
* **Round-trip octet-à-octet** : `parse → to_bytes` restitue les 2 838
  octets à l'identique (test `real_javazon_round_trips_byte_exact`).

### 2.2 Divergences trouvées dans NOS specs & corrections (citées)

Chaque correction = ce que le fichier réel impose, avec la valeur attendue
vs reçue sous l'ancienne règle.

1. **Checksum (evidence §2)** — ancienne règle : les 4 octets du champ
   (0x0C..0x10) étaient *sautés* (rotation comprise). Le réel stocke
   `0x7A36D958` ; l'ancienne formule calcule `0x6096C058` (mismatch).
   **Correction** : le champ compte comme des **zéros** mais la rotation
   gauche s'applique sur ses 4 octets comme sur le reste → `0x7A36D958`
   exact. (Symptôme initial du parseur python : « ID de stat inconnu 103 »
   — voir 3.) Fichiers : `d2d2s/src/checksum.rs`, `tools/d2s_parse.py`.

2. **Version 97 (0x61) = D2R — hors de nos specs** (evidence et krisives
   s'arrêtent à 0x60). Le layout 0x60 échoue dès le premier ID de stat.
   **Correction** : support 0x61 dans `header.rs` (version_name), `stats.rs`
   (nouveau format `GfPrefixed110` avec round-trip), nouveau module
   `d2d2s/src/d2r.rs` pour les items, branché dans `lib.rs`.

3. **Stats 0x61 : marqueur « gf »** — les 2 premiers octets de la section
   stats (765) sont `0x67 0x66` = `"gf"` (marqueur v1.09 réutilisé par D2R),
   suivis du flux 1.10+ standard (ID 9 bits + CSvBits, 0x1FF final, align).
   L'« ID de stat inconnu 103 » du parseur python = ces deux octets décodés
   comme un ID (103 = 0b001100111). **Attendu vs reçu** : premier ID lu 103
   (garbage) vs liste 0,1,2,3,6…15. Confirmé indépendamment par le parseur
   D2R fredimachado/Diablo2R (« `reader.ReadInt16(); // Header` »). Fichier :
   `d2d2s/src/stats.rs`.

4. **Items 0x61 : PAS de marqueur « JM » par item** (evidence §8.2 ligne
   « 0 | 16 | "JM" (répété par item) » — INVALIDE en D2R ; krisives concordait
   par erreur). Le réel n'a que deux `JM` dans tout le fichier (848 = liste
   caractère, 2829 = corpse). Symptôme : `4a 4d` attendu, `10 00 …` reçu à
   852. **Correction** : l'en-tête d'item D2R = **32 bits de drapeaux bruts**
   (identified@4, quantity@5, socketed@11, ear@16, newbie@17, simple@21,
   ethereal@22, personalized@24, runeword@26, chronicle@28-29) + version
   1+2 bits + mode 3 + pos 4 + col 4 + row 4 + stored 3 (−1 ; modes 3/5 =
   or, col/row 16+16).

5. **Items 0x61 : code type encodé HUFFMAN** (4 caractères, table 37
   symboles — D2SaveLibrary). Symptôme : « amu » lu `x~þ` en ASCII brut ;
   le décodage Huffman du même offset donne « hp1 » (4 potions de ceinture,
   vérifiées en x=0..3, y=0) puis « amu », « rin », « box », « uui », « cm1 »
   … **Correction** : table embarquée dans `d2d2s/src/d2r.rs`.

6. **Items 0x61 : layout étendu** — gems 3, guid 32, ilvl 7, quality 4,
   gfx 1+3, class 1+11, qualité (rare/crafted = 8+8 + 6×(1+11)),
   **runeword 12+4** (l'ID stocké = numéro « RunewordNN » + 26 :
   59 → Runeword33 = Jah+Ith+Ber = Enigma ; 39 → Runeword13 = Call to
   Arms ; 67 → Runeword41 = Fortitude ; 86 → Runeword60 = Infinity),
   perso 7 bits/char, realm 1+(4×32), def 11 (−10), maxdur 8 (+dur 9 si
   max>0 — 0 = indestructible : l'Enigma), quantité 9 (Titan's ×140),
   sockets 4, sets 5×1 bit, mods. Sources croisées : D2SaveLibrary,
   d2savior, nokka (pour les largeurs 0x60 identiques).

7. **Largeurs des mods — evidence §8.6 INVALIDE** (« paires min/max par
   nom : stat min + ID+1 stat max »). Le réel prouve : (a) `mindamage` 21
   et `maxdamage` 22 sont des stats **indépendantes** (le Razortail porte
   un `maxdamage` seul) ; (b) les vraies stats combinées sont
   **{17→18, 48→49, 50→51, 52→53, 54→55+56, 57→58+59}** (cold/poison
   incluent leur LENGTH — 25 et 29 bits) ; (c) **`Save Param Bits` se lit
   AVANT la valeur** : « +1 To Teleport » de l'Enigma = ID 97
   (nonclassskill) avec param 9 bits = skill 54 puis valeur 6 bits = 1 —
   l'ancienne règle lisait 6 bits (54) puis désynchronisait le fichier.
   **Correction** : `itemstatcost.rs` (`mod_shape`, `PAIRED`, colonne
   `Save Param Bits` ajoutée au TSV embarqué), `items.rs`/`d2r.rs`.
   Preuve d'ancrage : la liste RW de l'Enigma lit +775 defense, +14 life
   after kill, +45% FRW, +1 Téléport (p54), +2 skills, +0.75 str/lvl,
   +1% MF/lvl — exactement les mods publiés d'Enigma.

8. **IDs uniques : décalage de 1** — l'ID 12 bits stocké = index de table
   − 1 : l'anneau stocke 275 et Raven Frost est à la ligne 276 de
   uniqueitems.txt (l'amulette stocke 276 → ligne 277 = Highlord's Wrath).
   Confirmé par les mods des deux items. Convention d2savior (DB 1-based).

9. **Champ `count` des listes 0x61 périmé possible** — le fichier déclare
   50 items mais en contient 78 (fichier édité). Comportement retenu : les
   items se lisent en continu jusqu'au prochain marqueur sain (« JM » avec
   count plausible < 0x200, « jf », « kf ») ou la fin ; en D2R le mercenaire
   (« jf ») n'a PAS de count.

### 2.3 Ce qui est CONFIRMÉ sans changement (0x60, croisé avec le réel)

Signature 0xAA55AA55 (octets 55 AA 55 AA) ; en-tête 765 o ; quêtes @335
« Woo! » (298 o) ; waypoints @633 « WS » (81 o, bit 0 acte 1 = Rogue
Encampment) ; NPC @714 « w4 » (51 o) ; skills « if » + 30 niveaux (offset
classe Amazon = 6) ; stats (ID 9 bits, CSvBits, ValShift ÷256, 0x1FF,
alignement) ; difficultés 0xA8-0xAA (bit 7 = active, bits 0-2 = acte) ;
classe/niveau 0x28/0x2B ; conteneurs 1/4/5 ; to_summary. Les largeurs
CSvBits des 16 stats de base sont validées par le décodage exact du niveau
99, de l'XP (3 520 485 254) et de l'or.

### 2.4 Limites restantes [NON VÉRIFIÉ]

* Le champ « chronicle » (drapeaux 28-29) : jamais rencontré dans le réel
  (aucun item du fichier ne les pose) — implémenté d'après d2savior, brut
  préservé au round-trip, sémantique non validée.
* La sémantique `Save Add` des mods reste approximative (valeur = raw −
  Save Add, d'après d2savior) ; le round-trip n'en dépend pas (raw conservé).
* L'ID de runeword : mapping « numéro RunewordNN + 26 » validé sur 4
  runewords du réel (33/13/41/60) ; la borne exacte du +25/+26 de D2SLib
  (id < 75) n'est pas vérifiable sur ce seul fichier.
* La « realm data » (1+4×32) : drapeau jamais posé dans le réel.

### 2.5 Livrables de la validation

* `engine-rs/crates/d2d2s/tests/vectors/real/Javazon.d2s` — vecteur doré.
* `engine-rs/crates/d2d2s/tests/real_javazon.rs` — 11 tests (parse strict,
  en-tête, stats, skills, items D2R, uniques/runewords identifiés, round-trip
  octet-exact, summary).
* `engine-rs/crates/d2d2s/src/d2r.rs` — layout D2R complet.
* `engine-rs/crates/d2d2s/src/{checksum,stats,header,itemstatcost,lib}.rs` —
  corrections 1-4 + 7.
* `tools/d2s_parse.py` — copie locale corrigée (checksum, v97, stats « gf »,
  items D2R + Huffman) ; décode Javazon.d2s intégralement, non-régression
  OK sur les vecteurs 0x60/0x5C.
* `docs/d2r-prototypes/` — prototypes d'investigation (python + sources de
  référence D2R).
* Vecteurs synthétiques régénérés (checksums à la formule corrigée ;
  SHA-256 du MANIFEST mis à jour) — `bad_checksum` reste volontairement
  invalide, `corrupt_magic` inchangé.

*Suite (hors périmètre de cette étape) : brancher le save D2R dans le save
v3 du port, mapper les IDs de mods vers les descriptions (MagicPrefix /
MagicalAttribute), décoder les charges d'objets (id 204) pour l'UI.*
