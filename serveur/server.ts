// d2w-serveur — phase 1 : statique + /api/health. Phase 2 : routes REST de
// lobby. Phase 3a : WebSocket lockstep (upgrade /api/ws → relay.ts) + /meta
// pour le boot des miroirs. 4b : export du monde coop (POST /api/rooms/:code/
// save → saves/<code>.json, GET pour resservir) + reprise (POST
// /api/rooms/from-save) + purge des saves > 7 j branchée sur le sweep.
// Démarrage : bun server.ts  (PORT/HOST env).

import { StaticFiles } from "./static";
import { Rooms, LobbyError } from "./rooms";
import { Relay } from "./relay";
import { SaveStore } from "./save-world";

const PORT = Number(process.env.PORT ?? 8077);
const HOST = process.env.HOST ?? "0.0.0.0";
const ROOT = process.env.DEMO_ROOT ?? new URL("../web", import.meta.url).pathname;
const SAVES_DIR = process.env.D2W_SAVES_DIR ?? new URL("./saves", import.meta.url).pathname;

const files = new StaticFiles({ root: ROOT });
const rooms = new Rooms();
const saves = new SaveStore(SAVES_DIR);
rooms.onSweep(() => saves.purge()); // purge 7 j, throttlée côté SaveStore
const started = Date.now();

const relay = new Relay(
  rooms,
  (ws, data) => (ws as { send: (d: Uint8Array) => void }).send(data),
  (ws) => (ws as { close: () => void }).close(),
);

async function readJson(req: Request): Promise<Record<string, unknown>> {
  try {
    const b = await req.json();
    return b && typeof b === "object" ? (b as Record<string, unknown>) : {};
  } catch {
    return {};
  }
}

const str = (v: unknown, max: number): string => String(v ?? "").trim().slice(0, max);

async function lobbyRoute(req: Request, url: URL): Promise<Response | null> {
  const m = url.pathname.match(/^\/api\/rooms(?:\/([A-Za-z0-9]{5})(?:\/(join|start|leave|hero|save))?)?$/);
  if (!m) return null;
  const [, code, verb] = m;

  try {
    if (!code && req.method === "POST" && !verb) {
      const body = await readJson(req);
      const r = rooms.create(str(body.name, 16) || "Hôte");
      return Response.json(r, { status: 201 });
    }
    if (code && verb === "join" && req.method === "POST") {
      const body = await readJson(req);
      return Response.json(rooms.join(code, str(body.name, 16) || "Joueur"));
    }
    if (code && !verb && req.method === "GET") {
      return Response.json(rooms.view(code, url.searchParams.get("token") ?? ""));
    }
    if (code && verb === "start" && req.method === "POST") {
      const body = await readJson(req);
      return Response.json(rooms.start(code, str(body.token, 40)));
    }
    if (code && verb === "leave" && req.method === "POST") {
      const body = await readJson(req);
      rooms.leave(code, str(body.token, 40));
      return Response.json({ ok: true });
    }
    if (code && verb === "hero" && req.method === "POST") {
      // 3c (salon) : choix de classe PAR SLOT avant le lancement — stocké
      // par siège (rooms.setPlayerHero), visible dans la vue + /meta.
      const body = await readJson(req);
      return Response.json(rooms.chooseHero(code, str(body.token, 40), str(body.hero, 8)));
    }
    if (code && verb === "save" && req.method === "POST") {
      // 4b : l'hôte exporte l'état complet du monde (save v4 multi-joueurs
      // + seed du salon + tick lockstep) — stocké disque (saves/<code>.json),
      // resservi par GET /save et consommé par /api/rooms/from-save. Période
      // conseillée côté client : 5 min + à la fermeture propre du salon.
      const body = await readJson(req);
      const token = str(body.token, 40);
      rooms.requireHost(code, token); // 404/403
      const seed = rooms.seedOf(code);
      if (seed === undefined) throw new LobbyError(409, "partie non lancée");
      const tick = body.tick;
      const env = saves.store(code, seed, typeof tick === "number" ? Math.floor(tick) : -1, body.save);
      return Response.json({ ok: true, code: env.code, savedAt: env.savedAt, bytes: env.save.length, tick: env.tick });
    }
    return Response.json({ error: "méthode non supportée" }, { status: 405 });
  } catch (e) {
    if (e instanceof LobbyError) return Response.json({ error: e.message }, { status: e.status });
    throw e;
  }
}

