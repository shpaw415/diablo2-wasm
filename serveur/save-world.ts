// save-world.ts — stockage disque des saves de monde coop (4b, phase 4).
//
// L'hôte exporte périodiquement (toutes les 5 min + à la fermeture propre
// du salon) l'état complet du monde via POST /api/rooms/:code/save — le
// serveur ne simule rien : il stocke l'ENVELOPPE
//   { version, code, seed, tick, savedAt, players[], save }
// dans saves/<code>.json (save = la sortie TEXTE bit-exact de saveGame(),
// re-servie telle quelle à la reprise — l'identité vient du moteur, cf.
// crates/d2rules/tests/save_monde_coop.rs). La reprise recrée un salon
// depuis cette enveloppe (POST /api/rooms/from-save) ; les saves non
// rafraîchies depuis > 7 jours sont purgées (branché sur le sweep des
// salons, throttlé ici).

import { LobbyError } from "./rooms";
import { mkdirSync, readdirSync, readFileSync, renameSync, statSync, unlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";

/** Enveloppe stockée par salon — la dernière save écrase la précédente. */
export interface SaveEnvelope {
  /** Version de l'ENVELOPPE (≠ version du save moteur, dans `save`). */
  version: 1;
  /** Code du salon d'origine. */
  code: string;
  /** Seed du monde, figé au lancement — la reprise le REutilise. */
  seed: number;
  /** Tick lockstep au moment de l'export (les clients y règlent leur
   *  compteur — l'horloge du save, now_ms, colle : dt fixes de 50 ms). */
  tick: number;
  /** Horodatage de l'écriture (base de la purge 7 j). */
  savedAt: number;
  /** Classes PAR SLOT (résumé du save v4 `players[].code`) — pré-remplit
   *  l'écran du salon de reprise ; la save ne porte pas les pseudos. */
  players: string[];
  /** saveGame() — JSON TEXTE bit-exact, rechargé par loadGame(). */
  save: string;
}

/** Garde-fou : un save multi-niveaux embarque le manifest (~144 Ko+) —
 *  8 Mo couvre largement et borne un POST aberrant. */
export const SAVE_MAX_BYTES = 8 * 1024 * 1024;

/** Purge par défaut : une save non rafraîchie depuis 7 jours part. */
export const SAVE_MAX_AGE_MS = 7 * 24 * 3600 * 1000;

/** Scan de purge au plus toutes les 60 s (le sweep des salons passe à
 *  chaque requête — le readdir ne doit pas le suivre). */
const PURGE_THROTTLE_MS = 60_000;

const CODE_RE = /^[A-Z0-9]{5}$/;

export interface SaveStoreOptions {
  /** Âge max d'une save (défaut : 7 jours). */
  maxAgeMs?: number;
  /** Minimum entre deux scans de purge (défaut : 60 s ; 0 = à chaque appel). */
  throttleMs?: number;
  /** Horloge injectable (tests). */
  now?: () => number;
}

export class SaveStore {
  private lastPurge = 0;
  private tmpSeq = 0;
  private readonly maxAgeMs: number;
  private readonly throttleMs: number;
  private readonly now: () => number;

  constructor(private dir: string, opts: SaveStoreOptions = {}) {
    this.maxAgeMs = opts.maxAgeMs ?? SAVE_MAX_AGE_MS;
    this.throttleMs = opts.throttleMs ?? PURGE_THROTTLE_MS;
    this.now = opts.now ?? Date.now;
  }

  /** Stocke (et remplace) la save d'un salon. Valide l'enveloppe CÔTÉ
   *  serveur : le save lui-même n'est interprété que par le moteur —
   *  ici on vérifie juste que c'est un JSON de save plausible (version
   *  1..=4, au moins un code héros) pour ne pas stocker des déchets. */
  store(code: string, seed: number, tick: number, save: unknown): SaveEnvelope {
    const c = this.normalize(code);
    if (!Number.isSafeInteger(seed) || seed < 0) throw new LobbyError(422, "seed invalide");
    if (!Number.isSafeInteger(tick) || tick < 0) throw new LobbyError(422, "tick invalide");
    if (typeof save !== "string" || save.length === 0 || save.length > SAVE_MAX_BYTES) {
      throw new LobbyError(422, "save invalide");
    }
    let parsed: Record<string, unknown>;
    try {
      parsed = JSON.parse(save);
    } catch {
      throw new LobbyError(422, "save invalide");
    }
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
      throw new LobbyError(422, "save invalide");
    }
    const version = parsed.version;
    if (typeof version !== "number" || version < 1 || version > 4) {
      throw new LobbyError(422, "save invalide (version)");
    }
    const players = this.playerCodes(parsed);
    if (players.length === 0) throw new LobbyError(422, "save sans héros");

    const env: SaveEnvelope = {
      version: 1,
      code: c,
      seed,
      tick,
      savedAt: this.now(),
      players,
      save,
    };
    mkdirSync(this.dir, { recursive: true });
    const final = join(this.dir, `${c}.json`);
    const tmp = join(this.dir, `.${c}.${this.now()}.${this.tmpSeq++}.tmp`);
    writeFileSync(tmp, JSON.stringify(env));
    renameSync(tmp, final); // remplacement atomique : jamais de save à moitié écrite
    return env;
  }

  /** Dernière enveloppe stockée d'un salon (null si aucune). */
  load(code: string): SaveEnvelope | null {
    const c = this.normalize(code);
    let raw: string;
    try {
      raw = readFileSync(join(this.dir, `${c}.json`), "utf8");
    } catch {
      return null;
    }
    try {
      const env = JSON.parse(raw) as SaveEnvelope;
      if (!env || env.version !== 1 || env.code !== c || typeof env.save !== "string") return null;
      return env;
    } catch {
      return null;
    }
  }

  /** Purge : supprime les saves non rafraîchies depuis maxAgeMs (7 j).
   *  Un salon ACTIF re-store toutes les 5 min → savedAt frais, jamais
   *  purgé ; une save de salon fermé part donc 7 jours après sa dernière
   *  écriture. Throttlé (branché sur le sweep des salons) ; `force`
   *  court-circuite le throttle (tests). Renvoie le nombre supprimé. */
  purge(force = false): number {
    const t = this.now();
    if (!force && t - this.lastPurge < this.throttleMs) return 0;
    this.lastPurge = t;
    let files: string[];
    try {
      files = readdirSync(this.dir).filter((f) => f.endsWith(".json"));
    } catch {
      return 0; // pas encore de répertoire : rien à purger
    }
    let removed = 0;
    for (const f of files) {
      const path = join(this.dir, f);
      let age = Number.POSITIVE_INFINITY; // fichier illisible ⇒ traié comme vieux
      try {
        const savedAt = (JSON.parse(readFileSync(path, "utf8")) as { savedAt?: number }).savedAt;
        age = t - (typeof savedAt === "number" ? savedAt : 0);
      } catch {
        try {
          age = t - statSync(path).mtimeMs; // fallback : mtime du fichier
        } catch {
          /* disparu entre-temps : rien à faire */
        }
      }
      if (age > this.maxAgeMs) {
        try {
          unlinkSync(path);
          removed++;
        } catch {
          /* course bénigne : un autre sweep l'a déjà pris */
        }
      }
    }
    return removed;
  }

  /** Nombre de saves sur disque (health). */
  count(): number {
    try {
      return readdirSync(this.dir).filter((f) => f.endsWith(".json")).length;
    } catch {
      return 0;
    }
  }

  /** Classes PAR SLOT depuis le save moteur : v4 `players[].code`, sinon
   *  `hero_code` (v1..v3 solo). Codes nettoyés (A-Z, 2..8 caractères). */
  private playerCodes(save: Record<string, unknown>): string[] {
    const clean = (h: unknown): string | null => {
      const s = String(h ?? "").trim().toUpperCase().slice(0, 8);
      return /^[A-Z]{2,8}$/.test(s) ? s : null;
    };
    if (Array.isArray(save.players)) {
      const codes = save.players.map((p) => clean((p as Record<string, unknown>)?.code)).filter((c): c is string => c !== null);
      if (codes.length > 0) return codes;
    }
    const solo = clean(save.hero_code);
    return solo ? [solo] : [];
  }

  private normalize(code: string): string {
    const c = String(code ?? "").trim().toUpperCase();
    if (!CODE_RE.test(c)) throw new LobbyError(422, "code de salle invalide");
    return c;
  }
}
