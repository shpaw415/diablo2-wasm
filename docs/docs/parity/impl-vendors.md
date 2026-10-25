# Implémentation — marchands & gamble (gros morceaux, G1/G2)

> 2026-10-23. Chantier XL de `finish-systemes.md` §G (vendors/gamble). Suite
> directe de `impl-items-loot.md` (la chaîne items est livrée). Modules :
> `engine-rs/crates/d2rules/src/vendor.rs` (NOUVEAU) + `demo/vendor.js`
> (NOUVEAU) + 1 ligne `<script src="./vendor.js">` dans `demo/game.html`.
> **Aucun autre fichier moteur touché** (world.rs, game.rs, game.js, quests.rs
> intacts — contrainte conseil respectée ; seule addition : `pub mod vendor;`
> dans `lib.rs`).

## Données réelles extraites (patch_d2.mpq, `d2tool extract`, 2026-10-23)

| Table | Fichier | Records | Usage |
|---|---|---:|---|
| `gamble.txt` | `assets/patch_gamble.txt` | 125 codes | pool de gamble (sha256 `46a41a1d…c694`) |
| `StorePage.txt` | `assets/patch_storepage.txt` | 4 pages | armo/weap/mag/misc |
| `DifficultyLevels.txt` | `assets/patch_difficultylevels.txt` | 3 diff. | odds gamble + (NM/Hell restant) |

Le reste vient des tables DÉJÀ embarquées : les colonnes vendeurs de
`patch_weapons/armor/misc.txt` (`XxxMin/XxxMax/XxxMagicMin/XxxMagicMax/
XxxMagicLvl` pour les 20 vendeurs, 3 retenus) + `cost` + `gamble cost` +
`PermStoreItem`. **La table fait foi** : Akara ne vend que `hp1`/`mp1` au
v1.12 (les hp2-5 viennent des vendeurs d'actes suivants — le brief disait
« hp1-hp5 », les données tranchent).

## Règles implémentées (vs guide/brief)

1. **Stock par vendeur** — data-driven des colonnes réelles. Sentinelle
   v1.12 : vide/0/255 = non vendu. Exemples goldens testés : Akara `hp1` 1-1
   permanent, `tsc`/`isc` 11-17, baguettes `wnd` 4-6 normales + 7-9 magiques
   (MagicLvl 1) ; Charsi `hax` 0-1 + 1 magique (MagicLvl 1), `cap` MagicLvl 5 ;
   Gheed `key` 5-9, `hax` 1-1.
   - Permanents (`PermStoreItem = 1`) : toujours en rayon (pas de tirage).
   - Autres : **80 % de chance par exemplaire** (brief, guide
     Diablerie/Nefarius ; la formule exacte du guide reste à vérifier — écart
     documenté).
   - Versions magiques : **affixes roulés à la génération** à
     `alvl = max(qlvl, ilvl + MagicLvl vendeur)` (même moteur
     `affixes::pick_magic` que le loot), non identifiés (à identifier après
     achat — D2 réel). Bases normales : identifiées.
   - Misc non magic-capable (les « clés magiques » de la table = données
     mortes v1.12) : colonnes magiques ignorées.
2. **Économie** — achat = `cost` (colonne réelle) ; vente = `cost/4`
   (plancher) ; **réserve d'or** : Gheed 350-750 roulée au seed (guide v1.12),
   Akara/Charsi = constante MVP 10 000 (source table non trouvée — restant) ;
   **un vendeur refuse d'acheter au-delà de sa réserve** (règle D2 réelle) ;
   l'item vendu rejoint son rayon (revendable). Or du héros = PARAMÈTRE de
   `TradeSession` (zéro dépendance `world.rs`).
3. **Gamble (Gheed, MVP)** — pool = 125 codes `gamble.txt` ∩ tables, filtré
   `spawnable && qlvl ≤ niveau vendeur` ; 12 slots ; **non identifiés, prix
   majoré `gamble cost × (100 + 2·clvl)/100`** (rin 50 000 → 51 000 à clvl 1,
   58 000 à clvl 8) ; qualité roulée à la génération avec les poids RÉELS
   `DifficultyLevels` Normal (Rare 10000 / Set 100 / Unique 50 / Uber 90 /
   Ultra 33) — **écart MVP documenté** : magic reçoit le poids de Rare
   (~49 %), Uber/Ultra pliés dans unique ; la formule exacte scalée clvl
   reste à porter. Unique/set sans candidat à ce niveau ⇒ dégradation rare
   (même convention que `loot.rs`).
4. **Restock** — `Vendor::refresh` : même vendeur/niveau, seed dérivé
   (`rotate_left(17) ^ constante`) ; les permanents ne disparaissent jamais.
