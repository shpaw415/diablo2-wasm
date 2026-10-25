# RAPPORT-FIX — Animations du menu (d2w-fix-menu)

Étape : stabilisation + naturalité des animations du menu (écran-titre + sélection de héros).
Fichier livré : **`demo/menu.js`** (seul fichier modifié — disposition, contenu, modes et contrats inchangés).

---

## 1. Causes racines trouvées (avec mesures)

### (a) Écran-titre « instable/saccadé »

**C1 — Horloges `setInterval` non synchronisées avec la vsync (cause principale).**
Le logo et le feu tournaient sur des `setInterval(66 ms)` indépendants du rAF. 66 ms ne tombe
pas sur un multiple de la période d'affichage (16,67 ms à 60 Hz) : les frames duraient
irrégulièrement 3, 4 ou 5 vsyncs (battement), et tout travail principal (chargement, décodage,
changement d'écran) retardait le timer sans rattrapage — des graduels de cadence visibles.
Mesuré avant (sonde rAF + MutationObserver sur les permutations `src` du logo) :

| Mesure avant | Valeur |
|---|---|
| Intervalle réel entre 2 flips logo (régime établi) | p50 = 66,0 ms mais **max = 354,9 ms**, σ = 26,1 ms |
| Pendant le chargement des frames (démarrage) | flips sautés/gelés (test `complete` en vol), pics rAF 46–100 ms |
| Boucle rAF titre (saine, elle) | p50 = 16,70 ms, σ = 0,06 — la vsync n'était pas le problème |

**C2 — Travail DOM lourd dans la boucle :** permutation de `src` de 2 `<img>` par tick
(invalidation peinture à chaque flip) + décodages lancés en cours de route.

### (b) Sélection de héros « héritique, mouvements non naturels »

**C3 — Gel d'entrée de scène (mesuré) :** les planches PNG des 7 héros (jusqu'à 3068×198)
 étaient décodées **sur le thread principal au premier `drawImage`** : freeze mesuré de
 **249,9 ms** juste à l'arrivée sur l'écran (pics 30–98 ms en rafale), puis régime sain.

**C4 — Bug de frames de marche :** `fi = floor((t/1000)*fps) % frames` utilisait **l'heure
globale** (depuis le chargement de la page) même pendant une marche FW/BW : la marche
démarrait au milieu de la planche à une frame arbitraire, puis la fin était testée sur
`(t - start)` — d'où des enjambées incohérentes/téléportées à chaque survol/sélection.

**C5 — Idle en cadence verrouillée :** les 7 héros avançaient de frame **en phase**
(même base de temps, même fps) — effet « numéristes » robotique.

**C6 — Transitions brutales + travail par frame :** halo/nom qui « poppent » sans
fondu, `createRadialGradient` recréé par héros et par frame, `heroList.slice().sort()`
à CHAQUE frame, coordonnées de dessin fractionnaires sous `image-rendering: pixelated`
(scintillement de sous-pixel), feu du contrat sur un 3ᵉ timer désynchronisé du reste.

---

## 2. Corrections (fichier unique : `demo/menu.js`)

1. **HORLOGE UNIQUE** : une seule boucle `requestAnimationFrame` (`mainLoop`) pilote
   logo + feu + scène. Tous les `setInterval` supprimés. Horodatage rAF unique.
2. **Avancement temporel au fps EXACT** : `frame = floor(écoulé × fps) % frames` pour le
   logo (15 fps), le feu (15 fps), le feu du contrat (`heroes.json.fire.fps`) et chaque
   planche de héros (`heroes.json.*.fps`). Plus aucune avance par compteur d'itérations.
3. **Préchargement complet AVANT animation** : les 60 frames logo + fire-sheet + toutes
   les planches de héros sont chargées **et décodées** (`img.decode()`) avant de démarrer ;
   la scène ne s'ouvre qu'une fois tout prêt (`Promise.all` parallèle au lieu des `await`
   séquentiels). Plus aucun décodage sur la boucle, plus aucune frame « en vol ».
4. **Marche corrigée (C4)** : frame déduite du temps écoulé **depuis le début de la
   marche**, jouée une fois de 0 à `frames-1`, puis pose d'état — enjambées complètes.
5. **Idles désynchronisés (C5)** : phase propre à chaque héros (hash déterministe du code),
   appliquée à toutes ses boucles — plus de mouvement en lockstep.
6. **Transitions easées (C6)** : halo de sélection et nom de survol fondent en
   ease-in-out cubic (180/220 ms) depuis la valeur courante — aucun pop, aucun saut.
7. **Zéro travail lourd par frame** : ordre de dessin trié une fois (`buildDrawOrder`),
   positions pixel précalculées (`px/py`), dégradés de halo en cache (coords interprétées
   au remplissage), `getElementById/getContext` sortis de la boucle (feu), permutation
   DOM du logo uniquement quand l'index change.
8. **Snapping entier** : coordonnées `drawImage` arrondies — plus de scintillement.
9. **Dégradations BYO préservées** : frame 0 logo absente → repli 100 % CSS ;
   fire-sheet absente → feu CSS ; contrat/sheets absents → cartes. Testé (assets masqués) :
   les trois replis fonctionnent à l'identique.

## 3. Mesures avant/après (même sonde, Chromium 60 Hz, http local)

| Mesure | Avant | Après |
|---|---|---|
| Gel à l'entrée de la scène (max rAF) | **249,9 ms** | **50 ms** (1 frame, bascule d'écran) |
| rAF sélection, régime établi | 16,7 ms σ=0,06 | 16,7 ms σ=0,05 (0 spike) |
| rAF titre pendant chargement | σ=9,18, pics 100 ms | **σ=0,05, max 16,8 ms** |
| Cadence flips logo (p50 / max / σ) | 66,0 / **354,9** / 26,1 ms | 66,0 / **88,2** / 5,2 ms (1 blip à la bascule Solo) |
| Intervalle flips en régime | 66,0–66,7 ms irréguliers (battement) | 66,0–66,1 ms stables, zéro dérive |

## 4. Vérifications faites

- `node --check demo/menu.js` → OK.
- Visuel Chromium : titre (flammes animées, logo, boutons), sélection (7 héros, feu,
  panneau de stats — disposition identique), marche FW/BW capturée en pleine enjambée
  avec halo en fondu, survol, clavier ←/→/Entrée/Échap.
- Mode dégradé (assets/menu masqués, cache froid) : repli titre 100 % CSS + cartes + feu
  CSS opérationnels, aucune erreur console.
- `diff -r` contre l'arbre maître : **seul `demo/menu.js` diffère** (`assets/music`
  exclue de l'arbre de travail, non utilisée par les animations).
- Page de mesure temporaire (`menu-probe.html`) retirée de l'arbre livré.

## 5. Non touché (périmètres des coéquipiers)

`engine-rs/`, cartes/camp des Rousses (d2w-fix-cartes), orbes/skills/inventaire
(d2w-fix-ui), jambes du héros/échelle caméra (d2w-fix-heros), disposition/contenu/modes
du menu, contrat `heroes.json` et assets.
