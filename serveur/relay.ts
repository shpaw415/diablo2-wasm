// d2w-serveur — relais lockstep (phase 3a ; 3c départs en jeu ; 4a
// robustesse : reconnexion mi-partie, spectateur, timeout garde au camp).
// Le serveur ne simule RIEN : il collecte les entrées de chaque joueur à
// 20 Hz et diffuse à tous le même bundle par tick (entrées vides incluses).
// Chaque client exécute d2rules de façon déterministe ; un hash d'état
// (saveGame) échangé toutes les 60 ticks rompt la partie au premier écart.
//
// 4a — DÉCONNEXION ≠ DÉPART : un joueur qui perd sa connexion laisse SON
// héros dans le monde (IA idle — aucune entrée sur son slot, pas de mort ni
// de suppression automatique ; le slot/eslot reste le SIEN). Sa
// reconnexion (même token, même slot) rejoue le journal des ticks manquants
// (hello `resume` — replay seul) ou resynchronise par snapshot 0x23-0x25
// quand le retard dépasse le journal. Un héros orphelin (déco > 5 min) est
// mis en GARDE AU CAMP : une entrée synthétique {type:"guard_camp"} voyage
// dans un bundle, chaque client la résout en MoveTo vers le spawn du héros
// (save v4 `players[slot].spawn`, identique chez tous — déterministe) ;
// le héros N'EST JAMAIS supprimé (le retail garde le joueur).
//
// 4a — SPECTATEUR : rejoindre un salon EN JEU (rooms.join) crée un siège
// sans héros, HORS numérotation des slots ; le relais lui envoie 0x26,
// ne diffuse jamais ses entrées et le synchronise comme un invité
// (snapshot + replay) : il exécute le monde en observateur, caméra libre.
//
// 3c : quand un joueur QUITTE en jeu (retiré explicitement), une entrée
// {type:"leave"} est injectée dans SON flux — chaque client retire le héros
// de ce slot au même tick (compaction déterministe) et les étiquettes de
// slot des bundles suivent (eslot remappé, cf. NetPlayer.eslot). En 4a, la
// simple déconnexion N'UTILISE PLUS ce chemin (le héros reste — voir
// ci-dessus) ; le retrait explicite reste disponible pour l'hôte.
//
// Trame = 1 octet opcode + charge utile, tout en little-endian :
//  C→S 0x01 hello   [len:u16][json {hero,name,resume?}] — l'hôte publie le
//                    héros ; `resume` = dernier tick exécuté par un client
//                    qui REPRENDE avec son monde en mémoire (reconnexion
//                    4a) : replay du journal seul si le retard tient dedans
//  C→S 0x10 input   [len:u16][json [PlayerInput,…]] — entrées du joueur
//                    (4a : JAMAIS retenues d'un spectateur)
//  C→S 0x12 hash    [tick:u32][hash:u32]           — FNV-1a de saveGame()
//  C→S 0x23 snapReq — demande de snapshot (invité/spectateur en cours)
//  C→S 0x24 snap    [tick:u32][len:u32][json]      — réponse de l'hôte
//  C→S 0x30 ping    [t:u64]
//  S→C 0x11 bundle  [tick:u32][cnt:u8] ( [slot:u8][len:u16][json entrées] )×cnt
//  S→C 0x20 desync  [tick:u32][cnt:u8][slots u8…]  — puis fermeture de tous
//  S→C 0x21 hôte perdu                             — puis fermeture de tous
//  S→C 0x22 slot libre [slot:u8]                   — réservé (retrait
//                    explicite 3c ; la DÉCONNEXION 4a n'émet plus : le
//                    siège est réservé à la reconnexion)
//  S→C 0x25 snapTo  [tick:u32][len:u32][json]      — snapshot au client
//  S→C 0x26 spectateur                             — « vous êtes en mode
//                    spectateur » (4a) : caméra libre, aucune entrée
//  S→C 0x31 pong    [t:u64]

import { Rooms } from "./rooms";

export const TICK_MS = 50; // 20 Hz

const OP = {
  HELLO: 0x01, INPUT: 0x10, HASH: 0x12, PING: 0x30,
  SNAP_REQ: 0x23, SNAP: 0x24, SNAP_TO: 0x25, SPECTATOR: 0x26,
  BUNDLE: 0x11, DESYNC: 0x20, HOST_LOST: 0x21, SLOT_FREE: 0x22, PONG: 0x31,
};

