// d2w-serveur — salons de lobby (phase 2). En mémoire, sans dépendance :
// créer/rejoindre/présence/lancer. La présence est un ping implicite : chaque
// GET view rafraîchit lastSeen ; un joueur muet > 15 s est éjecté, une salle
// vide est supprimée. Phase 3 ajoutera le WebSocket (relay.ts) sur les mêmes
// salons. 4a (robustesse) : rejoindre une partie DÉJÀ LANCÉE = mode
// SPECTATEUR (siège sans héros, hors numérotation des slots — le retail fait
// de même) ; en jeu, la présence n'éjecte PLUS les sièges : un héros reste
// la propriété de son token, la reconnexion (même token) retrouve le MÊME
// slot. 4b : salons « reprise » créés depuis une save du monde (seed +
// classes par slot imposées par la save) et branchement de la purge des
// saves sur le sweep (save-world.ts).

const MAX_PLAYERS = 8;
/** 4a : plafond de spectateurs (sièges sans héros, hors verrouillage des
 *  slots — ils ne participent pas au lockstep, ils le regardent). */
const MAX_SPECTATORS = 8;
// Présence : les onglets en arrière-plan sont bridés par Chrome jusqu'à
// 1 timer/minute (intensive throttling) — le seuil doit couvrir ce cas,
// sinon un joueur qui change d'onglet est éjecté du lobby.
const PRESENCE_MS = 75_000;
const CODE_ALPHABET = "23456789ABCDEFGHJKMNPQRSTUVWXYZ"; // sans I, L, O, 0, 1

export interface PlayerView {
  name: string;
  host: boolean;
  /** 3c : classe choisie PAR SLOT (« BA »|« SOR »… — REST salon ou hello WS).
   *  null tant que le joueur n'a pas choisi. */
  hero: string | null;
  /** 4a : spectateur (rejoint une partie déjà lancée) — aucun héros, aucun
   *  slot moteur : regarde le lockstep, caméra libre. */
  spectator: boolean;
}

export interface RoomView {
  code: string;
  state: "lobby" | "playing";
  /** seed du monde, fixée au lancement (phase 3 : tick lockstep) — 4b :
   *  présente DÈS la création d'un salon « reprise » (celle de la save). */
  seed?: number;
  players: PlayerView[];
  you: { name: string; host: boolean; spectator: boolean };
  /** 4b : salon « reprise » (créé depuis une save du monde) — les classes
   *  par slot sont imposées par la save (chooseHero refusé) et le seed est
   *  déjà fixé. */
  reprise?: boolean;
}

interface Player {
  token: string;
  name: string;
  lastSeen: number;
  /** 3b : code de classe choisi par CE joueur ("BA"|"SOR"… — hello WS). */
  hero?: string;
  /** 4a : spectateur — rejoint une partie DÉJÀ LANCÉE. Aucun héros, aucun
   *  slot moteur (hors numérotation `slotOf`/`meta`), aucune entrée 0x10. */
  spectator?: boolean;
}

interface Room {
  code: string;
  host: string; // token de l'hôte
  state: "lobby" | "playing";
  seed?: number;
  createdAt: number;
  players: Map<string, Player>;
  /** 4b : salon « reprise » — créé depuis une save du monde. */
  reprise: boolean;
  /** 4b : capacité du salon (reprise = nb de slots de la save — le monde
   *  chargé a exactement ce nombre de héros, un 4e invité n'aurait pas de
   *  siège moteur). Défaut : MAX_PLAYERS. */
  seats: number;
  /** 4b : classes PAR SLOT imposées par la save d'origine (reprise) —
   *  présentes même pour les sièges pas encore repris. */
  heroSlots: string[] | null;
  /** 4b : code du salon d'origine de la save (reprise, traçabilité). */
  origin?: string;
}

export class LobbyError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

export class Rooms {
  private rooms = new Map<string, Room>();
  private seq = 0;
  /** 4b : branché sur le sweep (purge des saves > 7 j — save-world.ts). */
  private sweepHook: (() => void) | null = null;

  /** 4b : branche une action sur le sweep des salons (purge des saves —
   *  throttlée par le SaveStore, pas ici). */
  onSweep(fn: () => void): void {
    this.sweepHook = fn;
  }

  count(): number {
    this.sweep();
    return this.rooms.size;
  }

  create(name: string): { code: string; token: string; view: RoomView } {
    this.sweep();
    const code = this.newCode();
    const token = newToken();
    const room: Room = {
      code,
      host: token,
      state: "lobby",
      createdAt: Date.now(),
      players: new Map([[token, { token, name, lastSeen: Date.now() }]]),
      reprise: false,
      seats: MAX_PLAYERS,
      heroSlots: null,
    };
    this.rooms.set(code, room);
    return { code, token, view: this.viewOf(room, token) };
  }

