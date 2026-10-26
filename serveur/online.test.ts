// online.test.ts — tests logiques 3c (salon → jeu, retraits explicites) et
// 4a (robustesse : reconnexion mi-partie, spectateur, garde au camp).
// Déroulé E2E sans navigateur : REST lobby (créer/rejoindre/choix de classe
// PAR SLOT/vue/meta/start) puis relais lockstep (bundles par slot,
// reconnexion même token/même slot, spectateur hors slots, guard_camp).
// `bun test` dans serveur/.
import { describe, test, expect } from "bun:test";
import { Rooms } from "./rooms";
import { Relay, TICK_MS } from "./relay";

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

/** Bundles 0x11 d'une liste de trames (les SNAP_REQ/pong du hôte sont
 *  ignorés — même format que web/net-client.js parseBundle). */
function bundles(frames: Uint8Array[]): { tick: number; entries: { slot: number; json: string }[] }[] {
  const out: { tick: number; entries: { slot: number; json: string }[] }[] = [];
  for (const buf of frames) {
    const v = new DataView(buf.buffer, buf.byteOffset, buf.byteLength);
    if (v.getUint8(0) !== 0x11) continue;
    const tick = v.getUint32(1, true);
    const cnt = v.getUint8(5);
    const dec = new TextDecoder();
    let o = 6;
    const entries: { slot: number; json: string }[] = [];
    for (let i = 0; i < cnt; i++) {
      const slot = v.getUint8(o);
      const len = v.getUint16(o + 1, true);
      entries.push({ slot, json: dec.decode(new Uint8Array(buf.buffer, buf.byteOffset + o + 3, len)) });
      o += 3 + len;
    }
    out.push({ tick, entries });
  }
  return out;
}

/** Opcodes S→C reçus par un socket (premier octet de chaque trame). */
function opcodes(frames: Uint8Array[]): number[] {
  return frames.map((b) => new DataView(b.buffer, b.byteOffset, b.byteLength).getUint8(0));
}

/** Trame cliente (op + JSON) — même encodage que net-client.js jsonFrame. */
function jsonFrame(op: number, obj: unknown): ArrayBuffer {
  const b = new TextEncoder().encode(JSON.stringify(obj));
  const out = new Uint8Array(3 + b.length);
  out[0] = op;
  out[1] = b.length & 255;
  out[2] = b.length >> 8;
  out.set(b, 3);
  return out.buffer;
}

/** Trame 0x24 « snapshot de l'hôte » — [tick:u32][len:u32][json]. */
function hostSnapFrame(tick: number, save: string): ArrayBuffer {
  const b = new TextEncoder().encode(save);
  const out = new Uint8Array(9 + b.length);
  const v = new DataView(out.buffer);
  v.setUint8(0, 0x24);
  v.setUint32(1, tick, true);
  v.setUint32(5, b.length, true);
  out.set(b, 9);
  return out.buffer;
}

