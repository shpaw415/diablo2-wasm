// Diablo 2 WASM — étape 25b : mapping PUR zone -> thème musique + ambiance
// (migration TS fidèle des tables EXP-A1 + 14c de demo/music.js). Zéro DOM,
// zéro WebAudio : uniquement des données et des fonctions pures, testées par
// bun (zones.test.ts).
//
// Choix des thèmes — fondés sur les fichiers RÉELS de demo/assets/music/
// (« <thème>-t2.wav », extraits des MPQ v1.12, déjà taillés pour boucler) et
// le nommage officiel de music.txt (Diablo II v1.12) :
//   Acte I   : town1 · wild · caves · crypt · monastery · tristram
//   Acte II  : town2 · desert · tombs · sewer · harem · sanctuary · valley
//   Acte III : town3 · jungle · spider · kurast · kurastsewer
//   Acte IV  : town4 · mesa · lair · diablo
//   Events   : *action / *resolution (denofevil, bloodraven, andariel…)
//   Acte V   : aucune piste extraite -> thème null = thème courant conservé.
// L'ambiance (14c) est le miroir du contrat Rust `d2rules::audio_env`
// (SoundEnv réels de levels.txt col. 119 -> record SoundEnviron v1.12).
'use strict';

/** Fichiers thème -> « assets/music/<thème>-t2.wav ». */
export const MUSIC_BASE = 'assets/music/';

/** Boucles « scene » -> « assets/sfx/ambiance/<stem>.wav ». */
export const AMB_BASE = 'assets/sfx/ambiance/';

/** Mapping DATA-DRIVEN clé de niveau -> thème musique (null = aucun). */
export const BY_LEVEL: Readonly<Record<string, string>> = {
  town: 'town1',          // Camp des Rousses (Rogue Encampment)
  bloodmoor: 'wild',      // Landes du Sang (Acte I, extérieur)
  denofevil: 'denofevilaction',
  coldplains: 'wild', stonyfield: 'wild', darkwood: 'wild', blackmarsh: 'wild',
  tamoe: 'wild', tamoehighland: 'wild', bloodmoorwarp: 'wild',
  cave: 'caves', hole: 'caves',
  crypt: 'crypt', mausoleum: 'crypt',
  monasterygate: 'monastery', barracks: 'monastery', jail: 'monastery',
  cathedral: 'monastery', catacombs: 'monastery', andariel: 'andarielaction',
  tristram: 'tristram',
  lutgholein: 'town2',
  rockywaste: 'desert', dryhills: 'desert', faroasis: 'desert',
  lostcity: 'desert', ancienttunnels: 'desert',
  sewers: 'sewer', sewersa2: 'sewer',
  clawviper: 'tombs', talrasha: 'tombs',
  harem: 'harem', palace: 'harem',
  arcane: 'sanctuary', arcaneanctuary: 'sanctuary', sanctuary: 'sanctuary',
  canyon: 'valley', magi: 'valley',
  kurastdocks: 'town3',
  spiderforest: 'jungle', greatmarsh: 'jungle', flayerjungle: 'jungle',
  spidercavern: 'spider', arachnid: 'spider', spider: 'spider',
  kurastbazaar: 'kurast', templedistrict: 'kurast', travincal: 'kurast',
  kurastsewer: 'kurastsewer', ruinedtemple: 'kurastsewer', disusedfane: 'kurastsewer',
  pandemonium: 'town4', pandemoniumfortress: 'town4',
  outersteppes: 'mesa', plainsofdespair: 'mesa', mesa: 'mesa',
  cityofthedamned: 'lair', riverofflame: 'lair', lair: 'lair',
  chaossanctuary: 'diablo', diablo: 'diablo',
};

/** Heuristique par nom (fr + en), ordre du spécifique au générique.
 *  null = zone sans thème dédié (acte V…) -> thème courant conservé. */