  /** 4b : recrée un salon « reprise » depuis la save du monde d'un salon
   *  (enveloppe du stockage serveur, cf. save-world.ts) — le seed de la
   *  save est FIXÉ dès la création (`start` ne le re-tire pas) et les
   *  classes par slot viennent du save v4 (`players[].code`) : le monde
   *  relancé est LE MÊME (seed + loadGame de la save garantis côté moteur).
   *  Les sièges se remplissent par ORDRE D'ARRIVÉE (la save ne porte pas
   *  les pseudos) ; le créateur reprend le slot 0. */
  fromSave(
    name: string,
    env: { seed: number; players: string[]; origin?: string },
  ): { code: string; token: string; view: RoomView } {
    this.sweep();
    if (!Number.isSafeInteger(env.seed) || (env.seed as number) < 0) {
      throw new LobbyError(422, "seed invalide");
    }
    const slots = (Array.isArray(env.players) ? env.players : [])
      .map((h) => String(h ?? "").trim().toUpperCase().slice(0, 8))
      .filter((h) => /^[A-Z]{2,8}$/.test(h));
    if (slots.length === 0) throw new LobbyError(422, "save sans héros");
    const code = this.newCode();
    const token = newToken();
    const room: Room = {
      code,
      host: token,
      state: "lobby",
      seed: env.seed,
      createdAt: Date.now(),
      players: new Map([[token, { token, name, lastSeen: Date.now() }]]),
      reprise: true,
      seats: slots.length,
      heroSlots: slots,
      origin: env.origin ? String(env.origin).trim().toUpperCase() : undefined,
    };
    this.rooms.set(code, room);
    return { code, token, view: this.viewOf(room, token) };
  }

  /** 4a : rejoindre un salon. Lobby → siège (slot d'arrivée). Partie DÉJÀ
   *  LANCÉE → SPECTATEUR (ex-409 « partie déjà lancée ») : siège sans héros,
   *  hors numérotation des slots, plafonné à [`MAX_SPECTATORS`] ; le relais
   *  le synchronise (snapshot 0x23-0x25 + journal) et lui diffuse le
   *  lockstep, sans jamais retenir ses entrées. */
  join(code: string, name: string): { code: string; token: string; view: RoomView } {
    this.sweep();
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    if (room.state !== "lobby") {
      const spectators = [...room.players.values()].filter((p) => p.spectator).length;
      if (spectators >= MAX_SPECTATORS) throw new LobbyError(409, "salon complet (spectateurs)");
      const token = newToken();
      room.players.set(token, { token, name, lastSeen: Date.now(), spectator: true });
      return { code: room.code, token, view: this.viewOf(room, token) };
    }
    if ([...room.players.values()].filter((p) => !p.spectator).length >= room.seats) {
      throw new LobbyError(409, room.reprise ? `salle pleine (reprise : ${room.seats} sièges)` : "salle pleine");
    }
    const token = newToken();
    room.players.set(token, { token, name, lastSeen: Date.now() });
    return { code: room.code, token, view: this.viewOf(room, token) };
  }

  /** Vue de la salle + ping de présence du demandeur. */
  view(code: string, token: string): RoomView {
    this.sweep();
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    const me = room.players.get(token);
    if (!me) throw new LobbyError(403, "jeton refusé");
    me.lastSeen = Date.now();
    return this.viewOf(room, token);
  }

  /** Lancement par l'hôte : fige la seed du monde, passe en « playing ».
   *  4b : un salon « reprise » garde le seed de sa save (posé à la
   *  création) — re-tirer la seed casserait l'identité du monde repris. */
  start(code: string, token: string): RoomView {
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    if (room.host !== token) throw new LobbyError(403, "seul l'hôte peut lancer");
    if (room.state === "lobby") {
      room.state = "playing";
      if (room.seed === undefined) {
        room.seed = (crypto.getRandomValues(new Uint32Array(1))[0] >>> 0) || 0x0d200001;
      }
    }
    return this.view(code, token);
  }

  /** Départ volontaire : l'hôte transfère son rôle, la salle vide meurt. */
  leave(code: string, token: string): void {
    const room = this.rooms.get(normalize(code));
    if (!room) return;
    const me = room.players.get(token);
    if (!me) return;
    room.players.delete(token);
    if (room.players.size === 0) {
      this.rooms.delete(room.code);
      return;
    }
    if (room.host === token) {
      const next = room.players.keys().next().value as string;
      room.host = next;
    }
  }

