// Export / Import .d2s (étape 6b) — format d'échange OPTIONNEL.
// La conversion vit entièrement côté moteur WASM (d2rules::save_d2s, exposé
// par game-pkg en exportD2s/importD2s) ; ce fichier ne fait que l'UI :
// bouton « Exporter .d2s » (téléchargement <nom>.d2s) et « Importer .d2s »
// (input file → octets → le héros est remplacé dans le monde courant).
// Câblage minimal depuis game.js : window.D2sIO.init({ getGame, note, flash }).
window.D2sIO = (function () {
  let opts = null;

  function game() {
    return opts && opts.getGame ? opts.getGame() : null;
  }

  // Nom de fichier = nom de héros affiché, assaini (le moteur re-assainit).
  function heroName() {
    let name = 'heros';
    try {
      const hc = JSON.parse(game().heroClass() || '{}');
      const clean = String(hc.name || hc.code || '')
        .replace(/[^a-zA-Z0-9]+/g, '')
        .slice(0, 15);
      if (clean) name = clean;
    } catch { /* nom par défaut */ }
    return name;
  }

  function exportD2s() {
    const g = game();
    if (!g || typeof g.exportD2s !== 'function') {
      if (opts) opts.note('Export .d2s indisponible (moteur sans exportD2s).');
      return;
    }
    try {
      const name = heroName();
      const bytes = g.exportD2s(name); // .d2s v0x60 VALIDE (checksum inclus)
      const blob = new Blob([bytes], { type: 'application/octet-stream' });
      const a = document.createElement('a');
      a.href = URL.createObjectURL(blob);
      a.download = `${name}.d2s`;
      a.click();
      setTimeout(() => URL.revokeObjectURL(a.href), 2000);
      opts.flash(`Héros exporté : ${a.download} (format .d2s v1.12, checksum valide).`);
    } catch (e) {
      opts.flash(`Export .d2s impossible : ${e && e.message ? e.message : e}`);
    }
  }

  function importD2s(file) {
    const g = game();
    if (!g || typeof g.importD2s !== 'function') {
      if (opts) opts.note('Import .d2s indisponible (moteur sans importD2s).');
      return;
    }
    file
      .arrayBuffer()
      .then((buf) => {
        const report = JSON.parse(g.importD2s(new Uint8Array(buf))); // refus = throw
        const where = report.classChanged
          ? ` — classe ${report.class} (les sprites suivent au prochain lancement ?hero=${report.class})`
          : '';
        opts.flash(
          `Héros importé : ${report.name || file.name} — ${report.class} niv. ${report.level}` +
            `, ${report.items_loaded} objet(s), ${report.waypoints} waypoint(s)${where}.`
        );
        if (opts.onImported) opts.onImported(report);
      })
      .catch((e) => {
        opts.flash(`Import .d2s refusé : ${e && e.message ? e.message : e}`);
      });
  }

  function init(o) {
    opts = o || {};
    const btnExport = document.getElementById('btn-export-d2s');
    const btnImport = document.getElementById('btn-import-d2s');
    const fileInput = document.getElementById('file-d2s');
    if (!btnExport || !btnImport || !fileInput) return;
    btnExport.addEventListener('click', exportD2s);
    btnImport.addEventListener('click', () => fileInput.click());
    fileInput.addEventListener('change', () => {
      const f = fileInput.files && fileInput.files[0];
      if (f) importD2s(f);
      fileInput.value = ''; // permet de ré-importer le même fichier
    });
  }

  return { init, exportD2s, importD2s };
})();
