// save-world.test.ts — tests logiques 4b (sauvegarde du monde coop côté
// serveur). Déroulé sans navigateur : stockage disque (round-trip bit-exact,
// écrasement, refus, purge 7 j) puis salon « reprise » (seed conservée,
// classes par slot imposées, sièges limités, purge branchée sur le sweep).
// `bun test` dans serveur/.
import { describe, test, expect } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, utimesSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { SaveStore, SAVE_MAX_AGE_MS } from "./save-world";
import { Rooms, LobbyError } from "./rooms";

/** saveGame() factice : un save v4 multi-joueurs minimal (le moteur, lui,
 *  est validé par crates/d2rules/tests/save_monde_coop.rs). */
function fakeSave(players: string[] = ["BA", "SOR"], version = 4): string {
  return JSON.stringify({
    version,
    rng: 123,
    now_ms: 2000,
    collision: ["111"],
    manifest: {},
    hero_code: players[0] ?? null,
    hero: {},
    monsters: [],
    items: [],
    players: players.map((code) => ({
      code,
      ent: {},
      stats: {},
      path: [],
      swing_ms: 0,
      dead_ms: 0,
      anim_ms: 0,
      spawn: [0, 0],
      inv: [],
    })),
  });
}

function newStore(opts: { now?: () => number; throttleMs?: number } = {}): { store: SaveStore; dir: string } {
  const dir = mkdtempSync(join(tmpdir(), "d2w-saves-"));
  return { store: new SaveStore(dir, { throttleMs: 0, ...opts }), dir };
}

describe("4b — stockage des saves (saves/<code>.json)", () => {
  test("store → load : round-trip bit-exact, fichier disque, résumé par slot", () => {
    const { store, dir } = newStore();
    const env = store.store("AB234", 0x0d200001, 40, fakeSave());
    expect(env.version).toBe(1);
    expect(env.code).toBe("AB234");
    expect(env.seed).toBe(0x0d200001);
    expect(env.tick).toBe(40);
    expect(env.players).toEqual(["BA", "SOR"], "classes PAR SLOT extraites du save v4");
    expect(env.save).toBe(fakeSave(), "la sortie saveGame() est stockée TELLE QUELLE");
    expect(existsSync(join(dir, "AB234.json"))).toBe(true, "saves/<code>.json sur disque");

    const back = store.load("ab234"); // code insensible à la casse
    expect(back).not.toBeNull();
    expect(back!.save).toBe(env.save);
    expect(back!.seed).toBe(env.seed);
    expect(back!.tick).toBe(env.tick);
    // v3 solo : hero_code seul → un slot
    expect(store.store("CD678", 1, 0, fakeSave(["PA"], 3)).players).toEqual(["PA"]);
  });

  test("re-store : la dernière save écrase la précédente (remplacement atomique)", () => {
    const { store, dir } = newStore();
    store.store("EF789", 5, 10, fakeSave(["BA"]));
    store.store("EF789", 5, 999, fakeSave(["BA", "SOR"]));
    const env = store.load("EF789")!;
    expect(env.tick).toBe(999);
    expect(env.players).toEqual(["BA", "SOR"]);
    // pas de tmp orphelin : le répertoire ne contient QUE le .json final
    const files = readFileSync(join(dir, "EF789.json"), "utf8");
    expect(JSON.parse(files).tick).toBe(999);
  });

  test("refus : code invalide, seed/tick invalides, save illisible, version hors plage, sans héros", () => {
    const { store } = newStore();
    expect(() => store.store("abc", 1, 0, fakeSave())).toThrow("code de salle invalide"); // 4 car.
    expect(() => store.store("../..", 1, 0, fakeSave())).toThrow("code de salle invalide");
    expect(() => store.store("ABCDE", -1, 0, fakeSave())).toThrow("seed invalide");
    expect(() => store.store("ABCDE", 1.5, 0, fakeSave())).toThrow("seed invalide");
    expect(() => store.store("ABCDE", 1, -1, fakeSave())).toThrow("tick invalide");
    expect(() => store.store("ABCDE", 1, 0, "pas du json")).toThrow("save invalide");
    expect(() => store.store("ABCDE", 1, 0, "[]")).toThrow("save invalide");
    expect(() => store.store("ABCDE", 1, 0, fakeSave(["BA"], 99))).toThrow("version");
    expect(() => store.store("ABCDE", 1, 0, JSON.stringify({ version: 4, hero: {}, monsters: [] }))).toThrow(
      "save sans héros",
    );
    expect(store.load("ZZZZZ")).toBeNull();
  });

  test("purge : > 7 jours supprimée, récente gardée ; fallback mtime ; throttle", () => {
    let t = 1_700_000_000_000;
    const { store, dir } = newStore({ now: () => t });
    store.store("OLD01", 1, 0, fakeSave(["BA"]));
    expect(store.purge()).toBe(0, "rien à purger à t");

    // t + 7 j − 1 min : OLD01 (non rafraîchie) survit encore d'un cheveu
    t += SAVE_MAX_AGE_MS - 60_000;
    expect(store.purge(true)).toBe(0, "à 7 jours - 1 min : tout reste");
    // un salon ACTIF re-store (savedAt frais) ; un fichier hérité sans JSON
    // lisible vieillit par son MTIME (fallback)
    store.store("NEW02", 1, 0, fakeSave(["SOR"]));
    const legacy = join(dir, "LEG03.json");
    writeFileSync(legacy, "{corrompu");
    utimesSync(legacy, new Date(t - 30 * 24 * 3600e3), new Date(t - 30 * 24 * 3600e3));

    t += 120_000; // 7 jours + 1 min depuis OLD01
    expect(store.purge(true)).toBe(2, "OLD01 (> 7 j, savedAt) + LEG03 (mtime > 7 j)");
    expect(store.load("OLD01")).toBeNull();
    expect(existsSync(legacy)).toBe(false);
    expect(store.load("NEW02")).not.toBeNull("la save rafraîchie reste (salon actif ou repris)");
    expect(store.count()).toBe(1);

    // throttle : un scan < throttleMs après le précédent est sauté (le sweep
    // des salons passe à chaque requête), sauf avec force
    t += SAVE_MAX_AGE_MS;
    const throttled = new SaveStore(dir, { now: () => t }); // throttle 60 s défaut
    const stale = join(dir, "STL04.json"); // vieux fichier écrit en direct
    writeFileSync(
      stale,
      JSON.stringify({ version: 1, code: "STL04", seed: 1, tick: 0, savedAt: t - SAVE_MAX_AGE_MS - 1, players: ["BA"], save: "{}" }),
    );
    expect(throttled.purge()).toBe(2, "premier scan : STL04 + NEW02 (tous deux > 7 j maintenant) partent");
    writeFileSync(stale, JSON.stringify({ version: 1, code: "STL04", seed: 1, tick: 0, savedAt: t - SAVE_MAX_AGE_MS - 1, players: ["BA"], save: "{}" }));
    expect(throttled.purge()).toBe(0, "throttlé : pas de second scan dans la minute");
    expect(existsSync(stale)).toBe(true, "le fichier attend le prochain créneau");
    expect(throttled.purge(true)).toBe(1, "force court-circuite le throttle");
  });
});