describe("salon 3c — choix de classe PAR SLOT (REST)", () => {
  test("chaque slot publie SA classe ; vue + /meta redistribuent par slot", () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");

    // avant choix : classes absentes (null), pas de héros global
    const v0 = rooms.view(a.code, a.token);
    expect(v0.players.map((p) => p.hero)).toEqual([null, null]);

    // CHAQUE siège choisit SA classe (vérif par slot — pas seulement global)
    const va = rooms.chooseHero(a.code, a.token, "ba");
    expect(va.players[0].hero).toBe("BA");
    const vb = rooms.chooseHero(a.code, b.token, "SOR");
    expect(vb.players[0].hero).toBe("BA");
    expect(vb.players[1].hero).toBe("SOR");

    // /meta : liste PAR SLOT complète pour chaque siège (monde multi-héros
    // identique chez tous) + slot du demandeur
    const ma = rooms.meta(a.code, a.token);
    const mb = rooms.meta(a.code, b.token);
    expect(ma.heroes).toEqual([
      { slot: 0, hero: "BA" },
      { slot: 1, hero: "SOR" },
    ]);
    expect(mb.heroes).toEqual(ma.heroes);
    expect(ma.slot).toBe(0);
    expect(mb.slot).toBe(1);

    // start (hôte) : seed figée, la liste par slot survit au lancement
    const vs = rooms.start(a.code, a.token);
    expect(vs.state).toBe("playing");
    expect(typeof vs.seed).toBe("number");
    const ms = rooms.meta(a.code, b.token);
    expect(ms.state).toBe("playing");
    expect(ms.heroes).toEqual([
      { slot: 0, hero: "BA" },
      { slot: 1, hero: "SOR" },
    ]);
    expect(typeof ms.seed).toBe("number");
  });

  test("refus : salle inconnue 404, jeton refusé 403, classe invalide 422", () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    expect(() => rooms.chooseHero("ZZZZZ", a.token, "BA")).toThrow("salle inconnue");
    expect(() => rooms.chooseHero(a.code, "jeton-mauvais", "BA")).toThrow("jeton refusé");
    expect(() => rooms.chooseHero(a.code, a.token, "x!")).toThrow("classe invalide");
    expect(() => rooms.chooseHero(a.code, a.token, "")).toThrow("classe invalide");
    // l'état n'a pas bougé
    expect(rooms.view(a.code, a.token).players[0].hero).toBeNull();
  });

  test("la classe survit au départ d'un AUTRE joueur (slots stables)", () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    const c = rooms.join(a.code, "Clio");
    rooms.chooseHero(a.code, a.token, "BA");
    rooms.chooseHero(a.code, b.token, "SOR");
    rooms.chooseHero(a.code, c.token, "PA");
    rooms.leave(a.code, b.token); // Bob part AVANT le lancement
    const v = rooms.view(a.code, a.token);
    expect(v.players.map((p) => p.hero)).toEqual(["BA", "PA"], "le choix de Clio suit son slot");
    expect(rooms.slotOf(a.code, c.token)).toBe(1, "Clio compacte au slot 1 côté lobby");
  });
});

