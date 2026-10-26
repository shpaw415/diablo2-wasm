// d2w-serveur — build des assets OPTIONNELS (pattern « une ligne = un asset »).
//
// Chaque asset listé ici est publié au client via `web/generated-assets.js`
// (window.D2_ASSETS) : les pages ne fetchent QUE ce qui est listé — un
// fichier absent de la liste ne produit jamais de 404 en console, et un
// consommateur (vendor.js, game.js animdata, menu.js feu de camp) dégrade
// gracieusement. Ajouter un asset : AJOUTER UNE LIGNE ci-dessous puis
// `bun run build` (depuis serveur/). Le fichier lui-même doit exister dans
// web/ (il est servi par static.ts).
//
// Ce script n'utilise pas Bun.build (pas de bundling nécessaire : les pages
// sont des scripts classiques) — c'est le générateur de la liste, exécuté
// par Bun.

const ASSETS: string[] = [
  // --- Tables du marchand (web/vendor.js : achat/vente/gamble) -----------
  "assets/patch_weapons.txt",
  "assets/patch_armor.txt",
  "assets/patch_misc.txt",
  "assets/patch_gamble.txt",
  "assets/patch_difficultylevels.txt",
  // --- AnimData.d2 (frames/vitesses d'animation officiels, moteur) -------
  "assets/animdata/AnimData.d2",
  // --- Menu (web/menu.js) ------------------------------------------------
  // "assets/fire-sheet.png", // feu de camp animé du menu (absent : repli CSS)
];

const out = new URL("../web/generated-assets.js", import.meta.url);
const body =
  "// GÉNÉRÉ par serveur/build.ts (`bun run build`) — NE PAS ÉDITER.\n" +
  "// Source de vérité : la liste ASSETS de serveur/build.ts.\n" +
  "window.D2_ASSETS = Object.freeze(" +
  JSON.stringify(ASSETS, null, 2) +
  ");\n";
await Bun.write(out, body);
console.log(`build : web/generated-assets.js écrit (${ASSETS.length} assets).`);