  // ---------------- phase 3 : lockstep ----------------

  /** Vérifie le siège d'une connexion WS ; rafraîchit la présence (le WS
   *  maintient le joueur en vie pendant la partie). */
  validate(code: string, token: string): boolean {
    const room = this.rooms.get(normalize(code));
    if (!room) return false;
    const me = room.players.get(token);
    if (!me) return false;
    me.lastSeen = Date.now();
    return true;
  }

  isHost(code: string, token: string): boolean {
    return this.rooms.get(normalize(code))?.host === token;
  }

  /** Token de l'hôte (relais lockstep : snapshot, hôte perdu). */
  hostOf(code: string): string | null {
    return this.rooms.get(normalize(code))?.host ?? null;
  }

  /** Publie le héros du monde partagé (hôte uniquement) — 3a rétro-compat :
   *  délègue à [`Rooms.setPlayerHero`]. */
  setHero(code: string, token: string, hero: string): void {
    this.setPlayerHero(code, token, hero);
  }

  /** 3b : publie le héros de CE joueur (chaque siège au hello WS) — la
   *  liste par slot alimente `/meta` (monde multi-héros identique chez tous). */
  setPlayerHero(code: string, token: string, hero: string): void {
    const room = this.rooms.get(normalize(code));
    const me = room?.players.get(token);
    if (!room || !me) return;
    me.hero = hero.slice(0, 8).toUpperCase();
  }

  /** 3c (salon) : choix de classe PAR SLOT depuis l'écran salon (REST) —
   *  chaque siège publie SA classe AVANT le lancement ; la vue (liste des
   *  slots + classes) et `/meta` la redistribuent à tous. Contrairement à
   *  [`Rooms.setPlayerHero`] (silencieux, voie WS), valide le siège et la
   *  classe et renvoie la vue à jour. */
  chooseHero(code: string, token: string, hero: string): RoomView {
    this.sweep();
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    if (!room.players.has(token)) throw new LobbyError(403, "jeton refusé");
    if (room.reprise) throw new LobbyError(409, "classe imposée par la save (reprise)");
    const clean = String(hero || "").trim().toUpperCase();
    if (!/^[A-Z]{2,8}$/.test(clean)) throw new LobbyError(422, "classe invalide");
    this.setPlayerHero(code, token, clean);
    return this.view(code, token);
  }

  /** 4a : le siège est un SPECTATEUR (rejoint en cours de partie) — le
   *  relais ne le numérote pas, ne diffuse pas ses entrées et lui envoie
   *  l'opcode 0x26 à l'attachement. false si inconnu. */
  isSpectator(code: string, token: string): boolean {
    return this.rooms.get(normalize(code))?.players.get(token)?.spectator === true;
  }

  /** 3b : slot (ordre d'arrivée) du siège `token` — source unique partagée
   *  par le lobby ET le relais WS (bundles par slot). -1 si inconnu.
   *  4a : les spectateurs sont HORS numérotation (un spectateur qui rejoint
   *  en cours de partie ne décale jamais les héros déjà en monde). */
  slotOf(code: string, token: string): number {
    const room = this.rooms.get(normalize(code));
    if (!room) return -1;
    const seats = [...room.players.values()].filter((p) => !p.spectator);
    return seats.findIndex((p) => p.token === token);
  }

  /** Métadonnées de boot pour game.html en mode online. 3b : le siège
   *  demandeur reçoit SON slot (ordre d'arrivée — identique aux slots du
   *  relais WS) et la liste des héros du salon PAR SLOT (chaque joueur
   *  publie sa classe au hello WS) — chaque client construit le MÊME monde
   *  multi-héros. 4a : un spectateur reçoit slot = -1 (aucun héros à lui). */
  meta(
    code: string,
    token: string,
  ): {
    state: string;
    host: boolean;
    slot: number;
    heroes: { slot: number; hero: string | null }[];
    hero?: string;
    seed?: number;
  } {
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    if (!room.players.has(token)) throw new LobbyError(403, "jeton refusé");
    // 4a : les spectateurs sont hors numérotation — slot du demandeur parmi
    // les SEULS sièges à héros (-1 = spectateur, pas de héros à piloter).
    const seats = [...room.players.values()].filter((p) => !p.spectator);
    const meIsSpectator = this.isSpectator(code, token);
    const slot = meIsSpectator ? -1 : Math.max(0, seats.findIndex((p) => p.token === token));
    // 4b reprise : TOUS les slots de la save (même sans siège repris — le
    // monde chargé a exactement ces héros) ; sinon les classes choisies
    // (4a : sièges à héros uniquement — un spectateur n'y figure jamais).
    const heroes = room.heroSlots
      ? room.heroSlots.map((h, i) => ({ slot: i, hero: h ?? null }))
      : seats.map((p, i) => ({ slot: i, hero: p.hero ?? null }));
    const out: {
      state: string;
      host: boolean;
      slot: number;
      heroes: { slot: number; hero: string | null }[];
      hero?: string;
      seed?: number;
    } = {
      state: room.state,
      host: room.host === token, // vérité serveur : le siège fait foi
      slot,
      heroes,
    };
    const hostHero = heroes[0]?.hero;
    if (hostHero) out.hero = hostHero; // rétro-compat 3a : héros du slot 0
    if (room.seed !== undefined) out.seed = room.seed;
    return out;
  }