interface WsData {
  code: string;
  token: string;
  slot: number; // -1 tant que non attaché
}

interface NetPlayer {
  token: string;
  slot: number;
  /** 3c : slot MOTEUR du héros (monde d2rules) — le lobby attribue les
   *  sièges à l'arrivée (slot = eslot au boot), mais `remove_player` côté
   *  moteur COMPACTE les slots quand un joueur part en jeu : les eslots
   *  supérieurs décalent d'un cran, et les bundles restent étiquetés avec
   *  les slots que les clients exécutent réellement. 4a : la DÉCONNEXION
   *  ne compacte plus (le héros reste, même slot à la reconnexion) — seul
   *  un retrait explicite ({type:"leave"}) remappe encore. */
  eslot: number;
  pending: string[]; // entrées JSON reçues depuis le dernier bundle
  hashes: Map<number, number>; // tick → hash d'état (fenêtre glissante)
  alive: boolean;
  awaitingSnap: boolean; // attend le snapshot de l'hôte avant d'exécuter
  /** 4a : spectateur — siège sans héros (rejoint en cours de partie).
   *  Jamais dans les bundles, entrées 0x10 ignorées, 0x26 à l'attachement. */
  spectator: boolean;
  /** 4a : horodatage de la déconnexion (jeu en cours) — pilote le timeout
   *  garde au camp ; null si connecté ou hors partie. */
  disconnectedAt: number | null;
  /** 4a : le guard_camp synthétique a déjà voyagé (une seule fois). */
  guarded: boolean;
}

interface NetRoom {
  code: string;
  players: Map<string, NetPlayer>; // par token
  nextSlot: number;
  tick: number;
  timer: ReturnType<typeof setInterval> | null;
  dead: boolean;
  /** 3b : journal des bundles diffusés (replay de reconnexion/snapshot — un
   *  invité qui rejoint en cours de partie n'a reçu AUCUN bundle passé ; le
   *  snapshot de l'hôte peut dater de ticks en arrière (onglet throttlé) —
   *  sans replay, le trou [snapTick+1, arrivée] fige l'invité pour toujours).
   *  tick → entrées [{slot, json}] ; borné à JOURNAL_TICKS. */
  journal: Map<number, { slot: number; json: string }[]>;
}

/** Profondeur du journal de replay (3 min à 20 Hz — couvre un hôte dont
 *  l'onglet est bridé en arrière-plan, cf. rooms.ts PRESENCE_MS). */
const JOURNAL_TICKS = 3600;

/** 4a : délai avant qu'un héros orphelin (déconnecté, personne ne pilote
 *  son slot) soit mis en GARDE AU CAMP — 5 min, entrée synthétique
 *  {type:"guard_camp"} résolue côté client en MoveTo vers le spawn du héros
 *  (identique chez tous les clients : même monde, même save v4). Le héros
 *  n'est ni tué ni supprimé (le retail garde le joueur). */
const GUARD_CAMP_MS = 300_000;

export interface RelayOptions {
  /** Tests : délai garde au camp raccourci. */
  guardCampMs?: number;
  /** Tests : profondeur du journal raccourcie. */
  journalTicks?: number;
}

export class Relay {
  private rooms = new Map<string, NetRoom>();
  private sockets = new Map<unknown, WsData>();
  private guardCampMs: number;
  private journalTicks: number;

  constructor(
    private rooms_: Rooms,
    private send: (ws: unknown, data: Uint8Array) => void,
    private close: (ws: unknown) => void,
    opts?: RelayOptions,
  ) {
    this.guardCampMs = opts?.guardCampMs ?? GUARD_CAMP_MS;
    this.journalTicks = opts?.journalTicks ?? JOURNAL_TICKS;
  }

  // ---------------- cycle de vie ----------------

