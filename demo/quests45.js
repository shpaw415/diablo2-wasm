// Diablo 2 WASM — journal de quêtes ACTES IV-V (étape 10c) — glue navigateur
// SELF-CONTENUE, miroir du moteur d2rules/src/quests_actes45.rs.
//
// Rôle (mission 10c, point 3 — UI minimale) : afficher les 9 quêtes A4-A5
// dans un panneau séparé du journal W13 (demo/quests.js, figé acte I).
// Aucune modification de game.js / quests.js : ce fichier s'inclut avec
//   <script src="./quests45.js"></script>
// puis D2Quests45.init() — le panneau (id `d2-quests45`) s'ouvre via
// D2Quests45.show()/toggle() ou l'event DOM `d2quests45:toggle` (quand le
// panneau principal supportera un second journal, il n'aura qu'à émettre
// cet event ou appeler feed()).
//
// MÊME contrat d'events que le moteur (serde tagged de d2rules::ActEvent) :
//   {"ZoneEntered":{"level_id":105}} · {"MonsterKilled":{"code":"izual",
//    "superunique":null}} · {"NpcTalked":{"npc":"tyrael"}}
//   {"ItemPicked":{"code":"mss"}}
// MÊME format d'état JSON (version 1, 9 slots) pour le round-trip Rust :
// D2Quests45.importLog(D2Quests45.exportLog()).
//
// Ordre des ids = ORDRE DE SAUVEGARDE .d2s (⚠ acte IV : a4q2 = DIABLO,
// a4q3 = la Forge — clés qstsa4q* du string.tbl, PAS l'ordre du journal
// wiki). Offsets .d2s : a4q1 @395, a4q2 @397, a4q3 @399, a5q1..6 @415-425
// (Normal ; +96 Cauchemar, +192 Enfer).
(function () {
  'use strict';

  // ---------------------------------------------------------------------
  // Authoring A4/A5 — miroir de DEFS_ACT4/DEFS_ACT5 (quests_actes45.rs).
  // Triggers : { zone: id } · { kill: [aliases] } · { talk: npc } ·
  // { item: code } · step.any = [{...}×n] pour les sous-objectifs hors
  // ordre (sceaux, cages, Anciens — masque subdone).
  // ---------------------------------------------------------------------
  const DEFS = [
    { id: 'a4q1', act: 4, name: 'The Fallen Angel', gates: 'act5',
      steps: [
        { desc: 'Traversez les Plaines du Désespoir.', zone: 105 },
        { desc: 'Abattez Izual pour libérer son âme.', kill: ['izual'] },
      ], npc: 'tyrael' },
    { id: 'a4q2', act: 4, name: "Terror's End", gates: 'act5',
      steps: [
        { desc: 'Trouvez Diablo dans son sanctuaire (Chaos Sanctuaire).', zone: 108 },
        { desc: 'Brisez les sceaux et abattez leurs gardiens.',
          any: [{ kill: ['infector of souls', 'megademon3'] }, { kill: ['lord de seis', 'doomknight3'] }, { kill: ['grand vizier of chaos', 'fingermage3'] }] },
        { desc: 'Tuez Diablo.', kill: ['diablo'] },
      ], npc: 'tyrael' },
    { id: 'a4q3', act: 4, name: "Hell's Forge",
      steps: [
        { desc: "Portez la Pierre d'âme de Mephisto.", item: 'mss' },
        { desc: 'Descendez dans la Rivière de feu.', zone: 107 },
        { desc: 'Abattez Hephasto le forgeron.', kill: ['hephasto', 'The Feature Creep'] },
        { desc: 'Emparez-vous du Marteau de la Forge infernale.', item: 'hfh' },
      ], npc: 'cain' },
    { id: 'a5q1', act: 5, name: 'Siege on Harrogath',
      steps: [
        { desc: 'Percez les Contreforts sanglants et levez le siège.', zone: 110 },
        { desc: 'Abattez Shenk le Surveillant (Siege Boss).', kill: ['siegeboss', 'Siege Boss', 'overseer1', 'shenk'] },
      ], npc: 'larzuk' },
    { id: 'a5q2', act: 5, name: 'Rescue on Mount Arreat',
      steps: [
        { desc: 'Libérez les barbares prisonniers des Hautes-Glaces (4 extérieurs).',
          any: [{ zone: 110 }, { zone: 111 }, { zone: 112 }, { zone: 117 }], count: 4 },
      ], npc: 'qual-kehk' },
    { id: 'a5q3', act: 5, name: 'Prison of Ice',
      steps: [
        { desc: 'Trouvez Anya gelée sous le Passage Cristallin (Ice Cave 3A).', zone: 119 },
        { desc: 'Abattez Frozenstein qui garde la prison de glace.', kill: ['frozenstein', 'snowyeti4', 'Frozenstein'] },
        { desc: 'Donnez à Anya la potion de Malah et ramenez-la en ville.', talk: 'anya' },
      ], npc: 'malah' },
    { id: 'a5q4', act: 5, name: 'Betrayal of Harrogath', requires: 'a5q3',
      steps: [
        { desc: "Prenez le portail d'Anya vers le temple de Nihlathak.", talk: 'anya' },
        { desc: 'Descendez jusqu’au Repaire de Vaught (Halls of Vaught).', zone: 124 },
        { desc: 'Tuez Nihlathak le traître.', kill: ['nihlathak', 'nihlathakboss', 'Nihlathak Boss'] },
      ], npc: 'anya' },
    { id: 'a5q5', act: 5, name: 'Rite of Passage',
      steps: [
        { desc: "Consultez les Anciens en activant l'Autel des Cieux (Sommet d'Arreat).", zone: 120 },
        { desc: 'Affrontez les trois Anciens sans quitter le Mont Arreat.',
          any: [{ kill: ['ancientbarb1', 'Ancient Barbarian 1'] }, { kill: ['ancientbarb2', 'Ancient Barbarian 2'] }, { kill: ['ancientbarb3', 'Ancient Barbarian 3'] }] },
      ], npc: null },
    { id: 'a5q6', act: 5, name: 'Eve of Destruction', requires: 'a5q5', final: true,
      steps: [
        { desc: 'Affrontez les vagues de Baal dans le Trône de la Destruction.', zone: 131 },
        { desc: "Tuez Baal dans la Chambre de la Pierre d'Worldstone.", kill: ['baal', 'baalcrab'] },
        { desc: 'Parlez à Tyrael et prenez son portail.', talk: 'tyrael' },
      ], npc: null },
  ];

  const ACT_TITLES = { 4: 'Acte IV — la Forteresse de l’Enfer', 5: 'Acte V — Harrogath (fin de jeu)' };
  const STATE_VERSION = 1;

  function trigMatches(t, ev) {
    if (t.kill) {
      if (ev.kind !== 'MonsterKilled') return false;
      return t.kill.some((a) => a.toLowerCase() === (ev.code || '').toLowerCase()
        || a.toLowerCase() === (ev.superunique || '').toLowerCase());
    }
    if (t.zone !== undefined) return ev.kind === 'ZoneEntered' && ev.level_id === t.zone;
    if (t.item !== undefined) return ev.kind === 'ItemPicked' && String(ev.code).toLowerCase() === t.item.toLowerCase();
    if (t.talk !== undefined) return ev.kind === 'NpcTalked' && String(ev.npc).toLowerCase() === t.talk.toLowerCase();
    return false;
  }

  function stepOf(def, r) { return r.step < def.steps.length ? def.steps[r.step] : null; }

  class Quests45Log {
    constructor() {
      this.runs = DEFS.map(() => ({ id: null, phase: 'inactive', step: 0, count: 0, flags: 0, subdone: 0 }));
    }
    index(id) { return DEFS.findIndex((d) => d.id === id); }
    phase(id) { const i = this.index(id); return i < 0 ? null : this.runs[i].phase; }
    done(id) { return this.phase(id) === 'done'; }

    gateOpen(i) {
      const d = DEFS[i];
      if (d.requires && !this.done(d.requires)) return false;
      return true;
    }

    onEvent(event) {
      if (!event || typeof event !== 'object') return [];
      const keys = Object.keys(event);
      const ev = { kind: keys[0], ...(event[keys[0]] || {}) };
      const out = [];
      for (let i = 0; i < DEFS.length; i++) {
        const d = DEFS[i];
        const r = this.runs[i];
        if (r.phase === 'done') continue;
        if (r.phase === 'reward_pending') {
          if (ev.kind === 'NpcTalked' && d.npc && String(ev.npc).toLowerCase() === d.npc.toLowerCase()) {
            r.phase = 'done';
            r.flags = (r.flags & ~2) | 1;
            if (d.id === 'a5q3') r.flags |= 128; // bit 7 : parchemin de résistance consommé
            out.push({ id: d.id, phase: 'done' });
          }
          continue;
        }
        if (!this.gateOpen(i)) continue;
        const st = stepOf(d, r);
        if (!st) continue;
        let hit = -1;
        if (st.any) {
          hit = st.any.findIndex((t, j) => !((r.subdone >> j) & 1) && trigMatches(t, ev));
          if (hit < 0) continue;
          r.subdone |= 1 << hit;
        } else if (!trigMatches(st, ev)) {
          continue;
        }
        if (r.phase === 'inactive') {
          r.phase = 'active';
          if (ev.kind === 'NpcTalked') r.flags |= 4; // bit 2 « donnée par PNJ »
          out.push({ id: d.id, phase: 'active' });
        }
        r.count += 1;
        const req = Math.max(1, st.count || (st.any ? st.any.length : 1));
        if (r.count >= req) {
          r.count = 0;
          r.subdone = 0;
          if (r.step + 1 < d.steps.length) {
            r.step += 1;
          } else if (d.npc) {
            r.phase = 'reward_pending';
            r.flags |= 2; // bit 1 « récompense à prendre »
            out.push({ id: d.id, phase: 'reward_pending' });
          } else {
            r.phase = 'done';
            r.flags = (r.flags & ~2) | 1;
            if (d.id === 'a5q3') r.flags |= 128;
            out.push({ id: d.id, phase: 'done', final: !!d.final });
          }
        }
      }
      return out;
    }

    to_json() {
      return JSON.stringify({
        version: STATE_VERSION,
        quests: DEFS.map((d, i) => ({ id: d.id, act: d.act, ...this.runs[i] })),
      });
    }

    static from_json(json) {
      const s = JSON.parse(json); // throw si invalide (contrat from_json Rust)
      if (s.version !== STATE_VERSION || !Array.isArray(s.quests) || s.quests.length !== DEFS.length) {
        throw new Error('quests45: état inconnu');
      }
      const log = new Quests45Log();
      s.quests.forEach((q, i) => {
        if (q.id !== DEFS[i].id) throw new Error(`quests45: id ${q.id} ≠ ${DEFS[i].id}`);
        if (!['inactive', 'active', 'reward_pending', 'done'].includes(q.phase)) {
          throw new Error(`quests45: phase ${q.phase}`);
        }
        log.runs[i] = { id: null, phase: q.phase, step: q.step, count: q.count, flags: q.flags, subdone: q.subdone };
      });
      return log;
    }
  }

  // ---------------------------------------------------------------------
  // Panneau journal A4-A5 (self-contenu, même style que quests.js).
  // ---------------------------------------------------------------------
  const CSS = `
#d2-quests45 { position: fixed; top: 14px; right: 334px; width: 300px; max-height: 78vh;
  overflow-y: auto; background: #14110aee; border: 1px solid #5a4a2a; border-radius: 4px;
  padding: 10px 12px; z-index: 60; color: #cfc7b8; font: 12px/1.45 system-ui, sans-serif;
  box-shadow: 0 4px 24px #000c; }
#d2-quests45 h2 { margin: 0 0 2px; font-size: 13px; font-weight: 700; letter-spacing: .1em;
  color: #c7a45a; text-transform: uppercase; }
#d2-quests45 .d2q-act { color: #8f8676; font-size: 11px; margin: 6px 0 4px; letter-spacing: .06em; }
#d2-quests45 ul { list-style: none; margin: 0; padding: 0; }
#d2-quests45 li { padding: 6px 0; border-top: 1px solid #2e2718; }
#d2-quests45 .d2q-name { font-weight: 600; font-size: 12px; }
#d2-quests45 .d2q-status { float: right; font-size: 10px; font-weight: 700; letter-spacing: .08em;
  text-transform: uppercase; margin-left: 6px; }
#d2-quests45 .d2q-obj { color: #a89670; font-size: 11px; margin-top: 2px; }
#d2-quests45 .d2q-prog { color: #c7a45a; font-weight: 600; }
#d2-quests45 .inactive .d2q-name { color: #6e6555; }
#d2-quests45 .active .d2q-name { color: #e8dcbf; }
#d2-quests45 .active .d2q-status { color: #d9b96a; }
#d2-quests45 .reward_pending .d2q-status { color: #d9b96a; }
#d2-quests45 .done .d2q-name { color: #c7a45a; }
#d2-quests45 .done .d2q-status { color: #7ea05f; }
#d2-quests45[hidden] { display: none; }
`;

  const STATUS_FR = { inactive: 'À venir', active: 'En cours', reward_pending: 'Récompense', done: 'Terminée' };

  let doc = null;
  let panel = null;
  let listEl = null;
  let log = new Quests45Log();
  let isOpen = false;

  function render() {
    if (!listEl) return;
    listEl.textContent = '';
    let lastAct = 0;
    DEFS.forEach((d, i) => {
      const r = log.runs[i];
      if (d.act !== lastAct) {
        lastAct = d.act;
        const act = doc.createElement('div');
        act.className = 'd2q-act';
        act.textContent = ACT_TITLES[d.act];
        listEl.appendChild(act);
      }
      const li = doc.createElement('li');
      li.className = r.phase;
      const head = doc.createElement('div');
      const status = doc.createElement('span');
      status.className = 'd2q-status';
      status.textContent = (r.phase === 'done' ? '✔ ' : '') + STATUS_FR[r.phase];
      const name = doc.createElement('span');
      name.className = 'd2q-name';
      name.textContent = d.name;
      head.append(status, name);
      li.appendChild(head);
      if (r.phase === 'active') {
        const st = stepOf(d, r);
        const obj = doc.createElement('div');
        obj.className = 'd2q-obj';
        obj.textContent = st ? st.desc : '';
        if (st && st.any) {
          const prog = doc.createElement('span');
          prog.className = 'd2q-prog';
          prog.textContent = ` (${r.count}/${Math.max(1, st.count || st.any.length)})`;
          obj.appendChild(prog);
        }
        li.appendChild(obj);
      } else if (r.phase === 'reward_pending') {
        const obj = doc.createElement('div');
        obj.className = 'd2q-obj';
        obj.textContent = 'Retournez voir le PNJ pour la récompense.';
        li.appendChild(obj);
      } else if (r.phase === 'done' && d.final) {
        const obj = doc.createElement('div');
        obj.className = 'd2q-obj';
        obj.textContent = 'Fin de jeu — la Pierre d’Worldstone est détruite.';
        li.appendChild(obj);
      }
      listEl.appendChild(li);
    });
  }

  const api = {
    Quests45Log,
    DEFS,
    /** Construit le panneau. `dom` = Document (défaut). */
    init(dom) {
      doc = dom || (typeof document !== 'undefined' ? document : null);
      if (!doc || panel) return api;
      const style = doc.createElement('style');
      style.textContent = CSS;
      doc.head.appendChild(style);
      panel = doc.createElement('div');
      panel.id = 'd2-quests45';
      panel.hidden = true;
      const title = doc.createElement('h2');
      title.textContent = 'Journal — Actes IV-V';
      listEl = doc.createElement('ul');
      panel.append(title, listEl);
      doc.body.appendChild(panel);
      doc.addEventListener('d2quests45:toggle', () => api.toggle());
      return api;
    },
    /** Pousse un event (JSON sérialisé de d2rules::ActEvent). Renvoie les
     * changements de phase ; `{id:"a5q6", phase:"done", final:true}` = fin
     * de jeu (GameWon côté moteur). */
    feed(event) {
      const changes = log.onEvent(event);
      if (isOpen) render();
      return changes;
    },
    get log() { return log; },
    reset() { log = new Quests45Log(); if (isOpen) render(); },
    exportLog() { return log.to_json(); },
    importLog(json) { log = Quests45Log.from_json(json); if (isOpen) render(); return true; },
    show() { isOpen = true; if (panel) panel.hidden = false; render(); },
    hide() { isOpen = false; if (panel) panel.hidden = true; },
    toggle() { isOpen ? api.hide() : api.show(); },
    get isOpen() { return isOpen; },
  };

  const root = typeof window !== 'undefined' ? window : globalThis;
  root.D2Quests45 = api;
})();