const server = Bun.serve({
  port: PORT,
  hostname: HOST,
  async fetch(req) {
    const url = new URL(req.url);
    const t0 = performance.now();

    let res: Response;
    try {
      if (url.pathname === "/api/ws") {
        // upgrade lockstep : siège vérifié avant handshake
        const code = url.searchParams.get("code") ?? "";
        const token = url.searchParams.get("token") ?? "";
        if (!rooms.validate(code, token)) {
          res = Response.json({ error: "siège refusé" }, { status: 401 });
        } else if (server.upgrade(req, { data: { code: code.toUpperCase(), token } })) {
          return undefined; // la connexion appartient désormais au relay
        } else {
          res = Response.json({ error: "upgrade impossible" }, { status: 500 });
        }
      } else if (url.pathname === "/api/health") {
        res = Response.json({
          ok: true,
          role: "static+api+lobby",
          uptime_s: Math.round((Date.now() - started) / 1000),
          rooms: rooms.count(),
          saves: saves.count(),
        });
      } else if (url.pathname.startsWith("/api/")) {
        const saveGet = url.pathname.match(/^\/api\/rooms\/([A-Za-z0-9]{5})\/save$/);
        if (saveGet && req.method === "GET") {
          // 4b : un siège récupère l'enveloppe (le client la passe à
          // loadGame pour retrouver l'état exact du monde).
          try {
            const env = saves.load(saveGet[1]);
            if (!env) res = Response.json({ error: "aucune sauvegarde pour ce code" }, { status: 404 });
            else {
              rooms.view(env.code, url.searchParams.get("token") ?? ""); // 404/403 + ping présence
              res = Response.json(env);
            }
          } catch (e) {
            if (e instanceof LobbyError) res = Response.json({ error: e.message }, { status: e.status });
            else throw e;
          }
        } else if (url.pathname === "/api/rooms/from-save" && req.method === "POST") {
          // 4b : reprise — recrée un salon depuis la save stockée d'un salon
          // (seed + classes par slot imposés ; l'état exact est restauré
          // chez chaque client par loadGame de `save`).
          try {
            const body = await readJson(req);
            const env = saves.load(str(body.code, 8));
            if (!env) res = Response.json({ error: "aucune sauvegarde pour ce code" }, { status: 404 });
            else {
              const r = rooms.fromSave(str(body.name, 16) || "Hôte", {
                seed: env.seed,
                players: env.players,
                origin: env.code,
              });
              res = Response.json(r, { status: 201 });
            }
          } catch (e) {
            if (e instanceof LobbyError) res = Response.json({ error: e.message }, { status: e.status });
            else throw e;
          }
        } else {
          const meta = url.pathname.match(/^\/api\/rooms\/([A-Za-z0-9]{5})\/meta$/);
          if (meta && req.method === "GET") {
            res = Response.json(rooms.meta(meta[1], url.searchParams.get("token") ?? ""));
          } else {
            res = (await lobbyRoute(req, url)) ?? Response.json({ error: "route inconnue" }, { status: 404 });
          }
        }
      } else {
        res = (await files.handle(req)) ?? new Response("méthode non supportée\n", { status: 405 });
      }
    } catch (e) {
      console.error(`${url.pathname} ! ${e && (e as Error).message}`);
      res = Response.json({ error: "erreur serveur" }, { status: 500 });
    }

    console.log(`${req.method} ${url.pathname} -> ${res.status} (${Math.round(performance.now() - t0)}ms)`);
    return res;
  },
  websocket: {
    open(ws) { relay.attach(ws, (ws.data as { code: string }).code, (ws.data as { token: string }).token); },
    message(ws, msg) {
      if (typeof msg === "string") return;
      if (msg instanceof ArrayBuffer) relay.message(ws, msg);
      else if (ArrayBuffer.isView(msg)) {
        relay.message(ws, (msg as Uint8Array).buffer.slice((msg as Uint8Array).byteOffset, (msg as Uint8Array).byteOffset + (msg as Uint8Array).byteLength));
      }
    },
    close(ws) { relay.detach(ws); },
  },
});

console.log(`d2w-serveur : web/=${ROOT} sur http://${HOST}:${PORT}`);