describe("relais 4a — déconnexion = héros conservé, reconnexion même slot", () => {
  test("déco en jeu : PAS de leave, eslots stables ; reconnexion même token → même slot", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    const c = rooms.join(a.code, "Clio");
    rooms.chooseHero(a.code, a.token, "BA");
    rooms.chooseHero(a.code, b.token, "SOR");
    rooms.chooseHero(a.code, c.token, "PA");
    rooms.start(a.code, a.token);

    const sent = new Map<unknown, Uint8Array[]>();
    const closed: unknown[] = [];
    const relay = new Relay(
      rooms,
      (ws, data) => {
        if (!sent.has(ws)) sent.set(ws, []);
        sent.get(ws)!.push(data);
      },
      (ws) => closed.push(ws),
    );
    const wsA = { id: "A" }, wsB = { id: "B" }, wsC = { id: "C" };
    relay.attach(wsA, a.code, a.token); // hôte : le tick global démarre
    relay.attach(wsB, a.code, b.token);
    relay.attach(wsC, a.code, c.token);

    // Clio (siège 2) joue : son entrée revient étiquetée SON slot (2).
    relay.message(wsC, jsonFrame(0x10, [{ type: "moveTo", x: 180, y: 60 }]));
    await sleep(TICK_MS * 2 + 30);
    const before = bundles(sent.get(wsA)!);
    const cEntry = before.flatMap((f) => f.entries).find((e) => e.slot === 2 && e.json !== "[]");
    expect(cEntry).toBeDefined("l'entrée de Clio revient étiquetée slot 2");

    // Bob (slot 1) se DÉCONNECTE en jeu — 4a : son héros RESTE au monde.
    const mark = sent.get(wsA)!.length;
    relay.detach(wsB);
    await sleep(TICK_MS * 2 + 30);
    const frames = bundles(sent.get(wsA)!.slice(mark));
    expect(frames.length).toBeGreaterThan(0, "le lockstep continue pendant l'absence");
    // aucun {type:"leave"} ne voyage (le héros n'est PAS retiré du monde)
    const leaves = frames.flatMap((f) => f.entries).filter((e) => e.json.includes("leave"));
    expect(leaves).toEqual([], "4a : une déconnexion n'émet plus {type:\"leave\"}");
    // le slot du partant continue d'ÊTRE ÉMETTRE (entrées vides — IA idle)
    const empties = frames.flatMap((f) => f.entries).filter((e) => e.slot === 1);
    expect(empties.some((e) => e.json === "[]")).toBe(true, "le slot 1 émet des bundles vides (héros idle)");
    // aucune trame 0x22 « slot libre » : le siège est RÉSERVÉ
    expect(opcodes(sent.get(wsA)!.slice(mark)).filter((o) => o === 0x22)).toEqual([]);

    // Clio continue de jouer : son entrée reste étiquetée slot 2 (AUCUNE
    // compaction — les slots moteurs sont stables pendant une absence).
    const mark2 = sent.get(wsA)!.length;
    relay.message(wsC, jsonFrame(0x10, [{ type: "moveTo", x: 182, y: 62 }]));
    await sleep(TICK_MS * 2 + 30);
    const after = bundles(sent.get(wsA)!.slice(mark2));
    const cAfter = after.flatMap((f) => f.entries).find((e) => e.slot === 2 && e.json.includes("182"));
    expect(cAfter).toBeDefined("slot de Clio inchangé (2) — pas de remappage sur une déconnexion");

    // RECONNEXION : même token → MÊME slot/eslot, ses entrées repartent
    // étiquetées slot 1 (le héros qui n'a jamais quitté le monde).
    relay.attach(wsB, a.code, b.token);
    const mark3 = sent.get(wsA)!.length;
    relay.message(wsB, jsonFrame(0x10, [{ type: "moveTo", x: 10, y: 10 }]));
    await sleep(TICK_MS * 2 + 30);
    const reco = bundles(sent.get(wsA)!.slice(mark3));
    const bAfter = reco.flatMap((f) => f.entries).find((e) => e.slot === 1 && e.json.includes("10"));
    expect(bAfter).toBeDefined("le reconnecté joue SON slot (1) — même token, même slot");

    relay.detach(wsA);
    relay.detach(wsB);
    relay.detach(wsC);
  });

  test("timeout : héros orphelin (déco > délai) → un {type:\"guard_camp\"} unique sur SON slot", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    rooms.chooseHero(a.code, a.token, "BA");
    rooms.chooseHero(a.code, b.token, "SOR");
    rooms.start(a.code, a.token);

    const sent = new Map<unknown, Uint8Array[]>();
    const relay = new Relay(
      rooms,
      (ws, data) => {
        if (!sent.has(ws)) sent.set(ws, []);
        sent.get(ws)!.push(data);
      },
      () => {},
      { guardCampMs: 120 }, // test : délai réduit
    );
    const wsA = { id: "A" }, wsB = { id: "B" };
    relay.attach(wsA, a.code, a.token);
    relay.attach(wsB, a.code, b.token);

    relay.detach(wsB); // Bob disparaît
    await sleep(TICK_MS * 6 + 40); // > guardCampMs + quelques ticks
    const frames = bundles(sent.get(wsA)!);
    const guards = frames.flatMap((f) => f.entries).filter((e) => e.json.includes("guard_camp"));
    expect(guards.length).toBeGreaterThanOrEqual(1, "le guard_camp a voyagé");
    expect(guards.every((e) => e.slot === 1)).toBe(true, "étiqueté du slot du héros orphelin");
    expect(JSON.parse(guards[0].json)).toEqual([{ type: "guard_camp" }]);
    // UNE SEULE FOIS : les bundles suivants redeviennent vides (le client
    // résout lui-même le MoveTo vers le spawn — cf. net-client.js).
    const late = frames.slice(frames.findIndex((f) => f.entries.some((e) => e.json.includes("guard_camp"))) + 1);
    expect(
      late.flatMap((f) => f.entries).filter((e) => e.slot === 1 && e.json !== "[]"),
    ).toEqual([], "guard_camp émis une seule fois");

    relay.detach(wsA);
  });
});