  attach(ws: unknown, code: string, token: string): void {
    const key = normalize(code);
    // reconnexion / double onglet sur le même siège : l'ancien socket est
    // remplacé (un seul socket vivant par jeton). 4a : c'est LE chemin de
    // reconnexion — même token, même siège, le monde n'a pas supprimé le
    // héros entre-temps.
    for (const [old, d] of this.sockets) {
      if (d.code === key && d.token === token && old !== ws) this.close(old);
    }
    let net = this.rooms.get(key);
    if (!net) {
      net = { code: key, players: new Map(), nextSlot: 0, tick: 0, timer: null, dead: false, journal: new Map() };
      this.rooms.set(key, net);
    }
    let p = net.players.get(token);
    const isNew = !p;
    const spectator = this.rooms_.isSpectator(net.code, token);
    if (!p) {
      // 3b : le slot est celui du SIÈGE (ordre d'arrivée du lobby, source
      // unique partagée avec /meta — chaque joueur hérite du bon héros).
      // Repli : compteur local (siège introuvable — jamais observé car
      // l'upgrade WS valide le jeton). 3c : eslot (slot moteur) démarre
      // égal au siège — les retraits explicites en jeu le remappent par
      // compaction. 4a : un spectateur ne prend AUCUN slot (-1 — hors
      // bundles, hors numérotation, sans héros).
      const seat = spectator ? -1 : this.rooms_.slotOf(net.code, token);
      const s = seat >= 0 ? seat : spectator ? -1 : net.nextSlot++;
      p = { token, slot: s, eslot: s, pending: [], hashes: new Map(), alive: true, awaitingSnap: false, spectator, disconnectedAt: null, guarded: false };
      net.players.set(token, p);
    }
    if (!spectator) net.nextSlot = Math.max(net.nextSlot, net.players.size);
    p.alive = true;
    p.spectator = spectator;
    p.disconnectedAt = null; // 4a : reconnexion — le timeout garde au camp s'annule
    this.sockets.set(ws, { code: key, token, slot: p.slot });
    const isHost = this.rooms_.isHost(net.code, token);
    const running = !!net.timer;
    console.log(`[ws] attach ${net.code} slot=${p.slot} host=${isHost} running=${running} joueurs=${net.players.size}${spectator ? " (spectateur)" : ""}`);
    if (spectator) {
      // 4a : le client bascule en mode spectateur (caméra libre, aucune
      // entrée) — puis reçoit le flux comme un invité (snapshot + replay).
      this.send(ws, packFrame(OP.SPECTATOR, new Uint8Array(0)));
    }
    if (isHost && !net.timer && !net.dead) {
      // le tick global démarre à la connexion de l'hôte ; quiconque se joint
      // ensuite reçoit un snapshot de son état (saveGame → loadGame).
      net.timer = setInterval(() => this.bundle(net!), TICK_MS);
    } else if (running && !isHost) {
      // invité, RETOUR (4a) ou spectateur sur un monde déjà en cours :
      // demande un snapshot. Le hello `resume` d'un retour dont le retard
      // tient encore dans le journal annule cette demande (replay seul).
      p.awaitingSnap = true;
      this.sendToHost(net, packFrame(OP.SNAP_REQ, new Uint8Array(0)));
    }
    void isNew;
  }

  detach(ws: unknown): void {
    const d = this.sockets.get(ws);
    if (!d) return;
    this.sockets.delete(ws);
    const net = this.rooms.get(d.code);
    if (!net) return;
    const p = net.players.get(d.token);
    if (p) {
      p.alive = false;
      // 4a : la déconnexion NE RETIRE PLUS le héros du monde (3c faisait
      // voyager un {type:"leave"} — suppression définitive). Le slot/eslot
      // RESTENT LE SIEN : ses bundles deviennent vides (personne ne pilote
      // le héros — IA idle), le monde continue, et une reconnexion avec le
      // MÊME token retrouve le MÊME slot (replay du journal et/ou snapshot,
      // cf. attach + hello `resume`). Un héros orphelin depuis plus de
      // GUARD_CAMP_MS part GARDE AU CAMP (cf. bundle) — jamais supprimé.
      if (net.timer && !p.spectator) p.disconnectedAt = Date.now();
      console.log(`[ws] detach ${net.code} slot=${p.slot} eslot=${p.eslot}${p.spectator ? " (spectateur)" : ""} — héros conservé (idle), reconnexion possible`);
    }
    if (![...net.players.values()].some((x) => x.alive)) {
      console.log(`[ws] salle ${net.code} vide — suppression`);
      this.kill(net, null);
    }
  }

