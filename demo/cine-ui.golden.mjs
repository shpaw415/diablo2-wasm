// Golden 23b — déclenchement event → cinématique (demo/cine-ui.js, sans DOM).
// Run : node cine-ui.golden.mjs  (depuis demo/)
// Contrat : ActUnlocked{act:1..5} → actN ; GameWon → endgame ; autres events →
// null ; chaque film UNE fois par session (dédupe) ; sources dans assets/cine.
import { readdirSync, openSync, readSync, closeSync } from 'node:fs';
import './cine-ui.js';

const { cineForEvent, movies } = globalThis.D2CineTest;
let fails = 0;
const ok = (cond, label) => {
  if (cond) console.log(`  ok  ${label}`);
  else { console.error(`FAIL  ${label}`); fails++; }
};

// 1) Mapping event → film
const played = {};
ok(cineForEvent({ ActUnlocked: { act: 1 } }, played)?.key === 'act1', 'ActUnlocked(1) → act1.webm');
ok(cineForEvent({ ActUnlocked: { act: 2 } }, played)?.key === 'act2', 'ActUnlocked(2) → act2.webm');
ok(cineForEvent({ ActUnlocked: { act: 3 } }, played)?.key === 'act3', 'ActUnlocked(3) → act3.webm');
ok(cineForEvent({ ActUnlocked: { act: 4 } }, played)?.key === 'act4', 'ActUnlocked(4) → act4.webm');
ok(cineForEvent({ ActUnlocked: { act: 5 } }, played)?.key === 'act5', 'ActUnlocked(5) → act5.webm');
ok(cineForEvent({ GameWon: {} }, played)?.key === 'endgame', 'GameWon → endgame.webm');
ok(cineForEvent({ GoldGained: { amount: 5 } }, played) === null, 'GoldGained → pas de film');
ok(cineForEvent({ HeroLeveled: { level: 2 } }, played) === null, 'HeroLeveled → pas de film');

// 2) Dédupe : re-Jouer le même event ne rejoue pas le film
const p2 = {};
ok(cineForEvent({ ActUnlocked: { act: 2 } }, p2)?.key === 'act2', 'première ActUnlocked(2) joue');
ok(cineForEvent({ ActUnlocked: { act: 2 } }, p2) === null, 'seconde ActUnlocked(2) dédupé (null)');
ok(cineForEvent({ GameWon: {} }, p2)?.key === 'endgame', 'GameWon après act2 joue bien');

// 3) Cas limites
const p3 = {};
ok(cineForEvent({ ActUnlocked: {} }, p3)?.key === 'act1', 'ActUnlocked sans act → défaut act1');
ok(cineForEvent({ ActUnlocked: { act: 9 } }, p3) === null, 'act hors 1..5 → pas de film');
ok(cineForEvent(null, p3) === null, 'event null → pas de film');

// 4) Les 6 films branchés existent physiquement dans assets/cine/
const files = new Set(readdirSync(new URL('./assets/cine/', import.meta.url)));
for (const [k, src] of Object.entries(movies)) {
  const name = src.split('/').pop();
  ok(files.has(name), `asset présent : ${name}`);
}

// 5) Sources .webm valides côté signatur EBML (0x1A45DFA3)
for (const src of Object.values(movies)) {
  const fd = openSync(new URL('./' + src, import.meta.url), 'r');
  const buf = Buffer.alloc(4);
  readSync(fd, buf, 0, 4, 0);
  closeSync(fd);
  ok(buf.equals(Buffer.from([0x1a, 0x45, 0xdf, 0xa3])), `signature EBML : ${src.split('/').pop()}`);
}

console.log(fails === 0 ? 'GOLDEN OK' : `GOLDEN ÉCHEC (${fails})`);
process.exit(fails === 0 ? 0 : 1);