describe("relais 4a — reprise par replay (hello resume) ou snapshot", () => {
  test("retard DANS le journal : resume annule le snapshot, replay seul au reconnecté", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    rooms.start(a.code, a.token);

    const sent = new Map<unknown, Uint8Array[]>();
    const relay = new Relay(
      rooms,
      (ws, data) => {
        if (!sent.has(ws)) sent.set(ws, []);
        sent.get(ws)!.push(data);
      },
      () => {},
    );
    const wsA = { id: "A" }, wsB = { id: "B" };
    relay.attach(wsA, a.code, a.token); // hôte : le tick démarre
    await sleep(TICK_MS * 4 + 20); // le journal avance (~4 ticks)
    const snapMark = sent.get(wsA)!.length;
    relay.attach(wsB, a.code, b.token); // retour : demande de snapshot posée
    expect(opcodes(sent.get(wsA)!.slice(snapMark)).filter((o) => o === 0x23)).toHaveLength(1, "SNAP_REQ vers l'hôte");

    // le retour annonce SON dernier tick exécuté (monde en mémoire) — le
    // retard tient dans le journal : replay SEUL, snapshot annulé.
    const lastKnown = bundles(sent.get(wsB) ?? []).at(-1)?.tick ?? 0;
    const resume = Math.max(1, lastKnown - 1);
    const markB = (sent.get(wsB) ?? []).length;
    relay.message(wsB, jsonFrame(0x01, { hero: "SOR", name: "Bob", resume }));
    await sleep(TICK_MS * 2 + 30);

    // l'hôte répondrait à la demande : personne n'attend plus de snapshot
    relay.message(wsA, hostSnapFrame(1, "{}"));
    await sleep(TICK_MS * 2 + 30);
    const framesB = (sent.get(wsB) ?? []).slice(markB);
    expect(opcodes(framesB).filter((o) => o === 0x25)).toEqual([], "resume dans le journal ⇒ AUCUN snapshot");
    const replayed = bundles(framesB).map((f) => f.tick);
    expect(replayed.length).toBeGreaterThan(0, "les bundles manquants sont renvoyés");
    expect(Math.min(...replayed)).toBe(resume + 1, "le replay reprend EXACTEMENT au tick manquant");

    relay.detach(wsA);
    relay.detach(wsB);
  });

  test("retard HORS journal : le snapshot 0x25 est maintenu pour la reprise", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    rooms.start(a.code, a.token);

    const sent = new Map<unknown, Uint8Array[]>();
    const relay = new Relay(
      rooms,
      (ws, data) => {
        if (!sent.has(ws)) sent.set(ws, []);
        sent.get(ws)!.push(data);
      },
      () => {},
      { journalTicks: 2 }, // journal minuscule : tout retard le dépasse
    );
    const wsA = { id: "A" }, wsB = { id: "B" };
    relay.attach(wsA, a.code, a.token);
    await sleep(TICK_MS * 4 + 20);
    relay.attach(wsB, a.code, b.token);
    relay.message(wsB, jsonFrame(0x01, { hero: "SOR", name: "Bob", resume: 1 })); // tick 1 hors journal
    const markB = (sent.get(wsB) ?? []).length;
    relay.message(wsA, hostSnapFrame(4, "{\"v\":4}"));
    await sleep(TICK_MS * 2 + 30);
    const framesB = (sent.get(wsB) ?? []).slice(markB);
    const snaps = framesB.filter((f) => new DataView(f.buffer, f.byteOffset, f.byteLength).getUint8(0) === 0x25);
    expect(snaps).toHaveLength(1, "retard hors journal ⇒ snapshot 0x25 diffusé");
    const v = new DataView(snaps[0].buffer, snaps[0].byteOffset, snaps[0].byteLength);
    expect(v.getUint32(1, true)).toBe(4, "le tick du snapshot est porté tel quel");

    relay.detach(wsA);
    relay.detach(wsB);
  });

  test("hello WS publie la classe PAR SLOT (voie jeu, rétro-compat 3b)", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    const sent = new Map<unknown, Uint8Array[]>();
    const relay = new Relay(rooms, (ws, d) => sent.set(ws, [d]), () => {});
    const wsA = { id: "A" }, wsB = { id: "B" };
    relay.attach(wsA, a.code, a.token);
    relay.attach(wsB, a.code, b.token);
    // CHAQUE siège publie sa classe au hello — la liste par slot alimente /meta.
    relay.message(wsA, jsonFrame(0x01, { hero: "BA", name: "Alice" }));
    relay.message(wsB, jsonFrame(0x01, { hero: "sor", name: "Bob" }));
    expect(rooms.meta(a.code, b.token).heroes).toEqual([
      { slot: 0, hero: "BA" },
      { slot: 1, hero: "SOR" },
    ]);
    relay.detach(wsA);
    relay.detach(wsB);
  });
});

