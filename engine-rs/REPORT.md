# Rapport — port Rust du lecteur MPQ Diablo 2 (`d2mpq` + `d2tool`)

Date : 2026-10-22 · Rust 1.99.1 · Edition 2021 · cible `wasm32-unknown-unknown` ✅
Mise à jour 2026-10-22 (bis) : ajout `d2dc6` + `d2wasm` + démo navigateur — voir §6.

## 1. Implémentation

Workspace Cargo (`crates/d2mpq` lib + `crates/d2tool` bin), port fidèle —
table par table et boucle par boucle — de la référence Python validée
`tools/mpq_mini.py` + `tools/blast_pkware.py` (elle-même dérivée de
`blast.c` v1.3 de Mark Adler, conservée dans `evidence/blast.c`).

### d2mpq (lib, zéro IO fichier — consomme `Vec<u8>`, prête pour WASM)

| Module | Contenu | Fidélité |
|---|---|---|
| `crypto.rs` | table de chiffrement 0x500 entrées (seed 0x00100001, précalculée en `const fn` au compile-time), `hash_string` (types 0–4, ASCII uppercase, `/`→`\`), `decrypt_bytes` / `encrypt_bytes` (chiffrement par dwords Storm) | arithmétique `wrapping` 32 bits identique ; vecteurs de test **générés depuis le Python** et intégrés en constantes |
| `archive.rs` | header MPQ v0/v1 (`MPQ\x1a`, wrapper `MPQ\x1b` avec header_offset @+8 comme le Python), tables hash `<IIHHI` / block `<IIII` déchiffrées avec `hash_string("(hash|block table)", 3)`, sonde linéaire `hash_string(name,0) & (hash_n-1)`, slot libre = locale/platform 0xFFFF, clé fichier type 3 (+FIX_KEY `(key+offset)^fsize`, + key-index), lecture secteurs (table d'offsets déchiffrée `key-1` si ENCRYPTED, secteurs déchiffrés `key`, dernier secteur brut si ≥ sector_size), masques COMPRESS (0x02 zlib, 0x08 BZh-ou-DCL) et IMPLODE (DCL pur sans octet de masque) | chaque branche calquée sur `mpq_mini.py` |
| `dcl.rs` | explode PKWARE DCL complet : tables `LITLEN`/`LENLEN`/`DISTLEN`/`BASE`/`EXTRA` **reproduites telles quelles**, décodeur Huffman à bits inversés, garde de distance permanente (comme le Python dont `first` n'est jamais désactivé) | port direct de `blast.c` via `blast_pkware.py` |
| `error.rs` | `D2Error { NotFound, UnsupportedCompression(u8), Corrupt(&'static str), Io(String) }` + Display/Error | contrat API respecté |
| API publique | `Archive::open(Vec<u8>)`, `Archive::has(&str)` (`/`==`\`, insensible à la casse via hash), `Archive::read(&str)` + accesseurs d'en-tête pour `d2tool probe` | contrat API respecté |

Dépendances : `flate2` (backend pur-Rust miniz_oxide), `bzip2-rs` derrière la
feature par défaut `bz2` (désactivable : `--no-default-features` compile en
natif **et** en WASM, le chemin bz2 renvoie alors une erreur `Corrupt`).
Le `(listfile)` corrompu de 102 o n'est pas utilisé (piège D2 documenté).

### d2tool (CLI, tout l'IO fichier est ici)

`extract <mpq> <name> <out>` · `probe <mpq> <name>` (hit/miss + bloc +
flags décodés, exit 1 si miss) · `sha <mpq> <name>` (SHA-256). Les noms
acceptent indifféremment `\` et `/`.

### Bug trouvé et corrigé pendant la reprise

`encrypt_bytes` pliait le **chiffre** dans la chaîne seed2 ; l'inverse exact
du déchiffrement Python exige de plier le **clair** — c'est bien ce que fait
StormLib (`dwValue32 = DataPointer[i]` est affecté **avant** le XOR dans
`SBaseCommon.cpp`). Corrigé ; test aller-retour (round-trip) 5 clés × 64 o au vert.
(Cette fonction est un confort de test, le contrat de lecture n'en dépend pas.)

## 2. Vérifications SHA-256 — 7/7 OK ✅

Chaque extraction Rust est comparée à l'asset de référence déjà validé par le
pipeline Python. Test d'intégration `crates/d2mpq/tests/verify_game_files.rs`
(auto-skip si le workspace est absent, `D2_WS` pour surcharger) **et** CLI shell.

| # | Archive | Fichier | Référence | Taille | SHA-256 (identique des deux côtés) | Voie testée |
|---|---|---|---|---|---|---|
| 1 | data/d2data.mpq | data\global\excel\armor.txt | assets/armor.txt | 32 810 | `3239878e…cc3ce5b` | zlib multi-secteurs |
| 2 | data/d2data.mpq | data\global\excel\monstats.txt | assets/monstats.txt | 238 019 | `615e62f0…361ff` | zlib multi-secteurs |
| 3 | data/d2data.mpq | data\global\palette\act1\pal.dat | assets/pal_act1.dat | 767 | `de973423…968b3` | zlib petit |
| 4 | data/d2data.mpq | data\global\ui\spells\skillicon.dc6 | assets/skillicon.dc6 | 58 560 | `fe005c08…f65a6` | zlib |
| 5 | data/d2data.mpq | data\global\items\invcap.dc6 | assets/probe_d2data_invcap.dc6 | 1 383 | `6606c3d2…70f0d` | zlib + nom en `/` |
| 6 | engine/patch_d2.mpq | data\global\excel\armor.txt | assets/patch_armor.txt | 76 370 | `d8a4827d…b30fa` | IMPLODE / DCL pur |
| 7 | engine/patch_d2.mpq | data\global\excel\monstats.txt | assets/patch_monstats.txt | 432 635 | `40d480e5…01a39a` | IMPLODE / DCL pur |

`cmp` octet par octet : 7/7 identiques. Tests unitaires crypto : valeurs
`hash_string`/`decrypt_bytes` vérifiées contre le Python (ex. `"(hash table)",3
= 0xc3af3770`, `"(block table)",3 = 0xec83b3a3`). Exemple DCL de l'en-tête
blast.c (`00 04 82 24 25 8f 80 7f` → `AIAIAIAIAIAIA`) au vert.

## 3. Builds

| Cible | Résultat | Taille |
|---|---|---|
| `cargo test` (native) | 9 tests verts (7 unit + 1 intégration + 1 doctest) | — |
| `cargo build --release` | ✅ | d2tool 487 Ko, libd2mpq.rlib 305 Ko |
| `cargo build --target wasm32-unknown-unknown -p d2mpq` | ✅ (debug + release, avec et sans `bz2`) | **d2mpq.wasm release : 282 o** |
| `cargo clippy --all-targets` | 0 warning | — |

**Note sur la taille du .wasm** : le `cdylib` n'expose encore aucun symbole
(l'API est typée Rust ; la couche `wasm-bindgen` est la crate suivante du port),
le linker élimine donc tout le code → 282 o de coquille. La taille utile réelle
se mesurera une fois les exports ajoutés ; l'essentiel (toolchain, dépendances
pures-Rust flate2/bzip2-rs sans C, compilation sans IO) est prouvé.

## 4. Difficultés restantes / limites assumées

1. **Bit de masque 0x04** (DCL « table moderne » post-D2) : le Python le
   gérait, le brief impose `UnsupportedCompression` — D2 ne l'utilise jamais.
   Comportement contractuel, jamais déclenché sur les 7 vérifications.
2. **bzip2** (`0x08` + magie `BZh`) : aucun fichier D2 de test ne l'exerce ;
   code présent (bzip2-rs pur Rust, désactivable par feature) mais non validé
   sur données réelles.
3. **SECTOR_CRC** (0x04000000) : flag reconnu, CRC non vérifié (comme le Python).
4. **FLAG_PATCH / archives patch-chain** : flag exporté, pas de résolution de
   chaîne de patchs (hors périmètre du lecteur).
5. `find()` suppose `hash_n` puissance de 2 (`& (hash_n-1)`) — vrai pour tous
   les MPQ D2 rencontrés, comme le Python.
6. Archive entièrement résidente en mémoire : adapté au navigateur, mais une
   ouverture paresseuse (lecture des seules tables) sera souhaitable pour les
   MPQ de 250 Mo dans un contexte mémoire contraint.

## 5. Reproduire

```sh
export PATH=$HOME/.cargo/bin:$PATH   # cargo 1.99 (rustup)
cd /home/agent/workspace/diablo-2-wasm/engine-rs
cargo test                                   # 9 verts, 7/7 SHA game files
cargo build --target wasm32-unknown-unknown -p d2mpq
target/release/d2tool sha ../data/d2data.mpq "data\global\excel\armor.txt"
```

## 6. Mise à jour — `d2dc6` (DC6) + `d2wasm` (wasm-bindgen) + démo navigateur

### d2dc6 (lib, zéro IO fichier — port fidèle de `tools/dc6_mini.py`)

| Module | Contenu |
|---|---|
| `lib.rs` | Les DEUX layouts de pointeurs (A « flat » v1.12 digital release, B « classic » avec indirection `dirPtr`), validés exactement comme le Python (`_frame_pointers` : tentative A, repli B, sinon `Corrupt`) ; header de frame 28 o (flip/w/h/ox/oy, bornes flip≤1, 0<w,h≤2048) ; `dataLen` @+28, données @+32 (+4 si version≥7) ; RLE avec les DEUX grammaires (variant `encoding=0` polarité inversée + `0x80` sans opérande / classic `encoding=1` avec rewind `0x80 n`), scanlines bottom-up (rangée i → `height-1-i`), runs clampés à la largeur, arrêt après `dataLen` ou `height` lignes → le padding 4 o (0xCD/0xEE) n'est jamais décodé |
| `indices_to_rgba` | palette 768 o (256×RGB) ; index 0 → alpha 0, autres → alpha 255, index hors palette → noir opaque |
| Erreurs | `d2mpq::D2Error` (`Corrupt` avec message précis) |

Vecteurs : digests SHA-256 des pixels **de chaque frame** générés depuis le
Python et intégrés dans `crates/d2dc6/tests/game_files.rs` (skillicon 24×48×48,
invcap 56×56, 8 unitaires sur les deux grammaires/layout/rewind/clamps).

### d2wasm (wasm-bindgen 0.2.129, épinglé — CLI installée en 0.2.129)

`WasmMpq { new(Vec<u8>) -> Result, has(&str), read(&str) -> Option<Vec<u8>> }` ·
`decode_dc6(Vec<u8>) -> [{width,height,offsetX,offsetY,flip,indices:Uint8Array}]` ·
`indices_to_rgba(&[u8], &[u8]) -> Vec<u8>`.

Build : `cargo build --release --target wasm32-unknown-unknown -p d2wasm`
puis `wasm-bindgen --target web --out-dir ../demo/pkg …`.
**Taille `d2wasm_bg.wasm` : 187 277 o (183 Ko)** — (brut cargo 626 571 o,
opt-level=z + LTO + wasm-bindgen).

### demo/ (racine du workspace, statique, sans framework)

Section I : skillicon 8×3 zoom ×3 sur canvas · Section II : `<input type=file>`
MPQ → `WasmMpq` → invcap.dc6 + pal.dat rendus (bouton « asset local » pour la
démo sans archive ; `?mpq=<url>` en deep-link) · Section III : TSV patch_armor
10 colonnes × 12 lignes scrollable · fallback visible si le .wasm ne charge pas
+ collecteur d'erreurs JS (`window.error`/`unhandledrejection` → bandeau).

### Test navigateur (Chromium, ob-chrome)

- `python3 -m http.server 8077 --directory demo` → page OK, **zéro erreur JS**
  (collecteur en page + logs serveur sans 404 hors favicon).
- E2E réel : `?mpq=/data/d2data.mpq` (258 Mo) → archive ouverte en WASM,
  `invcap.dc6` ET `pal.dat` lus dans l'archive, casque rendu 56×56 zoom ×6.
- Capture : `demo/demo-screenshot.png` (icônes rendues ; les 4 frames roses
  en bas à droite sont les frames de remplissage réelles du DC6 — index 132,
  RGB 252,24,92, identiques côté Python).

### État des tests après ajout

`cargo test` : **21 verts** (8+7 unit, 3+1 intégration, 2 doctests) —
aucune régression d2mpq, les 7/7 SHA-256 du §2 restent au vert.
`cargo clippy --all-targets` : 0 warning.

## 7. Mise à jour — sélection de personnage (`heroes[code]`, 2026-10-22)

`D2Game.new` est paramétrable par héros : le navigateur passe le code de
classe choisi et le moteur (sprites + stats) bascule sur CE héros.

### API finale

* **d2rules** — `World::with_hero(grid, manifest, "SOR")` et
  `World::with_seed_and_hero(grid, manifest, seed, "SOR")` ; `World::new` /
  `with_seed` conservés (défaut `"BA"`). Résolution partagée
  `d2rules::resolve_hero_code(&manifest, demandé)` : `heroes[CODE]` si l'objet
  `heroes` existe et contient le code (insensible à la casse), sinon bloc
  `hero` historique — rétro-compat manifest à héros seul, jamais de panic sur
  code inconnu. `World::hero_code()` expose le code effectif ; la sauvegarde
  v1 embarque désormais `hero_code` (champ optionnel : les anciennes saves
  chargent tel quel, version inchangée).
* **d2game** — `new D2Game(manifestJson, hero)` (wasm-bindgen 0.2.129) ;
  `game.heroClass()` → `{"code":"BA","name":"Barbare"}` pour le HUD (valide
  avant `finalize`). `finalize()` construit le SpriteSet du héros depuis
  `heroes[code].walk` (COF + couches DCC via `provide`) et passe la même
  résolution à `World::with_hero` — sprite et stats toujours en phase.
  Monstres/carte/rendu inchangés.
* **demo/game.js** — lit `?hero=` (défaut `BA`), le passe à `D2Game.new`,
  charge les assets du héros résolu et affiche `Barbare · Niv. 1` dans le
  HUD (nom du manifest). Classe absente du manifest ⇒ message en journal
  (pas d'erreur) + partie en BA. Sonde E2E : tout `console.error` (panics
  WASM inclus) alimente `window.__d2.errors` et le titre de l'onglet (⚠ N).
  Sanité croisée JS/Rust de la résolution à chaque boot.

### Tests

`cargo test --workspace` : **tous verts** (d2rules 31 = 28 conservés + 3
nouveaux : stats par code, rétro-compat, save/load avec `hero_code` ;
d2game 19+3 = 17+3 conservés + 2 nouveaux : `heroClass` via `heroes`,
défaut/cas vides). Build WASM release régénéré vers `demo/game-pkg`.

### E2E (Chrome, :8077)

`game.html?hero=SOR` tourne à 60 fps, zéro erreur console — capture
`demo/game-sor-e2e.png`. Les COF SOR/PA sont en cours d'extraction par le
chantier parallèle mais `assets/manifest.json` n'a pas encore de bloc
`heroes` ni les DCC SOR/PA : la page le note et démarre en Barbare. Dès que
`heroes{BA,SOR,PA}` sera dans le manifest, `?hero=SOR` fonctionnera sans
toucher au moteur.