  /** Ferme tout et supprime la salle (desync ou hôte perdu). */
  private kill(net: NetRoom, opcode: Uint8Array | null): void {
    if (net.timer) clearInterval(net.timer);
    net.timer = null;
    net.dead = true;
    this.rooms.delete(net.code);
    if (opcode) this.broadcastOf(net.code, opcode);
    for (const [ws, d] of this.sockets) {
      if (d.code === net.code) this.close(ws);
    }
    this.rooms_.destroy(net.code);
  }

  // ---------------- réception client ----------------

  message(ws: unknown, data: ArrayBuffer): void {
    const d = this.sockets.get(ws);
    if (!d) return;
    const net = this.rooms.get(d.code);
    if (!net || net.dead) return;
    const p = net.players.get(d.token);
    if (!p) return;
    const v = new DataView(data);
    const op = v.getUint8(0);
    switch (op) {
      case OP.HELLO: {
        try {
          const j = JSON.parse(dec(data, 3));
          // 3b : CHAQUE joueur publie sa classe (la liste par slot alimente
          // /meta → monde multi-héros identique chez tous les clients) ;
          // l'hôte restait le seul publié en 3a (rétro-compatible).
          if (j && typeof j.hero === "string" && j.hero) {
            this.rooms_.setPlayerHero(d.code, d.token, j.hero);
          }
          // 4a : reconnexion avec monde EN MÉMOIRE côté client (`resume` =
          // dernier tick exécuté). Si le retard tient encore dans le
          // journal, le snapshot 0x23-0x25 est ANNULÉ : on ne renvoie que
          // les bundles manquants (replay seul — le client n'a jamais rien
          // perdu). Sinon (retard > journal) la demande de snapshot reste
          // en pied : le client, qui s'est annoncé `resume`, appliquera le
          // snapshot reçu même s'il était déjà « synced ».
          if (j && typeof j.resume === "number" && j.resume > 0 && net.timer && p.awaitingSnap) {
            const oldest = this.journalOldest(net);
            const replayable = j.resume <= net.tick && (oldest === null || j.resume + 1 >= oldest);
            if (replayable) {
              p.awaitingSnap = false;
              console.log(`[ws] reprise ${net.code} slot=${p.slot} depuis tick ${j.resume} — replay seul (snapshot annulé)`);
              this.replayTo(net, ws, j.resume);
            } else {
              console.log(`[ws] reprise ${net.code} slot=${p.slot} : tick ${j.resume} hors journal (plus ancien ${oldest}) — snapshot 0x23 maintenu`);
            }
          }
        } catch { /* hello malformé : ignoré */ }
        break;
      }
      case OP.INPUT: {
        // 4a : un spectateur n'a AUCUNE entrée (caméra libre) — défense en
        // profondeur : le client ne doit pas en envoyer, le relais ne les
        // retient pas.
        if (p.spectator) break;
        // les entrées partent dans le PROCHAIN bundle, peu importe leur tick d'émission
        p.pending.push(dec(data, 3));
        break;
      }
      case OP.HASH: {
        const tick = v.getUint32(1, true);
        const hash = v.getUint32(5, true);
        p.hashes.set(tick, hash);
        if (p.hashes.size > 8) {
          const first = p.hashes.keys().next().value as number;
          p.hashes.delete(first);
        }
        this.checkHash(net);
        break;
      }
      case OP.SNAP: {
        // snapshot de l'hôte [tick:u32][len:u32][json] → reconstruit à
        // l'identique avec l'opcode 0x25 (jamais de copie de trame brute).
        const vin = new DataView(data);
        const tick = vin.getUint32(1, true);
        const len = vin.getUint32(5, true);
        if (9 + len !== data.byteLength) {
          console.log(`[ws] snapshot ${net.code} invalide (len ${len} ≠ ${data.byteLength - 9})`);
          break;
        }
        const out = new Uint8Array(9 + len);
        const vo = new DataView(out.buffer);
        vo.setUint8(0, OP.SNAP_TO);
        vo.setUint32(1, tick, true);
        vo.setUint32(5, len, true);
        out.set(new Uint8Array(data, 9, len), 9);
        console.log(`[ws] snapshot ${net.code} tick=${tick} (${len} o) → diffusé`);
        let any = false;
        for (const x of net.players.values()) {
          if (x.awaitingSnap && x.alive) {
            x.awaitingSnap = false;
            any = true;
          }
        }
        if (any) {
          this.broadcast(net, out);
          // 3b : REPLAY du journal (tick, current] — l'invité qui rejoint en
          // cours de partie n'a reçu aucun bundle passé ; sans replay, le
          // trou [snapTick+1, arrivée] figerait son exécution (buffers vides).
          const oldest = this.journalOldest(net);
          const missing = tick + 1 < (oldest ?? tick + 1);
          if (missing) {
            console.log(`[ws] replay ${net.code} impossible : tick ${tick + 1} hors journal (plus ancien ${oldest})`);
          } else {
            this.replayTo(net, null, tick);
          }
        }
        break;
      }
      case OP.PING: {
        const out = new Uint8Array(9);
        const o = new DataView(out.buffer);
        o.setUint8(0, OP.PONG);
        try { o.setBigUint64(1, v.getBigUint64(1, true), true); } catch { /* trame courte */ }
        this.send(ws, out);
        break;
      }
    }
  }

