// Diablo 2 WASM — journal de quêtes (W13) — glue navigateur SELF-CONTENUE.
//
// Miroir JS du moteur d2rules/src/quests.rs : mêmes définitions Acte I,
// mêmes sémantiques (étapes séquentielles, auto-start étape 0, joker "*",
// BossDead ⇔ MonsterKilled même code, états figés, unlock d'acte) et le
// MÊME format d'état JSON (version 1) pour round-trip avec le Rust.
//
// Intégration (faite en intégration, cf. docs/parity/impl-quetes.md) :
//   <script src="./quests.js"></script>          (avant ou après game.js)
//   D2Quests.init();                             // construit le panneau + touche Q
//   D2Quests.feed(evJson);                       // 1× par GameEvent du moteur
//   D2Quests.importLog(D2Quests.exportLog());    // persistance localStorage
//
// Forme des événements : le JSON sérialisé de QuestEvent (serde, tagged) —
//   {"MonsterKilled":{"code":"FA"}} · {"ItemPickedUp":{"code":"bks"}}
//   {"ZoneEntered":{"level_id":8}} · {"NpcTalked":{"npc":"akara"}}
//   {"BossDead":{"code":"andariel"}}
// (+ raccourci {type:"MonsterKilled", code:"FA"} accepté pour la console).
//
// Libellés FR provisoires — les TBL fr manquent (W18) ; noms de quêtes EN
// canoniques (wiki D2, aucune table de quêtes dans les MPQ).
(function () {
  'use strict';

  // ---------------------------------------------------------------------
  // Authoring Acte I — MIROIR EXACT de quest_defs_act1() (quests.rs).
  // Codes réels : boss monstats (bloodraven/smith/andariel), objets
  // (bks Bark Scroll, hamm Malus, rin bague), niveaux patch_levels
  // (8 = Cave 1 Den of Evil, 38 = Tristram).
  // ---------------------------------------------------------------------
  const DEFS = [
    {
      id: 'a1q1', act: 1, name: 'Den of Evil', gates_act: false,
      summary: 'Purifiez la Tanière du Mal qui souille la forêt du Camp des Rousses.',
      steps: [
        { desc: 'Entrez dans la Tanière du Mal (Act 1 - Cave 1).', trigger: { ZoneEntered: { level_id: 8 } }, count: 1 },
        { desc: 'Purifiez la Tanière : abattez ses monstres.', trigger: { MonsterKilled: { code: '*' } }, count: 6 },
      ],
      rewards: [{ Unlock: { flag: 'skill_point' } }],
    },
    {
      id: 'a1q2', act: 1, name: "Sisters' Burial Grounds", gates_act: false,
      summary: 'Rendez le repos à Blood Raven, qui corrompt le cimetière des Blood Moor.',
      steps: [
        { desc: 'Abattez Blood Raven au cimetière des Blood Moor.', trigger: { BossDead: { code: 'bloodraven' } }, count: 1 },
      ],
      rewards: [{ Unlock: { flag: 'rogue_scout' } }],
    },
    {
      id: 'a1q3', act: 1, name: 'The Search for Cain', gates_act: false,
      summary: "Découvrez le sort du dernier Horadrim : l'Arbre d'Inifuss garde la clé des pierres de Cairn.",
      steps: [
        { desc: "Récoltez l'écorce sur l'Arbre d'Inifuss (Dark Wood).", trigger: { ItemPickedUp: { code: 'bks' } }, count: 1 },
        { desc: 'Faites déchiffrer le parchemin par Akara.', trigger: { NpcTalked: { npc: 'akara' } }, count: 1 },
        { desc: 'Ouvrez le portail et entrez à Tristram.', trigger: { ZoneEntered: { level_id: 38 } }, count: 1 },
        { desc: 'Délivrez Deckard Cain de sa cage.', trigger: { NpcTalked: { npc: 'cain' } }, count: 1 },
      ],
      rewards: [{ Item: { code: 'rin' } }, { Unlock: { flag: 'cain_free_id' } }],
    },
    {
      id: 'a1q4', act: 1, name: 'Tools of the Trade', gates_act: false,
      summary: 'Reprenez le marteau Horadrique aux mains du Smith, dans la Caserne du monastère.',
      steps: [
        { desc: 'Abattez le Smith dans la Caserne.', trigger: { BossDead: { code: 'smith' } }, count: 1 },
        { desc: 'Ramassez le Horadric Malus (hamm).', trigger: { ItemPickedUp: { code: 'hamm' } }, count: 1 },
        { desc: 'Rapportez le Malus à Charsi.', trigger: { NpcTalked: { npc: 'charsi' } }, count: 1 },
      ],
      rewards: [{ Unlock: { flag: 'charsi_imbue' } }],
    },
    {
      id: 'a1q5', act: 1, name: 'Sisters to the Slaughter', gates_act: true,
      summary: 'Descendez les Catacombes et abattez Andariel pour venger les Sœurs des Yeux Aveugles.',
      steps: [
        { desc: 'Abattez Andariel, au fond des Catacombes.', trigger: { BossDead: { code: 'andariel' } }, count: 1 },
      ],
      rewards: [], // l'unlock "act1_done" est émis par le moteur (gates_act)
    },
  ];

  // ---------------------------------------------------------------------
  // Moteur — miroir exact de QuestLog (quests.rs).
  // ---------------------------------------------------------------------
  const STATE_VERSION = 1;
  const STATE_KEYS = ['not_started', 'in_progress', 'done', 'failed'];

  function normalizeEvent(ev) {
    if (!ev || typeof ev !== 'object') return null;
    const tagged = (kind, obj) => ({ kind, ...obj });
    const keys = Object.keys(ev);
    if (keys.length === 1 && ['MonsterKilled', 'ItemPickedUp', 'ZoneEntered', 'NpcTalked', 'BossDead'].includes(keys[0])) {
      const [kind, obj] = [keys[0], ev[keys[0]]] ;
      return tagged(kind, obj && typeof obj === 'object' ? obj : {});
    }
    if (typeof ev.type === 'string') { // raccourci {type, code/level_id/npc}
      const { type, ...rest } = ev;
      if (['MonsterKilled', 'ItemPickedUp', 'ZoneEntered', 'NpcTalked', 'BossDead'].includes(type)) {
        return tagged(type, rest);
      }
    }
    return null;
  }

  function triggerMatches(trigger, kind, args) {
    const kinds = Object.keys(trigger);
    if (kinds.length !== 1) return false;
    const want = trigger[kinds[0]];
    switch (kinds[0]) {
      case 'MonsterKilled':
        if (kind !== 'MonsterKilled') return false;
        return want.code === '*' || want.code === args.code;
      case 'BossDead': // un boss tué "comme un monstre" compte aussi (contrat)
        if (kind !== 'BossDead' && kind !== 'MonsterKilled') return false;
        return want.code === args.code;
      case 'ItemPickedUp':
        return kind === 'ItemPickedUp' && want.code === args.code;
      case 'ZoneEntered':
        return kind === 'ZoneEntered' && want.level_id === args.level_id;
      case 'NpcTalked':
        return kind === 'NpcTalked' && want.npc === args.npc;
      default:
        return false;
    }
  }

  class QuestLog {
    constructor(defs) {
      this.defs = defs || DEFS.map((d) => JSON.parse(JSON.stringify(d)));
      this.runs = this.defs.map(() => ({ state: 'not_started', step: 0, count: 0 }));
    }

    index(id) { return this.defs.findIndex((d) => d.id === id); }

    state(id) {
      const i = this.index(id);
      return i < 0 ? null : this.runs[i].state;
    }

    states() { return this.defs.map((d, i) => [d.id, this.runs[i].state]); }

    progress(id) {
      const i = this.index(id);
      if (i < 0) return null;
      const d = this.defs[i];
      const r = this.runs[i];
      if (r.state === 'in_progress') {
        const st = d.steps[r.step];
        return { state: r.state, step: r.step, desc: st.desc, count: r.count, required: st.count };
      }
      if (r.state === 'done') {
        const last = d.steps[d.steps.length - 1];
        return { state: r.state, step: d.steps.length - 1, desc: last.desc, count: last.count, required: last.count };
      }
      return { state: r.state, step: r.step, desc: '', count: 0, required: 0 };
    }

    actDone(act) {
      const gates = this.defs
        .map((d, i) => ({ d, r: this.runs[i] }))
        .filter(({ d }) => d.act === act && d.gates_act);
      return gates.length > 0 && gates.every(({ r }) => r.state === 'done');
    }

    fail(id) {
      const i = this.index(id);
      if (i < 0 || this.runs[i].state !== 'in_progress') return false;
      this.runs[i].state = 'failed';
      return true;
    }

    onEvent(event) {
      const norm = normalizeEvent(event);
      if (!norm) return [];
      const { kind } = norm;
      const args = { ...norm };
      delete args.kind;
      const rewards = [];
      const actsCompleted = [];
      for (let i = 0; i < this.defs.length; i++) {
        const d = this.defs[i];
        const r = this.runs[i];
        if (r.state !== 'not_started' && r.state !== 'in_progress') continue;
        const step = r.step;
        const st = d.steps[step];
        if (!st || !triggerMatches(st.trigger, kind, args)) continue;
        const required = Math.max(1, st.count);
        if (r.state === 'not_started') r.state = 'in_progress';
        r.count += 1;
        if (r.count < required) continue;
        if (step + 1 < d.steps.length) { r.step = step + 1; r.count = 0; continue; }
        r.step = d.steps.length - 1;
        r.count = Math.max(1, d.steps[d.steps.length - 1].count);
        r.state = 'done';
        rewards.push(...JSON.parse(JSON.stringify(d.rewards)));
        if (d.gates_act && !actsCompleted.includes(d.act) && this.actDone(d.act)) {
          actsCompleted.push(d.act);
        }
      }
      for (const act of actsCompleted) rewards.push({ Unlock: { flag: `act${act}_done` } });
      return rewards;
    }

    to_json() {
      return JSON.stringify({
        version: STATE_VERSION,
        quests: this.defs.map((d, i) => ({ id: d.id, ...this.runs[i] })),
      });
    }

    static from_json(json) {
      const s = JSON.parse(json); // throw si invalide (comme Err(()) côté Rust)
      if (s.version !== STATE_VERSION || !Array.isArray(s.quests)) {
        throw new Error('quests: version inconnue');
      }
      const log = new QuestLog();
      if (s.quests.length !== log.defs.length) throw new Error('quests: nombre de quêtes');
      s.quests.forEach((q, i) => {
        const d = log.defs[i];
        if (q.id !== d.id) throw new Error(`quests: id ${q.id} ≠ ${d.id}`);
        if (!STATE_KEYS.includes(q.state)) throw new Error(`quests: état ${q.state}`);
        const last = d.steps.length - 1;
        const req = Math.max(1, d.steps[q.step] ? d.steps[q.step].count : 1);
        const lastReq = Math.max(1, d.steps[last].count);
        const ok =
          (q.state === 'not_started' && q.step === 0 && q.count === 0) ||
          (q.state === 'in_progress' && q.step <= last && q.count < req) ||
          (q.state === 'done' && q.step === last && q.count === lastReq) ||
          (q.state === 'failed' && q.step <= last && q.count <= req);
        if (!ok) throw new Error(`quests: état incohérent pour ${q.id}`);
        log.runs[i] = { state: q.state, step: q.step, count: q.count };
      });
      return log;
    }
  }

  // ---------------------------------------------------------------------
  // Panneau journal (touche Q) — self-contenu, style de la page game.html.
  // ---------------------------------------------------------------------
  const CSS = `
#d2-quests { position: fixed; top: 14px; right: 14px; width: 310px; max-height: 78vh;
  overflow-y: auto; background: #14110aee; border: 1px solid #5a4a2a; border-radius: 4px;
  padding: 10px 12px; z-index: 60; color: #cfc7b8; font: 12px/1.45 system-ui, sans-serif;
  box-shadow: 0 4px 24px #000c; }
#d2-quests h2 { margin: 0 0 2px; font-size: 13px; font-weight: 700; letter-spacing: .1em;
  color: #c7a45a; text-transform: uppercase; }
#d2-quests .d2q-act { color: #8f8676; font-size: 11px; margin-bottom: 8px; letter-spacing: .06em; }
#d2-quests ul { list-style: none; margin: 0; padding: 0; }
#d2-quests li { padding: 7px 0; border-top: 1px solid #2e2718; }
#d2-quests li:first-child { border-top: 0; }
#d2-quests .d2q-name { font-weight: 600; font-size: 12px; }
#d2-quests .d2q-status { float: right; font-size: 10px; font-weight: 700; letter-spacing: .08em;
  text-transform: uppercase; margin-left: 6px; }
#d2-quests .d2q-obj { color: #a89670; font-size: 11px; margin-top: 2px; }
#d2-quests .d2q-prog { color: #c7a45a; font-weight: 600; }
#d2-quests .d2q-sum { color: #8f8676; font-size: 11px; margin-top: 1px; font-style: italic; }
#d2-quests .not_started .d2q-name { color: #6e6555; }
#d2-quests .not_started .d2q-status { color: #6e6555; }
#d2-quests .in_progress .d2q-name { color: #e8dcbf; }
#d2-quests .in_progress .d2q-status { color: #d9b96a; }
#d2-quests .done .d2q-name { color: #c7a45a; }
#d2-quests .done .d2q-status { color: #7ea05f; }
#d2-quests .failed .d2q-name { color: #b06a55; text-decoration: line-through; }
#d2-quests .failed .d2q-status { color: #d06b4f; }
#d2-quests .d2q-hint { margin-top: 8px; color: #6e6555; font-size: 10px; text-align: center;
  letter-spacing: .06em; border-top: 1px solid #2e2718; padding-top: 6px; }
#d2-quests[hidden] { display: none; }
`;

  const STATUS_FR = {
    not_started: 'Non commencée',
    in_progress: 'En cours',
    done: 'Terminée',
    failed: 'Échouée',
  };

  let doc = null;
  let panel = null;
  let listEl = null;
  let log = new QuestLog();
  let isOpen = false;

  function render() {
    if (!listEl) return;
    listEl.textContent = '';
    for (const d of log.defs) {
      const p = log.progress(d.id);
      const li = doc.createElement('li');
      li.className = p.state;

      const head = doc.createElement('div');
      const status = doc.createElement('span');
      status.className = 'd2q-status';
      status.textContent = p.state === 'done' ? `✔ ${STATUS_FR[p.state]}` : STATUS_FR[p.state];
      const name = doc.createElement('span');
      name.className = 'd2q-name';
      const num = parseInt(d.id.slice(3), 10) || 0;
      name.textContent = `${num}. ${d.name}`;
      head.append(status, name);
      li.appendChild(head);

      if (p.state === 'in_progress') {
        const obj = doc.createElement('div');
        obj.className = 'd2q-obj';
        obj.append(doc.createTextNode(p.desc));
        if (p.required > 1) {
          const prog = doc.createElement('span');
          prog.className = 'd2q-prog';
          prog.textContent = ` (${p.count}/${p.required})`;
          obj.appendChild(prog);
        }
        li.appendChild(obj);
      } else if (p.state === 'done') {
        const sum = doc.createElement('div');
        sum.className = 'd2q-sum';
        sum.textContent = d.rewards.length
          ? 'Récompense obtenue.'
          : 'Acte débloqué.';
        li.appendChild(sum);
      } else if (p.state === 'not_started') {
        const sum = doc.createElement('div');
        sum.className = 'd2q-sum';
        sum.textContent = d.summary;
        li.appendChild(sum);
      }
      listEl.appendChild(li);
    }
  }

  function setOpen(open) {
    isOpen = open;
    if (panel) panel.hidden = !open;
    if (open) render();
  }

  // ---------------------------------------------------------------------
  // API publique : window.D2Quests
  // ---------------------------------------------------------------------
  const api = {
    QuestLog,
    DEFS,
    /** Construit le panneau + le raccourci Q. `dom` = Document (défaut). */
    init(dom) {
      doc = dom || (typeof document !== 'undefined' ? document : null);
      if (!doc || panel) return api;
      const style = doc.createElement('style');
      style.textContent = CSS;
      doc.head.appendChild(style);

      panel = doc.createElement('div');
      panel.id = 'd2-quests';
      panel.hidden = true;
      const title = doc.createElement('h2');
      title.textContent = 'Journal de quêtes';
      const act = doc.createElement('div');
      act.className = 'd2q-act';
      act.textContent = 'Acte I — Camp des Rousses';
      listEl = doc.createElement('ul');
      const hint = doc.createElement('div');
      hint.className = 'd2q-hint';
      hint.textContent = 'Q — ouvrir / fermer';
      panel.append(title, act, listEl, hint);
      doc.body.appendChild(panel);

      doc.addEventListener('keydown', (e) => {
        if (e.repeat) return;
        const t = e.target;
        if (t && (t.tagName === 'INPUT' || t.tagName === 'TEXTAREA' || t.isContentEditable)) return;
        if (e.key === 'q' || e.key === 'Q') {
          setOpen(!isOpen);
          e.preventDefault();
        }
      });
      return api;
    },
    /** Pousse un événement (JSON QuestEvent du moteur ou raccourci). */
    feed(event) {
      const rewards = log.onEvent(event);
      if (isOpen) render();
      return rewards;
    },
    get log() { return log; },
    reset() { log = new QuestLog(); if (isOpen) render(); },
    exportLog() { return log.to_json(); },
    /** Recharge un état exporté ; throw si corrompu (contrat from_json). */
    importLog(json) { log = QuestLog.from_json(json); if (isOpen) render(); return true; },
    show() { setOpen(true); },
    hide() { setOpen(false); },
    toggle() { setOpen(!isOpen); },
    get isOpen() { return isOpen; },
  };

  const root = typeof window !== 'undefined' ? window : globalThis;
  root.D2Quests = api;
})();
