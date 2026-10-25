# Plan — Serveur Bun + coop en ligne

État du référentiel pris en compte : moteur déterministe (`d2rules`, 28 tests déterministes,
sauvegarde/chargement bit-exact), rendu isolé dans `d2game` (WASM), assets ~559 Mo dans
`demo/assets`, demo servie ad hoc (tunnel cloudflared → 8077). Aucun serveur permanent aujourd'hui.

## 1. Forme du produit

Un seul processus Bun, un seul port (8077), trois rôles :

1. **CDN du jeu** — sert `demo/` (statique) : HTML, JS, WASM, assets. Cache long + compression.
2. **API de lobby** — créer/rejoindre une partie via `fetch` JSON (rooms, codes courts, présence).
3. **Relais temps réel** — WebSocket natif `Bun.serve`, rooms en mémoire, tick logique partagé.

Parcours joueur : ouvrir le site → menu existant → « Partie en ligne » → créer une salle (code à 5
caractères) ou rejoindre → choix de héros (écran existant) → l'hôte lance → tous chargent le même
niveau. Rien d'autre à installer pour un invité qu'un navigateur.

## 2. Décision d'architecture : relay lockstep (recommandé pour le MVP)

Deux voies possibles :

| Voie | Principe | Coût |
|---|---|---|
| **A. Lockstep relayé** (choisie) | Le serveur ne simule rien : il collecte les entrées de tous les joueurs, les diffuse horodatées par tick fixe (20 Hz), chaque client exécute `d2rules` de façon identique | Faible : le serveur reste un relais + gestion de salons ; le client route déjà ses entrées par une couche unique |
| B. Serveur autoritaire | `d2rules` tourne chez Bun (wasm ou FFI natif), clients ne font que rendre des snapshots | Élevé : prédiction/réconciliation côté client, serialisation complète du monde |

Pourquoi A d'abord : le projet est **déterministe par construction** (tests bit-exact), `d2rules`
est pur et déjà découplé du rendu. Le coop entre amis n'a pas d'enjeu anti-triche. Le delta de code
client se limite à : remplacer la source d'entrée directe par une file synchronisée au tick, et
ajouter un buffer de jitter (2–3 ticks ≈ 100–150 ms de latence d'entrée, imperceptible en coop).

Garde-fou anti-désynchro : hash de l'état `d2rules` toutes les 60 ticks, envoyé au serveur, qui
rompt le jeu et affiche l'erreur si divergence (le hash existe déjà via la sauvegarde bit-exact).

Évolution naturelle (phase 4+) : basculer vers B en réutilisant le même crate `d2rules` compilé
pour Bun — le travail de sérialisation des snapshots profitera alors au replay et à l'anti-triche.

## 3. Interfaces

### REST (lobby, JSON)
- `POST /api/rooms` → crée une salle `{ code, hostToken }`
- `POST /api/rooms/:code/join` → `{ playerToken, slot }` (refus si pleine ou en partie)
- `GET /api/rooms/:code` → état lobby (slots, classes prises)
- `POST /api/rooms/:code/start` (hôte seulement) → passe en phase jeu

### WebSocket (jeu, binaire après le handshake)
Trame = 1 octet opcode + charge utile. Contrôle en JSON (lobby), jeu en binaire.

| Opcode | Sens | Contenu |
|---|---|---|
| `0x01 hello` | C→S | version, nom, classe |
| `0x02 lobby` | S→C | slots/presence |
| `0x03 start` | S→C | seed du monde, liste des héros |
| `0x10 input` | C→S | tick visé, masque d'actions (déplacement, sorts, objets) |
| `0x11 tick` | S→C | ensemble d'entrées pour le tick N (diffusé à tous) |
| `0x12 hash` | C→S | hash d'état du tick N |
| `0x20 desync` | S→C | arrêt propre |
| `0x30 ping/pong` | S↔C | RTT, mesure de buffer |

Formats : seed + tick en u32/u64 little-endian ; entrées en masque de bits compact (quelques
octets/joueur/tick). Le journal des ticks récents est conservé pour la reconnexion (replay rapide).

### Statique
- WASM servi en `application/wasm`, `Cache-Control: immutable` sur les fichiers fingerprintés ;
  `no-cache` sur HTML/JS d'entrée ; brotli sur texte ; `Cache-Control` court sur `assets/`.
- Point d'entrée coop : extension de `menu.html` (écran « Partie en ligne »), pas de nouvelle page.

## 4. Structure de fichiers proposée

```
serveur/
  server.ts        # Bun.serve : statique + upgrade WS + routes /api
  rooms.ts         # cycle de vie des salons (lobby → jeu → fin), timeouts
  relay.ts         # collecte/diffusion des ticks, buffers, hashes
  protocol.ts      # opcodes, encodage/décodage (partagé avec le client)
  static.ts        # en-têtes cache, mime, brotli
demo/
  net-client.js    # client WS : file d'entrées, buffer de jitter, hash d'état
  online-ui.js     # écran de salon dans le menu existant
```

Côté moteur : `d2game` garde sa boucle de rendu 60 fps ; la simulation reste à tick fixe ; seul
change l'alimentation en entrées (`game.js` pousse dans la file réseau au lieu d'appeler
directement). Aucune modification de `d2rules`.