  /** Retire le joueur (WS fermé) ; renvoie true si la salle a disparu. */
  dropWsPlayer(code: string, token: string): boolean {
    // le WS fermé ne fait pas quitter la salle par lui-même (reconnexion
    // possible, phase 4) ; on ne touche à rien tant que la salle existe.
    return !this.rooms.has(normalize(code));
  }

  /** Supprime une salle morte (desync / hôte perdu). */
  destroy(code: string): void {
    this.rooms.delete(normalize(code));
  }

  // ---------------- 4b : export du monde (save) ----------------

  /** Vérifie que le siège existe ET est l'hôte (export du monde). */
  requireHost(code: string, token: string): void {
    const room = this.rooms.get(normalize(code));
    if (!room) throw new LobbyError(404, "salle inconnue");
    if (!room.players.has(token)) throw new LobbyError(403, "jeton refusé");
    if (room.host !== token) throw new LobbyError(403, "seul l'hôte peut sauvegarder");
  }

  /** Seed figé du salon (export du monde) — undefined hors « playing ». */
  seedOf(code: string): number | undefined {
    return this.rooms.get(normalize(code))?.seed;
  }

  // ---------------- interne ----------------

  private viewOf(room: Room, token: string): RoomView {
    const me = room.players.get(token)!;
    const view: RoomView = {
      code: room.code,
      state: room.state,
      players: [...room.players.values()].map((p, i) => ({
        name: p.name,
        host: p.token === room.host,
        // 4b reprise : la classe de la save d'origine s'impose au slot
        hero: room.heroSlots?.[i] ?? p.hero ?? null,
        // 4a : spectateur = siège sans héros, hors slots
        spectator: p.spectator === true,
      })),
      you: { name: me.name, host: token === room.host, spectator: me.spectator === true },
    };
    if (room.reprise) {
      view.reprise = true;
      if (room.seed !== undefined) view.seed = room.seed; // le monde est déjà désigné
    }
    if (room.state === "playing") view.seed = room.seed;
    return view;
  }

  private sweep(): void {
    this.sweepHook?.(); // 4b : purge des saves (throttlée côté SaveStore)
    const now = Date.now();
    for (const room of this.rooms.values()) {
      // 4a : en partie, les sièges NE SONT PAS éjectés par la présence —
      // un héros appartient à son token, la reconnexion (même token) doit
      // retrouver le MÊME slot, déco depuis > 75 s comprise (onglet bridé,
      // coupure réseau). Le cycle de vie d'une salle en jeu appartient au
      // relais (fermeture quand le dernier WS se lève — destroy).
      if (room.state !== "playing") {
        for (const [tok, p] of room.players) {
          if (now - p.lastSeen > PRESENCE_MS) {
            if (room.host !== tok || room.players.size === 1) room.players.delete(tok);
            else {
              // l'hôte muet transfère avant sa suppression
              const next = [...room.players.keys()].find((t) => t !== tok)!;
              room.host = next;
              room.players.delete(tok);
            }
          }
        }
        if (room.players.size === 0) this.rooms.delete(room.code);
      }
    }
  }

  private newCode(): string {
    for (let tries = 0; tries < 64; tries++) {
      let code = "";
      const buf = crypto.getRandomValues(new Uint32Array(5));
      for (let i = 0; i < 5; i++) code += CODE_ALPHABET[buf[i] % CODE_ALPHABET.length];
      if (!this.rooms.has(code)) return code;
    }
    throw new LobbyError(500, "génération de code épuisée");
  }
}

function normalize(code: string): string {
  return String(code || "").trim().toUpperCase();
}

function newToken(): string {
  const b = crypto.getRandomValues(new Uint8Array(16));
  return [...b].map((x) => x.toString(16).padStart(2, "0")).join("");
}
