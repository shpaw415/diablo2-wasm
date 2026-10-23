# Implémentation — scène animée de sélection de héros (menu)

> 2026-10-23. Refonte de l'écran de sélection (`demo/menu.html` + `demo/menu.js`
> + `demo/menu.css` — **ces 3 fichiers seuls touchés**) : scène façon Diablo 2
> classique, les 7 classes en animation idle disposées en arc autour du feu de
> camp central, sélectionnables. **Data-driven intégral** : la liste, l'ordre,
> les noms et les stats viennent TOUJOURS de `assets/manifest.json`
> (`heroes{}`) ; l'animation vient du contrat `assets/menu/heroes.json`.

## Contrat d'animation (membre parallèle) et dégradations

Contrat attendu sous `demo/assets/menu/` :
`heroes.json` = `{ CODE: {file, frames, w, h, fps} }` (CODE = AM, AS, BA, DZ,
NE, PA, SO) + `heroes/<CODE>.png` (sheets horizontales) + possiblement
`fire.png` (+ entrée optionnelle `heroes.json.fire` même forme).

| Situation | Comportement |
|---|---|
| `heroes.json` absent (état livré aujourd'hui, ex : Pages) | cartes historiques, à l'identique |
| `heroes.json` présent, ≥ 1 sheet chargée | scène animée (mode `.scene`) |
| sheets partielles | héros sans sheet dessinés en socle grisé « CODE — bientôt », le reste anime ; non sélectionnables |
| une sheet 404 | `Image.onerror` → `anim:null`, aucun plantage |
| `heroes.json.fire` valide | feu du contrat anime `#fireCanvas` (w/h/fps du contrat, prioritaire, coupe l'interval `fire-sheet`) |
| pas de `fire` dans le contrat | `fire-sheet.png` 30 cellules (existant) |
| ni l'un ni l'autre | feu 100 % CSS (`.no-fire`, existant) |

Alias de codes géré : le contrat nomme la Sorcière `SO`, le manifest `SOR`
(`ALIAS` dans menu.js, bidirectionnel). `file` sans `/` est résolu comme
`heroes/<file>` ; un `file` contenant un chemin est respecté tel quel.

## Scène (mode animé)

- Canvas plein écran `#sceneCanvas` 800×600 dans le stage, dessiné par **un
  seul rAF** (early-return si écran titre ou mode cartes — coût nul ailleurs) ;
  `imageSmoothingEnabled=false` + `image-rendering:pixelated`, sprites ×2.5.
- Positions en **pourcentage relatif** (`arcPos`) : x régulier 14 %→86 %,
  pieds sur un arc de cercle au-dessus du feu (63 %→49 %→63 %) — fonctionne
  pour n héros, pas seulement 7.
- Ordre de dessin du fond au premier plan (y croissant) ; le feu DOM
  (`#fireWrap`) reste peint au-dessus du canvas (ordre DOM) : les héros
  centrés passent derrière les flammes, comme l'original.
- Héros sans sheet : socle ellipsé + `CODE — bientôt`, alpha 0.5.

## Sélection et interactions

- **Survol** : halo doré au sol + nom au-dessus (clampé à y≥22 pour ne pas
  passer sous le titre) ; curseur pointer. Hit-box = rect central du sprite
  (35 % de la largeur, hauteur pleine) converti depuis les coords écran.
- **Clic** : encadré doré (glow) + panneau `#sceneStats` (nom, Vie, Mana,
  Force, Dextérité, Compétence — tout du manifest `heroes{}`), à gauche comme
  le panneau D2.
- **Clavier** : ←/→ cycle (wrap) des sélectionnables ; ↑/↓ = ±1 en scène
  (arc), ±4 conservé en mode cartes (grille) ; Entrée = Jouer ; Échap =
  retour au titre. Champ nom du héros conservé (Entrée dedans = jouer,
  Échap = blur). Boutons Jouer/Retour et Continuer inchangés.
- Présélection « dernier héros joué » (`localStorage d2hero`) conservée et
  resynchronisée vers le panneau scène quand le contrat arrive après le
  manifest.

## Performance

Sheets préchargées (`Image` avant activation du mode), une seule boucle rAF
qui ne dessine que si la scène est visible, `setInterval` du logo/feu
inchangés (déjà en place). Aucun layout thrash : tout le rendu héros est
canvas.

## Tests E2E réels (python http.server + Chromium, 2026-10-23)

Contrat simulé temporairement (sheets PIL 7 codes, `SO` pour la Sorcière pour
tester l'alias ; **fichiers retirés après test** — l'arbre est livré sans le
contrat, le membre parallèle déposera les siens) :

1. titre → Solo → scène animée : 7 héros en idle en arc, feu central, AM
   présélectionné avec cadre + panneau. ✓
2. clic DZ (centre) → encadré + halo + panneau « Druide 55/20/15/20, Tempête
   de feu ». ✓ — capture `demo/menu-anim-e2e.png`
3. nom « Kael » + Jouer → `game.html?hero=DZ&name=Kael` charge le monde
   (Camp des Rousses, « Druide · Niv. 1 », 55/55, 59 fps). ✓ — capture
   `demo/game-dz-e2e.png`
4. clavier : préselection dernier joué ✓, ←/→ cycle Druide→Nécromancien→
   Paladin ✓, survol effacé par mouvement réel de souris ✓.
5. feu du contrat : `heroes.json.fire` (flamme bleue de test) affichée à la
   place du `fire-sheet` orange, dimensionnée w/h du contrat, centrée sur
   l'âtre. ✓
6. **Mode dégradé Pages** : `assets/menu` renommé — cartes fallback avec feu
   CSS et boutons dégradés (`demo/menu-degraded-e2e.png`) ; re-vérifié sur
   port vierge (sans cache HTTP) : titre 100 % CSS « DIABLO II » + sélection
   cartes utilisable, aucune erreur bloquante. `assets/menu` remis après. ✓
7. contrat retiré → retour cartes, comportement d'avant intact. ✓

## Corrections annexes (dans les 3 fichiers)

- `#bg` : si l'erreur 404 survient avant l'attachement de l'écouteur
  (`complete && !naturalWidth`), l'image cassée est quand même masquée —
  plus d'icône d'image brisée sur build Pages.
- `#fireCanvas` centré dans `#fireWrap` (`left:50%; bottom:0`) : un feu de
  contrat plus petit que 180×140 reste centré sur l'âtre ; le cas
  `fire-sheet` 180×140 est visuellement identique à avant.

## Périmètre

Aucun autre fichier touché (manifest, game.js, assets, moteur : lecture
seule). Pas de commit git (consigne).