5. **Déterminisme** — un `Rng` (SplitMix64 du projet) seedé par appel ;
   or du vendeur sur un flux séparé (ne décale pas le tirage du stock) ;
   ordre de tirage fixe : bases (fichiers weap→armo→misc) → compte normal →
   exemplaires (80 %) → compte magique → exemplaires (80 % + affixes).

## Contrat d'intégration exact

### Rust (`d2rules::vendor` — API pure, zéro IO)

```rust
let cat = VendorCatalog::build().unwrap();               // tables embarquées
// stock d'un vendeur au niveau du héros (ilvl des items) :
let vendor = Vendor::generate(&cat, VendorId::Akara, hero_level, seed);
let mut s   = TradeSession::new(vendor, hero_gold);       // or héros EN PARAMÈTRE
let (item, prix) = s.buy(index_rayon)?;                   // Err(NotEnoughGold{..})
let remb  = s.sell(&cat, &item)?;                         // Err(VendorCantAfford{..}) si réserve
let liste = gamble_list(&cat, gheed_level, clvl, seed);   // 12 items non identifiés
let v2    = s.vendor.refresh(&cat, seed);                 // restock seed dérivé
```

Branchement moteur (quand les bindings existeront) : appeler `generate` au
changement de niveau du héros (G1), fournir `hero_gold`/récupérer l'item via
le `World` — **aucun appel dans l'autre sens, vendor.rs n'importe pas
world.rs**. Types exportés : `VendorCatalog, Vendor, VendorId, StockEntry,
TradeSession, TradeError, GambleOdds, VendorCounts, VendorBase`.

### Demo (`demo/vendor.js` — miroir JS, 1 ligne dans game.html)

```js
window.D2Vendor.init(dom, hooks)
// dom   : { toolbar?, root?, log? }        (défauts : #toolbar, #status)
// hooks : { getGold(), spend(n), earn(n), inventory(), heroLevel() }
//          tous optionnels — défauts = lecture window.__d2 (or/inventaire
//          réels AFFICHÉS ; achats/ventes en simulation locale car le moteur
//          WASM n'expose pas encore de mutation or/inventaire).
```

Auto-branché : bouton « Marchands » dans la toolbar, onglets
Akara/Charsi/Gheed/Gamble + Vendre + Restock ; mêmes formules que Rust
(80 %, valeur/4, Gheed 350-750, prix gamble ×(100+2·clvl)/100, pool
qlvl ≤ niveau). Le JS fetch les mêmes TSV (`assets/patch_*.txt`) — rien de
codé en dur. ÉCARTS JS documentés : items « magiques » marqués sans rouler
d'affixes (le Rust fait foi), PRNG miroir mulberry32 (pas SplitMix64).

Vérifié en jeu (60 fps, WASM chargé) : stock Akara/Charsi/Gheed conforme aux
colonnes réelles (wand 205 ⚑/magiques, scrolls 11-17, hp1 30 ⚑, clés Gheed 45),
gamble 12 slots « (?) » au bon prix, onglet Vendre, Restock.
Capture : `docs/parity/vendor-demo.png`.

## Tests

+16 nouveaux dans `vendor.rs` (goldens tables, déterminisme, comptes
vendeurs, marges, réserve, garde-fous, gamble pool/prix/affixes/invariants).
**`cargo test -p d2rules` : 153 passed / 0 failed** (137 → 153). Clippy :
zéro warning nouveau. ⚠ `cargo test --workspace` est ROUGE au moment du
rapport, mais sur `d2game` (imports `MonKind`/`SpawnRule`/`monster_tint`
absents de `world.rs`) — fichiers d'autres membres, en cours d'édition
parallèle ; hors périmètre de ce chantier (d2rules + d2missiles verts).

## Écarts / restant

- **Réparations/durabilité** (F10) : non — dépend de la durabilité d'item.
- **Stock par difficulté** : les colonnes vendeurs sont uniques (pas de
  (N)/(H)) ; le scaling passe par `DifficultyLevels`/niveau vendeur — restant.
- **Formule de gamble exacte** scalée clvl (Uber/Ultra séparés) — restant ;
  MVP = poids réels + magic = poids Rare (documenté).
- **Prix des items magiques** = base seule (formule ItemStatCost des affixes
  — restant, partagé avec le loot).
- **AutoMagic** (baguettes de Akara avec staff mods) : parsé, non branché.
- **MagicLvl d'ITEM** (colonne `magic lvl` de weapons.txt) non additionné à
  l'alvl vendeur (MagicLvl vendeur seul) — écart mineur documenté.
- **item_props** : mods `charged` + records d'affixes à `level` vide
  (ex. « of Telekinesis ») → affixe sans stat — écart partagé avec loot.rs.
- **Bindings WASM or/inventaire** (spend/earn réels depuis le panneau) —
  nécessite d'étendre `d2game` (fichier d'un autre membre) ; le contrat
  `hooks` de vendor.js est prêt à les recevoir.
- Libellés fr (TBL STRING) : projet-wide, restant (EXP-T1/T2).