  /** 4a : tick le plus ancien du journal, ou null si vide. */
  private journalOldest(net: NetRoom): number | null {
    if (net.journal.size === 0) return null;
    return Math.min(...net.journal.keys());
  }

  /** 3b/4a : REPLAY du journal — renvoie les bundles (from, current] au
   *  socket `to`, ou à TOUTE la salle si `to` est null (chemin 3b du
   *  snapshot, où tous les clients en attente ont besoin du même replay).
   *  Précondition : `from` est encore couvert par le journal. */
  private replayTo(net: NetRoom, to: unknown, from: number): void {
    let replayed = 0;
    for (let t = from + 1; t <= net.tick; t++) {
      const entries = net.journal.get(t);
      if (entries) {
        const frame = frameBundle(t, entries);
        if (to === null) this.broadcast(net, frame);
        else this.send(to, frame);
        replayed++;
      }
    }
    console.log(`[ws] replay ${net.code} ticks ${from + 1}..${net.tick} (${replayed} bundles)${to === null ? "" : " — reprise 4a (replay seul)"}`);
  }

  /** Compare les hashes sur les ticks COMMUNS aux joueurs (fenêtre glissante
   *  — les clients avancent à des rythmes légèrement différents, l'égalité
   *  stricte de tick rendrait le contrôle aveugle). Un écart = partie rompue. */
  private checkHash(net: NetRoom): void {
    const actives = [...net.players.values()].filter((x) => x.alive && x.hashes.size > 0);
    if (actives.length < 2) return;
    // ticks communs à au moins deux joueurs
    const counts = new Map<number, number>();
    for (const p of actives) for (const t of p.hashes.keys()) counts.set(t, (counts.get(t) ?? 0) + 1);
    for (const [tick, n] of counts) {
      if (n < 2) continue;
      let ref: number | null = null;
      let refSlot = -1;
      const bad: number[] = [];
      for (const p of actives) {
        const h = p.hashes.get(tick);
        if (h === undefined) continue;
        if (ref === null) { ref = h; refSlot = p.slot; continue; }
        if (h !== ref) bad.push(p.slot, refSlot);
      }
      if (bad.length) {
        const slots = [...new Set(bad)];
        console.log(`[ws] DÉSYNC ${net.code} tick=${tick} slots=${slots.join(",")}`);
        this.broadcast(net, frameDesync(tick, slots));
        this.kill(net, null);
        return;
      }
    }
  }

  // ---------------- diffusion 20 Hz ----------------

