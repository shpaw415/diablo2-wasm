# assets/tbl — Tables de chaînes fr-FR v1.12 (W18, texte in-engine)

Copies de travail de la campagne RE `d2w-re` (LECTURE SEULE) :
`/home/agent/workspace/d2w-re/tables/tbl-fra/` — voir `evidence.md` de ce
dossier pour les sources MPQ exactes, les SHA-256 et la spéc complète du format.

| Fichier | Source MPQ v1.12 | Taille | SHA-256 |
|---|---|---|---|
| `string.tbl` | data/d2data.mpq `data\local\lng\fra\string.tbl` | 419 255 o | `330bd43139e0e37bde46b39118d61922294d7904388464d57c93c59792f0bd78` |
| `expansionstring.tbl` | data/d2exp.mpq `data\local\lng\fra\expansionstring.tbl` | 196 761 o | `08d1604cdb81cc928eb80da7359617db6f4c647c5f809d6951280c177384aaba` |
| `patchstring.tbl` | engine/patch_d2.mpq (55 338 o retenu) | 55 338 o | `e6b6e7b266e45bfa19e608f39af3e59029a221e5af6fd39fc69dcca3565df4a5` |

- `*.tbl` — copies brutes binaires : embarquées dans le WASM via
  `d2rules::tbl::bundled_fra()` (`include_bytes!`) et utilisées comme fixtures
  golden par `engine-rs/crates/d2rules/tests/tbl_golden.rs`.
- `*.tsv` — décodage de référence livré par `tools/tbl_decode.py`
  (`index<TAB>key<TAB>text`, `\t` `\n` `\r` `\\` échappés) : or des golden
  tests, comparaison entrée par entrée (5 391 / 2 818 / 1 163 chaînes,
  UTF-8 strict à 100 %).

Décodage runtime : `d2rules::tbl::Tbl` (port fidèle de `tbl_decode.py`).
Priorité de résolution `TblSet::get` : **patchstring > expansionstring >
string** (convention retail : la table chargée en dernier gagne).