describe("4b — salon « reprise » (from-save)", () => {
  test("seed de la save conservée au start (identité), classes PAR SLOT imposées", () => {
    const rooms = new Rooms();
    const r = rooms.fromSave("Alice", { seed: 0x0d200001, players: ["BA", "SOR", "PA"], origin: "ORIG1" });
    expect(r.view.reprise).toBe(true);
    expect(r.view.seed).toBe(0x0d200001, "le seed est posé DÈS la création (view)");
    expect(r.view.players[0].hero).toBe("BA", "slot 0 = créateur, classe de la save");

    const b = rooms.join(r.code, "Bob");
    expect(b.view.players[1].hero).toBe("SOR", "Bob reprend le slot 1 (ordre d'arrivée)");
    // meta : TOUS les slots de la save (même sans siège repris — le monde
    // chargé a exactement ces héros) + seed dès le lobby
    const mb = rooms.meta(r.code, b.token);
    expect(mb.heroes).toEqual([
      { slot: 0, hero: "BA" },
      { slot: 1, hero: "SOR" },
      { slot: 2, hero: "PA" },
    ]);
    expect(mb.seed).toBe(0x0d200001);

    expect(() => rooms.chooseHero(r.code, b.token, "AM")).toThrow("classe imposée par la save (reprise)");
    expect(rooms.meta(r.code, b.token).heroes[1].hero).toBe("SOR", "le refus n'a rien changé");

    // LE point d'identité : start ne re-tire PAS la seed d'un salon reprise
    const vs = rooms.start(r.code, r.token);
    expect(vs.state).toBe("playing");
    expect(vs.seed).toBe(0x0d200001, "seed de la save intact après le lancement");
    expect(rooms.meta(r.code, b.token).seed).toBe(0x0d200001);
  });

  test("sièges : la reprise est pleine au nb de slots de la save", () => {
    const rooms = new Rooms();
    const r = rooms.fromSave("Alice", { seed: 7, players: ["BA", "SOR"] });
    rooms.join(r.code, "Bob"); // slot 1
    expect(() => rooms.join(r.code, "Clio")).toThrow("salle pleine (reprise : 2 sièges)");
    // un salon normal garde sa capacité et son fonctionnement d'origine
    const n = rooms.create("Zoe");
    expect(n.view.reprise).toBeUndefined();
    expect(n.view.players[0].hero).toBeNull();
    const v = rooms.chooseHero(n.code, n.token, "ba");
    expect(v.players[0].hero).toBe("BA", "chooseHero reste ouvert hors reprise");
    expect(typeof rooms.start(n.code, n.token).seed).toBe("number", "seed tirée au lancement (salon normal)");
  });

  test("refus d'enveloppe : seed invalide, save sans héros", () => {
    const rooms = new Rooms();
    expect(() => rooms.fromSave("Alice", { seed: Number.NaN, players: ["BA"] })).toThrow("seed invalide");
    expect(() => rooms.fromSave("Alice", { seed: 1, players: [] })).toThrow("save sans héros");
    expect(() => rooms.fromSave("Alice", { seed: 1, players: ["x!"] })).toThrow("save sans héros");
  });

  test("la purge est branchée sur le sweep des salons", () => {
    const rooms = new Rooms();
    let sweeps = 0;
    rooms.onSweep(() => {
      sweeps++;
    });
    rooms.create("Alice");
    expect(sweeps).toBeGreaterThanOrEqual(1, "count()/create() passent par le sweep");
  });

  test("erreurs d'export : requireHost/seedOf", () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    expect(() => rooms.requireHost("ZZZZZ", a.token)).toThrow("salle inconnue");
    expect(() => rooms.requireHost(a.code, "mauvais-jeton")).toThrow("jeton refusé");
    expect(() => rooms.requireHost(a.code, b.token)).toThrow("seul l'hôte peut sauvegarder");
    expect(rooms.requireHost(a.code, a.token), "l'hôte passe");
    expect(rooms.seedOf(a.code)).toBeUndefined("pas de seed avant le lancement (pas d'export possible)");
    rooms.start(a.code, a.token);
    expect(typeof rooms.seedOf(a.code)).toBe("number", "seed figée après le lancement");
  });
});