## 5. Phases

1. **Phase 1 — Serveur statique Bun** ✅ **livrée 2026-10-25** : `serveur/server.ts` +
   `serveur/static.ts` (processus persistant `d2w-serveur`, port 8077). Politique cache :
   `assets/` immuable 7 j, `game-pkg/` revalidé par ETag (304), entrées HTML/JS `no-cache`,
   brotli en mémoire sur le texte ≤ 1,5 Mo (via node:zlib — `Bun.brotliCompressSync`
   n'existe pas en Bun 1.4.4). `/api/health` en place. Validé : ETag/304, MIME wasm,
   404/403 traversal/405, brotli identité+br, menu rendu au navigateur
   (`docs/phase1-serveur-e2e.png`). Démarrage : `bun serveur/server.ts` (env `PORT`, `HOST`).
2. **Phase 2 — Lobby** ✅ **livrée 2026-10-25** : `serveur/rooms.ts` (salons en mémoire,
   présence par ping implicite sur le GET, sweep 75 s — calibré sur le bridage Chrome des
   timers d'onglets en arrière-plan, 1/min —, transfert d'hôte, codes 5 caractères sans
   ambiguïté), routes REST complètes (`create/join/view/start/leave`), et écran
   « Partie en ligne » dans le menu (`demo/online-ui.js`, contrat `window.D2Online` +
   `window.D2Menu`). Validé en curl (tous les codes d'erreur) et en E2E navigateur à deux
   onglets : création, jonction, survie 68 s en arrière-plan, lancement hôte → les deux
   clients passent à la sélection de héros avec le bandeau de salle, `Jouer` relaie
   `?online=CODE` vers game.html (`docs/phase2-lobby-e2e.png`). Les salons sont en mémoire :
   un redémarrage du serveur les efface (acceptable en phase 2).
3. **Phase 3 — Coop lockstep** 🔶 **3a livrée 2026-10-25** (transport complet, monde miroir) :
   - `serveur/relay.ts` : relais binaire 20 Hz (`/api/ws`, upgrade validé par siège) —
     opcodes 0x01 hello / 0x10 input / 0x11 bundle / 0x12 hash / 0x20 desync / 0x21 hôte
     perdu / 0x23-0x25 snapshot / 0x30 ping. Le serveur ne simule rien : bundles d'entrées
     identiques diffusés à tous (slots vides inclus), ordre = slots.
   - `demo/net-client.js` : buffer de jitter 3 ticks, dt fixe 50 ms réparti entier sur les
     événements + clôture (déterministe), burst de rattrapage 8 ticks/frame au retour de
     visibilité, hash FNV-1a de `saveGame()` toutes les 60 ticks, catchUp() avant snapshot
     (l'hôte endormi photographie un état à jour), reprise invité par `loadGame()`.
   - `game.js` : mode `?online=CODE[&token=]` — siège par URL (harnais/E2E) ou sessionStorage ;
     rôle tranché par `/meta` (jamais par le client) ; invité attend le héros publié par
     l'hôte ; les deux simulent le MÊME monde (miroir).
   - Validé E2E : deux onglets, mondes identiques au pixel (position 177,57 → 182,65 suivie
     des deux côtés), snapshot récent (tick 499), aucune désync ; `docs/phase3a-miroir-hote.png`
     / `phase3a-miroir-invite.png`.
   - **3b restant (chantier moteur)** : multi-héros — `d2rules`/`d2game` ne modélisent qu'UN
     héros (`World.hero`, allié mercenaire câblé en dur). Il faut `Vec<Player>` (ciblage IA,
     combat, drops/XP par joueur, save, compose/caméra par slot, input par slot) + le
     rattachement de l'écran salon. En 3a, le monde miroir est piloté par le héros de l'hôte ;
     les entrées des invités sont relayées et appliquées au héros du salon (coop « canapé »).
4. **Phase 4 — Robustesse** : reconnexion mi-partie, spectateur, sauvegarde du monde côté serveur
   (réutilise le format bit-exact), puis éventuelle migration autoritaire.

## 6. Risques et réponses

- **Non-déterminisme résiduel** (floats, itérations d'HashMap) : `d2rules` est déjà testé
  déterministe ; ajouter le hash périodique + un test CI à deux exécutions sur les mêmes entrées.
- **Latence variable web** : buffer de jitter adaptatif mesuré par ping ; dégradation gracieuse
  (le tick s'étire plutôt que de figer).
- **559 Mo d'assets** : inchangés par le coop ; le cache HTTP + le fait que chaque client les a
  déjà (extraction locale) rendent ça invisible après la première visite.
- **Tunnel cloudflared** : WS passe sans config particulière ; un seul port à exposer.
