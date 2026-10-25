// Diablo 2 WASM — BOOTSTRAP TS (étape 25a) — bundle demo/main.js servi par
// game.html via <script type="module" src="./main.js"> (1 ligne ajoutée,
// AVANT game.js pour un ordre d'évaluation déterministe).
//
// Rôle : assembler les 18 modules migrés. Chaque module s'installe SOI-MÊME
// au scope (contrats window.* identiques au Legacy) ; les cinq modules aussi
// chargés par game.js (automap, stash-ui, game-sfx, cube-ui, panels-ui)
// warm-importent d'abord la même URL (cache modules ES = exécution unique).
// Pendant la transition, les <script> Legacy de game.html restent en place :
//   * modules à effets de bord au scope (music, mon-sfx, game-sfx, automap,
//     vendor, panels, cube) : la voie TS ADOPTE l'objet Legacy (un seul
//     lecteur audio / une seule boucle rAF / un seul câblage clavier) ;
//   * modules init-driven sans effets de bord (slots, hud, waypoints,
//     corpse-ui, merc-ui, skills-ui, d2s-io, quests, quests45, stash) : la
//     voie TS REMPLACE le contrat avant que game.js n'appelle init().
// Après la transition (suppression des tags Legacy), toutes les voies TS
// complètes prennent automatiquement le relais.
import './legacy';
import './types';
import './slots';
import './hud';
import './waypoints';
import './corpse-ui';
import './music';
import './mon-sfx';
import './game-sfx';
import './automap';
import './vendor';
import './skills-ui';
import './merc-ui';
import './d2s-io';
import './quests';
import './quests45';
import './stash-ui';
import './cube-ui';
import './panels-ui';
import { TS25A_REGISTRY } from './types';

if (typeof window !== 'undefined') {
  (window as unknown as { __d2Ts?: unknown }).__d2Ts = {
    version: '25a',
    step: 'migration TypeScript',
    installed: TS25A_REGISTRY.installed, // voies TS pilotes
    adopted: TS25A_REGISTRY.adopted,     // Legacy adoptés pendant la transition
  };
}

export {};
