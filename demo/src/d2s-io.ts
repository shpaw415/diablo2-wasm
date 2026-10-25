// Export / Import .d2s (étape 6b) — migration TS 25a de demo/d2s-io.js.
// La conversion vit côté moteur WASM (d2rules::save_d2s) ; ce module ne fait
// que l'UI : boutons « Exporter/Importer .d2s ». Contrat window.D2sIO
// inchangé (init/exportD2s/importD2s) ; init() câble les listeners.
// Partie pure testable : assainissement du nom de héros + libellé de rapport.
import { registerModule,
   parseOr, type D2Game, type D2sImportReport, type D2Window } from './types';

export interface D2sIOOpts {
  getGame?: () => D2Game | null;
  note?: (msg: string) => void;
  flash?: (msg: string) => void;
  onImported?: (report: D2sImportReport) => void;
}

let opts: D2sIOOpts | null = null;

function game(): D2Game | null {
  return opts && opts.getGame ? opts.getGame() : null;
}

/** Nom de fichier = nom de héros assaini (lettres/chiffres, ≤15). */
export function sanitizeHeroName(raw: string | undefined): string {
  const clean = String(raw || '')
    .replace(/[^a-zA-Z0-9]+/g, '')
    .slice(0, 15);
  return clean || 'heros';
}

/** Nom du héros courant lu sur le moteur (repli « heros »). */
export function heroNameOf(g: D2Game | null): string {
  try {
    const hc = parseOr<{ name?: string; code?: string }>(g ? g.heroClass() : '{}', {});
    return sanitizeHeroName(hc.name || hc.code);
  } catch {
    return 'heros';
  }
}

/** Message FR du rapport d'import (pur, testable). */
export function importMessage(report: D2sImportReport, fileName: string): string {
  const where = report.classChanged
    ? ` — classe ${report.class} (les sprites suivent au prochain lancement ?hero=${report.class})`
    : '';
  return (
    `Héros importé : ${report.name || fileName} — ${report.class} niv. ${report.level}` +
    `, ${report.items_loaded} objet(s), ${report.waypoints} waypoint(s)${where}.`
  );
}

function downloadBytes(bytes: Uint8Array, fileName: string): void {
  const blob = new Blob([bytes as BlobPart], { type: 'application/octet-stream' });
  const a = document.createElement('a');
  a.href = URL.createObjectURL(blob);
  a.download = fileName;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}

function exportD2s(): void {
  const g = game();
  if (!g || typeof g.exportD2s !== 'function') {
    if (opts) opts.note?.('Export .d2s indisponible (moteur sans exportD2s).');
    return;
  }
  try {
    const name = heroNameOf(g);
    const bytes = g.exportD2s!(name); // .d2s v0x60 VALIDE (checksum inclus)
    downloadBytes(bytes, `${name}.d2s`);
    opts?.flash?.(`Héros exporté : ${name}.d2s (format .d2s v1.12, checksum valide).`);
  } catch (e) {
    opts?.flash?.(`Export .d2s impossible : ${e instanceof Error ? e.message : String(e)}`);
  }
}

function importD2s(file: File): void {
  const g = game();
  if (!g || typeof g.importD2s !== 'function') {
    if (opts) opts.note?.('Import .d2s indisponible (moteur sans importD2s).');
    return;
  }
  file
    .arrayBuffer()
    .then((buf) => {
      const report = parseOr<D2sImportReport>(g.importD2s!(new Uint8Array(buf)), {}); // refus = throw
      opts?.flash?.(importMessage(report, file.name));
      if (opts?.onImported) opts.onImported(report);
    })
    .catch((e) => {
      opts?.flash?.(`Import .d2s refusé : ${e instanceof Error ? e.message : String(e)}`);
    });
}

export function initD2sIO(o: D2sIOOpts): void {
  opts = o || {};
  const btnExport = document.getElementById('btn-export-d2s');
  const btnImport = document.getElementById('btn-import-d2s');
  const fileInput = document.getElementById('file-d2s');
  if (!btnExport || !btnImport || !fileInput) return;
  btnExport.addEventListener('click', exportD2s);
  btnImport.addEventListener('click', () => (fileInput as HTMLInputElement).click());
  fileInput.addEventListener('change', () => {
    const input = fileInput as HTMLInputElement;
    const f = input.files && input.files[0];
    if (f) importD2s(f);
    input.value = ''; // permet de ré-importer le même fichier
  });
}

export function installD2sIO(win: D2Window): void {
  registerModule('d2s-io', 'ts');
  win.D2sIO = { init: initD2sIO, exportD2s, importD2s };
}

if (typeof window !== 'undefined') installD2sIO(window as unknown as D2Window);