  private bundle(net: NetRoom): void {
    if (net.dead) return;
    net.tick += 1;
    // 4a : timeout garde au camp — un héros orphelin (déconnecté depuis
    // plus de GUARD_CAMP_MS) reçoit une entrée synthétique UNIQUE
    // {type:"guard_camp"} dans SON flux : elle voyage dans le prochain
    // bundle (même tick chez tous — déterminisme lockstep), chaque client
    // la résout en MoveTo vers le spawn du héros (save v4 players[slot]
    // .spawn, identique chez tous). Le héros N'EST NI TUÉ NI SUPPRIMÉ —
    // il marche au camp et y monte la garde (le retail garde le joueur) ;
    // sa reconnexion reste possible, même token, même slot.
    const now = Date.now();
    for (const p of net.players.values()) {
      if (!p.alive && !p.spectator && net.timer && p.disconnectedAt !== null && !p.guarded
        && now - p.disconnectedAt >= this.guardCampMs) {
        p.guarded = true;
        p.pending.push('[{"type":"guard_camp"}]');
        console.log(`[ws] garde au camp ${net.code} slot=${p.slot} eslot=${p.eslot} (déco > ${Math.round(this.guardCampMs / 1000)} s)`);
      }
    }
    const entries: { slot: number; json: string }[] = [];
    // 3c : étiquetage PAR SLOT MOTEUR (eslot — compacté aux retraits
    // explicites) — les clients exécutent chaque entrée sur le héros de ce
    // slot. L'ordre croissant des eslots est conservé. 4a : les
    // spectateurs (eslot -1) n'apparaissent JAMAIS — ils n'ont pas de héros.
    const ordered = [...net.players.values()].filter((p) => !p.spectator).sort((a, b) => a.eslot - b.eslot);
    for (const p of ordered) {
      // 4a : un slot déconnecté continue d'ÉMETTRE (entrées vides ou son
      // guard_camp) — le héros reste dans le monde de tous les clients.
      const inner = p.pending
        .map((s) => (s.length >= 2 ? s.slice(1, -1) : ""))
        .filter((s) => s.length > 0)
        .join(",");
      p.pending = [];
      entries.push({ slot: p.eslot, json: "[" + inner + "]" });
    }
    this.broadcast(net, frameBundle(net.tick, entries));
    // 3b : journal pour le replay des snapshots (borné — 3 min de lockstep)
    net.journal.set(net.tick, entries);
    while (net.journal.size > this.journalTicks) {
      const first = net.journal.keys().next().value as number;
      net.journal.delete(first);
    }
  }

  private broadcast(net: NetRoom, data: Uint8Array): void {
    for (const [ws, d] of this.sockets) if (d.code === net.code) this.send(ws, data);
  }

  private sendToHost(net: NetRoom, data: Uint8Array): void {
    const hostToken = this.rooms_.hostOf(net.code);
    if (!hostToken) return;
    for (const [ws, d] of this.sockets) {
      if (d.code === net.code && d.token === hostToken) this.send(ws, data);
    }
  }

  private broadcastOf(code: string, data: Uint8Array): void {
    for (const [ws, d] of this.sockets) if (d.code === code) this.send(ws, data);
  }
}

// ---------------- trames ----------------

function packFrame(op: number, payload: Uint8Array): Uint8Array {
  const out = new Uint8Array(1 + payload.length);
  out[0] = op;
  out.set(payload, 1);
  return out;
}

function dec(data: ArrayBuffer, offset: number): string {
  return new TextDecoder().decode(new Uint8Array(data, offset));
}

function frameBundle(tick: number, entries: { slot: number; json: string }[]): Uint8Array {
  const enc = new TextEncoder();
  const parts = entries.map((e) => ({ slot: e.slot, b: enc.encode(e.json) }));
  const size = 6 + parts.reduce((n, p) => n + 3 + p.b.length, 0);
  const out = new Uint8Array(size);
  const v = new DataView(out.buffer);
  v.setUint8(0, OP.BUNDLE);
  v.setUint32(1, tick, true);
  v.setUint8(5, parts.length);
  let o = 6;
  for (const p of parts) {
    v.setUint8(o, p.slot);
    v.setUint16(o + 1, p.b.length, true);
    out.set(p.b, o + 3);
    o += 3 + p.b.length;
  }
  return out;
}

function frameDesync(tick: number, slots: number[]): Uint8Array {
  const out = new Uint8Array(6 + slots.length);
  const v = new DataView(out.buffer);
  v.setUint8(0, OP.DESYNC);
  v.setUint32(1, tick, true);
  v.setUint8(5, slots.length);
  slots.forEach((s, i) => { out[6 + i] = s; });
  return out;
}

function normalize(code: string): string {
  return String(code || "").trim().toUpperCase();
}
