// d2w-serveur — service statique de web/ (phase 1 du plan serveur-coop).
// Politique cache : assets/ immuable (données extraites), game-pkg/ revalidé par
// ETag (artefacts de build), entrées HTML/JS en no-cache. Brotli en mémoire sur
// le texte ≤ 1,5 Mo.

import { brotliCompressSync, constants as zlibConstants } from "node:zlib";
import { join, resolve, sep } from "node:path";

export interface StaticOpts {
  root: string;
}

const MIME: Record<string, string> = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json; charset=utf-8",
  ".txt": "text/plain; charset=utf-8",
  ".md": "text/plain; charset=utf-8",
  ".wasm": "application/wasm",
  ".png": "image/png",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".webp": "image/webp",
  ".gif": "image/gif",
  ".svg": "image/svg+xml",
  ".ico": "image/x-icon",
  ".mp3": "audio/mpeg",
  ".ogg": "audio/ogg",
  ".wav": "audio/wav",
  ".glb": "model/gltf-binary",
  ".map": "application/json",
};

const TEXT = new Set([
  ".html", ".js", ".mjs", ".css", ".json", ".txt", ".md", ".svg", ".map",
]);

const MAX_COMPRESS = 1_572_864; // 1,5 Mo
const COMPRESS_LEVEL = 5; // vitesse > ratio : CPU logiciel, pas de temps à perdre

/** Policy de cache par chemin (relatif à la racine, avec « / » initial). */
function cacheControl(rel: string): string {
  // Le manifest et les tables .txt sont des DONNÉES VIVANTES (vagues de
  // sprites, tables marchandes) : revalidation systématique — JAMAIS
  // immuable, sinon le client joue des semaines avec un manifest périmé.
  if (rel === "/assets/manifest.json" || (rel.startsWith("/assets/") && rel.endsWith(".txt"))) {
    return "no-cache";
  }
  // Le gros des assets (dcc/map/sfx, décors) ne change jamais : immuable.
  if (rel.startsWith("/assets/")) return "public, max-age=604800, immutable";
  if (rel.startsWith("/game-pkg/")) return "no-cache"; // revalidation ETag (304)
  if (rel.endsWith(".html") || rel.endsWith(".css")) return "no-cache";
  if (/^\/[A-Za-z0-9_-]+\.js$/.test(rel)) return "no-cache"; // entrées JS racine
  return "public, max-age=3600";
}

export class StaticFiles {
  private root: string;
  /** brotli pré-calculé, clé = chemin|mtime|taille */
  private br = new Map<string, Uint8Array>();

  constructor(opts: StaticOpts) {
    this.root = resolve(opts.root) + sep;
  }

  /** Réponse statique pour une requête GET/HEAD, ou null si méthode non supportée. */
  async handle(req: Request): Promise<Response | null> {
    if (req.method !== "GET" && req.method !== "HEAD") return null;

    let pathname: string;
    try {
      pathname = decodeURIComponent(new URL(req.url).pathname);
    } catch {
      return this.text(400, "chemin invalide\n");
    }
    if (pathname.includes("\0")) return this.text(400, "chemin invalide\n");

    const rel = pathname === "/" ? "/index.html" : pathname;
    const abs = resolve(join(this.root, rel));
    if (abs !== this.root.slice(0, -1) && !abs.startsWith(this.root)) {
      return this.text(403, "interdit\n");
    }

    let f = Bun.file(abs);
    if (!(await f.exists())) return this.notFound();
    if (f.size === 0 && (await Bun.file(abs + "/").exists())) {
      f = Bun.file(abs + "/index.html"); // répertoire demandé
      if (!(await f.exists())) return this.notFound();
    }

    const st = await f.stat();
    const etag = `W/"${st.size.toString(36)}-${Math.round(st.mtime ?? 0).toString(36)}"`;
    const headers = new Headers({
      "Content-Type": MIME[ext(rel)] ?? "application/octet-stream",
      "Cache-Control": cacheControl(rel),
      ETag: etag,
      "X-Content-Type-Options": "nosniff",
    });

    if (req.headers.get("if-none-match") === etag) {
      return new Response(null, { status: 304, headers });
    }

    const wantsBr = /\bbr\b/.test(req.headers.get("accept-encoding") ?? "");
    if (wantsBr && TEXT.has(ext(rel)) && st.size <= MAX_COMPRESS) {
      const key = `${abs}|${st.mtime}|${st.size}`;
      let body = this.br.get(key);
      if (!body) {
        body = new Uint8Array(
          brotliCompressSync(new Uint8Array(await f.arrayBuffer()), {
            params: { [zlibConstants.BROTLI_PARAM_QUALITY]: COMPRESS_LEVEL },
          }),
        );
        if (this.br.size > 256) this.br.clear(); // garde-fou mémoire simple
        this.br.set(key, body);
      }
      headers.set("Content-Encoding", "br");
      headers.set("Vary", "Accept-Encoding");
      if (req.method === "HEAD") return new Response(null, { headers });
      return new Response(body, { headers });
    }

    headers.set("Content-Length", String(st.size));
    if (req.method === "HEAD") return new Response(null, { headers });
    return new Response(f, { headers });
  }

  private notFound(): Response {
    const page = Bun.file(join(this.root, "404.html"));
    return new Response(page, {
      status: 404,
      headers: { "Content-Type": MIME[".html"], "Cache-Control": "no-cache" },
    });
  }

  private text(status: number, body: string): Response {
    return new Response(body, {
      status,
      headers: { "Content-Type": "text/plain; charset=utf-8" },
    });
  }
}

function ext(p: string): string {
  const i = p.lastIndexOf(".");
  return i < 0 ? "" : p.slice(i).toLowerCase();
}
