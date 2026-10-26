// Diablo 2 WASM — gestionnaire de slots de sauvegarde (mission save P0).
//
// Stockage : localStorage["d2wasm.slots.v1"] = {"slots":[{…}]} — 5 slots
// nommés, identité par héros (un slot = un personnage ; re-sauvegarder le
// même héros écrase SA fiche, en conservant son nom personnalisé).
//
// Rétro-compat totale avec la clé historique "d2wasm.save.v1" : la
// sauvegarde la plus récente y est mirrorée à chaque écriture, et une base
// de slots vide l'importe comme slot fondateur au premier list(). Les
// anciens liens game.html?continue=1 continuent donc de fonctionner.
//
// Chaque slot porte les métadonnées d'affichage du menu (classe, niveau,
// temps de jeu, zone, date) lues DIRECTEMENT du JSON de sauvegarde d2rules
// (v3 : hero_code, hero.stats.level, now_ms, level) — aucune duplication
// d'état, la fiche est recalculée à chaque save.
//
// Script classique (pas un module) : chargé par menu.html ET game.html,
// exposé en window.D2Slots.
(function () {
  'use strict';
  const KEY = 'd2wasm.slots.v1';
  const LEGACY_KEY = 'd2wasm.save.v1';
  const MAX = 5;

  // Libellés FR des classes (mêmes codes que manifest.heroes / menu.js).
  const LABELS = {
    BA: 'Barbare', SOR: 'Sorcière', SO: 'Sorcière', PA: 'Paladin', AM: 'Amazone',
    AS: 'Assassin', DZ: 'Druide', NE: 'Nécromancien'
  };
  const ZONES = { town: 'Camp des Rousses', bloodmoor: 'Blood Moor' };

  function list() {
    let slots = readSlots();
    // Migration douce : une base vide + un save historique ⇒ slot fondateur.
    if (!slots.length) {
      const legacy = localStorage.getItem(LEGACY_KEY);
      if (legacy) {
        const meta = metaOf(legacy);
        if (meta) {
          slots = [Object.assign({ id: newId(), name: defaultName(meta), ts: Date.now() }, meta, { save: legacy })];
          store(slots);
        }
      }
    }
    return slots.slice().sort((a, b) => b.ts - a.ts);
  }

  function readSlots() {
    try {
      const r = JSON.parse(localStorage.getItem(KEY) || 'null');
      if (r && Array.isArray(r.slots)) return r.slots.filter((s) => s && s.id && typeof s.save === 'string');
    } catch (e) { /* base corrompue : repart à vide */ }
    return [];
  }

  function store(slots) {
    localStorage.setItem(KEY, JSON.stringify({ slots }));
    // miroir : la clé historique = save le plus récent (menu.js Continuer,
    // game.html?continue=1 des anciens onglets).
    const newest = slots.slice().sort((a, b) => b.ts - a.ts)[0];
    if (newest) localStorage.setItem(LEGACY_KEY, newest.save);
  }

  function newId() { return 's' + Date.now().toString(36) + Math.random().toString(36).slice(2, 7); }

  // Métadonnées extraites d'un JSON de sauvegarde d2rules (v1..v3).
  // null si le JSON est inexploitable (pas un save).
  function metaOf(saveJson) {
    try {
      const v = typeof saveJson === 'string' ? JSON.parse(saveJson) : saveJson;
      if (!v || typeof v !== 'object' || !v.hero || !v.version) return null;
      const heroCode = String(v.hero_code || 'BA').toUpperCase();
      const level = (v.hero && v.hero.stats && v.hero.stats.level) || 1;
      const playtime = v.now_ms || 0;
      const levelKey = typeof v.level === 'string' && v.level ? v.level.toLowerCase() : 'town';
      return {
        hero_code: heroCode,
        hero_label: LABELS[heroCode] || heroCode,
        level: level,
        playtime_ms: playtime,
        level_key: levelKey,
        zone: ZONES[levelKey] || levelKey,
        version: v.version,
      };
    } catch (e) { return null; }
  }

  function defaultName(meta) {
    return meta.hero_label + ' niv. ' + meta.level;
  }

  // Crée/écrase le slot du héros (identité hero_code) avec ce save.
  // Renvoie la fiche à jour {id, name, ts, …}.
  function upsertFromSave(saveJson, opts) {
    const meta = metaOf(saveJson);
    if (!meta) throw new Error('sauvegarde illisible (JSON d2rules attendu)');
    const slots = readSlots();
    const now = Date.now();
    let slot = slots.find((s) => s.hero_code === meta.hero_code);
    if (slot) {
      slot.ts = now;
      slot.save = saveJson;
      Object.assign(slot, meta);
    } else {
      if (slots.length >= MAX) {
        throw new Error('Slots pleins (' + MAX + ') — supprimez un héros depuis le menu « Parties ».');
      }
      slot = Object.assign({ id: newId(), name: (opts && opts.name) || defaultName(meta), ts: now }, meta, { save: saveJson });
      slots.push(slot);
    }
    store(slots);
    return slot;
  }

  function get(id) { return readSlots().find((s) => s.id === id) || null; }

  function remove(id) {
    const slots = readSlots().filter((s) => s.id !== id);
    store(slots);
    return slots;
  }

  function rename(id, name) {
    const slots = readSlots();
    const slot = slots.find((s) => s.id === id);
    const clean = String(name || '').trim().slice(0, 24);
    if (slot && clean) { slot.name = clean; store(slots); }
    return slot;
  }

  // Sauvegarde la plus récente (pour « Continuer »), null si aucune.
  function mostRecent() { return list()[0] || null; }

  // Export .json : le slot complet (métadonnées + save) — l'import le
  // relit tel quel. Nom de fichier lisible : d2-<nom>-<date>.json.
  function exportSlot(id) {
    const slot = get(id);
    if (!slot) throw new Error('slot inconnu');
    const blob = new Blob([JSON.stringify(slot, null, 1)], { type: 'application/json' });
    const a = document.createElement('a');
    a.href = URL.createObjectURL(blob);
    a.download = 'd2-' + slot.name.replace(/[^a-z0-9]+/gi, '-').toLowerCase() + '-' + new Date(slot.ts).toISOString().slice(0, 10) + '.json';
    document.body.appendChild(a);
    a.click();
    setTimeout(() => { URL.revokeObjectURL(a.href); a.remove(); }, 400);
    return slot;
  }

  // Import : lit le texte d'un .json exporté (slot complet OU save brut),
  // valide, l'installe. Format complet : remplace la fiche de CE héros
  // (même id si elle existe) — prévisible et sans doublon. Renvoie la fiche.
  function importSlot(text) {
    const v = JSON.parse(text); // throw = message erreur appelant
    if (v && typeof v.save === 'string' && v.hero_code) {
      // format export complet
      const meta = metaOf(v.save);
      if (!meta) throw new Error('fichier : sauvegarde interne illisible');
      const slots = readSlots();
      const existing = slots.find((s) => s.hero_code === String(v.hero_code).toUpperCase());
      const slot = Object.assign(
        { id: existing ? existing.id : newId(), name: v.name || defaultName(meta), ts: v.ts || Date.now() },
        meta,
        { save: v.save }
      );
      if (existing) slots.splice(slots.indexOf(existing), 1, slot);
      else {
        if (slots.length >= MAX) throw new Error('Slots pleins (' + MAX + ') — supprimez un héros.');
        slots.push(slot);
      }
      store(slots);
      return slot;
    }
    // format save brut (game.html Sauvegarder → export manuel)
    const slot = upsertFromSave(text);
    return slot;
  }

  window.D2Slots = { MAX, list, get, remove, rename, upsertFromSave, mostRecent, metaOf, exportSlot, importSlot };
})();
