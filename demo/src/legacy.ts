// Étape 25a — pont vers les modules Legacy chargés par game.js.
//
// Cinq modules Legacy sont chargés PAR game.js lui-même (pas par game.html) :
//   * automap.js    — import statique (`import './automap.js'`)
//   * stash-ui.js, game-sfx.js, cube-ui.js, panels-ui.js — imports dynamiques
//     en bas de game.js (`import("./x.js").catch(() => {})`).
//
// Pendant la transition, le bundle TS doit coexister avec ces Legacy sans
// JAMAIS les exécuter deux fois (ce sont des modules à effets de bord au
// scope : boucle rAF, listeners, setInterval). Solution : chaque module TS
// concerné fait un `await warmLegacy('x.js')` AVANT de décider adopter ou
// remplacer le contrat `window.*` — le spéculateur d'URL (non analysable par
// le bundler) laisse l'import au runtime, sur la MÊME URL absolue que
// game.js : le cache de modules ES garantit une exécution unique, et
// l'ordre d'évaluation devient déterministe quel que soit le navigateur.
//
// En test bun (`bun test src`) il n'y a ni window ni serveur : warmLegacy
// ne fait rien et les modules TS exportent leur logique pure.
export async function warmLegacy(name: string): Promise<void> {
  if (typeof window === 'undefined') return; // bun test : pas de DOM
  const url = new URL(name, import.meta.url).href;
  try {
    await import(url);
  } catch (e) {
    // Legacy absent (futur nettoyage) : l'implémentation TS prend le relais.
    console.info(`[ts-25a] legacy ${name} non chargé (${e instanceof Error ? e.message : String(e)}) — voie TS active.`);
  }
}