describe("relais/salon 4a — spectateur (rejoint en cours, sans héros)", () => {
  test("join en cours = spectateur : hors slots, méta slot -1, héros des sièges intacts", () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    rooms.chooseHero(a.code, a.token, "BA");
    rooms.chooseHero(a.code, b.token, "SOR");
    rooms.start(a.code, a.token); // partie lancée

    // 4a : rejoindre maintenant = SPECTATEUR (ex-409 « partie déjà lancée »)
    const s = rooms.join(a.code, "Spec");
    const v = rooms.view(a.code, s.token);
    expect(v.you.spectator).toBe(true);
    expect(v.players.at(-1)?.spectator).toBe(true);
    expect(v.players.filter((p) => p.spectator).length).toBe(1);

    // hors numérotation : les héros gardent leurs slots d'arrivée
    expect(rooms.slotOf(a.code, a.token)).toBe(0);
    expect(rooms.slotOf(a.code, b.token)).toBe(1);
    expect(rooms.slotOf(a.code, s.token)).toBe(-1, "un spectateur n'a pas de slot");
    const ms = rooms.meta(a.code, s.token);
    expect(ms.slot).toBe(-1, "/meta : slot -1 pour un spectateur");
    expect(ms.heroes).toEqual([
      { slot: 0, hero: "BA" },
      { slot: 1, hero: "SOR" },
    ], "les héros du monde ne changent pas (le spectateur n'y figure pas)");
    // un second spectateur passe aussi, un 9e serait refusé (plafond 8)
    expect(rooms.join(a.code, "Spec2").view.you.spectator).toBe(true);
  });

  test("relais : 0x26 à l'attachement, entrées ignorées, jamais dans les bundles", async () => {
    const rooms = new Rooms();
    const a = rooms.create("Alice");
    const b = rooms.join(a.code, "Bob");
    rooms.start(a.code, a.token);
    const s = rooms.join(a.code, "Spec"); // en cours = spectateur

    const sent = new Map<unknown, Uint8Array[]>();
    const relay = new Relay(
      rooms,
      (ws, data) => {
        if (!sent.has(ws)) sent.set(ws, []);
        sent.get(ws)!.push(data);
      },
      () => {},
    );
    const wsA = { id: "A" }, wsB = { id: "B" }, wsS = { id: "S" };
    relay.attach(wsA, a.code, a.token); // hôte : tick démarre
    relay.attach(wsB, a.code, b.token);
    await sleep(TICK_MS + 20);
    relay.attach(wsS, a.code, s.token); // spectateur en cours de partie

    // 0x26 envoyé au spectateur ; la demande de snapshot part à l'HÔTE
    expect(opcodes(sent.get(wsS)!).filter((o) => o === 0x26)).toHaveLength(1, "opcode 0x26 spectateur");
    expect(opcodes(sent.get(wsA)!).filter((o) => o === 0x23)).toHaveLength(
      2,
      "SNAP_REQ vers l'hôte : Bob (invité en cours) + spectateur (snapshot + replay)",
    );

    // ses entrées 0x10 sont IGNORÉES : aucun bundle ne grandit pour lui
    const mark = sent.get(wsA)!.length;
    relay.message(wsS, jsonFrame(0x10, [{ type: "moveTo", x: 3, y: 3 }]));
    relay.message(wsB, jsonFrame(0x10, [{ type: "moveTo", x: 5, y: 5 }]));
    await sleep(TICK_MS * 2 + 30);
    const ents = bundles(sent.get(wsA)!.slice(mark)).flatMap((f) => f.entries);
    expect(ents.filter((e) => e.json.includes("moveTo"))).toHaveLength(1, "seule l'entrée du joueur (Bob) voyage");
    expect(ents.every((e) => e.slot <= 1)).toBe(true, "aucune entrée/slot de spectateur dans les bundles");

    // il REGARDE : les bundles lui sont diffusés comme à tous
    expect(bundles(sent.get(wsS) ?? []).length).toBeGreaterThan(0, "le spectateur reçoit le lockstep");

    // sa déconnexion ne déclenche ni guard_camp ni 0x22 (pas de héros)
    const mark2 = sent.get(wsA)!.length;
    relay.detach(wsS);
    await sleep(TICK_MS * 2 + 30);
    expect(bundles(sent.get(wsA)!.slice(mark2)).flatMap((f) => f.entries).filter((e) => e.json.includes("guard_camp"))).toEqual([]);

    relay.detach(wsA);
    relay.detach(wsB);
  });
});