export const BY_NAME: ReadonlyArray<readonly [RegExp, string | null]> = [
  [/blood\s?raven|ravenne/i, 'bloodravenresolution'],
  [/andariel/i, 'andarielaction'],
  [/radament/i, 'radamentresolution'],
  [/izual/i, 'izualaction'],
  [/den\s?of\s?evil|antre du mal/i, 'denofevilaction'],
  [/hellforge|forge de l'enfer/i, 'forgeaction'],
  [/horadric|bâton horadrim/i, 'horadricaction'],
  [/tainted\s?sun|soleil (noirci|gâté)/i, 'taintedsunaction'],
  [/diablo|sanctuaire du chaos|chaos/i, 'diablo'],
  [/tristram/i, 'tristram'],
  [/spider\s?cav|arachnid|caverne d'araign/i, 'spider'],
  [/monast|cathédrale|cathedral|catacomb|caserne|barrack|prison|jail|geôl/i, 'monastery'],
  [/crypte|crypt|mausol/i, 'crypt'],
  [/caverne|grotte|\bcave|\bhole\b|\btrou\b/i, 'caves'],
  [/rousses|rogue/i, 'town1'],
  [/lut gholein/i, 'town2'],
  [/harem|palais|palace|sérail|serail/i, 'harem'],
  [/arcane|arcanique/i, 'sanctuary'],
  [/kurast.*(sewer|égout|egout)|(sewer|égout|egout).*kurast/i, 'kurastsewer'],
  [/égout|egout|sewer/i, 'sewer'],
  [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 'tombs'],
  [/maggot|larve/i, 'desert'],
  [/canyon|vallée des|valley of/i, 'valley'],
  [/désert|desert|waste|dry\s?hills|oasis|lost\s?city|cité perdue|tunnels/i, 'desert'],
  [/quai|dock/i, 'town3'],
  [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche|spider/i, 'jungle'],
  [/kurast/i, 'kurast'],
  [/pand[ée]monium/i, 'town4'],
  [/mesa|d[ée]sespoir|despair|marches|steppes/i, 'mesa'],
  [/damn[ée]s|damned|flamme|flame|fleuve|river/i, 'lair'],
  [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 'wild'],
  [/harrogath|arreat|worldstone|baal|acte\s*v|act\s*5/i, null],
];

/** Thème musique d'une zone : clé d'abord (data), puis heuristique par nom.
 *  Retourne null = ni erreur ni silence : le thème courant est conservé. */
export function themeFor(
  key: string | null | undefined,
  name: string | null | undefined,
): string | null {
  if (key && Object.prototype.hasOwnProperty.call(BY_LEVEL, key)) {
    return BY_LEVEL[key] || null;
  }
  if (name) {
    for (const [re, th] of BY_NAME) {
      if (re.test(name)) return th; // peut être null (acte V, etc.)
    }
  }
  return null;
}

// --- AMBIANCE 14c : région -> boucle « scene » (miroir d2rules::audio_env) ---
// SoundEnv = valeurs RÉELLES de `SoundEnv` (levels.txt col. 119) pour les
// clés démo (patch_levels.txt) ; valeur = [boucle jour, boucle nuit] parmi
// les 15 fichiers extraits de d2sfx.mpq. Fallbacks nuit résolus en data
// (57→56, 60→59, 69→68 — même table que audio_env::AMBIENCE_LOOPS).
export const ENV_STEMS: Readonly<
  Record<number, readonly [string | null, string | null]>
> = {
  0: [null, null],
  1: ['scene_wilderness_day', 'scene_wilderness_night'],
  2: ['scene_wilderness_day', 'scene_wilderness_night'],
  3: ['scene_wilderness_day', 'scene_wilderness_night'],
  4: ['scene_cave', 'scene_cave'],
  5: ['scene_crypt', 'scene_crypt'],
  6: ['scene_monastery', 'scene_monastery'],
  7: ['scene_monastery', 'scene_monastery'],
  8: ['scene_catacombs', 'scene_catacombs'],
  9: ['scene_cathedral', 'scene_cathedral'],
  10: ['scene_catacombs', 'scene_catacombs'],
  11: ['scene_catacombs', 'scene_catacombs'],
  12: ['scene_desert_day', 'scene_desert_day'],
  13: ['scene_desert_day', 'scene_desert_day'],
  14: ['scene_desert_day', 'scene_desert_day'],
  15: ['scene_sewer', 'scene_sewer'],
  16: ['scene_harem', 'scene_harem'],
  17: ['scene_harem', 'scene_harem'],
  18: ['scene_catacombs', 'scene_catacombs'],
  19: ['scene_cave', 'scene_cave'],
  20: ['scene_tomb', 'scene_tomb'],
  21: ['scene_tomb', 'scene_tomb'],
  22: ['scene_mesa', 'scene_mesa'],
  23: ['scene_town_3_day', 'scene_town_3_day'],
  24: ['scene_jungle_day', 'scene_jungle_day'],
  25: ['scene_town_3_day', 'scene_jungle_day'],
  26: ['scene_cave', 'scene_cave'],
  27: ['scene_sewer', 'scene_sewer'],
  28: ['scene_cathedral', 'scene_cathedral'],
  29: ['scene_cave', 'scene_cave'],
  30: ['scene_catacombs', 'scene_catacombs'],
  31: ['scene_mesa', 'scene_mesa'],
  32: ['scene_mesa', 'scene_mesa'],
  33: ['scene_lava', 'scene_lava'],
  34: ['scene_lava', 'scene_lava'],
  35: ['scene_wilderness_day', 'scene_wilderness_night'],
  36: ['scene_wilderness_day', 'scene_wilderness_night'],
  37: ['scene_wilderness_day', 'scene_wilderness_night'],
  38: ['scene_wilderness_day', 'scene_wilderness_night'],
  39: ['scene_cave', 'scene_cave'],
  40: ['scene_cave', 'scene_cave'],
  41: ['scene_cave', 'scene_cave'],
  42: ['scene_monastery', 'scene_monastery'],
  43: ['scene_monastery', 'scene_monastery'],
  44: ['scene_catacombs', 'scene_catacombs'],
  45: ['scene_catacombs', 'scene_catacombs'],
  46: ['scene_mesa', 'scene_mesa'],
  47: ['scene_catacombs', 'scene_catacombs'],
  48: ['scene_catacombs', 'scene_catacombs'],
  49: ['scene_cathedral', 'scene_cathedral'],
};

/** SoundEnv par clé de niveau (valeurs réelles patch_levels.txt). */
export const ENV_BY_LEVEL: Readonly<Record<string, number>> = {
  town: 1, bloodmoor: 2, denofevil: 2, coldplains: 2, stonyfield: 2,
  darkwood: 2, blackmarsh: 2, tamoe: 2, graveyard: 2,
  cave: 4, hole: 4, crypt: 5, mausoleum: 5,
  monasterygate: 6, monastery: 6, courtyard: 7, barracks: 8, jail: 8,
  cathedral: 9, catacombs: 10, andariel: 11,
  tristram: 3, lutgholein: 12,
  rockywaste: 13, dryhills: 13, faroasis: 13, lostcity: 13, ancienttunnels: 13,
  canyon: 14, magi: 14,
  sewers: 15, sewersa2: 15, harem: 16, palace: 16, basement: 18,
  clawviper: 19, talrasha: 19, maggotlair: 20, duriel: 21,
  arcane: 22, arcaneanctuary: 22, sanctuary: 22,
  kurastdocks: 23, spiderforest: 24, greatmarsh: 24, flayerjungle: 24,
  spidercavern: 29, arachnid: 29, spider: 29,
  kurastbazaar: 25, templedistrict: 25, travincal: 25,
  kurastsewer: 26, ruinedtemple: 26, disusedfane: 26,
  dungeon: 27, temple: 28, mephisto: 30,
  pandemonium: 31, pandemoniumfortress: 31,
  outersteppes: 32, plainsofdespair: 32, mesa: 32,
  cityofthedamned: 33, riverofflame: 33, lair: 33,
  chaossanctuary: 34, diablo: 34,
  harrogath: 36, siege: 37, barricade: 37, barricadesnow: 38,
  icecave: 39, templeentrance: 43, xtemple: 44, templeboss: 45,
  hell5: 46, baaltemple: 47, throne: 48, worldstone: 49,
};

/** Ordre du spécifique au générique (comme BY_NAME). */
export const ENV_BY_NAME: ReadonlyArray<readonly [RegExp, number]> = [
  [/tristram/i, 3],
  [/town|ville|\bcamp\b|rousses|rogue/i, 1], // TOWN_1 : extérieur « wilderness »
  [/lut gholein/i, 12],                       // TOWN_2 : ambiance désert (réel)
  [/spider\s?cav|arachnid|caverne d'araign/i, 29],
  [/catacomb/i, 10], [/cathédrale|cathedral/i, 9], [/monast|caserne|barrack|prison|jail|geôl/i, 6],
  [/crypte|crypt|mausol/i, 5],
  [/caverne|grotte|\bcave\b|\bhole\b|\btrou\b/i, 4],
  [/harem|palais|palace|sérail|serail/i, 16],
  [/égout|egout|sewer/i, 15],
  [/maggot|larve/i, 20], [/duriel/i, 21],
  [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 19],
  [/arcane|arcanique/i, 22],
  [/quai|dock/i, 23],
  [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche/i, 24],
  [/kurast/i, 25],
  [/donjon|dungeon/i, 27],
  [/temple/i, 28],
  [/lave|lava|flamme|flame|fleuve|river|damn[ée]s|damned/i, 33],
  [/mesa|marches|steppes|d[ée]sespoir|despair/i, 32],
  [/glace|ice/i, 39],
  [/enfer|hell/i, 46],
  [/pierre du monde|world\s?stone/i, 49],
  [/baal/i, 47],
  [/arreat|sommet|mountain/i, 42],
  [/désert|desert|waste|oasis|lost\s?city|cité perdue|tunnels/i, 13],
  [/pand[ée]monium/i, 31],
  [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 2],
];

/** SoundEnv d'une zone : clé d'abord, puis heuristique par nom ; null sinon. */
export function envFor(
  key: string | null | undefined,
  name: string | null | undefined,
): number | null {
  if (key && Object.prototype.hasOwnProperty.call(ENV_BY_LEVEL, key)) {
    return ENV_BY_LEVEL[key] ?? null;
  }
  if (name) {
    for (const [re, env] of ENV_BY_NAME) {
      if (re.test(name)) return env;
    }
  }
  return null;
}

/** Nuit D2 : ≥ 20 h ou < 6 h (pur — l'heure est injectée). */
export function isNight(hour: number): boolean {
  return hour >= 20 || hour < 6;
}

/** Paire [jour, nuit] de stems pour un SoundEnv (null si env inconnu). */
export function ambienceStems(
  env: number | null,
): readonly [string | null, string | null] | null {
  if (env === null) return null;
  return Object.prototype.hasOwnProperty.call(ENV_STEMS, env)
    ? (ENV_STEMS[env] ?? null)
    : null;
}

/** Résolution PURE région -> stem d'ambiance jour/nuit (même contrat que
 *  d2rules::audio_env::ambience). night injecté pour la testabilité. */
export function ambianceFor(
  key: string | null | undefined,
  name: string | null | undefined,
  night: boolean,
): string | null {
  const stems = ambienceStems(envFor(key, name));
  if (!stems) return null;
  return night ? stems[1] : stems[0];
}
