// src/legacy.ts
async function warmLegacy(name) {
  if (typeof window === "undefined")
    return;
  const url = new URL(name, import.meta.url).href;
  try {
    await import(url);
  } catch (e) {
    console.info(`[ts-25a] legacy ${name} non chargé (${e instanceof Error ? e.message : String(e)}) — voie TS active.`);
  }
}

// src/types.ts
function win() {
  return typeof window === "undefined" ? null : window;
}
function parseOr(text, fallback) {
  try {
    const v = JSON.parse(text ?? "");
    return v ?? fallback;
  } catch {
    return fallback;
  }
}
var TS25A_REGISTRY = {
  installed: [],
  adopted: []
};
function registerModule(name, mode) {
  (mode === "ts" ? TS25A_REGISTRY.installed : TS25A_REGISTRY.adopted).push(name);
}

// src/slots.ts
var SLOTS_KEY = "d2wasm.slots.v1";
var LEGACY_SAVE_KEY = "d2wasm.save.v1";
var MAX_SLOTS = 5;
var HERO_LABELS = {
  BA: "Barbare",
  SOR: "Sorcière",
  SO: "Sorcière",
  PA: "Paladin",
  AM: "Amazone",
  AS: "Assassin",
  DZ: "Druide",
  NE: "Nécromancien"
};
var ZONES = {
  town: "Camp des Rousses",
  bloodmoor: "Blood Moor"
};
var storage = null;
function store() {
  if (storage)
    return storage;
  try {
    const ls = globalThis.localStorage;
    if (ls)
      return ls;
  } catch {}
  const mem = new Map;
  return {
    getItem: (k) => mem.has(k) ? mem.get(k) : null,
    setItem: (k, v) => void mem.set(k, String(v))
  };
}
function readSlots() {
  try {
    const r = JSON.parse(store().getItem(SLOTS_KEY) || "null");
    if (r && typeof r === "object" && Array.isArray(r.slots)) {
      return r.slots.filter((s) => s && s.id && typeof s.save === "string");
    }
  } catch {}
  return [];
}
function writeSlots(slots) {
  store().setItem(SLOTS_KEY, JSON.stringify({ slots }));
  const newest = slots.slice().sort((a, b) => b.ts - a.ts)[0];
  if (newest)
    store().setItem(LEGACY_SAVE_KEY, newest.save);
}
function newId() {
  return "s" + Date.now().toString(36) + Math.random().toString(36).slice(2, 7);
}
function metaOf(saveJson) {
  try {
    const v = typeof saveJson === "string" ? JSON.parse(saveJson) : saveJson;
    if (!v || typeof v !== "object")
      return null;
    const o = v;
    if (!o.hero || !o.version)
      return null;
    const heroCode = String(o.hero_code || "BA").toUpperCase();
    const level = o.hero && o.hero.stats && o.hero.stats.level || 1;
    const playtime = o.now_ms || 0;
    const levelKey = typeof o.level === "string" && o.level ? o.level.toLowerCase() : "town";
    return {
      hero_code: heroCode,
      hero_label: HERO_LABELS[heroCode] ?? heroCode,
      level,
      playtime_ms: playtime,
      level_key: levelKey,
      zone: ZONES[levelKey] ?? levelKey,
      version: o.version
    };
  } catch {
    return null;
  }
}
function defaultName(meta) {
  return meta.hero_label + " niv. " + meta.level;
}
function listSlots() {
  let slots = readSlots();
  if (!slots.length) {
    const legacy = store().getItem(LEGACY_SAVE_KEY);
    if (legacy) {
      const meta = metaOf(legacy);
      if (meta) {
        slots = [Object.assign({ id: newId(), name: defaultName(meta), ts: Date.now() }, meta, { save: legacy })];
        writeSlots(slots);
      }
    }
  }
  return slots.slice().sort((a, b) => b.ts - a.ts);
}
function upsertFromSave(saveJson, opts) {
  const meta = metaOf(saveJson);
  if (!meta)
    throw new Error("sauvegarde illisible (JSON d2rules attendu)");
  const slots = readSlots();
  const now = Date.now();
  const existing = slots.find((s) => s.hero_code === meta.hero_code);
  if (existing) {
    existing.ts = now;
    existing.save = saveJson;
    Object.assign(existing, meta);
    writeSlots(slots);
    return existing;
  }
  if (slots.length >= MAX_SLOTS) {
    throw new Error("Slots pleins (" + MAX_SLOTS + ") — supprimez un héros depuis le menu « Parties ».");
  }
  const slot = Object.assign({ id: newId(), name: opts && opts.name || defaultName(meta), ts: now }, meta, { save: saveJson });
  slots.push(slot);
  writeSlots(slots);
  return slot;
}
function getSlot(id) {
  return readSlots().find((s) => s.id === id) ?? null;
}
function removeSlot(id) {
  const slots = readSlots().filter((s) => s.id !== id);
  writeSlots(slots);
  return slots;
}
function renameSlot(id, name) {
  const slots = readSlots();
  const slot = slots.find((s) => s.id === id);
  const clean = String(name || "").trim().slice(0, 24);
  if (slot && clean) {
    slot.name = clean;
    writeSlots(slots);
  }
  return slot;
}
function mostRecentSlot() {
  return listSlots()[0] ?? null;
}
function downloadFilename(slot) {
  const slug = slot.name.replace(/[^a-z0-9]+/gi, "-").toLowerCase();
  const day = new Date(slot.ts).toISOString().slice(0, 10);
  return `d2-${slug}-${day}.json`;
}
function exportSlot(id, doc = document) {
  const slot = getSlot(id);
  if (!slot)
    throw new Error("slot inconnu");
  const blob = new Blob([JSON.stringify(slot, null, 1)], { type: "application/json" });
  const a = doc.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = downloadFilename(slot);
  doc.body.appendChild(a);
  a.click();
  setTimeout(() => {
    URL.revokeObjectURL(a.href);
    a.remove();
  }, 400);
  return slot;
}
function importSlot(text) {
  const v = JSON.parse(text);
  if (v && typeof v === "object") {
    const o = v;
    if (typeof o.save === "string" && o.hero_code) {
      const meta = metaOf(o.save);
      if (!meta)
        throw new Error("fichier : sauvegarde interne illisible");
      const slots = readSlots();
      const heroCode = String(o.hero_code).toUpperCase();
      const existingIdx = slots.findIndex((s) => s.hero_code === heroCode);
      const slot = Object.assign({
        id: existingIdx >= 0 ? slots[existingIdx].id : newId(),
        name: typeof o.name === "string" && o.name ? o.name : defaultName(meta),
        ts: typeof o.ts === "number" ? o.ts : Date.now()
      }, meta, { save: o.save });
      if (existingIdx >= 0)
        slots.splice(existingIdx, 1, slot);
      else {
        if (slots.length >= MAX_SLOTS)
          throw new Error("Slots pleins (" + MAX_SLOTS + ") — supprimez un héros.");
        slots.push(slot);
      }
      writeSlots(slots);
      return slot;
    }
  }
  return upsertFromSave(text);
}
function installSlots(win) {
  registerModule("slots", "ts");
  const api = {
    MAX: MAX_SLOTS,
    list: listSlots,
    get: getSlot,
    remove: removeSlot,
    rename: renameSlot,
    upsertFromSave,
    mostRecent: mostRecentSlot,
    metaOf,
    exportSlot,
    importSlot
  };
  win.D2Slots = api;
  return api;
}
if (typeof window !== "undefined")
  installSlots(window);

// src/hud.ts
var D2HUD = {
  ready: false,
  game: null,
  canvas: null,
  skills: [],
  hands: { L: -1, R: -1 },
  pendingHand: "R",
  altDown: false,
  mouse: { x: -1, y: -1, inside: false },
  els: {},
  labelPool: new Map,
  init(_manifest, game) {
    init(game);
  },
  update(stats) {
    updateStamina(stats);
  },
  events(events) {
    handleEvents(events);
  },
  frame(canvas, game) {
    frame(canvas, game);
  }
};
function el(tag, id, parent) {
  const n = document.createElement(tag);
  if (id)
    n.id = id;
  (parent ?? D2HUD.els.root).appendChild(n);
  return n;
}
function shortSkillName(name) {
  if (!name || name === "Attaque")
    return "⚔";
  return name.split(/\s+/).map((w) => w[0]).join("").toUpperCase().slice(0, 3);
}
function skillOf(hand) {
  const idx = D2HUD.hands[hand];
  if (idx < 0)
    return { index: -1, code: "Attaque", name: "Attaque", level: 0, missile: null };
  return D2HUD.skills[idx] ?? { index: -1, code: "Attaque", name: "Attaque", level: 0, missile: null };
}
function refreshSkillButtons() {
  for (const hand of ["L", "R"]) {
    const btn = D2HUD.els["skbtn" + hand];
    if (!btn)
      continue;
    const s = skillOf(hand);
    btn.textContent = shortSkillName(s.name);
    btn.title = `${hand === "L" ? "Gauche" : "Droite"} : ${s.name}` + (s.level ? ` (niv. ${s.level})` : "") + (s.missile ? " · projectile" : "") + " — cliquer pour choisir";
    btn.classList.toggle("selected", s.missile != null);
  }
}
function init(game) {
  try {
    D2HUD.game = game ?? win()?.__d2?.game ?? null;
    D2HUD.canvas = document.getElementById("game");
    if (!D2HUD.canvas || !D2HUD.game)
      return;
    const stage = D2HUD.canvas.parentElement;
    const root = document.createElement("div");
    root.id = "hudx";
    stage?.appendChild(root);
    D2HUD.els.root = root;
    const stw = el("div", "staminawrap");
    stw.title = "Endurance";
    D2HUD.els.staminaFill = el("div", "staminafill", stw);
    D2HUD.els.staminaWrap = stw;
    const mh = el("div", "monhover");
    D2HUD.els.monName = el("div", "monname", mh);
    const hpw = el("div", "monhpwrap", mh);
    D2HUD.els.monHpFill = el("div", "monhpfill", hpw);
    D2HUD.els.monHover = mh;
    D2HUD.els.labels = el("div", "itemlabels");
    D2HUD.els.floats = el("div", "dmgfloats");
    for (const hand of ["L", "R"]) {
      const b = el("button", "skbtn-" + hand);
      b.className = "skbtn";
      b.type = "button";
      b.addEventListener("click", (e) => {
        e.stopPropagation();
        openSkillPopup(hand);
      });
      D2HUD.els["skbtn" + hand] = b;
    }
    D2HUD.els.skPopup = el("div", "skpopup");
    const mp = el("div", "minipanel");
    const defs = [
      ["Menu", "Menu jeu / pause — à venir (EXP-E4)", null],
      ["Inv", "Inventaire (I)", () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "i" }))],
      ["Stats", "Panneau de personnage (C)", () => window.dispatchEvent(new KeyboardEvent("keydown", { key: "c" }))],
      ["Skills", "Arbre de compétences — à venir (EXP-E8)", () => openSkillPopup("R")],
      ["Quêtes", "Journal de quêtes — à venir (EXP-E7)", null],
      ["Carte", "Automap (Tab)", () => {
        const w = window;
        if (w.D2Automap)
          w.D2Automap.toggle();
      }],
      ["Aide", "Aide (H) — à venir (EXP-E5)", null],
      ["Son", "Activer/couper le son", () => {
        const b = document.getElementById("btn-sound");
        if (b)
          b.click();
      }]
    ];
    const mpButtons = {};
    for (const [label, title, fn] of defs) {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = label;
      b.title = title;
      if (!fn)
        b.classList.add("soon");
      else
        b.addEventListener("click", (e) => {
          e.stopPropagation();
          fn();
        });
      mp.appendChild(b);
      mpButtons[label] = b;
    }
    D2HUD.els.mpButtons = mpButtons;
    wireInput();
    loadSkills();
    refreshSkillButtons();
    D2HUD.ready = true;
  } catch (err) {
    console.info("D2HUD init ignoré :", err instanceof Error ? err.message : String(err));
  }
}
function loadSkills() {
  try {
    D2HUD.skills = parseOr(D2HUD.game ? D2HUD.game.heroSkills() : "[]", []);
  } catch {
    D2HUD.skills = [];
  }
  D2HUD.hands.L = -1;
  const s = D2HUD.skills.find((k) => k.missile) ?? D2HUD.skills[0];
  D2HUD.hands.R = s ? s.index : -1;
}
function openSkillPopup(hand) {
  D2HUD.pendingHand = hand;
  const pop = D2HUD.els.skPopup;
  if (!pop)
    return;
  pop.textContent = "";
  const h = document.createElement("h3");
  h.textContent = `Main ${hand === "L" ? "gauche" : "droite"} — choisir`;
  pop.appendChild(h);
  const rows = [
    { index: -1, code: "Attaque", name: "Attaque", level: 0, missile: null },
    ...D2HUD.skills
  ];
  for (const s of rows) {
    const row = document.createElement("div");
    row.className = "skrow";
    const nm = document.createElement("span");
    nm.textContent = s.name;
    const meta = document.createElement("span");
    meta.className = s.missile ? "missile" : "lvl";
    meta.textContent = s.missile ? "projectile" : s.level ? `niv. ${s.level}` : "mêlée";
    row.appendChild(nm);
    row.appendChild(meta);
    row.addEventListener("click", () => assignSkill(hand, s.index));
    pop.appendChild(row);
  }
  pop.classList.add("open");
}
function assignSkill(hand, index) {
  D2HUD.hands[hand] = index;
  if (index >= 0) {
    try {
      if (D2HUD.game)
        D2HUD.game.selectSkill(index);
    } catch {}
  }
  D2HUD.els.skPopup?.classList.remove("open");
  refreshSkillButtons();
}
function wireInput() {
  const cv = D2HUD.canvas;
  cv.addEventListener("mousemove", (e) => {
    const r = cv.getBoundingClientRect();
    D2HUD.mouse.x = (e.clientX - r.left) * (cv.width / r.width);
    D2HUD.mouse.y = (e.clientY - r.top) * (cv.height / r.height);
    D2HUD.mouse.inside = true;
  });
  cv.addEventListener("mouseleave", () => {
    D2HUD.mouse.inside = false;
    if (D2HUD.els.monHover)
      D2HUD.els.monHover.style.display = "none";
  });
  window.addEventListener("keydown", (e) => {
    if (e.key === "Alt") {
      D2HUD.altDown = true;
      e.preventDefault();
    }
    if (e.key === "Escape" && D2HUD.els.skPopup?.classList.contains("open")) {
      D2HUD.els.skPopup.classList.remove("open");
    }
  });
  window.addEventListener("keyup", (e) => {
    if (e.key === "Alt")
      D2HUD.altDown = false;
  });
  window.addEventListener("blur", () => {
    D2HUD.altDown = false;
  });
  cv.addEventListener("contextmenu", (e) => {
    e.preventDefault();
    castWithHand("R", e);
  });
  window.addEventListener("click", (e) => {
    if (e.target !== cv || e.button !== 0)
      return;
    const s = skillOf("L");
    if (s.missile) {
      e.stopPropagation();
      castWithHand("L", e);
    }
  }, true);
}
function castWithHand(hand, e) {
  if (!D2HUD.ready || !D2HUD.game || !D2HUD.canvas)
    return;
  const r = D2HUD.canvas.getBoundingClientRect();
  const px = (e.clientX - r.left) * (D2HUD.canvas.width / r.width);
  const py = (e.clientY - r.top) * (D2HUD.canvas.height / r.height);
  const s = skillOf(hand);
  let wx, wy;
  try {
    const world = D2HUD.game.screenToWorld(px, py);
    wx = world[0];
    wy = world[1];
  } catch {
    return;
  }
  const tx = Math.round(wx), ty = Math.round(wy);
  const queue = win()?.__d2?.queue;
  if (!queue)
    return;
  if (s.missile) {
    try {
      if (D2HUD.game)
        D2HUD.game.selectSkill(s.index);
    } catch {}
    refreshSkillButtons();
    queue.push({ type: "rangedAttack", x: tx, y: ty });
  } else {
    let near = -1;
    try {
      if (D2HUD.game)
        near = D2HUD.game.monsterNear(wx, wy, 2.5);
    } catch {}
    queue.push(near >= 0 ? { type: "attack", x: tx, y: ty } : { type: "move", x: tx, y: ty });
  }
}
function updateStamina(stats) {
  if (!D2HUD.ready || !stats || stats.max_stamina === undefined)
    return;
  const wrap = D2HUD.els.staminaWrap;
  const fill = D2HUD.els.staminaFill;
  if (!wrap || !fill)
    return;
  const max = Math.max(1, stats.max_stamina);
  const f = Math.max(0, Math.min(1, (stats.stamina || 0) / max));
  fill.style.width = `${(f * 100).toFixed(1)}%`;
  wrap.classList.toggle("low", f < 0.5 && f >= 0.2);
  wrap.classList.toggle("empty", f < 0.2);
  wrap.title = `Endurance : ${stats.stamina}/${stats.max_stamina}`;
}
function handleEvents(events) {
  if (!D2HUD.ready || !Array.isArray(events))
    return;
  for (const ev of events) {
    if (ev.LevelSwitch) {
      const floats = D2HUD.els.floats;
      if (floats)
        floats.textContent = "";
      clearLabels();
      hideMonHover();
      continue;
    }
    if (ev.HeroAttacked) {
      spawnFloatAtMonster(ev.HeroAttacked.target, `−${ev.HeroAttacked.dmg}`, "#ffb347");
    } else if (ev.MissileHit) {
      spawnFloatAtMonster(ev.MissileHit.target, `−${ev.MissileHit.dmg}`, "#ffb347");
    } else if (ev.HeroMissed) {
      spawnFloatAtMonster(ev.HeroMissed.target, "raté", "#b8ad98");
    } else if (ev.MissileMissed) {
      spawnFloatAtMonster(ev.MissileMissed.target, "raté", "#b8ad98");
    } else if (ev.HeroHit) {
      spawnFloatAtHero(`−${ev.HeroHit.dmg}`, "#ff5545");
    }
  }
}
function monsterScreen(index) {
  try {
    const info = parseOr(D2HUD.game ? D2HUD.game.monsterInfo(index) : "null", null);
    return info ? [info.sx, info.sy] : null;
  } catch {
    return null;
  }
}
function spawnFloatAtMonster(index, text, color) {
  const pos = monsterScreen(index);
  if (pos)
    spawnFloat(pos[0], pos[1] - 34, text, color);
}
function spawnFloatAtHero(text, color) {
  try {
    const screen = D2HUD.game ? D2HUD.game.heroScreen() : null;
    if (!screen)
      return;
    spawnFloat(screen[0], screen[1] - 44, text, color);
  } catch {}
}
function spawnFloat(x, y, text, color) {
  const floats = D2HUD.els.floats;
  if (!floats)
    return;
  const n = document.createElement("span");
  n.className = "dmgfloat";
  n.textContent = text;
  n.style.left = `${Math.round(x + (Math.random() * 16 - 8))}px`;
  n.style.top = `${Math.round(y)}px`;
  n.style.color = color;
  floats.appendChild(n);
  setTimeout(() => n.remove(), 950);
}
function frame(_canvas, game) {
  if (!D2HUD.ready)
    return;
  try {
    frameMonsterHover(game);
  } catch {}
  try {
    frameItemLabels(game);
  } catch {}
  try {
    refreshSonLabel();
  } catch {}
}
function hideMonHover() {
  if (D2HUD.els.monHover)
    D2HUD.els.monHover.style.display = "none";
}
var MON_RANKS = {
  champion: "Champion",
  unique: "Unique",
  super_unique: "Super-unique",
  boss: "Boss"
};
function setMonRank(kind) {
  const name = D2HUD.els.monName;
  if (!name)
    return;
  const old = name.querySelector(".monrank");
  if (old)
    old.remove();
  const label = MON_RANKS[kind];
  if (!label)
    return;
  const tag = document.createElement("span");
  tag.className = "monrank";
  tag.textContent = label;
  name.appendChild(tag);
}
function frameMonsterHover(game) {
  if (!D2HUD.mouse.inside) {
    hideMonHover();
    return;
  }
  const world = game.screenToWorld(D2HUD.mouse.x, D2HUD.mouse.y);
  const idx = game.monsterNear(world[0], world[1], 1.4);
  if (idx < 0) {
    hideMonHover();
    return;
  }
  const info = parseOr(game.monsterInfo(idx) || "null", null);
  if (!info || !info.alive) {
    hideMonHover();
    return;
  }
  const mh = D2HUD.els.monHover;
  if (!mh)
    return;
  mh.className = "k-" + info.kind;
  mh.id = "monhover";
  const monName = D2HUD.els.monName;
  if (monName)
    monName.textContent = info.name;
  setMonRank(info.kind);
  const frac = info.maxHp > 0 ? Math.max(0, Math.min(1, info.hp / info.maxHp)) : 0;
  const fill = D2HUD.els.monHpFill;
  if (fill)
    fill.style.width = `${(frac * 100).toFixed(0)}%`;
  mh.style.left = `${Math.round(info.sx)}px`;
  mh.style.top = `${Math.round(info.sy - 30)}px`;
  mh.style.display = "block";
}
function clearLabels() {
  const labels = D2HUD.els.labels;
  if (labels)
    labels.textContent = "";
  D2HUD.labelPool.clear();
}
function frameItemLabels(game) {
  if (!D2HUD.altDown) {
    if (D2HUD.labelPool.size)
      clearLabels();
    return;
  }
  const labels = D2HUD.els.labels;
  if (!labels)
    return;
  const items = parseOr(game.groundItems() || "[]", []);
  const seen = new Set;
  for (const it of items) {
    const key = `${it.index}:${it.code}:${it.amount}`;
    seen.add(key);
    let div = D2HUD.labelPool.get(key);
    if (!div) {
      div = document.createElement("div");
      div.className = "ilabel k-" + it.kind;
      div.textContent = it.kind === "Gold" ? `${it.name} (${it.amount})` : it.name;
      div.title = "Cliquer pour ramasser (E au contact)";
      div.addEventListener("click", (e) => {
        e.stopPropagation();
        const q = win()?.__d2?.queue;
        if (q)
          q.push({ type: "pickup" });
      });
      labels.appendChild(div);
      D2HUD.labelPool.set(key, div);
    }
    div.style.left = `${Math.round(it.sx)}px`;
    div.style.top = `${Math.round(it.sy - 14)}px`;
    div.style.display = "block";
  }
  for (const [key, div] of D2HUD.labelPool) {
    if (!seen.has(key)) {
      div.remove();
      D2HUD.labelPool.delete(key);
    }
  }
}
function refreshSonLabel() {
  const b = document.getElementById("btn-sound");
  const son = D2HUD.els.mpButtons?.Son;
  if (b && son) {
    const on = /oui/i.test(b.textContent ?? "");
    son.textContent = on ? "Son" : "Muet";
  }
}
function installHUD(win) {
  registerModule("hud", "ts");
  win.D2HUD = D2HUD;
}
if (typeof window !== "undefined")
  installHUD(window);

// src/waypoints.ts
var api = {
  ready: false,
  open: false,
  opts: null,
  state: null,
  panel: null,
  init(o) {
    return initWaypoints(o);
  },
  refresh(jsonText) {
    refresh(jsonText);
  },
  nameOf(slot) {
    return nameOf(slot);
  },
  toggle() {
    setOpenState(!api.open);
  },
  setOpen(open) {
    setOpenState(open);
  },
  e2eTour() {
    return e2eTour();
  }
};
function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" })[c] ?? c);
}
function render() {
  if (!api.panel || !api.state)
    return;
  const list = (api.state.list || []).slice().sort((a, b) => a.act - b.act || a.slot - b.slot);
  const found = list.filter((w) => w.activated);
  const cur = api.state.current || {};
  const rows = found.map((w) => {
    const tags = [
      w.here ? '<span class="wp-here">● ici</span>' : "",
      w.mvp ? '<span class="wp-mvp" title="Extension MVP : la table retail (patch_levels.txt) ne mappe pas ce niveau à un slot (255) — cf. impl-waypoints.md">MVP</span>' : ""
    ].join(" ");
    const travel = !!(w.travel && !w.here);
    return `<button class="wp-row${travel ? "" : " wp-cur"}" data-slot="${w.slot}"` + `${travel ? "" : " disabled"}>` + `<span class="wp-name">${esc(w.name)}</span>${tags}</button>`;
  });
  const todo = list.filter((w) => !w.activated);
  const todoHtml = todo.length ? `<div class="wp-locked">${todo.map((w) => `<div class="wp-locked-row">${esc(w.name)}</div>`).join("")}</div>` : "";
  const listEl = api.panel.querySelector("#wp-list");
  const zoneEl = api.panel.querySelector("#wp-zone");
  if (listEl) {
    listEl.innerHTML = (rows.length ? rows.join("") : '<div class="wp-empty">Aucun waypoint découvert — marchez jusqu’à une pierre.</div>') + (todoHtml ? `<div class="wp-locked-title">${todo.length} non découvert${todo.length > 1 ? "s" : ""}</div>${todoHtml}` : "");
  }
  if (zoneEl)
    zoneEl.textContent = cur.name ? `Zone : ${cur.name}` : "";
}
async function travel(slot) {
  const o = api.opts;
  const g = o?.getGame?.() ?? null;
  if (!g || !o)
    return;
  try {
    const out = parseOr(g.travelWaypoint(slot), { slot, key: "" });
    o.note?.(`Voyage : ${out.name ?? nameOf(slot)}.`);
    if (out.name !== undefined)
      o.onTraveled?.({ ...out, name: out.name });
    sfxUi(0.9);
  } catch (e) {
    o.note?.(`Voyage impossible : ${e instanceof Error ? e.message : String(e)}`);
  }
}
function sfxUi(rate) {
  try {
    const m = api.opts?.manifest;
    if (m && m.sfx && api.opts?.sfx && m.sfx.uiClick)
      api.opts.sfx(m.sfx.uiClick, 0.5, rate);
  } catch {}
}
function initWaypoints(o) {
  if (api.ready)
    return api;
  api.opts = o;
  const doc = document;
  const panel = doc.createElement("div");
  panel.id = "d2-waypoints";
  panel.hidden = true;
  panel.innerHTML = `<div class="wp-title">WAYPOINTS</div>` + `<div class="wp-zone" id="wp-zone"></div>` + `<div id="wp-list"></div>` + `<div class="wp-hint">Tab / bouton : ouvrir · clic = voyager (niveau régénéré seedé)</div>`;
  doc.body.appendChild(panel);
  api.panel = panel;
  panel.addEventListener("click", (e) => {
    const btn = e.target.closest("button.wp-row");
    if (btn && !btn.disabled)
      travel(Number(btn.dataset.slot));
  });
  doc.addEventListener("keydown", (e) => {
    if (e.key === "Tab") {
      e.preventDefault();
      setOpenState(!api.open);
    }
  });
  const btn = doc.getElementById("btn-waypoints");
  if (btn)
    btn.addEventListener("click", () => setOpenState(!api.open));
  api.ready = true;
  const g = o.getGame && o.getGame();
  if (g && g.waypoints)
    refresh(g.waypoints());
  return api;
}
function setOpenState(open) {
  api.open = !!open;
  if (api.panel)
    api.panel.hidden = !api.open;
  sfxUi(api.open ? 1.3 : 1);
}
function refresh(jsonText) {
  try {
    api.state = parseOr(jsonText, { list: [] });
    render();
  } catch {}
}
function nameOf(slot) {
  const w = api.state && (api.state.list || []).find((x) => x.slot === slot);
  return w ? w.name : `slot ${slot}`;
}
async function e2eTour() {
  const g = api.opts?.getGame?.() ?? null;
  const q = api.opts?.queue;
  if (!g || !q || !api.opts)
    throw new Error("E2E: moteur ou queue absent");
  const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
  const activated = (slot) => {
    refresh(g.waypoints());
    const w = (api.state?.list || []).find((x) => x.slot === slot);
    return !!(w && w.activated);
  };
  const walkUntil = async (slot, deadlineMs) => {
    const t0 = Date.now();
    while (Date.now() - t0 < deadlineMs) {
      if (activated(slot))
        return true;
      const entry = api.state?.list.find((x) => x.slot === slot);
      if (entry?.pos) {
        q.push({ type: "move", x: Math.round(entry.pos[0] ?? 0), y: Math.round(entry.pos[1] ?? 0) });
      }
      await sleep(400);
    }
    return false;
  };
  if (!await walkUntil(0, 25000))
    throw new Error("E2E: waypoint camp non activé");
  api.opts.flash?.(`Waypoint découvert : ${nameOf(0)}`);
  g.switchLevel("bloodmoor", 137, 162);
  await sleep(300);
  if (!await walkUntil(39, 25000))
    throw new Error("E2E: waypoint bloodmoor non activé");
  api.opts.flash?.(`Waypoint découvert : ${nameOf(39)}`);
  setOpenState(true);
  await sleep(400);
  await travel(0);
  await sleep(600);
  setOpenState(true);
  refresh(g.waypoints());
  return api.state ?? { list: [] };
}
function installWaypoints(win) {
  registerModule("waypoints", "ts");
  win.D2Waypoints = api;
}
if (typeof window !== "undefined")
  installWaypoints(window);

// src/corpse-ui.ts
var overlay = null;
var hint = null;
var hintUntil = 0;
var dead = false;
var CORPSE_CSS = `
  #corpse-death {
    position: fixed; inset: 0; display: none; z-index: 90;
    background: rgba(8, 4, 2, 0.78);
    align-items: center; justify-content: center;
    font-family: Georgia, 'Times New Roman', serif; color: #d8c9a3;
  }
  #corpse-death.show { display: flex; }
  #corpse-death .panel {
    text-align: center; padding: 28px 44px;
    background: rgba(20, 14, 8, 0.94);
    border: 2px solid #5a4526; border-radius: 6px;
    box-shadow: 0 0 40px rgba(120, 20, 10, 0.5);
  }
  #corpse-death h2 { margin: 0 0 10px; font-size: 30px; color: #c03424;
    letter-spacing: 2px; text-shadow: 0 0 12px rgba(180, 30, 20, 0.7); }
  #corpse-death p { margin: 6px 0; font-size: 14px; color: #b7a681; }
  #corpse-death button {
    margin-top: 16px; padding: 10px 26px; font-size: 15px; cursor: pointer;
    font-family: inherit; color: #e8d9b0;
    background: linear-gradient(#3a2c14, #241a0a);
    border: 1px solid #7a5c2e; border-radius: 4px;
  }
  #corpse-death button:hover { background: linear-gradient(#4d3a1a, #2e2110); }
  #corpse-hint {
    position: absolute; display: none; z-index: 40; pointer-events: none;
    font: 11px/1.4 monospace; color: #ffd9a0; text-align: center;
    background: rgba(20, 12, 6, 0.85); border: 1px solid #5a4526;
    border-radius: 3px; padding: 3px 8px; white-space: nowrap;
    text-shadow: 0 1px 2px #000;
  }
`;
function deathText(info) {
  const gold = info && info.gold || 0;
  const where = info ? ` (${info.x}, ${info.y})` : "";
  return gold > 0 ? `Votre corps garde votre équipement${where} ; ${gold} or sont tombés à côté.` : `Votre corps garde votre équipement${where}.`;
}
function corpseHint(info) {
  const items = info.items ?? 0;
  return `☠ Votre corps${items ? ` — ${items} objet${items > 1 ? "s" : ""}` : " (vide)"} — cliquez dessus pour récupérer`;
}
function ensureDom() {
  if (overlay)
    return;
  const style = document.createElement("style");
  style.textContent = CORPSE_CSS;
  document.head.appendChild(style);
  overlay = document.createElement("div");
  overlay.id = "corpse-death";
  overlay.innerHTML = `
    <div class="panel">
      <h2>Vous êtes mort…</h2>
      <p id="corpse-death-text">Votre corps reste là où vous êtes tombé.</p>
      <p>Réapparition au camp dans un instant.</p>
      <button id="corpse-return-btn">Retourner au camp</button>
    </div>`;
  document.body.appendChild(overlay);
  document.getElementById("corpse-return-btn")?.addEventListener("click", () => {
    overlay?.classList.remove("show");
  });
  hint = document.createElement("div");
  hint.id = "corpse-hint";
  document.body.appendChild(hint);
}
function onDeath(info) {
  ensureDom();
  dead = true;
  const t = document.getElementById("corpse-death-text");
  if (t)
    t.textContent = deathText(info);
  overlay?.classList.add("show");
}
function onRespawned() {
  dead = false;
  overlay?.classList.remove("show");
}
function onRecovered() {
  ensureDom();
  hintUntil = performance.now() + 1800;
}
function frame2(canvas, game) {
  ensureDom();
  const info = parseOr(parseRaw(game), null);
  const ctx = canvas.getContext("2d");
  const rect = canvas.getBoundingClientRect();
  const scale = canvas.width / Math.max(rect.width, 1);
  if (!info || !ctx || !hint) {
    if (hint)
      hint.style.display = "none";
    return;
  }
  const sx = info.sx * scale;
  const sy = info.sy * scale;
  const now = performance.now();
  const pulse = 0.5 + 0.5 * Math.sin(now / 180);
  ctx.save();
  ctx.strokeStyle = `rgba(230, 60, 40, ${0.35 + 0.45 * pulse})`;
  ctx.lineWidth = 2;
  ctx.beginPath();
  ctx.ellipse(sx, sy, 16 + 5 * pulse, 8 + 2.5 * pulse, 0, 0, Math.PI * 2);
  ctx.stroke();
  ctx.fillStyle = `rgba(255, 210, 160, ${0.8 + 0.2 * pulse})`;
  ctx.font = "13px monospace";
  ctx.textAlign = "center";
  ctx.fillText("☠", sx, sy - 10 - 2 * pulse);
  ctx.restore();
  hint.style.display = "block";
  hint.style.left = `${rect.left + window.scrollX + sx - hint.offsetWidth / 2}px`;
  hint.style.top = `${rect.top + window.scrollY + sy - 34}px`;
  if (now < hintUntil) {
    hint.textContent = "Équipement récupéré !";
    hint.style.color = "#b8e0a0";
  } else {
    hint.style.color = "#ffd9a0";
    hint.textContent = corpseHint(info);
  }
  if (dead)
    hint.style.display = "none";
}
function parseRaw(game) {
  try {
    return game.corpseInfo() || "null";
  } catch {
    return "null";
  }
}
function installCorpseUI(win) {
  registerModule("corpse-ui", "ts");
  const api = { init: ensureDom, onDeath, onRespawned, onRecovered, frame: frame2 };
  win.D2CorpseUI = api;
}
if (typeof window !== "undefined")
  installCorpseUI(window);

// src/audio/engine.ts
var MUTE_KEY = "d2wasm.musicMuted";
var VOLUME_KEYS = {
  music: "d2wasm.vol.music",
  amb: "d2wasm.vol.amb",
  sfx: "d2wasm.vol.sfx"
};
var DEFAULT_VOLUMES = {
  music: 0.4,
  amb: 0.18,
  sfx: 0.5
};
var PAN_MAX = 0.85;
var DUCK_GAIN = 0.3;
var RAMP_S = 0.05;
function clamp01(v) {
  const n = Number(v);
  if (!Number.isFinite(n))
    return 0;
  return Math.min(1, Math.max(0, n));
}
function panFor(xScreen, refWidth) {
  if (!(refWidth > 0))
    return 0;
  const n = Number(xScreen) / refWidth * 2 - 1;
  if (!Number.isFinite(n))
    return 0;
  return Math.max(-PAN_MAX, Math.min(PAN_MAX, n));
}
function safeStorage() {
  try {
    const ls = globalThis.localStorage;
    if (ls) {
      ls.setItem("d2wasm.probe", "1");
      ls.removeItem("d2wasm.probe");
      return ls;
    }
  } catch {}
  const mem = new Map;
  return {
    getItem: (k) => mem.has(k) ? mem.get(k) : null,
    setItem: (k, v) => void mem.set(k, String(v))
  };
}
function loadVolumes(store) {
  const out = { ...DEFAULT_VOLUMES };
  try {
    for (const k of Object.keys(VOLUME_KEYS)) {
      const raw = store.getItem(VOLUME_KEYS[k]);
      if (raw !== null && raw !== "")
        out[k] = clamp01(raw);
    }
  } catch {}
  return out;
}
function readMuteKey(store) {
  try {
    return store.getItem(MUTE_KEY) === "1";
  } catch {
    return false;
  }
}
function writeMuteKey(store, muted) {
  try {
    store.setItem(MUTE_KEY, muted ? "1" : "0");
  } catch {}
}

class AudioEngine {
  storage;
  ctx = null;
  musicBus = null;
  ambBus = null;
  sfxBus = null;
  muteBus = null;
  panNode = null;
  duckMusic = null;
  duckAmb = null;
  panValue = 0;
  ducking = false;
  muted;
  volumes;
  buffers = new Map;
  unlockCallbacks = [];
  constructor(store = safeStorage()) {
    this.storage = store;
    this.volumes = loadVolumes(store);
    this.muted = readMuteKey(store);
  }
  unlocked() {
    return this.ctx !== null;
  }
  isMuted() {
    return this.muted;
  }
  isDucking() {
    return this.ducking;
  }
  volumeOf(kind) {
    return this.volumes[kind] ?? 0;
  }
  panValueNow() {
    return this.panValue;
  }
  setVolume(kind, v) {
    if (!Object.prototype.hasOwnProperty.call(this.volumes, kind)) {
      return this.volumeOf(kind);
    }
    this.volumes[kind] = clamp01(v);
    try {
      this.storage.setItem(VOLUME_KEYS[kind], String(this.volumes[kind]));
    } catch {}
    const ctx = this.ctx;
    const bus = kind === "music" ? this.musicBus : kind === "amb" ? this.ambBus : this.sfxBus;
    if (ctx && bus) {
      try {
        bus.gain.setTargetAtTime(this.volumes[kind], ctx.currentTime, RAMP_S);
      } catch {}
    }
    return this.volumes[kind];
  }
  setMuted(m) {
    this.muted = !!m;
    writeMuteKey(this.storage, this.muted);
    const ctx = this.ctx;
    if (ctx && this.muteBus) {
      try {
        this.muteBus.gain.setTargetAtTime(this.muted ? 0 : 1, ctx.currentTime, RAMP_S);
      } catch {}
    }
  }
  toggleMuted() {
    this.setMuted(!this.muted);
    return this.muted;
  }
  pan(xScreen, refWidth) {
    const w = typeof refWidth === "number" && refWidth > 0 ? refWidth : globalThis.innerWidth || 1280;
    this.panValue = panFor(xScreen, w);
    const ctx = this.ctx;
    const node = this.panNode;
    if (ctx && node && "pan" in node && node.pan) {
      try {
        node.pan.setTargetAtTime(this.panValue, ctx.currentTime, RAMP_S);
      } catch {}
    }
    return this.panValue;
  }
  setDucking(on) {
    this.ducking = !!on;
    const ctx = this.ctx;
    if (!ctx)
      return;
    const target = this.ducking ? DUCK_GAIN : 1;
    for (const node of [this.duckMusic, this.duckAmb]) {
      if (node) {
        try {
          node.gain.setTargetAtTime(target, ctx.currentTime, 0.15);
        } catch {}
      }
    }
  }
  unlock() {
    if (this.ctx) {
      if (this.ctx.state === "suspended")
        this.ctx.resume().catch(() => {});
      return true;
    }
    try {
      const Ctor = globalThis.AudioContext || globalThis.webkitAudioContext;
      if (!Ctor)
        throw new Error("WebAudio indisponible");
      const ctx = new Ctor;
      this.ctx = ctx;
      this.buildGraph(ctx);
      if (ctx.state === "suspended")
        ctx.resume().catch(() => {});
    } catch {
      this.ctx = null;
      return false;
    }
    const cbs = this.unlockCallbacks;
    this.unlockCallbacks = [];
    for (const cb of cbs) {
      try {
        cb();
      } catch {}
    }
    return true;
  }
  onUnlock(cb) {
    if (this.ctx)
      cb();
    else
      this.unlockCallbacks.push(cb);
  }
  buildGraph(ctx) {
    this.muteBus = ctx.createGain();
    this.muteBus.gain.value = this.muted ? 0 : 1;
    this.muteBus.connect(ctx.destination);
    this.duckMusic = ctx.createGain();
    this.duckMusic.gain.value = 1;
    this.duckMusic.connect(this.muteBus);
    this.musicBus = ctx.createGain();
    this.musicBus.gain.value = this.volumeOf("music");
    this.musicBus.connect(this.duckMusic);
    this.duckAmb = ctx.createGain();
    this.duckAmb.gain.value = 1;
    this.duckAmb.connect(this.muteBus);
    this.ambBus = ctx.createGain();
    this.ambBus.gain.value = this.volumeOf("amb");
    this.ambBus.connect(this.duckAmb);
    this.sfxBus = ctx.createGain();
    this.sfxBus.gain.value = this.volumeOf("sfx");
    this.panNode = ctx.createStereoPanner ? ctx.createStereoPanner() : ctx.createGain();
    if ("pan" in this.panNode && this.panNode.pan) {
      this.panNode.pan.value = this.panValue;
    }
    this.sfxBus.connect(this.panNode);
    this.panNode.connect(this.muteBus);
  }
  bufferOf(url) {
    const hit = this.buffers.get(url);
    if (hit)
      return hit;
    const job = (async () => {
      const r = await fetch(url);
      if (!r.ok)
        throw new Error(`HTTP ${r.status}`);
      const bytes = await r.arrayBuffer();
      const ctx = this.ctx;
      if (!ctx)
        throw new Error("contexte absent");
      return await ctx.decodeAudioData(bytes);
    })().catch((e) => {
      const msg = e && typeof e === "object" && "message" in e ? String(e.message) : String(e);
      console.info(`[audio] ${url} indisponible (${msg}) — silence.`);
      return null;
    });
    this.buffers.set(url, job);
    return job;
  }
  playBuffer(buf, opts = {}) {
    const ctx = this.ctx;
    if (!ctx)
      return null;
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      if (typeof opts.rate === "number" && Number.isFinite(opts.rate)) {
        src.playbackRate.value = opts.rate;
      }
      src.loop = !!opts.loop;
      const g = ctx.createGain();
      g.gain.value = typeof opts.gain === "number" ? clamp01(opts.gain) : 1;
      src.connect(g);
      let tail = g;
      if (typeof opts.pan === "number" && ctx.createStereoPanner) {
        const sp = ctx.createStereoPanner();
        sp.pan.value = Math.max(-1, Math.min(1, opts.pan));
        g.connect(sp);
        tail = sp;
      }
      const bus = this.sfxBus || this.muteBus;
      if (bus)
        tail.connect(bus);
      else
        tail.connect(ctx.destination);
      src.start();
      return src;
    } catch {
      return null;
    }
  }
}
var singleton = null;
function getAudioEngine() {
  if (!singleton)
    singleton = new AudioEngine;
  return singleton;
}
// src/audio/zones.ts
var MUSIC_BASE = "assets/music/";
var AMB_BASE = "assets/sfx/ambiance/";
var BY_LEVEL = {
  town: "town1",
  bloodmoor: "wild",
  denofevil: "denofevilaction",
  coldplains: "wild",
  stonyfield: "wild",
  darkwood: "wild",
  blackmarsh: "wild",
  tamoe: "wild",
  tamoehighland: "wild",
  bloodmoorwarp: "wild",
  cave: "caves",
  hole: "caves",
  crypt: "crypt",
  mausoleum: "crypt",
  monasterygate: "monastery",
  barracks: "monastery",
  jail: "monastery",
  cathedral: "monastery",
  catacombs: "monastery",
  andariel: "andarielaction",
  tristram: "tristram",
  lutgholein: "town2",
  rockywaste: "desert",
  dryhills: "desert",
  faroasis: "desert",
  lostcity: "desert",
  ancienttunnels: "desert",
  sewers: "sewer",
  sewersa2: "sewer",
  clawviper: "tombs",
  talrasha: "tombs",
  harem: "harem",
  palace: "harem",
  arcane: "sanctuary",
  arcaneanctuary: "sanctuary",
  sanctuary: "sanctuary",
  canyon: "valley",
  magi: "valley",
  kurastdocks: "town3",
  spiderforest: "jungle",
  greatmarsh: "jungle",
  flayerjungle: "jungle",
  spidercavern: "spider",
  arachnid: "spider",
  spider: "spider",
  kurastbazaar: "kurast",
  templedistrict: "kurast",
  travincal: "kurast",
  kurastsewer: "kurastsewer",
  ruinedtemple: "kurastsewer",
  disusedfane: "kurastsewer",
  pandemonium: "town4",
  pandemoniumfortress: "town4",
  outersteppes: "mesa",
  plainsofdespair: "mesa",
  mesa: "mesa",
  cityofthedamned: "lair",
  riverofflame: "lair",
  lair: "lair",
  chaossanctuary: "diablo",
  diablo: "diablo"
};
var BY_NAME = [
  [/blood\s?raven|ravenne/i, "bloodravenresolution"],
  [/andariel/i, "andarielaction"],
  [/radament/i, "radamentresolution"],
  [/izual/i, "izualaction"],
  [/den\s?of\s?evil|antre du mal/i, "denofevilaction"],
  [/hellforge|forge de l'enfer/i, "forgeaction"],
  [/horadric|bâton horadrim/i, "horadricaction"],
  [/tainted\s?sun|soleil (noirci|gâté)/i, "taintedsunaction"],
  [/diablo|sanctuaire du chaos|chaos/i, "diablo"],
  [/tristram/i, "tristram"],
  [/spider\s?cav|arachnid|caverne d'araign/i, "spider"],
  [/monast|cathédrale|cathedral|catacomb|caserne|barrack|prison|jail|geôl/i, "monastery"],
  [/crypte|crypt|mausol/i, "crypt"],
  [/caverne|grotte|\bcave|\bhole\b|\btrou\b/i, "caves"],
  [/rousses|rogue/i, "town1"],
  [/lut gholein/i, "town2"],
  [/harem|palais|palace|sérail|serail/i, "harem"],
  [/arcane|arcanique/i, "sanctuary"],
  [/kurast.*(sewer|égout|egout)|(sewer|égout|egout).*kurast/i, "kurastsewer"],
  [/égout|egout|sewer/i, "sewer"],
  [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, "tombs"],
  [/maggot|larve/i, "desert"],
  [/canyon|vallée des|valley of/i, "valley"],
  [/désert|desert|waste|dry\s?hills|oasis|lost\s?city|cité perdue|tunnels/i, "desert"],
  [/quai|dock/i, "town3"],
  [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche|spider/i, "jungle"],
  [/kurast/i, "kurast"],
  [/pand[ée]monium/i, "town4"],
  [/mesa|d[ée]sespoir|despair|marches|steppes/i, "mesa"],
  [/damn[ée]s|damned|flamme|flame|fleuve|river/i, "lair"],
  [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, "wild"],
  [/harrogath|arreat|worldstone|baal|acte\s*v|act\s*5/i, null]
];
function themeFor(key, name) {
  if (key && Object.prototype.hasOwnProperty.call(BY_LEVEL, key)) {
    return BY_LEVEL[key] || null;
  }
  if (name) {
    for (const [re, th] of BY_NAME) {
      if (re.test(name))
        return th;
    }
  }
  return null;
}
var ENV_STEMS = {
  0: [null, null],
  1: ["scene_wilderness_day", "scene_wilderness_night"],
  2: ["scene_wilderness_day", "scene_wilderness_night"],
  3: ["scene_wilderness_day", "scene_wilderness_night"],
  4: ["scene_cave", "scene_cave"],
  5: ["scene_crypt", "scene_crypt"],
  6: ["scene_monastery", "scene_monastery"],
  7: ["scene_monastery", "scene_monastery"],
  8: ["scene_catacombs", "scene_catacombs"],
  9: ["scene_cathedral", "scene_cathedral"],
  10: ["scene_catacombs", "scene_catacombs"],
  11: ["scene_catacombs", "scene_catacombs"],
  12: ["scene_desert_day", "scene_desert_day"],
  13: ["scene_desert_day", "scene_desert_day"],
  14: ["scene_desert_day", "scene_desert_day"],
  15: ["scene_sewer", "scene_sewer"],
  16: ["scene_harem", "scene_harem"],
  17: ["scene_harem", "scene_harem"],
  18: ["scene_catacombs", "scene_catacombs"],
  19: ["scene_cave", "scene_cave"],
  20: ["scene_tomb", "scene_tomb"],
  21: ["scene_tomb", "scene_tomb"],
  22: ["scene_mesa", "scene_mesa"],
  23: ["scene_town_3_day", "scene_town_3_day"],
  24: ["scene_jungle_day", "scene_jungle_day"],
  25: ["scene_town_3_day", "scene_jungle_day"],
  26: ["scene_cave", "scene_cave"],
  27: ["scene_sewer", "scene_sewer"],
  28: ["scene_cathedral", "scene_cathedral"],
  29: ["scene_cave", "scene_cave"],
  30: ["scene_catacombs", "scene_catacombs"],
  31: ["scene_mesa", "scene_mesa"],
  32: ["scene_mesa", "scene_mesa"],
  33: ["scene_lava", "scene_lava"],
  34: ["scene_lava", "scene_lava"],
  35: ["scene_wilderness_day", "scene_wilderness_night"],
  36: ["scene_wilderness_day", "scene_wilderness_night"],
  37: ["scene_wilderness_day", "scene_wilderness_night"],
  38: ["scene_wilderness_day", "scene_wilderness_night"],
  39: ["scene_cave", "scene_cave"],
  40: ["scene_cave", "scene_cave"],
  41: ["scene_cave", "scene_cave"],
  42: ["scene_monastery", "scene_monastery"],
  43: ["scene_monastery", "scene_monastery"],
  44: ["scene_catacombs", "scene_catacombs"],
  45: ["scene_catacombs", "scene_catacombs"],
  46: ["scene_mesa", "scene_mesa"],
  47: ["scene_catacombs", "scene_catacombs"],
  48: ["scene_catacombs", "scene_catacombs"],
  49: ["scene_cathedral", "scene_cathedral"]
};
var ENV_BY_LEVEL = {
  town: 1,
  bloodmoor: 2,
  denofevil: 2,
  coldplains: 2,
  stonyfield: 2,
  darkwood: 2,
  blackmarsh: 2,
  tamoe: 2,
  graveyard: 2,
  cave: 4,
  hole: 4,
  crypt: 5,
  mausoleum: 5,
  monasterygate: 6,
  monastery: 6,
  courtyard: 7,
  barracks: 8,
  jail: 8,
  cathedral: 9,
  catacombs: 10,
  andariel: 11,
  tristram: 3,
  lutgholein: 12,
  rockywaste: 13,
  dryhills: 13,
  faroasis: 13,
  lostcity: 13,
  ancienttunnels: 13,
  canyon: 14,
  magi: 14,
  sewers: 15,
  sewersa2: 15,
  harem: 16,
  palace: 16,
  basement: 18,
  clawviper: 19,
  talrasha: 19,
  maggotlair: 20,
  duriel: 21,
  arcane: 22,
  arcaneanctuary: 22,
  sanctuary: 22,
  kurastdocks: 23,
  spiderforest: 24,
  greatmarsh: 24,
  flayerjungle: 24,
  spidercavern: 29,
  arachnid: 29,
  spider: 29,
  kurastbazaar: 25,
  templedistrict: 25,
  travincal: 25,
  kurastsewer: 26,
  ruinedtemple: 26,
  disusedfane: 26,
  dungeon: 27,
  temple: 28,
  mephisto: 30,
  pandemonium: 31,
  pandemoniumfortress: 31,
  outersteppes: 32,
  plainsofdespair: 32,
  mesa: 32,
  cityofthedamned: 33,
  riverofflame: 33,
  lair: 33,
  chaossanctuary: 34,
  diablo: 34,
  harrogath: 36,
  siege: 37,
  barricade: 37,
  barricadesnow: 38,
  icecave: 39,
  templeentrance: 43,
  xtemple: 44,
  templeboss: 45,
  hell5: 46,
  baaltemple: 47,
  throne: 48,
  worldstone: 49
};
var ENV_BY_NAME = [
  [/tristram/i, 3],
  [/town|ville|\bcamp\b|rousses|rogue/i, 1],
  [/lut gholein/i, 12],
  [/spider\s?cav|arachnid|caverne d'araign/i, 29],
  [/catacomb/i, 10],
  [/cathédrale|cathedral/i, 9],
  [/monast|caserne|barrack|prison|jail|geôl/i, 6],
  [/crypte|crypt|mausol/i, 5],
  [/caverne|grotte|\bcave\b|\bhole\b|\btrou\b/i, 4],
  [/harem|palais|palace|sérail|serail/i, 16],
  [/égout|egout|sewer/i, 15],
  [/maggot|larve/i, 20],
  [/duriel/i, 21],
  [/tal\s?rasha|claw\s?viper|temple du serpent|halls of (the )?dead|salles des morts|tombe|tomb/i, 19],
  [/arcane|arcanique/i, 22],
  [/quai|dock/i, 23],
  [/jungle|for[êe]t|forest|marsh|marais|flayer|f[ée]tiche/i, 24],
  [/kurast/i, 25],
  [/donjon|dungeon/i, 27],
  [/temple/i, 28],
  [/lave|lava|flamme|flame|fleuve|river|damn[ée]s|damned/i, 33],
  [/mesa|marches|steppes|d[ée]sespoir|despair/i, 32],
  [/glace|ice/i, 39],
  [/enfer|hell/i, 46],
  [/pierre du monde|world\s?stone/i, 49],
  [/baal/i, 47],
  [/arreat|sommet|mountain/i, 42],
  [/désert|desert|waste|oasis|lost\s?city|cité perdue|tunnels/i, 13],
  [/pand[ée]monium/i, 31],
  [/blood\s?moor|landes|moor|plaine|plain|field|champ|wood|bois|wild|land|highland/i, 2]
];
function envFor(key, name) {
  if (key && Object.prototype.hasOwnProperty.call(ENV_BY_LEVEL, key)) {
    return ENV_BY_LEVEL[key] ?? null;
  }
  if (name) {
    for (const [re, env] of ENV_BY_NAME) {
      if (re.test(name))
        return env;
    }
  }
  return null;
}
function isNight(hour) {
  return hour >= 20 || hour < 6;
}
function ambienceStems(env) {
  if (env === null)
    return null;
  return Object.prototype.hasOwnProperty.call(ENV_STEMS, env) ? ENV_STEMS[env] ?? null : null;
}
function ambianceFor(key, name, night) {
  const stems = ambienceStems(envFor(key, name));
  if (!stems)
    return null;
  return night ? stems[1] : stems[0];
}
// src/audio/music.ts
var FADE_S = 1.5;
var FADE_AMB_S = 1.5;
var DEFAULT_THEME = "town1";

class ZoneMusic {
  engine;
  fade;
  fadeAmb;
  hourNow;
  wanted = DEFAULT_THEME;
  wantedAmb = null;
  current = null;
  ambNow = null;
  epoch = 0;
  ambEpoch = 0;
  nightOverride = null;
  currentEnv = null;
  constructor(engine, opts = {}) {
    this.engine = engine;
    this.fade = typeof opts.fade === "number" && opts.fade > 0 ? opts.fade : FADE_S;
    this.fadeAmb = typeof opts.fadeAmb === "number" && opts.fadeAmb > 0 ? opts.fadeAmb : FADE_AMB_S;
    this.hourNow = opts.hourNow || (() => new Date().getHours());
  }
  theme() {
    return this.wanted;
  }
  ambient() {
    return this.ambNow ? this.ambNow.stem : null;
  }
  night() {
    return this.nightOverride !== null ? this.nightOverride : isNight(this.hourNow());
  }
  stopCurrent(fadeOut = 0.5) {
    const cur = this.current;
    const ctx = this.engine.ctx;
    if (!cur || !ctx)
      return;
    this.current = null;
    try {
      cur.gain.gain.setTargetAtTime(0, ctx.currentTime, fadeOut / 3);
      cur.src.stop(ctx.currentTime + fadeOut + 0.1);
    } catch {}
  }
  stopAmbience(fadeOut = this.fadeAmb) {
    const cur = this.ambNow;
    const ctx = this.engine.ctx;
    if (!cur || !ctx)
      return;
    this.ambNow = null;
    try {
      cur.gain.gain.setTargetAtTime(0, ctx.currentTime, fadeOut / 3);
      cur.src.stop(ctx.currentTime + fadeOut + 0.1);
    } catch {}
  }
  async apply() {
    const ctx = this.engine.ctx;
    const bus = this.engine.musicBus;
    if (!ctx || !bus)
      return;
    const my = ++this.epoch;
    const muted = this.engine.isMuted();
    if (muted || !this.wanted) {
      this.stopCurrent();
      this.applyAmbience();
      return;
    }
    if (this.current && this.current.theme === this.wanted)
      return;
    const wantedTheme = this.wanted;
    const buf = await this.engine.bufferOf(`${MUSIC_BASE}${wantedTheme}-t2.wav`);
    if (my !== this.epoch)
      return;
    if (!buf || this.engine.isMuted())
      return;
    this.stopCurrent(this.fade);
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      const g = ctx.createGain();
      g.gain.value = 0;
      src.connect(g);
      g.connect(bus);
      src.start();
      g.gain.setTargetAtTime(1, ctx.currentTime, this.fade / 3);
      src.onended = () => {
        if (this.current && this.current.src === src)
          this.current = null;
      };
      this.current = { theme: wantedTheme, src, gain: g };
    } catch {
      console.info("[musique] lecture impossible.");
    }
  }
  async applyAmbience() {
    const ctx = this.engine.ctx;
    const bus = this.engine.ambBus;
    if (!ctx || !bus)
      return;
    const my = ++this.ambEpoch;
    const muted = this.engine.isMuted();
    if (muted || !this.wantedAmb) {
      this.stopAmbience();
      return;
    }
    if (this.ambNow && this.ambNow.stem === this.wantedAmb)
      return;
    const wantedStem = this.wantedAmb;
    const buf = await this.engine.bufferOf(`${AMB_BASE}${wantedStem}.wav`);
    if (my !== this.ambEpoch)
      return;
    if (!buf || this.engine.isMuted())
      return;
    this.stopAmbience(this.fadeAmb);
    try {
      const src = ctx.createBufferSource();
      src.buffer = buf;
      src.loop = true;
      const g = ctx.createGain();
      g.gain.value = 0;
      src.connect(g);
      g.connect(bus);
      src.start();
      g.gain.setTargetAtTime(1, ctx.currentTime, this.fadeAmb / 3);
      src.onended = () => {
        if (this.ambNow && this.ambNow.src === src)
          this.ambNow = null;
      };
      this.ambNow = { stem: wantedStem, src, gain: g };
    } catch {
      console.info("[ambiance] lecture impossible.");
    }
  }
  start() {
    this.engine.onUnlock(() => {
      this.apply().catch(() => {});
    });
  }
  setLevel(key, name) {
    const th = themeFor(key, name);
    if (th === null) {
      if (key || name) {
        console.info(`[musique] zone sans thème dédié (${key || "?"}) — thème courant conservé.`);
      }
    } else if (th !== this.wanted) {
      this.wanted = th;
      this.apply().catch(() => {});
    }
    const stem = ambianceFor(key, name, this.night());
    if (!stem)
      return;
    this.currentEnv = envFor(key, name);
    if (stem !== this.wantedAmb) {
      this.wantedAmb = stem;
      this.applyAmbience().catch(() => {});
    }
  }
  setMuted(m) {
    this.engine.setMuted(m);
    this.apply().catch(() => {});
  }
  toggle() {
    this.setMuted(!this.engine.isMuted());
    return this.engine.isMuted();
  }
  isMuted() {
    return this.engine.isMuted();
  }
  setNight(n) {
    this.nightOverride = typeof n === "boolean" ? n : null;
    if (this.currentEnv === null)
      return;
    const stems = ambienceStems(this.currentEnv);
    const stem = stems ? this.night() ? stems[1] : stems[0] : null;
    if (stem && stem !== this.wantedAmb) {
      this.wantedAmb = stem;
      this.applyAmbience().catch(() => {});
    }
  }
  setAmbience(stem) {
    if (!stem || stem === this.wantedAmb)
      return;
    this.wantedAmb = stem;
    this.applyAmbience().catch(() => {});
  }
  ambianceForNow(key, name) {
    return ambianceFor(key, name, this.night());
  }
  static rampConstant() {
    return RAMP_S;
  }
}
// src/audio/sfx.ts
var MAX_VOICES = 4;
var ANTISPAM_MS = 60;

class SfxQueue {
  engine;
  base;
  voices = 0;
  last = new Map;
  nowMs;
  constructor(engine, base, opts = {}) {
    this.engine = engine;
    this.base = base;
    this.nowMs = opts.nowMs || (() => performance.now());
  }
  mutedNow() {
    return this.engine.isMuted();
  }
  play(file, vol = 1, rate = 1) {
    if (!file || !this.engine.unlocked() || this.mutedNow())
      return false;
    const now = this.nowMs();
    if (now - (this.last.get(file) ?? -1e9) < ANTISPAM_MS)
      return false;
    if (this.voices >= MAX_VOICES)
      return false;
    this.voices += 1;
    this.last.set(file, now);
    (async () => {
      try {
        const buf = await this.engine.bufferOf(`${this.base}${file}`);
        if (!buf || this.mutedNow()) {
          this.voices = Math.max(0, this.voices - 1);
          return;
        }
        const src = this.engine.playBuffer(buf, { gain: vol, rate });
        if (src) {
          src.onended = () => {
            this.voices = Math.max(0, this.voices - 1);
          };
        } else {
          this.voices = Math.max(0, this.voices - 1);
        }
      } catch {
        this.voices = Math.max(0, this.voices - 1);
        console.info("[sfx] lecture impossible.");
      }
    })();
    return true;
  }
  async playAt(name, xScreen) {
    if (!name || !this.engine.unlocked() || this.mutedNow())
      return null;
    if (typeof xScreen === "number")
      this.engine.pan(xScreen);
    let buf = null;
    try {
      buf = await this.engine.bufferOf(`${this.base}${name}`);
    } catch {
      return null;
    }
    if (!buf || this.mutedNow())
      return null;
    const src = this.engine.playBuffer(buf, { gain: 1 });
    if (!src)
      console.info(`[sfx] ${name} non joué.`);
    return src;
  }
  activeVoices() {
    return this.voices;
  }
}
// src/music.ts
var btn = null;
var panel = null;
var SLIDERS = [
  ["music", "Musique"],
  ["amb", "Ambiance"],
  ["sfx", "Effets"]
];
function buildPanel(engine) {
  panel = document.createElement("div");
  panel.id = "audio-mix";
  panel.style.cssText = "display:none;position:fixed;right:10px;bottom:10px;z-index:60;" + "background:rgba(20,17,10,.94);border:1px solid #5a3c1e;padding:10px 12px;" + "font:11px/1.6 monospace;color:#d0b070;min-width:190px;";
  const title = document.createElement("div");
  title.textContent = "MIXAGE AUDIO";
  title.style.cssText = "color:#ffe9ad;margin-bottom:6px;letter-spacing:1px;";
  panel.appendChild(title);
  const rows = [];
  for (const [kind, label] of SLIDERS) {
    const lab = document.createElement("label");
    lab.style.cssText = "display:flex;align-items:center;gap:6px;";
    lab.textContent = label + " ";
    const range = document.createElement("input");
    range.type = "range";
    range.min = "0";
    range.max = "100";
    range.step = "1";
    range.value = String(Math.round(engine.volumeOf(kind) * 100));
    range.style.cssText = "flex:1;accent-color:#c7a45a;";
    range.addEventListener("input", () => {
      engine.setVolume(kind, Number(range.value) / 100);
      applyMuteVisual(engine);
    });
    lab.appendChild(range);
    panel.appendChild(lab);
    rows.push([kind, lab, range]);
  }
  panel.rows = rows;
  const hint = document.createElement("div");
  hint.textContent = "la nuit tombe après 20 h (D2Audio.setNight)";
  hint.style.cssText = "margin-top:6px;opacity:.6;";
  panel.appendChild(hint);
  document.body.appendChild(panel);
}
function applyMuteVisual(engine) {
  if (btn)
    btn.textContent = engine.isMuted() ? "Musique : non" : "Musique : oui";
  if (panel) {
    const rows = panel.rows ?? [];
    for (const [, lab, range] of rows) {
      range.disabled = engine.isMuted();
      lab.style.opacity = engine.isMuted() ? "0.5" : "1";
    }
  }
}
function initButton(engine, zoneMusic) {
  try {
    btn = document.createElement("button");
    btn.id = "btn-music";
    btn.title = "Musique par zone (fondu doux au changement — EXP-A1)";
    btn.addEventListener("click", (ev) => {
      ev.stopPropagation();
      engine.unlock();
      zoneMusic.toggle();
      applyMuteVisual(engine);
    });
    const mix = document.createElement("button");
    mix.id = "btn-audio";
    mix.title = "Volumes : musique / ambiance / effets (persistés)";
    mix.textContent = "Audio ⚙";
    mix.addEventListener("click", (ev) => {
      ev.stopPropagation();
      engine.unlock();
      if (!panel)
        buildPanel(engine);
      if (!panel)
        return;
      const open = panel.style.display !== "none";
      panel.style.display = open ? "none" : "block";
    });
    const ref = document.getElementById("btn-sound");
    if (ref && ref.parentNode) {
      ref.parentNode.insertBefore(btn, ref);
      ref.parentNode.insertBefore(mix, btn);
    } else {
      document.body.appendChild(mix);
      document.body.appendChild(btn);
    }
    applyMuteVisual(engine);
  } catch {}
}
function installTsMusic(win) {
  registerModule("music", "ts");
  const engine = getAudioEngine();
  const zoneMusic = new ZoneMusic(engine);
  const ambQueue = new SfxQueue(engine, AMB_BASE);
  window.addEventListener("pointerdown", () => engine.unlock());
  window.addEventListener("keydown", () => engine.unlock());
  zoneMusic.start();
  const musicApi = {
    setLevel: (key, name) => zoneMusic.setLevel(key, name),
    setMuted: (m) => {
      zoneMusic.setMuted(m);
      applyMuteVisual(engine);
    },
    toggle: () => {
      zoneMusic.toggle();
      applyMuteVisual(engine);
    },
    theme: () => zoneMusic.theme(),
    isMuted: () => zoneMusic.isMuted()
  };
  const audioApi = {
    pan: (xScreen, refWidth) => engine.pan(xScreen, refWidth),
    play: (name, xScreen) => ambQueue.playAt(name, xScreen),
    setAmbience: (stem) => zoneMusic.setAmbience(stem),
    setNight: (n) => zoneMusic.setNight(n),
    ambianceFor: (key, name) => zoneMusic.ambianceForNow(key, name),
    setVolume: (kind, v) => engine.setVolume(kind, clamp01(v)),
    volume: (kind) => engine.volumeOf(kind),
    panValue: () => engine.panValueNow(),
    ambient: () => zoneMusic.ambient()
  };
  win.D2Music = musicApi;
  win.D2Audio = audioApi;
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", () => initButton(engine, zoneMusic), { once: true });
  } else {
    initButton(engine, zoneMusic);
  }
}
function installMusic(win) {
  if (win.D2Music) {
    registerModule("music", "adopt");
    return;
  }
  installTsMusic(win);
}
if (typeof window !== "undefined")
  installMusic(window);

// src/mon-sfx.ts
var MONSFX_BASE = "assets/sfx/monsounds/";
var MONSFX_LEGACY = "assets/sfx/";
var MONSFX_MAX_DIST_PX = 900;
var MONSFX_VOL_FLOOR = 0.22;
var MONSFX_SPAM_MS = 70;
var CODE_FAMILY = {
  FA: "fallen",
  ZM: "zombie",
  SI: "quillrat",
  SK: "skeleton",
  FS: "fallenshaman",
  YE: "brute",
  CR: "corruptrogue",
  GM: "goatman",
  RD: "radament",
  SU: "summoner",
  GZ: "zombieunique",
  AN: "andariel",
  DU: "duriel",
  MP: "mephisto",
  DI: "diablo",
  "22": "izual",
  "5P": "smithdemon",
  "0D": "ancient",
  "0E": "ancient",
  "0F": "ancient",
  GY: "unraveler",
  "42": "baalcrab",
  BK: "foulcrow"
};
var FILES = {
  ancient: { attack: null, hit: null, death: null, taunt: null },
  andariel: { attack: "andariel_attack1.wav", hit: "andariel_gethit1.wav", death: "andariel_death.wav", taunt: "andariel_taunt1.wav" },
  arach: { attack: "spider_attack1.wav", hit: "spider_gethit1.wav", death: "spider_death1.wav", taunt: null },
  baalcrab: { attack: null, hit: null, death: null, taunt: null },
  baalminion: { attack: null, hit: null, death: null, taunt: null },
  batdemon: { attack: "batdemon_attack1.wav", hit: "batdemon_gethit1.wav", death: "batdemon_death1.wav", taunt: null },
  bighead: { attack: "bighead_attack1.wav", hit: "bighead_gethit1.wav", death: "bighead_death1.wav", taunt: null },
  bloodlord: { attack: null, hit: null, death: null, taunt: null },
  bloodraven: { attack: null, hit: "bloodraven_gethit1.wav", death: "bloodraven_death.wav", taunt: "bloodraven_taunt1.wav" },
  brute: { attack: "yeti_attack1.wav", hit: "yeti_gethit1.wav", death: "yeti_death1.wav", taunt: null },
  clawviper: { attack: "serpentdemon_attacktail1.wav", hit: "serpentdemon_gethit1.wav", death: "serpentdemon_death1.wav", taunt: null },
  corruptrogue: { attack: "corrupt_attack1.wav", hit: "corrupt_gethit1.wav", death: null, taunt: null },
  councilmember: { attack: "zakarumhighpriest_attack1.wav", hit: "zakarumhighpriest_gethit1.wav", death: "zakarumhighpriest_death1.wav", taunt: null },
  cr_archer: { attack: null, hit: "corrupt_gethit1.wav", death: null, taunt: null },
  cr_lancer: { attack: "corrupt_attack1.wav", hit: "corrupt_gethit1.wav", death: null, taunt: null },
  deathmauler: { attack: null, hit: null, death: null, taunt: null },
  diablo: { attack: "diablo_attack1.wav", hit: "diablo_gethit1.wav", death: "diablo_death.wav", taunt: null },
  doomknight: { attack: "undeadhorror_attack1.wav", hit: "undeadhorror_gethit1.wav", death: "undeadhorror_death1.wav", taunt: null },
  duriel: { attack: "duriel_attack1.wav", hit: "duriel_gethit1.wav", death: null, taunt: "duriel_taunt1.wav" },
  fallen: { attack: null, hit: "fallen_gethit1.wav", death: "fallen_death1.wav", taunt: null },
  fallenshaman: { attack: null, hit: "fallenshaman_gethit1.wav", death: "fallenshaman_death1.wav", taunt: null },
  fetishshaman: { attack: "pygmyshaman_spell1.wav", hit: "pygmyshaman_gethit1.wav", death: "pygmyshaman_death1.wav", taunt: null },
  fingermage: { attack: "fingermage_attack1.wav", hit: "fingermage_gethit1.wav", death: "fingermage_death1.wav", taunt: null },
  foulcrow: { attack: "hawk_attack1.wav", hit: "hawk_gethit1.wav", death: "hawk_death1.wav", taunt: null },
  frozenhorror: { attack: null, hit: null, death: null, taunt: null },
  goatman: { attack: "goatman_attack1.wav", hit: "goatman_gethit1.wav", death: "goatman_death1.wav", taunt: null },
  hellbovine: { attack: "moo_attack1.wav", hit: "moo_gethit1.wav", death: "moo_death1.wav", taunt: null },
  imp: { attack: null, hit: null, death: null, taunt: null },
  izual: { attack: "izual_attack1.wav", hit: "izual_gethit1.wav", death: "izual_death.wav", taunt: "izual_taunt1.wav" },
  maggotqueen: { attack: null, hit: "sandmaggot_gethit1.wav", death: "sandmaggotmama_death.wav", taunt: null },
  megademon: { attack: "megademon_attack1.wav", hit: "megademon_gethit1.wav", death: "megademon_death1.wav", taunt: null },
  mephisto: { attack: "mephisto_attack1.wav", hit: "mephisto_gethit1.wav", death: "mephisto_death.wav", taunt: "mephisto_taunt1.wav" },
  minion: { attack: null, hit: null, death: null, taunt: null },
  mummy: { attack: "mummy_attack1.wav", hit: "mummy_gethit1.wav", death: "mummy_death1.wav", taunt: null },
  nihlathak: { attack: null, hit: null, death: null, taunt: null },
  overseer: { attack: null, hit: null, death: null, taunt: null },
  pantherwoman: { attack: "pantherwoman_attack1.wav", hit: "pantherwoman_gethit1.wav", death: "pantherwoman_death1.wav", taunt: null },
  quillrat: { attack: "spikefiend_attack1.wav", hit: "spikefiend_gethit1.wav", death: "spikefiend_death1.wav", taunt: null },
  radament: { attack: "greatermummy_attack1.wav", hit: "greatermummy_gethit1.wav", death: "greatermummy_death1.wav", taunt: "radament_taunt1.wav" },
  reanimatedhorde: { attack: null, hit: null, death: null, taunt: null },
  regurgitator: { attack: "regurgitator_attack1.wav", hit: "regurgitator_gethit1.wav", death: "regurgitator_death1.wav", taunt: null },
  sandraider: { attack: "sandraider_attack1.wav", hit: "sandraider_gethit1.wav", death: "sandraider_death1.wav", taunt: null },
  scarab: { attack: "beetle_attack1.wav", hit: "beetle_gethit1.wav", death: "beetle_death1.wav", taunt: null },
  siegebeast: { attack: null, hit: null, death: null, taunt: null },
  sk_mage: { attack: null, hit: "skeleton_gethit1.wav", death: "skeleton_death1.wav", taunt: null },
  skeleton: { attack: null, hit: "skeleton_gethit1.wav", death: "skeleton_death1.wav", taunt: null },
  smith: { attack: "smith_attack1.wav", hit: "smith_gethit1.wav", death: "smith_death.wav", taunt: "smith_taunt2.wav" },
  smithdemon: { attack: "smith_attack1.wav", hit: "smith_gethit1.wav", death: "smith_death.wav", taunt: null },
  succubus: { attack: null, hit: null, death: null, taunt: null },
  summoner: { attack: null, hit: "summoner_gethit1.wav", death: "summoner_death.wav", taunt: "summoner_taunt1.wav" },
  thornhulk: { attack: null, hit: "thornhulk_gethit1.wav", death: "thornhulk_death1.wav", taunt: null },
  unraveler: { attack: "greatermummy_attack1.wav", hit: "greatermummy_gethit1.wav", death: "greatermummy_death1.wav", taunt: null },
  vilemother: { attack: "vilemother_attack1.wav", hit: "vilemother_gethit1.wav", death: "vilemother_death1.wav", taunt: null },
  willowisp: { attack: "willowisp_attack1.wav", hit: "willowisp_gethit1.wav", death: "willowisp_death1.wav", taunt: null },
  zombie: { attack: "zombie_attack1.wav", hit: "zombie_gethit1.wav", death: "zombie_death1.wav", taunt: null },
  zombieunique: { attack: "zombie_attack1.wav", hit: "zombie_gethit1.wav", death: "zombie_death1.wav", taunt: null }
};
var GENERIC = {
  attack: null,
  hit: "hit.wav",
  death: "fallen_die.wav",
  taunt: null
};
var LEGACY_FALLBACK = {};
function familyOf(code) {
  return code != null && CODE_FAMILY[code] || null;
}
function fileFor(fam, ev) {
  const f = FILES[fam]?.[ev];
  if (f)
    return MONSFX_BASE + f;
  const legacy = LEGACY_FALLBACK[fam]?.[ev];
  if (legacy)
    return MONSFX_LEGACY + legacy;
  return GENERIC[ev] ? MONSFX_LEGACY + GENERIC[ev] : null;
}
function feedTargets(ev) {
  if (!ev || typeof ev !== "object")
    return [];
  if (ev.HeroAttacked)
    return [{ index: ev.HeroAttacked.target, kind: "hit" }];
  if (ev.MissileHit)
    return [{ index: ev.MissileHit.target, kind: "hit" }];
  if (ev.MercAttacked)
    return [{ index: ev.MercAttacked.target, kind: "hit" }];
  if (ev.HeroHit)
    return [{ index: ev.HeroHit.index, kind: "attack" }];
  if (ev.MonsterDied)
    return [{ index: ev.MonsterDied.index, kind: "death" }];
  return [];
}
function installTsMonSfx(win) {
  registerModule("mon-sfx", "ts");
  const engine = getAudioEngine();
  let enabled = true;
  const buffers = new Map;
  const last = new Map;
  const announced = new Set;
  const unlock = () => {
    engine.unlock();
  };
  const bufferOf = (path) => {
    const hit = buffers.get(path);
    if (hit)
      return hit;
    const job = (async () => {
      const r = await fetch(path);
      if (!r.ok)
        throw new Error(`HTTP ${r.status}`);
      const bytes = await r.arrayBuffer();
      const ctx = engine.ctx;
      if (!ctx)
        throw new Error("contexte absent");
      return await ctx.decodeAudioData(bytes);
    })().catch((e) => {
      const msg = e instanceof Error ? e.message : String(e);
      console.info(`[mon-sfx] ${path} indisponible (${msg}) — silence.`);
      return null;
    });
    buffers.set(path, job);
    return job;
  };
  const play = (path, pan, vol, rate) => {
    if (!enabled || !path || !engine.unlocked())
      return;
    const now = performance.now();
    const key = path + "@" + Math.round(pan * 4);
    if (now - (last.get(key) ?? 0) < MONSFX_SPAM_MS)
      return;
    last.set(key, now);
    (async () => {
      const buf = await bufferOf(path);
      if (!buf || !engine.unlocked() || !enabled)
        return;
      const src = engine.playBuffer(buf, { gain: Math.min(1, vol), pan, rate });
      if (!src)
        console.info("[mon-sfx] lecture impossible.");
    })().catch(() => {});
  };
  const panAndVolOf = (index) => {
    let pan = 0;
    let vol = 0.6;
    try {
      const game = win.__d2?.game;
      if (!game)
        return [pan, vol];
      const info = JSON.parse(game.monsterInfo(index) || "null");
      if (info) {
        const canvas = document.querySelector("canvas");
        const w = canvas && canvas.width || window.innerWidth || 960;
        const half = w / 2;
        pan = Math.max(-1, Math.min(1, ((info.sx ?? half) - half) / half));
        try {
          const screen = game.heroScreen();
          const d = Math.hypot((info.sx ?? 0) - screen[0], (info.sy ?? 0) - screen[1]);
          vol = Math.max(MONSFX_VOL_FLOOR, 1 - d / MONSFX_MAX_DIST_PX);
        } catch {}
      }
    } catch {}
    return [pan, vol];
  };
  const monsterEvent = (index, ev) => {
    if (!enabled)
      return;
    let code = null;
    try {
      code = (win.__d2?.monCodes ?? [])[index] ?? null;
    } catch {
      code = null;
    }
    const fam = familyOf(code);
    if (!fam)
      return;
    const [pan, vol] = panAndVolOf(index);
    play(fileFor(fam, ev), pan, vol, 1);
    if (ev === "hit" || ev === "attack") {
      try {
        const game = win.__d2?.game;
        const info = game ? JSON.parse(game.monsterInfo(index) || "null") : null;
        const rank = info && info.kind;
        if (info && (rank === "super_unique" || rank === "boss" || rank === "champion") && !announced.has(index)) {
          announced.add(index);
          const t = FILES[fam]?.taunt;
          if (t)
            play(MONSFX_BASE + t, pan, Math.min(1, vol + 0.2), 1);
        }
      } catch {}
    }
  };
  const watchSoundButton = () => {
    try {
      const b = document.getElementById("btn-sound");
      if (b)
        b.addEventListener("click", () => setMuted(enabled), { passive: true });
    } catch {}
  };
  const setMuted = (m) => {
    enabled = !m;
    if (enabled)
      unlock();
  };
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", watchSoundButton, { once: true });
  } else {
    watchSoundButton();
  }
  window.addEventListener("pointerdown", unlock);
  window.addEventListener("keydown", unlock);
  win.D2MonSfx = {
    feed(ev) {
      for (const t of feedTargets(ev))
        monsterEvent(t.index, t.kind);
    },
    setMuted,
    familyOf: (code) => familyOf(code),
    isEnabled: () => enabled
  };
}
function installMonSfx(win) {
  if (win.D2MonSfx) {
    registerModule("mon-sfx", "adopt");
    return;
  }
  installTsMonSfx(win);
}
if (typeof window !== "undefined")
  installMonSfx(window);

// src/game-sfx.ts
var GAMEPLAY_BASE = "assets/sfx/gameplay/";
var SFX = {
  levelup: "levelup.wav",
  heroDeath: "hero_death.wav",
  heroRespawn: "hero_respawn.wav",
  gold: "gold.wav",
  pickup: "pickup.wav",
  potionDrink: "potion_drink.wav",
  inventoryFull: "inventory_full.wav",
  itemDrop: "item_drop.wav",
  missileHit: "missile_hit.wav",
  questFlash: "quest_flash.wav",
  actUnlock: "act_unlock.wav",
  gameWon: "game_won.wav",
  corpseDrop: "corpse_drop.wav",
  corpseRecover: "corpse_recover.wav",
  npcTalk: "npc_talk.wav",
  mercHired: "merc_hired.wav",
  waypoint: "waypoint.wav",
  monResurrect: "mon_resurrect.wav",
  shrine: "shrine.wav",
  chest: "chest.wav",
  portalOpen: "portal_open.wav",
  portalEnter: "portal_enter.wav"
};
var CAST_KEYWORDS = [
  ["cast_fire", ["fire", "inferno", "meteor", "blaze", "enchant", "hydra"]],
  ["cast_cold", ["ice", "cold", "frost", "blizz", "glacial", "frozen", "shiver", "chill"]],
  ["cast_lightning", ["lightning", "charg", "static", "thunder", "nova", "chain", "shock"]]
];
var CAST_FILES = {
  cast_fire: "cast_fire.wav",
  cast_cold: "cast_cold.wav",
  cast_lightning: "cast_lightning.wav"
};
function missileCastSfx(skill) {
  const s = String(skill || "").toLowerCase();
  if (!s)
    return null;
  for (const [key, words] of CAST_KEYWORDS) {
    if (words.some((w) => s.includes(w))) {
      return CAST_FILES[key] ?? null;
    }
  }
  return null;
}
function classifyEvent(ev, activeSkill) {
  if (!ev || typeof ev !== "object")
    return null;
  const kind = Object.keys(ev)[0];
  const p = ev[kind] ?? {};
  switch (kind) {
    case "HeroAttacked":
    case "HeroMissed":
    case "HeroHit":
    case "MonsterDied":
    case "HeroMoved":
    case "MonFled":
    case "MissileMissed":
    case "MercAttacked":
    case "MercMissed":
    case "MercHit":
    case "MercDied":
    case "MercLeveled":
    case "MercResurrected":
      return null;
    case "MissileSpawned":
    case "MercMissileSpawned": {
      const file = missileCastSfx(activeSkill);
      return file ? { file, vol: 0.7 } : null;
    }
    case "ActQuestChanged":
      return p.phase === "done" ? { file: SFX.questFlash, vol: 0.8 } : null;
    case "HeroLeveled":
      return { file: SFX.levelup, vol: 0.9 };
    case "HeroDied":
      return { file: SFX.heroDeath, vol: 0.9 };
    case "HeroRespawned":
      return { file: SFX.heroRespawn, vol: 0.7 };
    case "GoldGained":
      return { file: SFX.gold, vol: 0.75 };
    case "HeroPickedUp":
      return { file: SFX.pickup, vol: 0.9 };
    case "PotionUsed":
      return { file: SFX.potionDrink, vol: 0.8 };
    case "InventoryFull":
      return { file: SFX.inventoryFull, vol: 0.8 };
    case "ItemDropped":
      return { file: SFX.itemDrop, vol: 0.6 };
    case "MissileHit":
      return { file: SFX.missileHit, vol: 0.55 };
    case "ActUnlocked":
      return { file: SFX.actUnlock, vol: 0.9 };
    case "GameWon":
      return { file: SFX.gameWon, vol: 1 };
    case "CorpseDropped":
      return { file: SFX.corpseDrop, vol: 0.8 };
    case "CorpseRecovered":
      return { file: SFX.corpseRecover, vol: 0.8 };
    case "NpcTalked":
      return { file: SFX.npcTalk, vol: 0.7 };
    case "MercHired":
      return { file: SFX.mercHired, vol: 0.8 };
    case "WaypointActivated":
      return { file: SFX.waypoint, vol: 0.8 };
    case "MonResurrected":
      return { file: SFX.monResurrect, vol: 0.7 };
    case "LevelSwitch":
      return { file: SFX.portalEnter, vol: 0.7 };
    case "LevelSwitchFailed":
      return null;
    default:
      return null;
  }
}
function activeSkillOf(win) {
  try {
    const stats = parseOr(win.__d2?.game ? win.__d2.game.heroStats() : "{}", {});
    return stats.skill ?? "";
  } catch {
    return "";
  }
}
function installTsGameSfx(win) {
  registerModule("game-sfx", "ts");
  const engine = getAudioEngine();
  const queue = new SfxQueue(engine, GAMEPLAY_BASE);
  const handleEvent = (ev) => {
    const hit = classifyEvent(ev, activeSkillOf(win));
    if (hit)
      queue.play(hit.file, hit.vol);
  };
  let wired = false;
  const tryWire = () => {
    if (wired)
      return true;
    const g = win.__d2?.game;
    if (!g || typeof g.tick !== "function")
      return false;
    const orig = g.tick.bind(g);
    g.tick = (dt, inputJson) => {
      const eventsJson = orig(dt, inputJson);
      try {
        if (eventsJson && eventsJson !== "[]") {
          const events = parseOr(eventsJson, []);
          if (Array.isArray(events) && engine.unlocked()) {
            for (const ev of events)
              handleEvent(ev);
          }
        }
      } catch {}
      return eventsJson;
    };
    wired = true;
    return true;
  };
  window.addEventListener("pointerdown", () => engine.unlock(), { passive: true });
  window.addEventListener("keydown", () => engine.unlock());
  const wireTimer = setInterval(() => {
    if (tryWire())
      clearInterval(wireTimer);
  }, 250);
  setTimeout(() => clearInterval(wireTimer), 60000);
  win.D2SFX = {
    play: (file, vol = 1, rate = 1) => file ? queue.play(file, vol, rate) : false,
    handleEvent,
    missileCastSfx: (skill) => missileCastSfx(skill),
    shrine: () => queue.play(SFX.shrine, 0.8),
    chest: () => queue.play(SFX.chest, 0.8),
    portalOpen: () => queue.play(SFX.portalOpen, 0.8),
    unlock: () => void engine.unlock(),
    isMuted: () => {
      try {
        return localStorage.getItem(MUTE_KEY) === "1";
      } catch {
        return false;
      }
    },
    toggleMute: () => {
      const m = engine.toggleMuted();
      if (win.D2Music?.setMuted)
        win.D2Music.setMuted(m);
      return m;
    },
    wired: () => wired
  };
}
function installGameSfx(win) {
  if (win.D2SFX) {
    registerModule("game-sfx", "adopt");
    return;
  }
  installTsGameSfx(win);
}
if (typeof window !== "undefined") {
  await warmLegacy("game-sfx.js");
  installGameSfx(window);
}

// src/automap.ts
var AUTOMAP_RADIUS = 9;
var AUTOMAP_POLL_MS = 200;
var PANEL_W = 240;
var COLORS = {
  bg: "rgba(10,8,5,0.78)",
  walk: "#c8b57a",
  wall: "#5c4c30",
  special: "#7ec8e3",
  hero: "#ffffff",
  border: "#5a4a2a",
  text: "#cfc7b8"
};
function proj(d, s, cx, cy) {
  return [(cx - cy + d.cellH) * s, (cx + cy) * s / 2 + 1];
}
function modeLineText(mode, coverage, hasGlyphs) {
  const pct = Math.round((coverage || 0) * 100);
  if (mode !== "automap")
    return `repli : polygone marchable · ${pct} %`;
  return hasGlyphs ? `AutoMap · traits réels · ${pct} %` : `AutoMap résolu · ${pct} %`;
}
function revealIndices(d, visited, byCell, cx, cy) {
  const out = [];
  const r2 = AUTOMAP_RADIUS * AUTOMAP_RADIUS;
  const x0 = Math.max(0, cx - AUTOMAP_RADIUS), x1 = Math.min(d.cellW - 1, cx + AUTOMAP_RADIUS);
  const y0 = Math.max(0, cy - AUTOMAP_RADIUS), y1 = Math.min(d.cellH - 1, cy + AUTOMAP_RADIUS);
  for (let y = y0;y <= y1; y++) {
    for (let x = x0;x <= x1; x++) {
      const dx = x - cx, dy = y - cy;
      if (dx * dx + dy * dy > r2)
        continue;
      const k = y * d.cellW + x;
      if (visited[k])
        continue;
      visited[k] = 1;
      const i = byCell.get(k);
      if (i !== undefined)
        out.push(i);
    }
  }
  return out;
}
function buildByCell(d) {
  const map = new Map;
  d.cells.forEach((c, i) => map.set(c[1] * d.cellW + c[0], i));
  return map;
}
function installTsAutomap(win) {
  registerModule("automap", "ts");
  const st = {
    open: false,
    game: null,
    canvas: null,
    panel: null,
    title: null,
    modeLine: null,
    inner: null,
    ictx: null,
    data: null,
    levelKey: null,
    visited: null,
    byCell: null,
    paths: [],
    pending: [],
    s: 2,
    lastPoll: 0,
    heroCell: null
  };
  function ensureDom() {
    if (st.canvas)
      return;
    const stage = document.getElementById("game")?.parentElement;
    if (!stage)
      return;
    const panel = document.createElement("div");
    panel.id = "d2-automap";
    Object.assign(panel.style, {
      position: "absolute",
      top: "34px",
      right: "10px",
      zIndex: "35",
      background: COLORS.bg,
      border: `1px solid ${COLORS.border}`,
      padding: "6px 8px 4px",
      display: "none",
      pointerEvents: "none"
    });
    const title = document.createElement("div");
    Object.assign(title.style, {
      font: "600 10px system-ui",
      letterSpacing: ".12em",
      color: COLORS.text,
      textShadow: "0 1px 2px #000",
      marginBottom: "4px",
      textTransform: "uppercase"
    });
    const cv = document.createElement("canvas");
    cv.id = "d2-automap-cv";
    Object.assign(cv.style, { display: "block", imageRendering: "pixelated" });
    const mode = document.createElement("div");
    mode.id = "d2-automap-mode";
    Object.assign(mode.style, {
      font: "9px system-ui",
      color: "#8a7f6a",
      marginTop: "3px",
      textShadow: "0 1px 2px #000"
    });
    panel.appendChild(title);
    panel.appendChild(cv);
    panel.appendChild(mode);
    stage.appendChild(panel);
    st.panel = panel;
    st.title = title;
    st.canvas = cv;
    st.modeLine = mode;
  }
  function cvSize(w, h) {
    const cv = st.canvas;
    const dcv = st.inner;
    if (!cv)
      return;
    if (!dcv || dcv.width !== w || dcv.height !== h) {
      const off = document.createElement("canvas");
      off.width = w;
      off.height = h;
      st.inner = off;
      st.ictx = off.getContext("2d");
    }
    cv.width = w;
    cv.height = h;
  }
  function loadLevelData(game) {
    const d = parseOr(safeAutomap(game), null);
    if (!d || !d.cells || !d.cellW) {
      st.data = null;
      return;
    }
    st.data = d;
    st.levelKey = d.level;
    st.visited = new Uint8Array(d.cellW * d.cellH);
    st.pending = [];
    st.heroCell = null;
    st.byCell = null;
    st.paths = (d.glyphs || []).map((segs) => {
      const p = new Path2D;
      for (const s of segs) {
        p.moveTo(s[0], s[1]);
        p.lineTo(s[2], s[3]);
      }
      return p;
    });
    st.s = Math.max(1.4, Math.min(4, PANEL_W / (d.cellW + d.cellH)));
    const s = st.s;
    const w = Math.ceil((d.cellW + d.cellH) * s);
    const h = Math.ceil((d.cellW + d.cellH) * s * 0.5) + 2;
    cvSize(w, h);
    st.ictx?.clearRect(0, 0, w, h);
    if (st.title)
      st.title.textContent = d.name || d.level || "";
  }
  function paintPending() {
    const d = st.data;
    const ctx = st.ictx;
    if (!d || !ctx)
      return;
    const s = st.s;
    const rw = Math.max(1, s), rh = Math.max(1, s / 2);
    for (const idx of st.pending) {
      const c = d.cells[idx];
      if (!c)
        continue;
      const [px, py] = proj(d, s, c[0], c[1]);
      const flags = c[2];
      const glyphIdx = c[4];
      const glyph = glyphIdx !== undefined && glyphIdx >= 0 ? st.paths[glyphIdx] : undefined;
      if (glyph) {
        const k = s / 160;
        ctx.save();
        ctx.translate(px, py);
        ctx.scale(k, k);
        ctx.lineWidth = Math.max(0.5, s / 4) / k;
        ctx.lineJoin = "miter";
        ctx.strokeStyle = flags & 4 ? COLORS.special : flags & 1 ? COLORS.walk : COLORS.wall;
        ctx.stroke(glyph);
        if (flags & 1) {
          ctx.globalAlpha = 0.55;
          ctx.fillStyle = flags & 4 ? COLORS.special : COLORS.walk;
          ctx.fill(glyph);
          ctx.globalAlpha = 1;
        }
        ctx.restore();
        continue;
      }
      ctx.fillStyle = flags & 1 ? COLORS.walk : COLORS.wall;
      if (flags & 4)
        ctx.fillStyle = COLORS.special;
      if (d.mode === "fallback" && !(flags & 1))
        continue;
      ctx.fillRect(px, py, rw, rh);
    }
    st.pending = [];
  }
  function showMode() {
    const el = st.modeLine;
    if (!el)
      return;
    if (!st.data) {
      el.textContent = "";
      return;
    }
    el.textContent = modeLineText(st.data.mode, st.data.coverage, !!st.paths.length);
  }
  function poll(now) {
    const game = win.__d2 && win.__d2.mode === "wasm" ? win.__d2.game : undefined;
    if (!game)
      return;
    st.game = game;
    if (now - st.lastPoll < AUTOMAP_POLL_MS)
      return;
    st.lastPoll = now;
    let key = null;
    try {
      key = parseOr(game.levelInfo() || "{}", {}).key ?? null;
    } catch {
      key = null;
    }
    if (!st.data || key !== st.levelKey) {
      st.byCell = null;
      loadLevelData(game);
      if (st.title) {
        st.title.textContent = st.data && (st.data.name || st.data.level) || "";
      }
      showMode();
    }
    const d = st.data;
    if (!d || !d.cells)
      return;
    try {
      const screen = game.heroScreen();
      const world = game.screenToWorld(screen[0], screen[1]);
      const wx = world[0];
      const wy = world[1];
      const sub = d.sub || 5;
      const cx = Math.floor(wx / sub), cy = Math.floor(wy / sub);
      if (cx >= 0 && cy >= 0 && cx < d.cellW && cy < d.cellH && st.visited) {
        st.heroCell = [cx, cy];
        if (!st.byCell)
          st.byCell = buildByCell(d);
        st.pending.push(...revealIndices(d, st.visited, st.byCell, cx, cy));
      }
    } catch {}
  }
  function draw() {
    const d = st.data;
    const cv = st.canvas;
    if (!d || !cv || !st.open)
      return;
    const ctx = cv.getContext("2d");
    if (!ctx || !st.inner)
      return;
    ctx.clearRect(0, 0, cv.width, cv.height);
    ctx.drawImage(st.inner, 0, 0);
    if (st.heroCell) {
      const [px, py] = proj(d, st.s, st.heroCell[0], st.heroCell[1]);
      const s = st.s;
      ctx.fillStyle = COLORS.hero;
      ctx.fillRect(px - s * 0.5, py - s * 0.25, Math.max(2, s), Math.max(2, s * 0.5));
    }
  }
  function loop(now) {
    try {
      ensureDom();
      poll(now);
      paintPending();
      draw();
    } catch {}
    requestAnimationFrame(loop);
  }
  requestAnimationFrame(loop);
  const automap = {
    open: false,
    toggle() {
      ensureDom();
      automap.open = !automap.open;
      st.open = automap.open;
      if (st.panel)
        st.panel.style.display = automap.open ? "block" : "none";
      if (automap.open && st.data === null && st.game) {
        st.levelKey = null;
      }
    },
    show() {
      if (!automap.open)
        automap.toggle();
    },
    hide() {
      if (automap.open)
        automap.toggle();
    }
  };
  window.addEventListener("keydown", (e) => {
    if (e.key !== "Tab")
      return;
    const t = document.activeElement?.tagName;
    if (t === "INPUT" || t === "TEXTAREA")
      return;
    if (win.__d2 && win.__d2.mode === "wasm") {
      e.preventDefault();
      automap.toggle();
    }
  });
  win.D2Automap = automap;
}
function safeAutomap(game) {
  try {
    return game.automap() || "null";
  } catch {
    return "null";
  }
}
function installAutomap(win) {
  if (win.D2Automap) {
    registerModule("automap", "adopt");
    return;
  }
  installTsAutomap(win);
}
if (typeof window !== "undefined") {
  await warmLegacy("automap.js");
  installAutomap(window);
}

// src/vendor.ts
var VENDOR_IDS = ["Akara", "Charsi", "Gheed"];
var SPAWN_PCT = 80;
var GAMBLE_SLOTS = 12;
var GHEED_GOLD = [350, 750];
var DEFAULT_GOLD = 1e4;
function mulberry32(seed) {
  let a = seed | 0;
  return () => {
    a = a + 1831565813 | 0;
    let t = Math.imul(a ^ a >>> 15, 1 | a);
    t = t + Math.imul(t ^ t >>> 7, 61 | t) ^ t;
    return ((t ^ t >>> 14) >>> 0) / 4294967296;
  };
}
function ri(rand, lo, hi) {
  return hi <= lo ? lo : lo + Math.floor(rand() * (hi - lo + 1));
}
function chance(rand, pct) {
  return Math.floor(rand() * 100) < pct;
}
function tsv(text) {
  return text.split(/\r?\n/).filter((l) => l.trim()).map((l) => l.split("\t").map((c) => c.trim().replace(/^"|"$/g, "")));
}
function num(s) {
  const v = parseInt(s, 10);
  return Number.isNaN(v) ? 0 : v;
}
function count(s) {
  return s === "" || s === "0" || s === "255" ? 0 : num(s);
}
function kindOfMisc(wtype) {
  return wtype === "hpot" || wtype === "rpot" ? "Potion" : "Misc";
}
function loadBases(text, kindOf, out) {
  const rows = tsv(text);
  if (!rows.length)
    return;
  const hdr = rows[0];
  const idx = {};
  hdr.forEach((h, i) => {
    idx[h.toLowerCase()] = i;
  });
  const need = ["code", "level", "spawnable", "cost", "gamble cost"];
  if (need.some((n) => idx[n] === undefined))
    return;
  for (const v of VENDOR_IDS) {
    for (const s of ["min", "max", "magicmin", "magicmax", "magiclvl"]) {
      const c = (v + s).toLowerCase();
      if (idx[c] === undefined)
        idx[c] = -1;
    }
  }
  const col = (r, name) => {
    const i = idx[name.toLowerCase()];
    return i !== undefined && i >= 0 ? r[i] ?? "" : "";
  };
  for (const r of rows.slice(1)) {
    const code = col(r, "code");
    if (!code)
      continue;
    const wtype = idx.type !== undefined ? r[idx.type] ?? "" : "";
    const stock = {};
    for (const v of VENDOR_IDS) {
      const max = count(col(r, v + "max"));
      const mmax = count(col(r, v + "magicmax"));
      if (max > 0 || mmax > 0) {
        stock[v] = {
          min: count(col(r, v + "min")),
          max,
          mmin: count(col(r, v + "magicmin")),
          mmax,
          mlvl: count(col(r, v + "magiclvl"))
        };
      }
    }
    const kind = kindOf(wtype);
    const permIdx = idx.permstoreitem;
    out.push({
      code,
      name: col(r, "name") || code,
      level: num(col(r, "level")),
      spawnable: col(r, "spawnable") === "1",
      cost: num(col(r, "cost")),
      gambleCost: num(col(r, "gamble cost")),
      kind,
      permanent: permIdx !== undefined && permIdx >= 0 && r[permIdx] === "1",
      stock,
      magic: kind === "Weapon" || kind === "Armor" || ["ring", "amul", "jewl", "ques"].includes(wtype)
    });
  }
}
function goldRange(id) {
  return id === "Gheed" ? GHEED_GOLD : [DEFAULT_GOLD, DEFAULT_GOLD];
}
function generate(catalog, id, vendorLevel, seed) {
  const rand = mulberry32(seed ^ 3366);
  const vendor = { id, name: id, level: vendorLevel, gold: 0, stock: [] };
  const [lo, hi] = goldRange(id);
  vendor.gold = ri(rand, lo, hi);
  for (const b of catalog) {
    const cnt = b.stock[id];
    if (!cnt)
      continue;
    if (cnt.max > 0) {
      const n = ri(rand, Math.min(cnt.min, cnt.max), cnt.max);
      for (let i = 0;i < n; i++) {
        if (!b.permanent && !chance(rand, SPAWN_PCT))
          continue;
        vendor.stock.push({
          code: b.code,
          name: b.name,
          kind: b.kind,
          quality: "Normal",
          buy: b.cost,
          sell: Math.floor(b.cost / 4),
          permanent: b.permanent
        });
      }
    }
    if (cnt.mmax > 0 && b.magic) {
      const n = ri(rand, Math.min(cnt.mmin, cnt.mmax), cnt.mmax);
      for (let i = 0;i < n; i++) {
        if (!b.permanent && !chance(rand, SPAWN_PCT))
          continue;
        vendor.stock.push({
          code: b.code,
          name: b.name + " (magique)",
          kind: b.kind,
          quality: "Magic",
          buy: b.cost,
          sell: Math.floor(b.cost / 4),
          permanent: b.permanent
        });
      }
    }
  }
  return vendor;
}
function gambleList(gambleCodes, byCode, odds, vendorLevel, clvl, seed) {
  const rand = mulberry32((seed ^ 27163) >>> 0);
  const pool = gambleCodes.map((c) => byCode[c]).filter((b) => b && b.spawnable && b.level <= vendorLevel);
  if (!pool.length)
    return [];
  const wU = odds.unique + odds.uber + odds.ultra;
  const wS = odds.set;
  const wR = odds.rare;
  const wM = odds.rare;
  const total = wU + wS + wR + wM;
  const out = [];
  for (let s = 0;s < GAMBLE_SLOTS; s++) {
    const b = pool[Math.floor(rand() * pool.length)];
    const roll = Math.floor(rand() * total);
    let q = roll < wU ? "Unique" : roll < wU + wS ? "Set" : roll < wU + wS + wR ? "Rare" : "Magic";
    if (q !== "Magic" && q !== "Rare" && !b.magic)
      q = "Normal";
    const price = Math.floor(b.gambleCost * (100 + 2 * Math.min(clvl, 50)) / 100);
    out.push({
      code: b.code,
      name: q === "Normal" ? b.name : b.name + " (?)",
      kind: b.kind,
      quality: q,
      buy: price,
      sell: Math.floor(b.cost / 4),
      permanent: false,
      gamble: true
    });
  }
  return out;
}
function sellPrice(cost) {
  return Math.floor(cost / 4);
}
function defaultHooks(win) {
  let localGold = null;
  const snapshot = () => {
    try {
      return win.__d2 && win.__d2.game ? JSON.parse(win.__d2.game.heroStats()).gold ?? 500 : 500;
    } catch {
      return 500;
    }
  };
  const getGold = () => localGold === null ? snapshot() : localGold;
  return {
    getGold,
    spend(n) {
      const g = getGold();
      if (g < n)
        return false;
      localGold = g - n;
      return true;
    },
    earn(n) {
      localGold = getGold() + n;
    },
    inventory() {
      try {
        return win.__d2?.invCache ?? [];
      } catch {
        return [];
      }
    },
    heroLevel() {
      try {
        return win.__d2 && win.__d2.game ? JSON.parse(win.__d2.game.heroStats()).level ?? 1 : 1;
      } catch {
        return 1;
      }
    },
    simulated: true
  };
}
var KIND_COLORS = {
  Potion: "#b03a2e",
  Weapon: "#8d99a6",
  Armor: "#8a6d3b",
  Gold: "#d9b96a",
  Misc: "#6e7a5e"
};
function installTsVendor(win) {
  registerModule("vendor", "ts");
  const ROOT = new URL("..", location.href);
  const catalog = [];
  const byCode = {};
  let gambleCodes = [];
  let odds = null;
  let seed = 210;
  let hooks = null;
  let logFn = (m) => {
    const s = document.getElementById("status");
    if (s)
      s.textContent = s.textContent.replace(/ · \d+ fps.*$/, "") + " — " + m;
  };
  let vendor = null;
  let vendorGamble = null;
  let vendorGambleLevel = -1;
  function build(done) {
    const files = [
      "patch_weapons.txt",
      "patch_armor.txt",
      "patch_misc.txt",
      "patch_gamble.txt",
      "patch_difficultylevels.txt"
    ];
    Promise.all(files.map((f) => fetch(new URL("assets/" + f, ROOT)).then((r) => {
      if (!r.ok)
        throw new Error(f + " HTTP " + r.status);
      return r.text();
    }))).then((texts) => {
      loadBases(texts[0], () => "Weapon", catalog);
      loadBases(texts[1], () => "Armor", catalog);
      loadBases(texts[2], kindOfMisc, catalog);
      for (const b of catalog)
        byCode[b.code] = b;
      gambleCodes = tsv(texts[3]).slice(1).map((r) => r[1] ?? "").filter((c) => c && byCode[c]);
      const d = tsv(texts[4]);
      const hdr = (d[0] ?? []).map((h) => h.toLowerCase());
      const row = d[1] ?? [];
      const at = (name) => num(row[hdr.indexOf(name)] ?? "0");
      odds = {
        rare: at("gamblerare"),
        set: at("gambleset"),
        unique: at("gambleunique"),
        uber: at("gambleuber"),
        ultra: at("gambleultra")
      };
      done();
    }).catch((e) => {
      console.info("[vendor] tables indisponibles :", e instanceof Error ? e.message : e);
      logFn("Tables marchandes indisponibles (" + (e instanceof Error ? e.message : e) + ").");
    });
  }
  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls)
      e.className = cls;
    if (text !== undefined)
      e.textContent = text;
    return e;
  }
  function init(dom, h) {
    hooks = h ?? defaultHooks(win);
    const toolbar = dom && dom.toolbar || document.getElementById("toolbar");
    if (dom?.log)
      logFn = dom.log;
    if (!toolbar)
      return;
    const openBtn = el("button", undefined, "Marchands");
    openBtn.id = "btn-vendors";
    openBtn.title = "Achat / vente / gamble (domaine G)";
    toolbar.appendChild(openBtn);
    const css = el("style");
    css.textContent = "#d2vendor{position:fixed;right:10px;top:64px;width:330px;max-height:70vh;overflow:auto;" + "background:#141019f2;border:1px solid #5a4a2a;border-radius:4px;padding:8px;z-index:50;display:none}" + "#d2vendor h3{margin:0 0 6px;color:#c7a45a;font:600 12px system-ui;letter-spacing:.08em}" + "#d2vendor .tabs{display:flex;gap:4px;margin-bottom:6px}" + "#d2vendor .tabs button{padding:3px 8px;font-size:11px}" + "#d2vendor .tabs button.on{background:#5a4a2a;color:#ffe9b0}" + "#d2vendor .stock{display:grid;grid-template-columns:1fr 1fr;gap:4px}" + "#d2vendor .it{display:flex;justify-content:space-between;gap:6px;background:#1d1712;" + "border:1px solid #3a3226;padding:4px 6px;font:11px system-ui;cursor:pointer;border-radius:2px}" + "#d2vendor .it:hover{border-color:#c7a45a}" + "#d2vendor .it.na{opacity:.45;cursor:not-allowed}" + "#d2vendor .it .p{color:#c7a45a;font-weight:600;white-space:nowrap}" + "#d2vendor .q-Magic{color:#7f8fe0}.q-Rare{color:#e0c14f}.q-Set{color:#4fd07a}.q-Unique{color:#c99e3f}" + "#d2vendor .foot{margin-top:6px;font:10px system-ui;color:#8f8676}";
    document.head.appendChild(css);
    const panel = el("div");
    panel.id = "d2vendor";
    document.body.appendChild(panel);
    let tab = "Akara";
    let invMode = false;
    function tryBuy(stock, i, price, gamble) {
      const hh = hooks;
      if (hh.getGold() < price) {
        logFn("Pas assez d’or (" + price + " requis).");
        return;
      }
      if (!hh.spend(price)) {
        logFn("Achat refusé par l’intégration.");
        return;
      }
      stock.splice(i, 1);
      logFn((gamble ? "Gamble acheté et révélé" : "Acheté") + " pour " + price + " or.");
      render();
    }
    function render() {
      if (!catalog.length) {
        panel.innerHTML = "<h3>MARCHANDS</h3><div>Chargement des tables…</div>";
        return;
      }
      panel.innerHTML = "";
      const hh = hooks;
      const clvl = hh.heroLevel ? hh.heroLevel() : 1;
      const title = el("h3", undefined, (invMode ? "VENDRE" : tab.toUpperCase() + " — MAGASIN") + " · or : " + hh.getGold() + (hh.simulated ? " (simulation)" : ""));
      panel.appendChild(title);
      const tabs = el("div", "tabs");
      for (const t of [...VENDOR_IDS, "Gamble"]) {
        const b = el("button", t === tab && !invMode ? "on" : undefined, t === "Gamble" ? "Gamble (Gheed)" : t);
        b.addEventListener("click", () => {
          tab = t;
          invMode = false;
          render();
        });
        tabs.appendChild(b);
      }
      const sell = el("button", invMode ? "on" : undefined, "Vendre");
      sell.addEventListener("click", () => {
        invMode = !invMode;
        render();
      });
      const rs = el("button", undefined, "Restock");
      rs.title = "Rafraîchit le stock (nouveau seed dérivé — vendor.rs)";
      rs.addEventListener("click", () => {
        seed = seed * 1103515245 + 12345 >>> 0;
        vendor = null;
        render();
      });
      tabs.appendChild(sell);
      tabs.appendChild(rs);
      panel.appendChild(tabs);
      const grid = el("div", "stock");
      const gold = hh.getGold();
      if (invMode) {
        const items = hh.inventory && hh.inventory() || [];
        if (!items.length)
          panel.appendChild(el("div", "foot", "Inventaire vide (ou non connecté : hooks.inventory())."));
        items.forEach((it) => {
          const base = byCode[it.code];
          if (!base)
            return;
          const price = sellPrice(base.cost);
          const row = el("div", "it");
          row.appendChild(el("span", undefined, it.name || it.code));
          row.appendChild(el("span", "p", "+" + price));
          row.addEventListener("click", () => {
            const got = sellPrice(byCode[it.code].cost);
            hh.earn(got);
            logFn("Vendu " + (it.name || it.code) + " (+" + got + " or).");
            render();
          });
          grid.appendChild(row);
        });
      } else if (tab === "Gamble") {
        if (!vendorGamble || vendorGambleLevel !== clvl) {
          vendorGamble = odds ? gambleList(gambleCodes, byCode, odds, clvl, clvl, seed) : [];
          vendorGambleLevel = clvl;
        }
        vendorGamble.forEach((e2, i) => {
          const row = el("div", "it" + (gold < e2.buy ? " na" : ""));
          row.appendChild(el("span", "q-" + e2.quality, e2.name));
          row.appendChild(el("span", "p", String(e2.buy)));
          row.addEventListener("click", () => tryBuy(vendorGamble, i, e2.buy, true));
          grid.appendChild(row);
        });
        panel.appendChild(grid);
        panel.appendChild(el("div", "foot", "Gamble : items non identifiés — prix = gamble cost × (100+2·clvl)/100. " + "Acheté = révélé (affixes déjà roulés côté Rust)."));
        return;
      } else {
        if (!vendor || vendor.id !== tab)
          vendor = generate(catalog, tab, clvl, seed);
        vendor.stock.forEach((e2, i) => {
          const row = el("div", "it" + (gold < e2.buy ? " na" : ""));
          row.appendChild(el("span", "q-" + e2.quality, e2.name));
          row.appendChild(el("span", "p", e2.buy + (e2.permanent ? " ⚑" : "")));
          row.addEventListener("click", () => tryBuy(vendor.stock, i, e2.buy, false));
          grid.appendChild(row);
        });
        panel.appendChild(grid);
        panel.appendChild(el("div", "foot", "Réserve " + tab + " : " + vendor.gold + " or · prix = valeur tables v1.12 · " + "vente = valeur/4 · ⚑ permanent."));
      }
      panel.appendChild(grid);
    }
    openBtn.addEventListener("click", () => {
      panel.style.display = panel.style.display === "block" ? "none" : "block";
      render();
    });
    build(render);
    return { panel, render };
  }
  function auto() {
    if (document.getElementById("toolbar") && !document.getElementById("btn-vendors")) {
      init({ toolbar: document.getElementById("toolbar") });
    }
  }
  if (document.readyState === "loading")
    document.addEventListener("DOMContentLoaded", auto);
  else
    auto();
  win.D2Vendor = {
    init,
    generate: (id, vendorLevel, s) => generate(catalog, id, vendorLevel, s),
    gambleList: (vendorLevel, clvl, s) => odds ? gambleList(gambleCodes, byCode, odds, vendorLevel, clvl, s) : [],
    _state: {
      get catalog() {
        return catalog;
      },
      get odds() {
        return odds;
      },
      get gambleCodes() {
        return gambleCodes;
      }
    }
  };
}
function installVendor(win) {
  if (win.D2Vendor) {
    registerModule("vendor", "adopt");
    return;
  }
  installTsVendor(win);
}
if (typeof window !== "undefined")
  installVendor(window);

// src/skills-ui.ts
var PANEL_X = 320;
var PANEL_Y = 32;
var GRID_X0 = 265;
var GRID_Y0 = 27;
var DX = 69;
var DY = 68;
var ICON = 48;
var TAB_W = 93;
var TAB_Y = { 3: 138, 2: 245, 1: 352 };
var api2 = {
  ready: false,
  open: false,
  opts: null,
  panel: null,
  init(o) {
    return initSkills(o);
  },
  refresh() {
    render2();
  },
  toggle() {
    togglePanel();
  }
};
var state = { tab: 1, tree: null, iconsUrl: null, bgUrls: {}, palBytes: null, loadedClass: null, err: "" };
var ROOT_URL = typeof location !== "undefined" ? new URL("..", location.href) : null;
function injectStyle() {
  if (document.getElementById("d2-skills-style"))
    return;
  const st = document.createElement("style");
  st.id = "d2-skills-style";
  st.textContent = `
#d2-skills { position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%);
  width: 640px; height: 480px; max-height: none; overflow: visible; padding: 0;
  z-index: 45; background: rgba(6, 4, 2, .88);
  border: 1px solid #5a3c1e; box-shadow: 0 0 24px #000; font: 12px/1.35 serif;
  color: #e8dcbf; user-select: none; image-rendering: pixelated; }
#d2-skills * { box-sizing: content-box; }
#d2-skills .skt-bg { position: absolute; left: ${PANEL_X}px; top: ${PANEL_Y}px;
  width: 320px; height: 432px; background-size: 320px 432px; }
#d2-skills .skt-tabzone { position: absolute; width: ${TAB_W}px; height: 107px;
  cursor: pointer; z-index: 6; }
#d2-skills svg.skt-lines { position: absolute; left: 0; top: 0; width: 640px; height: 480px;
  pointer-events: none; z-index: 2; }
#d2-skills .skt-icon { position: absolute; width: ${ICON}px; height: ${ICON}px;
  cursor: pointer; z-index: 4; background-repeat: no-repeat;
  image-rendering: pixelated; }
#d2-skills .skt-icon.locked { filter: grayscale(1) brightness(.5); cursor: default; }
#d2-skills .skt-icon.unspent:hover { outline: 1px solid #e8dcbf; filter: brightness(1.15); }
#d2-skills .skt-lvl { position: absolute; left: 49px; top: -5px; font: bold 11px monospace;
  color: #fff; text-shadow: 1px 1px 0 #000; z-index: 5; }
#d2-skills .skt-points { position: absolute; left: 8px; top: 8px; color: #d9b96a;
  z-index: 6; font-weight: bold; }
#d2-skills .skt-heroname { position: absolute; left: 8px; top: 24px; color: #a89670; z-index: 6; }
#d2-skills .skt-tip { position: absolute; width: 240px; max-height: 300px; overflow-y: auto;
  background: rgba(10, 7, 3, .96); border: 1px solid #6a5228; padding: 8px 10px;
  z-index: 20; pointer-events: none; font-size: 11px; }
#d2-skills .skt-tip h4 { margin: 0 0 2px; color: #d9b96a; font-size: 13px; }
#d2-skills .skt-tip .t-desc { color: #8f9fd0; white-space: pre-line; margin-bottom: 4px; }
#d2-skills .skt-tip .t-dmg { color: #e8dcbf; }
#d2-skills .skt-tip .t-syn { color: #9ab09a; }
#d2-skills .skt-tip .t-meta { color: #a89670; margin-top: 4px; }
#d2-skills .skt-tip .t-lock { color: #b05a4a; margin-top: 4px; }
#d2-skills .skt-close { position: absolute; right: 6px; top: 6px; z-index: 7; cursor: pointer;
  background: #241c10; color: #e8dcbf; border: 1px solid #5a3c1e; width: 22px; height: 22px; }
#d2-skills .skt-hint { position: absolute; left: 8px; bottom: 6px; color: #7a6a4a;
  font-size: 10px; z-index: 6; }
#d2-skills .skt-err { position: absolute; left: 8px; top: 44px; color: #b05a4a; z-index: 6; }
`;
  document.head.appendChild(st);
}
async function fetchBytes(rel) {
  if (!ROOT_URL)
    throw new Error("pas de racine");
  const r = await fetch(new URL(rel, ROOT_URL));
  if (!r.ok)
    throw new Error(`fetch ${rel} → HTTP ${r.status}`);
  return new Uint8Array(await r.arrayBuffer());
}
function pngUrl(bytes) {
  const blob = new Blob([bytes], { type: "image/png" });
  return URL.createObjectURL(blob);
}
async function loadAssets(g, tree) {
  if (!state.palBytes)
    state.palBytes = await fetchBytes("assets/pal_act1.dat");
  const cls = tree.class;
  state.err = "";
  if (state.loadedClass !== cls) {
    state.loadedClass = cls;
    state.iconsUrl = null;
    state.bgUrls = {};
    try {
      const sheet = await fetchBytes(tree.iconFile ?? "");
      const png = g.skillIconsPng ? g.skillIconsPng(sheet, state.palBytes) : null;
      if (png && png.length)
        state.iconsUrl = pngUrl(png);
    } catch (e) {
      state.err = e instanceof Error ? e.message : String(e);
    }
    try {
      const bg = await fetchBytes(tree.bgFile ?? "");
      for (const tab of [0, 1, 2]) {
        const p = g.skillTreePanelPng ? g.skillTreePanelPng(bg, tab, state.palBytes) : null;
        if (p && p.length)
          state.bgUrls[tab] = pngUrl(p);
      }
    } catch (e) {
      state.err = e instanceof Error ? e.message : String(e);
    }
  }
}
function dmgText(s) {
  const parts = [];
  const [pn, px] = s.dmgPhys ?? [0, 0];
  if (px > 0)
    parts.push(`${pn}-${px} dégâts`);
  const [en, ex] = s.dmgElem ?? [0, 0];
  if (ex > 0) {
    const elName = s.elem ? ` ${s.elem}` : "";
    parts.push(`${en}-${ex}${elName}`);
    if ((s.elemLen ?? 0) > 0)
      parts.push(`${Math.round((s.elemLen ?? 0) / 25)}s`);
  }
  if (!parts.length && (s.srcDam ?? 0) > 0) {
    parts.push(`${Math.round((s.srcDam ?? 0) * 100 / 256)} % des dégâts de l'arme`);
  }
  return parts.join(", ");
}
function esc2(s) {
  return String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);
}
function tooltipHtml(s, tree) {
  const lvl = s.level || 0;
  let h = `<h4>${esc2(s.nameFr || s.code)}</h4>`;
  if (s.descFr)
    h += `<div class="t-desc">${esc2(s.descFr).replace(/\\n/g, `
`)}</div>`;
  const cur = dmgText(s);
  if (cur)
    h += `<div class="t-dmg">dégâts : ${esc2(cur)}${lvl ? ` (niv ${lvl})` : " (niv 1 prévu)"}</div>`;
  if ((s.mana ?? 0) > 0)
    h += `<div class="t-dmg">mana : ${s.mana}</div>`;
  const syn = (s.synergies || []).filter((x) => x.pctPerLvl || x.pctTotal);
  for (const x of syn) {
    h += `<div class="t-syn">+${x.pctPerLvl} %/niv ${esc2(x.nameFr || x.code)}` + (x.pctTotal ? ` — total +${x.pctTotal} %` : "") + "</div>";
  }
  if (s.dmgElemNext || s.dmgPhysNext) {
    const nx = dmgText({ dmgPhys: s.dmgPhysNext, dmgElem: s.dmgElemNext, elem: s.elem, elemLen: 0, srcDam: 0 });
    if (nx)
      h += `<div class="t-meta">niv ${lvl + 1} : ${esc2(nx)}</div>`;
  }
  if ((s.reqLevel ?? 1) > 1 && tree.charLevel < (s.reqLevel ?? 1)) {
    h += `<div class="t-lock">Niveau ${s.reqLevel} requis (héros niv ${tree.charLevel})</div>`;
  }
  for (const p of s.prereqs || []) {
    const known = (tree.skills || []).some((k) => k.code === p && k.level > 0);
    if (!known)
      h += `<div class="t-lock">Requiert : ${esc2(p)}</div>`;
  }
  return h;
}
function iconXY(s) {
  return [GRID_X0 + (s.column || 0) * DX, GRID_Y0 + (s.row || 0) * DY];
}
function render2() {
  if (!api2.open || !api2.panel)
    return;
  const g = api2.opts?.getGame?.() ?? null;
  if (!g || !g.skillTree)
    return;
  const t = parseOr(g.skillTree(), null);
  if (!t)
    return;
  state.tree = t;
  loadAssets(g, t).then(() => renderInto(t)).catch((e) => {
    state.err = String(e instanceof Error ? e.message : e);
    renderInto(t);
  });
}
function renderInto(t) {
  const root = api2.panel;
  if (!root)
    return;
  const bg = state.bgUrls[state.tab - 1];
  const bgEl = root.querySelector(".skt-bg");
  if (bg && bgEl)
    bgEl.style.backgroundImage = `url(${bg})`;
  const pointsEl = root.querySelector(".skt-points");
  if (pointsEl)
    pointsEl.textContent = `Points : ${t.points ?? 0}`;
  const heroEl = root.querySelector(".skt-heroname");
  if (heroEl)
    heroEl.textContent = `${t.hero ?? ""} — ${t.tabs ? t.tabs[state.tab - 1] || "" : ""}`;
  const errEl = root.querySelector(".skt-err");
  if (errEl) {
    errEl.textContent = state.err || (state.iconsUrl ? "" : state.loadedClass ? "icônes DC6 indisponibles (placeholder)" : "chargement…");
  }
  const skills = (t.skills || []).filter((s) => (s.page || 0) === state.tab);
  let lines = "";
  const pos = new Map;
  for (const s of skills)
    pos.set(s.code, iconXY(s));
  for (const s of skills) {
    for (const p of s.prereqs || []) {
      const a = pos.get(p);
      if (!a)
        continue;
      const [x1, y1] = a;
      const [x2, y2] = iconXY(s);
      lines += `<line x1="${x1 + ICON / 2}" y1="${y1 + ICON / 2}"` + ` x2="${x2 + ICON / 2}" y2="${y2 + ICON / 2}"` + ` stroke="rgba(210,190,150,.5)" stroke-width="2"/>`;
    }
  }
  const svg = root.querySelector("svg.skt-lines");
  if (svg)
    svg.innerHTML = lines;
  let html = "";
  for (const s of skills) {
    const [x, y] = iconXY(s);
    const known = (s.level || 0) > 0;
    const learnable = !!s.canLearn;
    const cls = "skt-icon" + (known ? "" : " locked") + (learnable ? " unspent" : "");
    const cel = Math.max(0, s.icon || 0);
    let style = `left:${x}px;top:${y}px;`;
    if (state.iconsUrl) {
      style += `background-image:url(${state.iconsUrl});background-position:-${cel * ICON}px 0;`;
    }
    html += `<div class="${cls}" data-code="${esc2(s.code)}" style="${style}">`;
    if (known)
      html += `<span class="skt-lvl">${s.level}</span>`;
    html += "</div>";
  }
  const holder = root.querySelector(".skt-icons");
  if (holder) {
    holder.innerHTML = html;
    for (const elm of holder.querySelectorAll(".skt-icon")) {
      elm.addEventListener("mouseenter", () => showTip(elm));
      elm.addEventListener("mouseleave", hideTip);
      elm.addEventListener("click", () => learn(elm.dataset.code ?? ""));
    }
  }
}
function showTip(elm) {
  const t = state.tree;
  if (!t || !api2.panel)
    return;
  const s = (t.skills || []).find((k) => k.code === elm.dataset.code);
  if (!s)
    return;
  const tip = api2.panel.querySelector(".skt-tip");
  if (!tip)
    return;
  tip.innerHTML = tooltipHtml(s, t);
  tip.style.display = "block";
  const [x, y] = iconXY(s);
  const tw = 240;
  tip.style.left = (x - tw - 8 >= 4 ? x - tw - 8 : x + ICON + 8) + "px";
  tip.style.top = Math.max(4, Math.min(480 - tip.offsetHeight - 4, y - 8)) + "px";
}
function hideTip() {
  const tip = api2.panel?.querySelector(".skt-tip");
  if (tip)
    tip.style.display = "none";
}
function learn(code) {
  const g = api2.opts?.getGame?.() ?? null;
  if (!g || !code)
    return;
  const t = state.tree;
  const s = t && (t.skills || []).find((k) => k.code === code);
  if (s && !(s.level > 0) && !s.canLearn)
    return;
  let r;
  try {
    r = parseOr(g.learnSkill(code), {});
  } catch (err) {
    api2.opts?.flash?.(`Erreur : ${err instanceof Error ? err.message : String(err)}`);
    return;
  }
  if (r.ok) {
    const nf = s && (s.nameFr || s.code);
    api2.opts?.flash?.(`${nf} niveau ${r.level}`);
    api2.opts?.note?.(`${nf} niveau ${r.level}`);
  } else {
    api2.opts?.flash?.(`Refusé : ${r.error}`);
  }
  render2();
}
function buildPanel2() {
  api2.panel = document.createElement("div");
  api2.panel.id = "d2-skills";
  api2.panel.hidden = true;
  api2.panel.innerHTML = `<div class="skt-bg"></div>` + `<svg class="skt-lines"></svg>` + `<div class="skt-icons"></div>` + `<div class="skt-points"></div>` + `<div class="skt-heroname"></div>` + `<div class="skt-err"></div>` + `<button class="skt-close" title="Fermer (T)">✕</button>` + `<div class="skt-hint">T : fermer · clic = dépenser 1 point · survol = détails</div>` + `<div class="skt-tip" style="display:none"></div>`;
  for (const page of [1, 2, 3]) {
    const z = document.createElement("div");
    z.className = "skt-tabzone";
    z.style.left = PANEL_X + 320 - TAB_W + "px";
    z.style.top = (TAB_Y[page] ?? 0) + "px";
    z.title = "Onglet " + page;
    z.addEventListener("click", () => {
      state.tab = page;
      render2();
    });
    api2.panel.appendChild(z);
  }
  document.body.appendChild(api2.panel);
  api2.panel.querySelector(".skt-close")?.addEventListener("click", togglePanel);
}
function togglePanel() {
  if (!api2.panel)
    return;
  api2.open = !api2.open;
  api2.panel.hidden = !api2.open;
  if (api2.open)
    render2();
  else
    hideTip();
}
function initSkills(o) {
  if (api2.ready)
    return api2;
  api2.opts = o;
  injectStyle();
  buildPanel2();
  document.addEventListener("keydown", (e) => {
    if (e.key === "t" || e.key === "T") {
      e.preventDefault();
      togglePanel();
    }
  });
  const btn = document.getElementById("btn-skills");
  if (btn)
    btn.addEventListener("click", togglePanel);
  api2.ready = true;
  return api2;
}
function installSkills(win) {
  registerModule("skills-ui", "ts");
  win.D2Skills = api2;
}
if (typeof window !== "undefined")
  installSkills(window);

// src/merc-ui.ts
var api3 = {
  ready: false,
  opts: null,
  card: null,
  lastPoll: 0,
  init(o) {
    return initMerc(o);
  },
  frame(game) {
    frameMerc(game);
  }
};
function mercStateLine(m) {
  return m.alive ? `${m.hp}/${m.maxHp} PV` : "KO — résurrection chez le NPC";
}
function initMerc(o) {
  if (api3.ready)
    return api3;
  api3.opts = o;
  const card = document.createElement("div");
  card.id = "d2-merc";
  card.style.cssText = "position:absolute; left:10px; top:34px; z-index:25;" + "min-width:150px; font:11px monospace; color:#d0b070;" + "background:rgba(16,12,6,.85); border:1px solid #5a3c1e; padding:4px 8px; display:none;";
  card.innerHTML = `<div id="merc-name">—</div>` + `<div style="height:5px; background:#241c10; border:1px solid #5a3c1e; margin-top:3px;">` + `<div id="merc-hp" style="height:100%; width:0%; background:#8c2b1e;"></div></div>` + `<div id="merc-state" style="color:#7a6a4a;"></div>`;
  document.body.appendChild(card);
  api3.card = card;
  api3.ready = true;
  return api3;
}
function frameMerc(game) {
  if (!api3.card || !game || !game.mercInfo)
    return;
  let m = null;
  try {
    m = parseOr(game.mercInfo(), null);
  } catch {
    m = null;
  }
  if (!m) {
    api3.card.style.display = "none";
    return;
  }
  api3.card.style.display = "block";
  const name = api3.card.querySelector("#merc-name");
  if (name)
    name.textContent = `${m.kind || "Mercenaire"} · niv ${m.level ?? "?"}`;
  const pct = m.maxHp > 0 ? Math.round(100 * m.hp / m.maxHp) : 0;
  const bar = api3.card.querySelector("#merc-hp");
  if (bar) {
    bar.style.width = `${pct}%`;
    bar.style.background = m.alive ? "#8c2b1e" : "#333";
  }
  const state = api3.card.querySelector("#merc-state");
  if (state)
    state.textContent = mercStateLine(m);
}
function installMerc(win) {
  registerModule("merc-ui", "ts");
  win.D2Merc = api3;
}
if (typeof window !== "undefined")
  installMerc(window);

// src/d2s-io.ts
var opts = null;
function game() {
  return opts && opts.getGame ? opts.getGame() : null;
}
function sanitizeHeroName(raw) {
  const clean = String(raw || "").replace(/[^a-zA-Z0-9]+/g, "").slice(0, 15);
  return clean || "heros";
}
function heroNameOf(g) {
  try {
    const hc = parseOr(g ? g.heroClass() : "{}", {});
    return sanitizeHeroName(hc.name || hc.code);
  } catch {
    return "heros";
  }
}
function importMessage(report, fileName) {
  const where = report.classChanged ? ` — classe ${report.class} (les sprites suivent au prochain lancement ?hero=${report.class})` : "";
  return `Héros importé : ${report.name || fileName} — ${report.class} niv. ${report.level}` + `, ${report.items_loaded} objet(s), ${report.waypoints} waypoint(s)${where}.`;
}
function downloadBytes(bytes, fileName) {
  const blob = new Blob([bytes], { type: "application/octet-stream" });
  const a = document.createElement("a");
  a.href = URL.createObjectURL(blob);
  a.download = fileName;
  a.click();
  setTimeout(() => URL.revokeObjectURL(a.href), 2000);
}
function exportD2s() {
  const g = game();
  if (!g || typeof g.exportD2s !== "function") {
    if (opts)
      opts.note?.("Export .d2s indisponible (moteur sans exportD2s).");
    return;
  }
  try {
    const name = heroNameOf(g);
    const bytes = g.exportD2s(name);
    downloadBytes(bytes, `${name}.d2s`);
    opts?.flash?.(`Héros exporté : ${name}.d2s (format .d2s v1.12, checksum valide).`);
  } catch (e) {
    opts?.flash?.(`Export .d2s impossible : ${e instanceof Error ? e.message : String(e)}`);
  }
}
function importD2s(file) {
  const g = game();
  if (!g || typeof g.importD2s !== "function") {
    if (opts)
      opts.note?.("Import .d2s indisponible (moteur sans importD2s).");
    return;
  }
  file.arrayBuffer().then((buf) => {
    const report = parseOr(g.importD2s(new Uint8Array(buf)), {});
    opts?.flash?.(importMessage(report, file.name));
    if (opts?.onImported)
      opts.onImported(report);
  }).catch((e) => {
    opts?.flash?.(`Import .d2s refusé : ${e instanceof Error ? e.message : String(e)}`);
  });
}
function initD2sIO(o) {
  opts = o || {};
  const btnExport = document.getElementById("btn-export-d2s");
  const btnImport = document.getElementById("btn-import-d2s");
  const fileInput = document.getElementById("file-d2s");
  if (!btnExport || !btnImport || !fileInput)
    return;
  btnExport.addEventListener("click", exportD2s);
  btnImport.addEventListener("click", () => fileInput.click());
  fileInput.addEventListener("change", () => {
    const input = fileInput;
    const f = input.files && input.files[0];
    if (f)
      importD2s(f);
    input.value = "";
  });
}
function installD2sIO(win) {
  registerModule("d2s-io", "ts");
  win.D2sIO = { init: initD2sIO, exportD2s, importD2s };
}
if (typeof window !== "undefined")
  installD2sIO(window);

// src/quests.ts
var DEFS = [
  {
    id: "a1q1",
    act: 1,
    name: "Den of Evil",
    gates_act: false,
    summary: "Purifiez la Tanière du Mal qui souille la forêt du Camp des Rousses.",
    steps: [
      { desc: "Entrez dans la Tanière du Mal (Act 1 - Cave 1).", trigger: { ZoneEntered: { level_id: 8 } }, count: 1 },
      { desc: "Purifiez la Tanière : abattez ses monstres.", trigger: { MonsterKilled: { code: "*" } }, count: 6 }
    ],
    rewards: [{ Unlock: { flag: "skill_point" } }]
  },
  {
    id: "a1q2",
    act: 1,
    name: "Sisters' Burial Grounds",
    gates_act: false,
    summary: "Rendez le repos à Blood Raven, qui corrompt le cimetière des Blood Moor.",
    steps: [
      { desc: "Abattez Blood Raven au cimetière des Blood Moor.", trigger: { BossDead: { code: "bloodraven" } }, count: 1 }
    ],
    rewards: [{ Unlock: { flag: "rogue_scout" } }]
  },
  {
    id: "a1q3",
    act: 1,
    name: "The Search for Cain",
    gates_act: false,
    summary: "Découvrez le sort du dernier Horadrim : l'Arbre d'Inifuss garde la clé des pierres de Cairn.",
    steps: [
      { desc: "Récoltez l'écorce sur l'Arbre d'Inifuss (Dark Wood).", trigger: { ItemPickedUp: { code: "bks" } }, count: 1 },
      { desc: "Faites déchiffrer le parchemin par Akara.", trigger: { NpcTalked: { npc: "akara" } }, count: 1 },
      { desc: "Ouvrez le portail et entrez à Tristram.", trigger: { ZoneEntered: { level_id: 38 } }, count: 1 },
      { desc: "Délivrez Deckard Cain de sa cage.", trigger: { NpcTalked: { npc: "cain" } }, count: 1 }
    ],
    rewards: [{ Item: { code: "rin" } }, { Unlock: { flag: "cain_free_id" } }]
  },
  {
    id: "a1q4",
    act: 1,
    name: "Tools of the Trade",
    gates_act: false,
    summary: "Reprenez le marteau Horadrique aux mains du Smith, dans la Caserne du monastère.",
    steps: [
      { desc: "Abattez le Smith dans la Caserne.", trigger: { BossDead: { code: "smith" } }, count: 1 },
      { desc: "Ramassez le Horadric Malus (hamm).", trigger: { ItemPickedUp: { code: "hamm" } }, count: 1 },
      { desc: "Rapportez le Malus à Charsi.", trigger: { NpcTalked: { npc: "charsi" } }, count: 1 }
    ],
    rewards: [{ Unlock: { flag: "charsi_imbue" } }]
  },
  {
    id: "a1q5",
    act: 1,
    name: "Sisters to the Slaughter",
    gates_act: true,
    summary: "Descendez les Catacombes et abattez Andariel pour venger les Sœurs des Yeux Aveugles.",
    steps: [
      { desc: "Abattez Andariel, au fond des Catacombes.", trigger: { BossDead: { code: "andariel" } }, count: 1 }
    ],
    rewards: []
  }
];
var STATE_VERSION = 1;
var STATE_KEYS = ["not_started", "in_progress", "done", "failed"];
var EVENT_KINDS = ["MonsterKilled", "ItemPickedUp", "ZoneEntered", "NpcTalked", "BossDead"];
function normalizeEvent(ev) {
  if (!ev || typeof ev !== "object")
    return null;
  const o = ev;
  const keys = Object.keys(o);
  if (keys.length === 1 && EVENT_KINDS.includes(keys[0])) {
    const kind = keys[0];
    const obj = o[kind];
    const rest = obj && typeof obj === "object" ? obj : {};
    return { kind, ...rest };
  }
  if (typeof o.type === "string" && EVENT_KINDS.includes(o.type)) {
    const { type, ...rest } = o;
    return { kind: type, ...rest };
  }
  return null;
}
function triggerMatches(trigger, kind, args) {
  const kinds = Object.keys(trigger);
  if (kinds.length !== 1)
    return false;
  const key = kinds[0];
  const want = trigger[key] ?? {};
  switch (key) {
    case "MonsterKilled":
      if (kind !== "MonsterKilled")
        return false;
      return want.code === "*" || want.code === args.code;
    case "BossDead":
      if (kind !== "BossDead" && kind !== "MonsterKilled")
        return false;
      return want.code === args.code;
    case "ItemPickedUp":
      return kind === "ItemPickedUp" && want.code === args.code;
    case "ZoneEntered":
      return kind === "ZoneEntered" && want.level_id === args.level_id;
    case "NpcTalked":
      return kind === "NpcTalked" && want.npc === args.npc;
    default:
      return false;
  }
}

class QuestLog {
  defs;
  runs;
  constructor(defs) {
    this.defs = defs ?? structuredClone(DEFS);
    this.runs = this.defs.map(() => ({ state: "not_started", step: 0, count: 0 }));
  }
  index(id) {
    return this.defs.findIndex((d) => d.id === id);
  }
  state(id) {
    const i = this.index(id);
    return i < 0 ? null : this.runs[i].state;
  }
  states() {
    return this.defs.map((d, i) => [d.id, this.runs[i].state]);
  }
  progress(id) {
    const i = this.index(id);
    if (i < 0)
      return null;
    const d = this.defs[i];
    const r = this.runs[i];
    if (r.state === "in_progress") {
      const st = d.steps[r.step];
      return { state: r.state, step: r.step, desc: st.desc, count: r.count, required: st.count };
    }
    if (r.state === "done") {
      const last = d.steps[d.steps.length - 1];
      return { state: r.state, step: d.steps.length - 1, desc: last.desc, count: last.count, required: last.count };
    }
    return { state: r.state, step: r.step, desc: "", count: 0, required: 0 };
  }
  actDone(act) {
    const gates = this.defs.map((d, i) => ({ d, r: this.runs[i] })).filter(({ d }) => d.act === act && d.gates_act);
    return gates.length > 0 && gates.every(({ r }) => r.state === "done");
  }
  fail(id) {
    const i = this.index(id);
    if (i < 0 || this.runs[i].state !== "in_progress")
      return false;
    this.runs[i].state = "failed";
    return true;
  }
  onEvent(event) {
    const norm = normalizeEvent(event);
    if (!norm)
      return [];
    const { kind, ...args } = norm;
    const rewards = [];
    const actsCompleted = [];
    for (let i = 0;i < this.defs.length; i++) {
      const d = this.defs[i];
      const r = this.runs[i];
      if (r.state !== "not_started" && r.state !== "in_progress")
        continue;
      const step = r.step;
      const st = d.steps[step];
      if (!st || !triggerMatches(st.trigger, kind, args))
        continue;
      const required = Math.max(1, st.count);
      if (r.state === "not_started")
        r.state = "in_progress";
      r.count += 1;
      if (r.count < required)
        continue;
      if (step + 1 < d.steps.length) {
        r.step = step + 1;
        r.count = 0;
        continue;
      }
      r.step = d.steps.length - 1;
      r.count = Math.max(1, d.steps[d.steps.length - 1].count);
      r.state = "done";
      rewards.push(...structuredClone(d.rewards));
      if (d.gates_act && !actsCompleted.includes(d.act) && this.actDone(d.act)) {
        actsCompleted.push(d.act);
      }
    }
    for (const act of actsCompleted)
      rewards.push({ Unlock: { flag: `act${act}_done` } });
    return rewards;
  }
  to_json() {
    return JSON.stringify({
      version: STATE_VERSION,
      quests: this.defs.map((d, i) => ({ id: d.id, ...this.runs[i] }))
    });
  }
  static from_json(json) {
    const s = JSON.parse(json);
    const o = s;
    if (o.version !== STATE_VERSION || !Array.isArray(o.quests)) {
      throw new Error("quests: version inconnue");
    }
    const log = new QuestLog;
    if (o.quests.length !== log.defs.length)
      throw new Error("quests: nombre de quêtes");
    o.quests.forEach((qRaw, i) => {
      const q = qRaw;
      const d = log.defs[i];
      if (q.id !== d.id)
        throw new Error(`quests: id ${q.id} ≠ ${d.id}`);
      if (!STATE_KEYS.includes(q.state))
        throw new Error(`quests: état ${q.state}`);
      const last = d.steps.length - 1;
      const req = Math.max(1, d.steps[q.step ?? 0] ? d.steps[q.step ?? 0].count : 1);
      const lastReq = Math.max(1, d.steps[last].count);
      const step = q.step ?? 0;
      const count = q.count ?? 0;
      const state = q.state;
      const ok = state === "not_started" && step === 0 && count === 0 || state === "in_progress" && step <= last && count < req || state === "done" && step === last && count === lastReq || state === "failed" && step <= last && count <= req;
      if (!ok)
        throw new Error(`quests: état incohérent pour ${q.id}`);
      log.runs[i] = { state, step, count };
    });
    return log;
  }
}
var CSS = `
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
var STATUS_FR = {
  not_started: "Non commencée",
  in_progress: "En cours",
  done: "Terminée",
  failed: "Échouée"
};
var doc = null;
var panel2 = null;
var listEl = null;
var log = new QuestLog;
var isOpen = false;
function render3() {
  if (!listEl || !doc)
    return;
  listEl.textContent = "";
  for (const d of log.defs) {
    const p = log.progress(d.id);
    const li = doc.createElement("li");
    li.className = p.state;
    const head = doc.createElement("div");
    const status = doc.createElement("span");
    status.className = "d2q-status";
    status.textContent = p.state === "done" ? `✔ ${STATUS_FR[p.state]}` : STATUS_FR[p.state];
    const name = doc.createElement("span");
    name.className = "d2q-name";
    const num = parseInt(d.id.slice(3), 10) || 0;
    name.textContent = `${num}. ${d.name}`;
    head.append(status, name);
    li.appendChild(head);
    if (p.state === "in_progress") {
      const obj = doc.createElement("div");
      obj.className = "d2q-obj";
      obj.append(doc.createTextNode(p.desc));
      if (p.required > 1) {
        const prog = doc.createElement("span");
        prog.className = "d2q-prog";
        prog.textContent = ` (${p.count}/${p.required})`;
        obj.appendChild(prog);
      }
      li.appendChild(obj);
    } else if (p.state === "done") {
      const sum = doc.createElement("div");
      sum.className = "d2q-sum";
      const def = log.defs.find((x) => x.id === d.id);
      sum.textContent = def.rewards.length ? "Récompense obtenue." : "Acte débloqué.";
      li.appendChild(sum);
    } else if (p.state === "not_started") {
      const sum = doc.createElement("div");
      sum.className = "d2q-sum";
      sum.textContent = d.summary;
      li.appendChild(sum);
    }
    listEl.appendChild(li);
  }
}
function setOpen(open) {
  isOpen = open;
  if (panel2)
    panel2.hidden = !open;
  if (open)
    render3();
}
var questsApi = {
  QuestLog,
  DEFS,
  init(dom) {
    doc = dom ?? (typeof document !== "undefined" ? document : null);
    if (!doc || panel2)
      return questsApi;
    const style = doc.createElement("style");
    style.textContent = CSS;
    doc.head.appendChild(style);
    panel2 = doc.createElement("div");
    panel2.id = "d2-quests";
    panel2.hidden = true;
    const title = doc.createElement("h2");
    title.textContent = "Journal de quêtes";
    const act = doc.createElement("div");
    act.className = "d2q-act";
    act.textContent = "Acte I — Camp des Rousses";
    listEl = doc.createElement("ul");
    const hint = doc.createElement("div");
    hint.className = "d2q-hint";
    hint.textContent = "Q — ouvrir / fermer";
    panel2.append(title, act, listEl, hint);
    doc.body.appendChild(panel2);
    doc.addEventListener("keydown", (e) => {
      if (e.repeat)
        return;
      const t = e.target;
      if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable))
        return;
      if (e.key === "q" || e.key === "Q") {
        setOpen(!isOpen);
        e.preventDefault();
      }
    });
    return questsApi;
  },
  feed(event) {
    const rewards = log.onEvent(event);
    if (isOpen)
      render3();
    return rewards;
  },
  get log() {
    return log;
  },
  reset() {
    log = new QuestLog;
    if (isOpen)
      render3();
  },
  exportLog() {
    return log.to_json();
  },
  importLog(json) {
    log = QuestLog.from_json(json);
    if (isOpen)
      render3();
    return true;
  },
  show() {
    setOpen(true);
  },
  hide() {
    setOpen(false);
  },
  toggle() {
    setOpen(!isOpen);
  },
  get isOpen() {
    return isOpen;
  }
};
function installQuests(win) {
  registerModule("quests", "ts");
  win.D2Quests = questsApi;
}
if (typeof window !== "undefined")
  installQuests(window);

// src/quests45.ts
var DEFS45 = [
  {
    id: "a4q1",
    act: 4,
    name: "The Fallen Angel",
    gates: "act5",
    steps: [
      { desc: "Traversez les Plaines du Désespoir.", zone: 105 },
      { desc: "Abattez Izual pour libérer son âme.", kill: ["izual"] }
    ],
    npc: "tyrael"
  },
  {
    id: "a4q2",
    act: 4,
    name: "Terror's End",
    gates: "act5",
    steps: [
      { desc: "Trouvez Diablo dans son sanctuaire (Chaos Sanctuaire).", zone: 108 },
      {
        desc: "Brisez les sceaux et abattez leurs gardiens.",
        any: [{ kill: ["infector of souls", "megademon3"] }, { kill: ["lord de seis", "doomknight3"] }, { kill: ["grand vizier of chaos", "fingermage3"] }]
      },
      { desc: "Tuez Diablo.", kill: ["diablo"] }
    ],
    npc: "tyrael"
  },
  {
    id: "a4q3",
    act: 4,
    name: "Hell's Forge",
    steps: [
      { desc: "Portez la Pierre d'âme de Mephisto.", item: "mss" },
      { desc: "Descendez dans la Rivière de feu.", zone: 107 },
      { desc: "Abattez Hephasto le forgeron.", kill: ["hephasto", "The Feature Creep"] },
      { desc: "Emparez-vous du Marteau de la Forge infernale.", item: "hfh" }
    ],
    npc: "cain"
  },
  {
    id: "a5q1",
    act: 5,
    name: "Siege on Harrogath",
    steps: [
      { desc: "Percez les Contreforts sanglants et levez le siège.", zone: 110 },
      { desc: "Abattez Shenk le Surveillant (Siege Boss).", kill: ["siegeboss", "Siege Boss", "overseer1", "shenk"] }
    ],
    npc: "larzuk"
  },
  {
    id: "a5q2",
    act: 5,
    name: "Rescue on Mount Arreat",
    steps: [
      {
        desc: "Libérez les barbares prisonniers des Hautes-Glaces (4 extérieurs).",
        any: [{ zone: 110 }, { zone: 111 }, { zone: 112 }, { zone: 117 }],
        count: 4
      }
    ],
    npc: "qual-kehk"
  },
  {
    id: "a5q3",
    act: 5,
    name: "Prison of Ice",
    steps: [
      { desc: "Trouvez Anya gelée sous le Passage Cristallin (Ice Cave 3A).", zone: 119 },
      { desc: "Abattez Frozenstein qui garde la prison de glace.", kill: ["frozenstein", "snowyeti4", "Frozenstein"] },
      { desc: "Donnez à Anya la potion de Malah et ramenez-la en ville.", talk: "anya" }
    ],
    npc: "malah"
  },
  {
    id: "a5q4",
    act: 5,
    name: "Betrayal of Harrogath",
    requires: "a5q3",
    steps: [
      { desc: "Prenez le portail d'Anya vers le temple de Nihlathak.", talk: "anya" },
      { desc: "Descendez jusqu’au Repaire de Vaught (Halls of Vaught).", zone: 124 },
      { desc: "Tuez Nihlathak le traître.", kill: ["nihlathak", "nihlathakboss", "Nihlathak Boss"] }
    ],
    npc: "anya"
  },
  {
    id: "a5q5",
    act: 5,
    name: "Rite of Passage",
    steps: [
      { desc: "Consultez les Anciens en activant l'Autel des Cieux (Sommet d'Arreat).", zone: 120 },
      {
        desc: "Affrontez les trois Anciens sans quitter le Mont Arreat.",
        any: [{ kill: ["ancientbarb1", "Ancient Barbarian 1"] }, { kill: ["ancientbarb2", "Ancient Barbarian 2"] }, { kill: ["ancientbarb3", "Ancient Barbarian 3"] }]
      }
    ],
    npc: null
  },
  {
    id: "a5q6",
    act: 5,
    name: "Eve of Destruction",
    requires: "a5q5",
    final: true,
    steps: [
      { desc: "Affrontez les vagues de Baal dans le Trône de la Destruction.", zone: 131 },
      { desc: "Tuez Baal dans la Chambre de la Pierre d'Worldstone.", kill: ["baal", "baalcrab"] },
      { desc: "Parlez à Tyrael et prenez son portail.", talk: "tyrael" }
    ],
    npc: null
  }
];
var ACT_TITLES = {
  4: "Acte IV — la Forteresse de l’Enfer",
  5: "Acte V — Harrogath (fin de jeu)"
};
var STATE_VERSION_45 = 1;
function normalizeActEvent(event) {
  if (!event || typeof event !== "object")
    return null;
  const keys = Object.keys(event);
  if (!keys.length)
    return null;
  const kind = keys[0];
  const rest = event[kind];
  const payload = rest && typeof rest === "object" ? rest : {};
  return { kind, ...payload };
}
function trigMatches(t, ev) {
  if (t.kill) {
    if (ev.kind !== "MonsterKilled")
      return false;
    return t.kill.some((a) => a.toLowerCase() === (ev.code || "").toLowerCase() || a.toLowerCase() === (ev.superunique || "").toLowerCase());
  }
  if (t.zone !== undefined)
    return ev.kind === "ZoneEntered" && ev.level_id === t.zone;
  if (t.item !== undefined)
    return ev.kind === "ItemPicked" && String(ev.code).toLowerCase() === t.item.toLowerCase();
  if (t.talk !== undefined)
    return ev.kind === "NpcTalked" && String(ev.npc).toLowerCase() === t.talk.toLowerCase();
  return false;
}
function stepOf(def, r) {
  return r.step < def.steps.length ? def.steps[r.step] : null;
}

class Quests45Log {
  runs;
  constructor() {
    this.runs = DEFS45.map(() => ({ id: null, phase: "inactive", step: 0, count: 0, flags: 0, subdone: 0 }));
  }
  index(id) {
    return DEFS45.findIndex((d) => d.id === id);
  }
  phase(id) {
    const i = this.index(id);
    return i < 0 ? null : this.runs[i].phase;
  }
  done(id) {
    return this.phase(id) === "done";
  }
  gateOpen(i) {
    const d = DEFS45[i];
    if (d.requires && !this.done(d.requires))
      return false;
    return true;
  }
  onEvent(event) {
    const ev = normalizeActEvent(event);
    if (!ev)
      return [];
    const out = [];
    for (let i = 0;i < DEFS45.length; i++) {
      const d = DEFS45[i];
      const r = this.runs[i];
      if (r.phase === "done")
        continue;
      if (r.phase === "reward_pending") {
        if (ev.kind === "NpcTalked" && d.npc && String(ev.npc).toLowerCase() === d.npc.toLowerCase()) {
          r.phase = "done";
          r.flags = r.flags & ~2 | 1;
          if (d.id === "a5q3")
            r.flags |= 128;
          out.push({ id: d.id, phase: "done" });
        }
        continue;
      }
      if (!this.gateOpen(i))
        continue;
      const st = stepOf(d, r);
      if (!st)
        continue;
      let hit = -1;
      if (st.any) {
        hit = st.any.findIndex((t, j) => !(r.subdone >> j & 1) && trigMatches({ desc: "", ...t }, ev));
        if (hit < 0)
          continue;
        r.subdone |= 1 << hit;
      } else if (!trigMatches(st, ev)) {
        continue;
      }
      if (r.phase === "inactive") {
        r.phase = "active";
        if (ev.kind === "NpcTalked")
          r.flags |= 4;
        out.push({ id: d.id, phase: "active" });
      }
      r.count += 1;
      const req = Math.max(1, st.count ?? (st.any ? st.any.length : 1));
      if (r.count >= req) {
        r.count = 0;
        r.subdone = 0;
        if (r.step + 1 < d.steps.length) {
          r.step += 1;
        } else if (d.npc) {
          r.phase = "reward_pending";
          r.flags |= 2;
          out.push({ id: d.id, phase: "reward_pending" });
        } else {
          r.phase = "done";
          r.flags = r.flags & ~2 | 1;
          if (d.id === "a5q3")
            r.flags |= 128;
          out.push({ id: d.id, phase: "done", final: !!d.final });
        }
      }
    }
    return out;
  }
  to_json() {
    return JSON.stringify({
      version: STATE_VERSION_45,
      quests: DEFS45.map((d, i) => ({ ...this.runs[i], id: d.id, act: d.act }))
    });
  }
  static from_json(json) {
    const s = JSON.parse(json);
    const o = s;
    if (o.version !== STATE_VERSION_45 || !Array.isArray(o.quests) || o.quests.length !== DEFS45.length) {
      throw new Error("quests45: état inconnu");
    }
    const log = new Quests45Log;
    o.quests.forEach((qRaw, i) => {
      const q = qRaw;
      if (q.id !== DEFS45[i].id)
        throw new Error(`quests45: id ${q.id} ≠ ${DEFS45[i].id}`);
      if (!["inactive", "active", "reward_pending", "done"].includes(q.phase ?? "")) {
        throw new Error(`quests45: phase ${q.phase}`);
      }
      log.runs[i] = {
        id: null,
        phase: q.phase,
        step: q.step ?? 0,
        count: q.count ?? 0,
        flags: q.flags ?? 0,
        subdone: q.subdone ?? 0
      };
    });
    return log;
  }
}
var CSS2 = `
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
var STATUS_FR2 = {
  inactive: "À venir",
  active: "En cours",
  reward_pending: "Récompense",
  done: "Terminée"
};
var doc2 = null;
var panel3 = null;
var listEl2 = null;
var log2 = new Quests45Log;
var isOpen2 = false;
function render4() {
  if (!listEl2 || !doc2)
    return;
  listEl2.textContent = "";
  let lastAct = 0;
  DEFS45.forEach((d, i) => {
    const r = log2.runs[i];
    if (d.act !== lastAct) {
      lastAct = d.act;
      const act = doc2.createElement("div");
      act.className = "d2q-act";
      act.textContent = ACT_TITLES[d.act] ?? "";
      listEl2.appendChild(act);
    }
    const li = doc2.createElement("li");
    li.className = r.phase;
    const head = doc2.createElement("div");
    const status = doc2.createElement("span");
    status.className = "d2q-status";
    status.textContent = (r.phase === "done" ? "✔ " : "") + STATUS_FR2[r.phase];
    const name = doc2.createElement("span");
    name.className = "d2q-name";
    name.textContent = d.name;
    head.append(status, name);
    li.appendChild(head);
    if (r.phase === "active") {
      const st = stepOf(d, r);
      const obj = doc2.createElement("div");
      obj.className = "d2q-obj";
      obj.textContent = st ? st.desc : "";
      if (st && st.any) {
        const prog = doc2.createElement("span");
        prog.className = "d2q-prog";
        prog.textContent = ` (${r.count}/${Math.max(1, st.count ?? st.any.length)})`;
        obj.appendChild(prog);
      }
      li.appendChild(obj);
    } else if (r.phase === "reward_pending") {
      const obj = doc2.createElement("div");
      obj.className = "d2q-obj";
      obj.textContent = "Retournez voir le PNJ pour la récompense.";
      li.appendChild(obj);
    } else if (r.phase === "done" && d.final) {
      const obj = doc2.createElement("div");
      obj.className = "d2q-obj";
      obj.textContent = "Fin de jeu — la Pierre d’Worldstone est détruite.";
      li.appendChild(obj);
    }
    listEl2.appendChild(li);
  });
}
var quests45Api = {
  Quests45Log,
  DEFS: DEFS45,
  init(dom) {
    doc2 = dom ?? (typeof document !== "undefined" ? document : null);
    if (!doc2 || panel3)
      return quests45Api;
    const style = doc2.createElement("style");
    style.textContent = CSS2;
    doc2.head.appendChild(style);
    panel3 = doc2.createElement("div");
    panel3.id = "d2-quests45";
    panel3.hidden = true;
    const title = doc2.createElement("h2");
    title.textContent = "Journal — Actes IV-V";
    listEl2 = doc2.createElement("ul");
    panel3.append(title, listEl2);
    doc2.body.appendChild(panel3);
    doc2.addEventListener("d2quests45:toggle", () => quests45Api.toggle());
    return quests45Api;
  },
  feed(event) {
    const changes = log2.onEvent(event);
    if (isOpen2)
      render4();
    return changes;
  },
  get log() {
    return log2;
  },
  reset() {
    log2 = new Quests45Log;
    if (isOpen2)
      render4();
  },
  exportLog() {
    return log2.to_json();
  },
  importLog(json) {
    log2 = Quests45Log.from_json(json);
    if (isOpen2)
      render4();
    return true;
  },
  show() {
    isOpen2 = true;
    if (panel3)
      panel3.hidden = false;
    render4();
  },
  hide() {
    isOpen2 = false;
    if (panel3)
      panel3.hidden = true;
  },
  toggle() {
    if (isOpen2)
      quests45Api.hide();
    else
      quests45Api.show();
  },
  get isOpen() {
    return isOpen2;
  }
};
function installQuests45(win) {
  registerModule("quests45", "ts");
  win.D2Quests45 = quests45Api;
}
if (typeof window !== "undefined")
  installQuests45(window);

// src/stash-ui.ts
var STASH_KEY = "d2w.stash.v1";
var STASH_W = 6;
var STASH_H = 4;
var CARRY_CAP_PER_LEVEL = 1e4;
var STASH_CAP = {
  normal: 300000,
  nightmare: 500000,
  hell: 800000
};
var state2 = { gold: 0, cells: [], hooks: null, btn: null, panel: null };
var storage2 = null;
function store2() {
  if (storage2)
    return storage2;
  try {
    const ls = globalThis.localStorage;
    if (ls)
      return ls;
  } catch {}
  const mem = new Map;
  return {
    getItem: (k) => mem.has(k) ? mem.get(k) : null,
    setItem: (k, v) => void mem.set(k, String(v))
  };
}
function load() {
  try {
    const raw = JSON.parse(store2().getItem(STASH_KEY) || "{}");
    state2.gold = (raw.gold ?? 0) >>> 0;
    state2.cells = Array.isArray(raw.cells) ? raw.cells.slice(0, STASH_W * STASH_H) : [];
    while (state2.cells.length < STASH_W * STASH_H)
      state2.cells.push(null);
  } catch {
    state2.gold = 0;
    state2.cells = [];
  }
}
function persist() {
  try {
    store2().setItem(STASH_KEY, JSON.stringify({ gold: state2.gold, cells: state2.cells }));
  } catch {}
}
function firstFit(w, h, cells) {
  const occupied = (x, y) => {
    for (let cy = y;cy < y + h; cy++) {
      for (let cx = x;cx < x + w; cx++) {
        if (cells[cy * STASH_W + cx])
          return true;
      }
    }
    return false;
  };
  for (let y = 0;y <= STASH_H - h; y++) {
    for (let x = 0;x <= STASH_W - w; x++) {
      if (!occupied(x, y))
        return { x, y };
    }
  }
  return null;
}
function depositGoldPure(n, _gold, stashGold, diff) {
  const room = (STASH_CAP[diff] ?? STASH_CAP.normal) - stashGold;
  const done = Math.min(n >>> 0, Math.max(0, room));
  return { done, left: (n >>> 0) - done, newStash: stashGold + done };
}
function withdrawGoldPure(n, gold, stashGold, level) {
  const room = Math.max(0, CARRY_CAP_PER_LEVEL * Math.max(1, level) - gold);
  const done = Math.min(n >>> 0, stashGold, room);
  return { done, left: (n >>> 0) - done, newStash: stashGold - done };
}
function stats() {
  const g = typeof window !== "undefined" ? window.__d2?.game : null;
  try {
    return g ? JSON.parse(g.heroStats()) : {};
  } catch {
    return {};
  }
}
function diffName() {
  const g = typeof window !== "undefined" ? window.__d2?.game : null;
  try {
    const raw = g ? String(g.difficultyInfo()).toLowerCase() : "normal";
    return raw.includes("night") || raw.includes("cauch") ? "nightmare" : raw.includes("hell") || raw.includes("enfer") ? "hell" : "normal";
  } catch {
    return "normal";
  }
}
function defaultHooks2() {
  return {
    gold: () => (stats().gold ?? 0) >>> 0,
    level: () => (stats().level ?? 1) >>> 0,
    diff: diffName,
    spend: () => false,
    earn: () => {}
  };
}
function stashCap() {
  return STASH_CAP[state2.hooks?.diff() ?? "normal"] ?? STASH_CAP.normal;
}
function carryCap() {
  return CARRY_CAP_PER_LEVEL * Math.max(1, state2.hooks ? state2.hooks.level() : 1);
}
function el2(tag, cls, text) {
  const e = document.createElement(tag);
  if (cls)
    e.className = cls;
  if (text !== undefined)
    e.textContent = text;
  return e;
}
function logLine(m) {
  const s = document.getElementById("status");
  if (s)
    s.textContent = s.textContent.replace(/ · \d+ fps.*$/, "") + " — " + m;
}
function render5() {
  const p = state2.panel;
  const h = state2.hooks;
  if (!p || !h)
    return;
  p.innerHTML = "";
  p.appendChild(el2("h3", undefined, "STASH — or : " + state2.gold + " / " + stashCap() + " · bourse : " + h.gold() + " / " + carryCap()));
  const goldRow = el2("div", "goldrow");
  const depositDefs = [
    ["Déposer 100", 100],
    ["Déposer 1 000", 1000],
    ["Déposer tout", Infinity]
  ];
  for (const [label, amount] of depositDefs) {
    const b = el2("button", undefined, label);
    b.addEventListener("click", () => {
      const have = h.gold();
      const room = stashCap() - state2.gold;
      const n = Math.min(amount, have, Math.max(0, room));
      if (n <= 0) {
        logLine("Dépôt refusé : " + (room <= 0 ? "stash plein" : "bourse vide") + ".");
        return;
      }
      state2.gold += n;
      persist();
      render5();
      logLine("Déposé " + n + " or au stash (bourse " + have + " → " + (have - n) + ").");
    });
    goldRow.appendChild(b);
  }
  const withdrawDefs = [
    ["Retirer 100", 100],
    ["Retirer 1 000", 1000],
    ["Retirer tout", Infinity]
  ];
  for (const [label, amount] of withdrawDefs) {
    const b = el2("button", undefined, label);
    b.addEventListener("click", () => {
      const have = h.gold();
      const room = carryCap() - have;
      const n = Math.min(amount, state2.gold, Math.max(0, room));
      if (n <= 0) {
        logLine("Retrait refusé : " + (room <= 0 ? "bourse au plafond 10 000 × niveau" : "stash vide") + ".");
        return;
      }
      state2.gold -= n;
      persist();
      render5();
      logLine("Retiré " + n + " or du stash (bourse → " + (have + n) + ").");
    });
    goldRow.appendChild(b);
  }
  p.appendChild(goldRow);
  const grid = el2("div", "grid");
  for (let i = 0;i < STASH_W * STASH_H; i++) {
    const c = state2.cells[i];
    const cell = el2("div", "cell" + (c ? " full" : ""), c ? c.name : "");
    if (c) {
      cell.title = c.name + " — clic : reprendre (miroir démo)";
      const idx = i;
      cell.addEventListener("click", () => {
        const item = state2.cells[idx];
        logLine("Repris : " + (item ? item.name : "?") + " (miroir démo — branchement wasm restant).");
        state2.cells[idx] = null;
        persist();
        render5();
      });
    }
    grid.appendChild(cell);
  }
  p.appendChild(grid);
  p.appendChild(el2("div", "foot", "Plafonds D2 : porté 10 000 × niveau · stash 300k/500k/800k " + "(Normal/NM/Enfer) — economy.rs · save additive `gold_stash` (rétro-compat v3)."));
}
function initStash(dom, hooks) {
  load();
  state2.hooks = hooks ?? defaultHooks2();
  const toolbar = dom && dom.toolbar || document.getElementById("toolbar");
  if (toolbar && !state2.btn) {
    const btn = el2("button", undefined, "Stash");
    btn.id = "btn-stash";
    btn.title = "Stash 6×4 + or (étape 11c — economy.rs)";
    toolbar.appendChild(btn);
    btn.addEventListener("click", toggleStash);
    state2.btn = btn;
  }
  if (!state2.panel) {
    const css = el2("style");
    css.textContent = "#d2stash{position:fixed;right:10px;top:64px;width:262px;max-height:70vh;overflow:auto;" + "background:#141019f2;border:1px solid #5a4a2a;border-radius:4px;padding:8px;z-index:50;display:none}" + "#d2stash h3{margin:0 0 6px;color:#c7a45a;font:600 12px system-ui;letter-spacing:.08em}" + "#d2stash .goldrow{display:flex;flex-wrap:wrap;gap:4px;margin-bottom:6px}" + "#d2stash .goldrow button{padding:3px 6px;font-size:11px}" + "#d2stash .grid{display:grid;grid-template-columns:repeat(6,1fr);gap:2px}" + "#d2stash .cell{aspect-ratio:1;background:#1d1712;border:1px solid #3a3226;border-radius:2px;" + "font:9px system-ui;color:#8f8676;display:flex;align-items:center;justify-content:center;" + "overflow:hidden;text-align:center;padding:1px}" + "#d2stash .cell.full{border-color:#c7a45a;color:#ffe9b0;cursor:pointer}" + "#d2stash .foot{margin-top:6px;font:9px system-ui;color:#8f8676}";
    document.head.appendChild(css);
    const panel = el2("div");
    panel.id = "d2stash";
    document.body.appendChild(panel);
    state2.panel = panel;
  }
  render5();
}
function toggleStash() {
  if (!state2.panel) {
    initStash();
    if (!state2.panel)
      return;
  }
  const on = state2.panel.style.display !== "block";
  state2.panel.style.display = on ? "block" : "none";
  if (on)
    render5();
}
function installStash(win) {
  registerModule("stash-ui", "ts");
  const api = {
    init: initStash,
    toggle: toggleStash,
    depositGold(n) {
      const res = depositGoldPure(n, 0, state2.gold, state2.hooks ? state2.hooks.diff() : "normal");
      state2.gold = res.newStash;
      persist();
      return { done: res.done, left: res.left };
    },
    withdrawGold(n) {
      const res = withdrawGoldPure(n, state2.hooks ? state2.hooks.gold() : 0, state2.gold, state2.hooks ? state2.hooks.level() : 1);
      state2.gold = res.newStash;
      persist();
      return { done: res.done, left: res.left };
    },
    depositItem(name) {
      const spot = firstFit(1, 1, state2.cells);
      if (!spot)
        return null;
      state2.cells[spot.y * STASH_W + spot.x] = { name: String(name) };
      persist();
      return spot;
    },
    state() {
      return { gold: state2.gold, cells: state2.cells.slice() };
    }
  };
  win.D2Stash = api;
}
if (typeof window !== "undefined") {
  await warmLegacy("stash-ui.js");
  installStash(window);
}

// src/cube-ui.ts
var CUBE_W = 800;
var CUBE_H = 600;
var CUBE_INV_COLS = 10;
var CUBE_INV_ROWS = 4;
var CUBE_COLS = 3;
var CUBE_ROWS = 4;
var KIND_COLORS2 = {
  Potion: ["#b03a2e", "#57150e"],
  Weapon: ["#8d99a6", "#39424c"],
  Armor: ["#8a6d3b", "#41331a"],
  Gold: ["#d9b96a", "#6e5626"],
  Misc: ["#7d6f58", "#3c3428"]
};
var QUALITY_HEX = {
  Magic: "#6969ff",
  Rare: "#ffff64",
  Unique: "#a59263",
  Set: "#00ff00",
  Crafted: "#ff8040"
};
function itemLabel(it) {
  if (!it)
    return "";
  if (it.kind === "Gold")
    return "or";
  const n = String(it.name || it.code || "");
  return n.length > 5 ? n.slice(0, 5) : n;
}
function itemColor(it) {
  return KIND_COLORS2[it.kind] ?? KIND_COLORS2.Misc;
}
function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c] ?? c);
}
function tipHtml(tip) {
  let html = "";
  const name = tip.name ?? "";
  html += '<div style="color:' + (tip.colorHex || "#e8e8e8") + ';font-weight:700">' + escapeHtml(name) + "</div>";
  if (tip.baseName) {
    html += '<div style="color:#e8e8e8">' + escapeHtml(tip.baseName) + "</div>";
  }
  const lines = tip.lines || [];
  for (const line of lines) {
    html += '<div style="color:#7f7fff">' + escapeHtml(line) + "</div>";
  }
  return html;
}
function transmutePayload(slots, nowMs) {
  return JSON.stringify({ slots, seed: nowMs & 2147483647 });
}
function installTsCube(win) {
  registerModule("cube-ui", "ts");
  const state = {
    open: false,
    built: false,
    selected: -1,
    placed: [],
    products: {},
    message: "",
    msgErr: false,
    resultTip: null,
    root: null,
    overlay: null,
    octx: null,
    invWrap: null,
    cubeWrap: null,
    msgWrap: null,
    btnTrans: null,
    belt: null,
    tipEl: null
  };
  let lastSig = "";
  function el(tag, cls, parent, text) {
    const e = document.createElement(tag);
    if (cls)
      e.className = cls;
    if (text !== undefined)
      e.textContent = text;
    (parent ?? state.root).appendChild(e);
    return e;
  }
  function game() {
    return win.__d2?.game ?? null;
  }
  function invCache() {
    return win.__d2?.invCache ?? [];
  }
  function build() {
    if (state.built)
      return;
    state.built = true;
    const stage = document.getElementById("stage") || document.body;
    const root = document.createElement("div");
    root.id = "d2cube-root";
    stage.appendChild(root);
    state.root = root;
    const css = [
      "#d2cube-root { position:absolute; inset:0; pointer-events:none; z-index:45;",
      "  font:12px monospace; color:#d0b070; }",
      "#d2cube-root * { box-sizing:border-box; }",
      "#d2cube-tip { position:absolute; z-index:60; pointer-events:none; display:none;",
      "  max-width:280px; background:#141019f2; border:1px solid #5a4a2a; padding:6px 9px;",
      "  font:11px/1.45 monospace; color:#e8e8e8; white-space:pre-wrap;",
      "  box-shadow:0 4px 14px #000d; }",
      "#d2cube-panel { position:absolute; left:50%; top:44px; transform:translateX(-50%);",
      "  display:none; pointer-events:auto; width:560px;",
      "  background:rgba(16,12,6,.95); border:2px solid #5a3c1e; padding:10px 14px 12px;",
      "  box-shadow:0 6px 24px #000d; }",
      "#d2cube-panel.open { display:block; }",
      "#d2cube-panel h3 { margin:0 0 8px; text-align:center; letter-spacing:2px;",
      "  color:#e8dcbf; font:700 13px monospace; text-transform:uppercase; }",
      ".d2cube-cols { display:flex; gap:16px; justify-content:center; }",
      ".d2cube-side { text-align:center; }",
      ".d2cube-side .cap { color:#a89670; margin:2px 0 4px; letter-spacing:1px; }",
      ".d2cube-grid { display:grid; gap:3px; padding:6px; background:#171208;",
      "  border:1px solid #5a3c1e; }",
      ".d2cube-cell { width:32px; height:32px; background:#2c2c34; cursor:pointer;",
      "  border:1px solid #1c1c22; position:relative; padding:0;",
      "  box-shadow:inset 1px 1px 0 #ffffff10, inset -1px -1px 0 #00000088; }",
      ".d2cube-cell:hover { outline:1px solid #c7a45a; }",
      ".d2cube-cell.sel { outline:2px solid #ffe9ad; }",
      ".d2cube-cell .it { position:absolute; inset:3px; border:1px solid #000000aa;",
      "  font:9px monospace; color:#f0e8d0; overflow:hidden; display:flex;",
      "  align-items:center; justify-content:center; text-align:center; }",
      ".d2cube-actions { display:flex; gap:8px; justify-content:center; margin-top:10px; }",
      "#d2cube-panel button { cursor:pointer; background:linear-gradient(#3a2f1c,#241c10);",
      "  color:#d9b96a; border:1px solid #5a4a2a; padding:4px 12px; font:700 11px monospace;",
      "  letter-spacing:.05em; }",
      "#d2cube-panel button:hover:not(:disabled) { border-color:#c7a45a; color:#ffe9ad; }",
      "#d2cube-panel button:disabled { opacity:.45; cursor:default; }",
      "#d2cube-msg { min-height:16px; margin-top:8px; text-align:center; }",
      "#d2cube-msg.err { color:#ffb59a; } #d2cube-msg.ok { color:#d9b96a; }",
      "#d2cube-result { margin-top:4px; text-align:center; font:11px/1.5 monospace;",
      "  color:#e8e8e8; white-space:pre-wrap; }",
      "#d2cube-belt { position:absolute; left:50%; bottom:6px; transform:translateX(-50%);",
      "  display:flex; gap:8px; align-items:flex-end; pointer-events:auto; }",
      "#d2cube-belt .belt-cols { display:flex; gap:4px; padding:5px; background:#171208cc;",
      "  border:1px solid #5a3c1e; }",
      ".d2cube-beltcol { display:flex; flex-direction:column; gap:3px; }",
      ".d2cube-beltcol .bkey { text-align:center; color:#8f7a50; font:9px monospace; }",
      ".d2cube-beltcol .bcell { width:24px; height:24px; background:#2c2c34;",
      "  border:1px solid #1c1c22; position:relative; }",
      ".d2cube-beltcol .bcell .it { position:absolute; inset:2px; display:flex;",
      "  align-items:center; justify-content:center; font:9px monospace; color:#f0e8d0; }",
      "#d2cube-belt .cubebtn { width:auto; height:auto; padding:6px 8px; }"
    ].join(`
`);
    const st = document.createElement("style");
    st.textContent = css;
    document.head.appendChild(st);
    const overlay = document.createElement("canvas");
    overlay.width = CUBE_W;
    overlay.height = CUBE_H;
    overlay.style.cssText = "position:absolute; inset:0; pointer-events:none;";
    root.appendChild(overlay);
    state.overlay = overlay;
    state.octx = overlay.getContext("2d");
    const panel = el("div", "", root);
    panel.id = "d2cube-panel";
    const h3 = el("h3", "", panel, "Cube Horadrim");
    h3.title = "Touche C pour ouvrir/fermer";
    const cols = el("div", "d2cube-cols", panel);
    const sideInv = el("div", "d2cube-side", cols);
    el("div", "cap", sideInv, "Inventaire (10×4)");
    const invWrap = el("div", "d2cube-grid", sideInv);
    invWrap.style.gridTemplateColumns = "repeat(" + CUBE_INV_COLS + ", 32px)";
    state.invWrap = invWrap;
    const sideCube = el("div", "d2cube-side", cols);
    el("div", "cap", sideCube, "Cube de transmutation (3×4)");
    const cubeWrap = el("div", "d2cube-grid", sideCube);
    cubeWrap.style.gridTemplateColumns = "repeat(" + CUBE_COLS + ", 32px)";
    state.cubeWrap = cubeWrap;
    const actions = el("div", "d2cube-actions", panel);
    const btnTrans = el("button", "", actions, "Transmuter");
    btnTrans.title = "Appelle le moteur (145 recettes réelles v1.12)";
    state.btnTrans = btnTrans;
    const btnBack = el("button", "", actions, "Tout reprendre");
    btnBack.title = "Reprendre les objets posés dans le cube";
    const btnClose = el("button", "", actions, "Fermer (C)");
    const msgWrap = el("div", "", panel);
    msgWrap.id = "d2cube-msg";
    state.msgWrap = msgWrap;
    const result = el("div", "", panel);
    result.id = "d2cube-result";
    btnTrans.addEventListener("click", transmute);
    btnBack.addEventListener("click", () => {
      state.placed = [];
      state.selected = -1;
      setMessage("Objets repris.", false);
      render();
    });
    btnClose.addEventListener("click", close);
    const belt = el("div", "", root);
    belt.id = "d2cube-belt";
    const bbtn = el("button", "cubebtn", belt, "Cube (C)");
    bbtn.title = "Ouvrir le Cube Horadrim";
    bbtn.addEventListener("click", () => toggle());
    const bcols = el("div", "belt-cols", belt);
    for (let c = 0;c < 4; c++) {
      const col = el("div", "d2cube-beltcol", bcols);
      el("div", "bkey", col, String(c + 1));
      for (let r = 0;r < 4; r++) {
        el("div", "bcell", col).dataset.b = String(c * 4 + r);
      }
    }
    state.belt = belt;
    const tip = el("div", "", root);
    tip.id = "d2cube-tip";
    state.tipEl = tip;
    wireTooltipEvents();
    setInterval(refreshBelt, 500);
    requestAnimationFrame(frame);
  }
  function open() {
    build();
    state.open = true;
    document.getElementById("d2cube-panel")?.classList.add("open");
    setMessage("Placez des objets dans le cube (clic-sélection), puis Transmuter.", false);
    render();
  }
  function close() {
    state.open = false;
    state.selected = -1;
    state.placed = [];
    hideTip();
    document.getElementById("d2cube-panel")?.classList.remove("open");
    drawOverlay();
  }
  function toggle() {
    if (state.open)
      close();
    else
      open();
  }
  function setMessage(msg, isErr) {
    state.message = msg || "";
    state.msgErr = !!isErr;
    if (!state.msgWrap)
      return;
    state.msgWrap.textContent = state.message;
    state.msgWrap.className = state.msgErr ? "err" : state.message ? "ok" : "";
  }
  function render() {
    if (!state.open || !state.built)
      return;
    const slots = invCache();
    state.placed = state.placed.filter((p) => !!slots[p.slot]);
    const inv = state.invWrap;
    if (!inv)
      return;
    inv.textContent = "";
    for (let i = 0;i < CUBE_INV_COLS * CUBE_INV_ROWS; i++) {
      const slot = i;
      const cell = el("div", "d2cube-cell", inv);
      const it = slots[slot];
      const inCube = state.placed.some((p) => p.slot === slot);
      if (it && !inCube) {
        const d = el("div", "it", cell, itemLabel(it));
        const pair = itemColor(it);
        d.style.background = pair[0];
        d.style.borderColor = pair[1];
        const rolled = state.products[slot]?.rolled;
        if (rolled) {
          d.style.boxShadow = "0 0 4px " + (QUALITY_HEX[rolled.quality ?? ""] ?? "#c7a45a");
        }
        d.title = "";
      }
      if (slot === state.selected)
        cell.classList.add("sel");
      cell.addEventListener("click", (e) => {
        e.stopPropagation();
        if (!it || inCube) {
          state.selected = -1;
          render();
          return;
        }
        state.selected = state.selected === slot ? -1 : slot;
        setMessage(state.selected >= 0 ? "Objet sélectionné — cliquez une cellule du cube pour le placer." : "Sélection annulée.", false);
        hideTip();
        render();
      });
      cell.addEventListener("mouseenter", () => {
        if (it && !inCube)
          showTipForSlot(slot, cell);
      });
      cell.addEventListener("mouseleave", hideTip);
    }
    const cube = state.cubeWrap;
    if (!cube)
      return;
    cube.textContent = "";
    for (let c = 0;c < CUBE_COLS * CUBE_ROWS; c++) {
      const cellIdx = c;
      const cell = el("div", "d2cube-cell", cube);
      const entry = state.placed.find((p) => p.cell === cellIdx) ?? null;
      if (entry) {
        const it = slots[entry.slot];
        if (it) {
          const d = el("div", "it", cell, itemLabel(it));
          const pair = itemColor(it);
          d.style.background = pair[0];
          d.style.borderColor = pair[1];
        }
        cell.addEventListener("click", (e) => {
          e.stopPropagation();
          state.placed = state.placed.filter((p) => p.cell !== cellIdx);
          setMessage("Objet repris du cube.", false);
          hideTip();
          render();
        });
        cell.addEventListener("mouseenter", () => {
          if (entry)
            showTipForSlot(entry.slot, cell);
        });
        cell.addEventListener("mouseleave", hideTip);
      } else {
        cell.addEventListener("click", (e) => {
          e.stopPropagation();
          if (state.selected < 0) {
            setMessage("Sélectionnez d'abord un objet dans l'inventaire.", true);
            return;
          }
          if (state.placed.some((p) => p.slot === state.selected))
            return;
          state.placed.push({ slot: state.selected, cell: cellIdx });
          state.selected = -1;
          setMessage("Dans le cube — " + state.placed.length + " objet(s).", false);
          hideTip();
          render();
        });
      }
    }
    if (state.btnTrans) {
      state.btnTrans.disabled = state.placed.length === 0;
      state.btnTrans.textContent = "Transmuter (" + state.placed.length + ")";
    }
    drawOverlay();
    lastSig = signature();
  }
  function drawOverlay() {
    const ctx = state.octx;
    if (!ctx)
      return;
    ctx.clearRect(0, 0, CUBE_W, CUBE_H);
    if (!state.open || !state.placed.length)
      return;
    const pad = 14, header = 30, cell = 42;
    const pw = pad * 2 + CUBE_INV_COLS * cell;
    const ph = pad * 2 + header + CUBE_INV_ROWS * cell;
    const x0 = (CUBE_W - pw) / 2, y0 = (CUBE_H - ph) / 2;
    const slots = invCache();
    ctx.save();
    ctx.lineWidth = 2;
    for (const p of state.placed) {
      if (!slots[p.slot])
        continue;
      const cx = x0 + pad + p.slot % CUBE_INV_COLS * cell;
      const cy = y0 + pad + header + Math.floor(p.slot / CUBE_INV_COLS) * cell;
      ctx.strokeStyle = "#6969ff";
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
      ctx.strokeStyle = "#00000088";
      ctx.beginPath();
      ctx.moveTo(cx + 6, cy + cell / 2);
      ctx.lineTo(cx + cell - 6, cy + cell / 2);
      ctx.stroke();
    }
    if (state.selected >= 0 && slots[state.selected]) {
      const cx = x0 + pad + state.selected % CUBE_INV_COLS * cell;
      const cy = y0 + pad + header + Math.floor(state.selected / CUBE_INV_COLS) * cell;
      ctx.strokeStyle = "#ffe9ad";
      ctx.strokeRect(cx + 0.5, cy + 0.5, cell - 1, cell - 1);
    }
    ctx.restore();
  }
  function transmute() {
    const g = game();
    if (!g || typeof g.cubeTransmute !== "function") {
      setMessage("Moteur indisponible (pkg wasm sans endpoint cube).", true);
      return;
    }
    const slots = state.placed.map((p) => p.slot);
    if (!slots.length)
      return;
    let res;
    try {
      res = parseOr(g.cubeTransmute(transmutePayload(slots, Date.now())), {});
    } catch (err) {
      setMessage(err instanceof Error ? err.message : String(err), true);
      state.resultTip = null;
      render();
      return;
    }
    if (res.item && res.item.slot !== undefined) {
      state.products[res.item.slot] = res.item;
      state.resultTip = res.item.tooltip ?? null;
    } else {
      state.resultTip = null;
    }
    state.placed = [];
    state.selected = -1;
    setMessage(res.message || "Transmutation réussie.", false);
    renderResult(res);
    try {
      if (win.__d2 && g)
        win.__d2.invCache = parseOr(g.inventory(), []);
    } catch {}
    render();
  }
  function renderResult(res) {
    const box = document.getElementById("d2cube-result");
    if (!box)
      return;
    box.textContent = "";
    if (res.portal) {
      box.style.color = "#d9b96a";
      box.textContent = "⚗ " + (res.message ?? "");
      return;
    }
    const tip = state.resultTip;
    if (!tip)
      return;
    box.style.color = "";
    const name = document.createElement("div");
    name.textContent = tip.name ?? "";
    name.style.color = tip.colorHex || "#e8e8e8";
    name.style.fontWeight = "700";
    box.appendChild(name);
    if (tip.baseName) {
      const b = document.createElement("div");
      b.textContent = tip.baseName;
      b.style.color = "#e8e8e8";
      box.appendChild(b);
    }
    for (const line of tip.lines ?? []) {
      const l = document.createElement("div");
      l.textContent = line;
      l.style.color = "#7f7fff";
      box.appendChild(l);
    }
  }
  function requestTooltip(item, anchor) {
    const g = game();
    if (!g || typeof g.cubeItemTooltip !== "function" || !item)
      return;
    let json;
    try {
      json = g.cubeItemTooltip(JSON.stringify(item));
    } catch {
      return;
    }
    const tip = parseOr(json, null);
    if (tip)
      showTip(tipHtml(tip), anchor);
  }
  function showTipForSlot(slot, anchor) {
    const it = invCache()[slot];
    if (!it)
      return;
    const known = state.products[slot];
    const view = {
      code: it.code,
      name: it.name
    };
    const rolled = known?.rolled;
    if (rolled) {
      view.quality = rolled.quality;
      view.affixIds = rolled.affixIds;
      view.sockets = rolled.sockets;
      view.min_dmg = rolled.minDmg;
      view.max_dmg = rolled.maxDmg;
      view.ac = rolled.ac;
      view.req_str = rolled.reqStr;
      view.req_dex = rolled.reqDex;
      view.req_level = rolled.reqLevel;
      if (rolled.runeword)
        view.runeword = rolled.runeword;
    }
    requestTooltip(view, anchor);
  }
  function showTip(html, anchor) {
    const tip = state.tipEl;
    if (!tip || !state.root)
      return;
    tip.innerHTML = html;
    tip.style.display = "block";
    const rect = anchor.getBoundingClientRect();
    const host = state.root.getBoundingClientRect();
    let x = rect.right - host.left + 8;
    const y = rect.top - host.top;
    if (x + 290 > CUBE_W)
      x = rect.left - host.left - 292;
    if (x < 0)
      x = 4;
    tip.style.left = Math.max(2, Math.round(x)) + "px";
    tip.style.top = Math.max(2, Math.round(Math.min(y, CUBE_H - 160))) + "px";
  }
  function hideTip() {
    if (state.tipEl)
      state.tipEl.style.display = "none";
  }
  function wireTooltipEvents() {
    const labels = document.getElementById("itemlabels");
    if (labels) {
      labels.addEventListener("mouseover", (e) => {
        const target = e.target;
        const lab = target.closest?.(".ilabel");
        if (!lab)
          return;
        const g = game();
        if (!g)
          return;
        const lx = parseInt(lab.style.left, 10);
        const ly = parseInt(lab.style.top, 10) + 14;
        let items = [];
        try {
          items = parseOr(g.groundItems() || "[]", []);
        } catch {
          return;
        }
        for (const it of items) {
          if (Math.round(it.sx) === lx && Math.round(it.sy) === ly) {
            requestTooltip({ code: it.code, name: it.name }, lab);
            return;
          }
        }
      });
      labels.addEventListener("mouseout", hideTip);
    }
    if (state.belt) {
      state.belt.addEventListener("mouseover", (e) => {
        const cell = e.target.closest?.(".bcell");
        if (!cell || !cell.dataset.item)
          return;
        let it;
        try {
          it = JSON.parse(cell.dataset.item);
        } catch {
          return;
        }
        requestTooltip(it, cell);
      });
      state.belt.addEventListener("mouseout", hideTip);
    }
  }
  function refreshBelt() {
    const belt = state.belt;
    if (!belt || document.hidden)
      return;
    const g = game();
    if (!g || typeof g.beltInfo !== "function")
      return;
    const info = parseOr(g.beltInfo() || "{}", {});
    const grid = info.belt ?? [];
    const cells = belt.querySelectorAll(".bcell");
    for (const cell of cells) {
      const b = parseInt(cell.dataset.b ?? "0", 10);
      const col = Math.floor(b / 4), row = b % 4;
      const it = grid[col]?.[row] ?? null;
      cell.dataset.item = it ? JSON.stringify({ code: it.code, name: it.name }) : "";
      cell.textContent = "";
      if (it) {
        const d = document.createElement("div");
        d.className = "it";
        d.textContent = it.code && it.code[0] ? it.code[0].toUpperCase() : "?";
        const pair = KIND_COLORS2[it.kind] ?? KIND_COLORS2.Misc;
        d.style.background = pair[0];
        cell.appendChild(d);
        cell.title = "";
      } else {
        cell.removeAttribute("title");
      }
    }
  }
  function signature() {
    const slots = invCache();
    let sig = state.placed.map((p) => p.slot + ":" + p.cell).join(",") + "|" + state.selected;
    for (let i = 0;i < slots.length; i++) {
      sig += "|" + (slots[i] ? slots[i].code : "-");
    }
    return sig;
  }
  let lastFrame = 0;
  function frame(now) {
    if (now - lastFrame > 120) {
      lastFrame = now;
      if (state.open && state.built) {
        const sig = signature();
        if (sig !== lastSig) {
          lastSig = sig;
          render();
        }
      }
    }
    requestAnimationFrame(frame);
  }
  window.addEventListener("keydown", (e) => {
    const t = e.target;
    if (t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.isContentEditable))
      return;
    if (e.key === "c" || e.key === "C") {
      if (!game())
        return;
      toggle();
    } else if (e.key === "Escape" && state.open) {
      close();
    }
  });
  win.D2CubeUI = { open, close, toggle };
}
function installCube(win) {
  if (win.D2CubeUI) {
    registerModule("cube-ui", "adopt");
    return;
  }
  installTsCube(win);
}
if (typeof window !== "undefined") {
  await warmLegacy("cube-ui.js");
  installCube(window);
}

// src/panels-ui.ts
var INV_W = 10;
var INV_H = 4;
var BELT_W = 4;
var BELT_H = 4;
var PANELS_KEY = "d2w.panels.invl.v1";
var CELL = 30;
var XP_THR = [0, 500, 1500, 3750, 7875, 14175, 22680, 32886, 44396, 57715, 72144];
function xpFrac(level, xp) {
  const lvl = Math.min(level, 10);
  const base = XP_THR[lvl - 1] ?? 0;
  const next = XP_THR[lvl] ?? XP_THR[10];
  return next > base ? Math.max(0, Math.min(1, (xp - base) / (next - base))) : 1;
}
function fmt(n) {
  return String(n).replace(/\B(?=(\d{3})+(?!\d))/g, " ");
}
function itemKey(it) {
  return it.code + "|" + it.name + "|" + (it.amount || 0) + "|" + (it.slot === undefined ? -1 : it.slot);
}
function kindOf(it) {
  if (it.kind === "Gold")
    return "Gold";
  if (it.kind === "Potion")
    return "Potion";
  if (it.kind === "Weapon")
    return "Weapon";
  if (it.kind === "Armor")
    return "Armor";
  return "Misc";
}
function firstFitPure(w, h, gw, gh, blocks, skip) {
  const occ = new Set;
  for (const b of blocks) {
    if (b === skip)
      continue;
    for (let y = b.y;y < b.y + b.h; y++) {
      for (let x = b.x;x < b.x + b.w; x++)
        occ.add(x + "," + y);
    }
  }
  for (let cy = 0;cy <= gh - h; cy++) {
    for (let cx = 0;cx <= gw - w; cx++) {
      let ok = true;
      for (let y2 = cy;y2 < cy + h && ok; y2++) {
        for (let x2 = cx;x2 < cx + w && ok; x2++) {
          if (occ.has(x2 + "," + y2))
            ok = false;
        }
      }
      if (ok)
        return { x: cx, y: cy };
    }
  }
  return null;
}
var FALLBACK_SIZES = {
  gld: [1, 1],
  hp1: [1, 1],
  hp2: [1, 1],
  hp3: [1, 1],
  hp4: [1, 1],
  hp5: [1, 1],
  mp1: [1, 1],
  mp2: [1, 1],
  mp3: [1, 1],
  mp4: [1, 1],
  mp5: [1, 1],
  rvs: [1, 1],
  rvl: [1, 1],
  tsc: [1, 1],
  tbk: [1, 2],
  isc: [1, 1],
  ibk: [1, 2],
  key: [1, 1],
  hax: [1, 3],
  buc: [2, 2],
  box: [2, 2]
};
function sizeOfPure(sizeTable, code) {
  if (sizeTable && sizeTable[code])
    return sizeTable[code];
  return FALLBACK_SIZES[code] ?? [1, 1];
}
var SLOTS = [
  ["head", "Tête", 2, 2, 4, 0],
  ["neck", "Amulette", 1, 1, 7, 0],
  ["torso", "Torse", 2, 3, 4, 1],
  ["rArm", "Arme", 2, 3, 2, 1],
  ["lArm", "Bouclier", 2, 3, 6, 1],
  ["gloves", "Gants", 2, 2, 0, 3],
  ["rHand", "Anneau", 1, 1, 2, 4],
  ["lHand", "Anneau", 1, 1, 3, 4],
  ["belt", "Ceinture", 2, 1, 4, 4],
  ["feet", "Bottes", 2, 2, 6, 3]
];
var KIND_COLORS3 = {
  Potion: ["#b03a2e", "#57150e"],
  Weapon: ["#8d99a6", "#39424c"],
  Armor: ["#8a6d3b", "#41331a"],
  Gold: ["#d9b96a", "#6e5626"],
  Misc: ["#7a8a5a", "#39442a"]
};
function installTsPanels(win) {
  registerModule("panels-ui", "ts");
  const state = {
    inited: false,
    live: false,
    sizeTable: null,
    layout: {},
    sel: null,
    panels: {},
    tablesNote: false
  };
  function el(tag, cls, text) {
    const e = document.createElement(tag);
    if (cls)
      e.className = cls;
    if (text !== undefined)
      e.textContent = text;
    return e;
  }
  function game() {
    return win.__d2?.game ?? null;
  }
  function stats() {
    const g = game();
    return g ? parseOr(g.heroStats(), {}) : {};
  }
  function invItems() {
    const g = game();
    const slots = g ? parseOr(g.inventory(), []) : [];
    const out = [];
    for (let i = 0;i < slots.length; i++) {
      const it = slots[i];
      if (it)
        out.push({ ...it, slot: i });
    }
    return out;
  }
  function sizeOf(code) {
    return sizeOfPure(state.sizeTable, code);
  }
  function layoutOf(items) {
    const seen = new Set;
    const blocks = [];
    const laid = [];
    const saved = state.layout;
    for (const it of items) {
      const k = itemKey(it);
      const s = sizeOf(it.code);
      const p = saved[k];
      if (p && p.w === s[0] && p.h === s[1] && !seen.has(k) && p.x + s[0] <= INV_W && p.y + s[1] <= INV_H) {
        seen.add(k);
        const b = { x: p.x, y: p.y, w: s[0], h: s[1] };
        blocks.push(b);
        laid.push({ item: it, box: b });
      }
    }
    for (const it of items) {
      const k = itemKey(it);
      if (seen.has(k))
        continue;
      seen.add(k);
      const s = sizeOf(it.code);
      const spot = firstFitPure(s[0], s[1], INV_W, INV_H, blocks);
      const b = spot ? { x: spot.x, y: spot.y, w: s[0], h: s[1] } : { x: 0, y: 0, w: s[0], h: s[1], overflow: true };
      blocks.push(b);
      laid.push({ item: it, box: b });
    }
    const out = {};
    for (const l of laid)
      out[itemKey(l.item)] = l.box;
    state.layout = out;
    try {
      localStorage.setItem(PANELS_KEY, JSON.stringify(out));
    } catch {}
    return laid;
  }
  function loadSizes() {
    if (state.sizeTable)
      return;
    const roots = [new URL("..", location.href), new URL(".", location.href)];
    const files = ["assets/patch_misc.txt", "assets/patch_weapons.txt", "assets/patch_armor.txt"];
    const table = {};
    let done = 0;
    let failed = 0;
    for (const root of roots) {
      for (const f of files) {
        fetch(new URL(f, root)).then((r) => {
          if (!r.ok)
            throw new Error(String(r.status));
          return r.text();
        }).then((txt) => {
          const lines = txt.split(`
`);
          const hdr = lines[0].split("\t");
          const iw = hdr.indexOf("invwidth"), ih = hdr.indexOf("invheight"), ic = hdr.indexOf("code");
          if (iw < 0 || ih < 0 || ic < 0)
            return;
          for (let i = 1;i < lines.length; i++) {
            const cols = lines[i].split("\t");
            if (cols.length <= Math.max(iw, ih, ic))
              continue;
            const code = cols[ic].trim();
            const w = parseInt(cols[iw], 10), h = parseInt(cols[ih], 10);
            if (code && w > 0 && h > 0)
              table[code] = [w, h];
          }
        }).catch(() => {
          failed++;
        }).finally(() => {
          if (++done === roots.length * files.length) {
            if (Object.keys(table).length)
              state.sizeTable = table;
            else if (!state.tablesNote)
              state.tablesNote = true;
            refreshOpen();
          }
        });
      }
    }
  }
  function statRow(label, value, cls) {
    const r = el("div", "prow" + (cls ? " " + cls : ""));
    r.appendChild(el("span", "plab", label));
    r.appendChild(el("span", "pval", String(value)));
    return r;
  }
  function renderChar(root) {
    const g = game();
    const p = parseOr(g && g.heroPanel ? g.heroPanel() : "{}", {});
    root.textContent = "";
    if (!p.level) {
      root.appendChild(el("div", "foot", "Moteur non initialisé."));
      return;
    }
    const head = el("div", "phead");
    head.appendChild(el("h3", undefined, (p.className ?? "") + " — Niveau " + p.level));
    const xp = el("div", "xpbar");
    const xpFill = el("div", "xpfill");
    xpFill.style.width = (xpFrac(p.level, p.xp ?? 0) * 100).toFixed(1) + "%";
    xp.appendChild(xpFill);
    head.appendChild(xp);
    head.appendChild(el("div", "foot", "XP " + fmt(p.xp ?? 0) + " — prochain niveau dans " + fmt(p.xpToNext ?? 0)));
    root.appendChild(head);
    const cols = el("div", "pcols");
    const left = el("div", "pcol");
    left.appendChild(el("h4", undefined, "Caractéristiques"));
    left.appendChild(statRow("Force", p.strength));
    left.appendChild(statRow("Dextérité", p.dexterity));
    left.appendChild(statRow("Vitalité", p.vitality));
    left.appendChild(statRow("Énergie", p.energy));
    left.appendChild(statRow("Points à dépenser", (p.statPoints || 0) + " / " + (p.skillPoints || 0) + " (car. / comp.)", "dim"));
    left.appendChild(el("h4", undefined, "Vie et mana"));
    left.appendChild(statRow("Vie", p.life + " / " + p.maxLife));
    left.appendChild(statRow("Mana", p.mana + " / " + p.maxMana));
    left.appendChild(statRow("Endurance", p.stamina + " / " + p.maxStamina));
    const right = el("div", "pcol");
    right.appendChild(el("h4", undefined, "Combat"));
    right.appendChild(statRow("Dégâts", p.dmgMin + " – " + p.dmgMax, "gold"));
    right.appendChild(statRow("Arme", p.weapon || "—", "dim"));
    right.appendChild(statRow("Défense", p.defense));
    right.appendChild(statRow("Toucher", p.toHit));
    right.appendChild(el("h4", undefined, "Résistances (max " + (p.resMax ?? 75) + " %)"));
    const res = (label, v) => {
      right.appendChild(statRow(label, (v > 0 ? "+" : "") + v + " %", v < 0 ? "neg" : ""));
    };
    res("Feu", p.resFire ?? 0);
    res("Foudre", p.resLight ?? 0);
    res("Froid", p.resCold ?? 0);
    res("Poison", p.resPoison ?? 0);
    right.appendChild(el("h4", undefined, "Chance de trouver"));
    right.appendChild(statRow("Objets magiques (MF)", "+" + (p.mf ?? 0) + " %"));
    right.appendChild(statRow("Or (GF)", "+" + (p.gf ?? 0) + " %"));
    right.appendChild(statRow("Or porté", fmt(p.gold ?? 0) + " / " + fmt(p.goldCap ?? 0), "gold"));
    cols.appendChild(left);
    cols.appendChild(right);
    root.appendChild(cols);
    root.appendChild(el("div", "foot", "Dégâts dérivés de l'arme (×(1 + Force/100), formule du moteur) · résists : " + "pénalité de difficulté patch_difficultylevels (0/−40/−100) · or : plafond 10 000 × niveau."));
  }
  function gridEl(w, h, cell) {
    const g = el("div", "pgrid");
    g.style.gridTemplateColumns = "repeat(" + w + ", " + cell + "px)";
    return g;
  }
  function itemBlock(laid, onClick, onUse) {
    const it = laid.item;
    const b = laid.box;
    const colors = KIND_COLORS3[kindOf(it)] ?? KIND_COLORS3.Misc;
    const d = el("div", "pitem");
    d.style.left = b.x * CELL + "px";
    d.style.top = b.y * CELL + "px";
    d.style.width = b.w * CELL - 2 + "px";
    d.style.height = b.h * CELL - 2 + "px";
    d.style.background = colors[0];
    d.style.borderColor = colors[1];
    d.title = it.name + (it.amount ? " — " + it.amount + " or" : "") + " (" + b.w + "×" + b.h + ")" + (onUse ? " — double-clic : utiliser" : " — clic : saisir/poser");
    if (onUse)
      d.addEventListener("dblclick", (e) => {
        e.stopPropagation();
        onUse(it);
      });
    if (onClick)
      d.addEventListener("click", (e) => {
        e.stopPropagation();
        onClick(it, d);
      });
    if (state.sel && itemKey(state.sel) === itemKey(it))
      d.classList.add("sel");
    d.appendChild(el("span", "pitemname", it.name));
    return d;
  }
  function note(m) {
    const s = document.getElementById("msglog");
    if (s) {
      const line = el("div", undefined, "• " + m);
      s.appendChild(line);
      while (s.children.length > 12)
        s.removeChild(s.firstChild);
      s.scrollTop = s.scrollHeight;
    } else {
      console.info("[d2panels]", m);
    }
  }
  function useConsumable(it) {
    const g = game();
    if (!g || (it.slot ?? -1) < 0)
      return;
    const scrolls = ["tsc", "tbk", "isc", "ibk"];
    if (it.kind !== "Potion" && !scrolls.includes(it.code)) {
      note(it.name + " — équipement non branché au moteur (phase 3).");
      return;
    }
    try {
      const r = parseOr(g.useConsumable(it.slot), {});
      if (r.use) {
        const u = r.use;
        note(u.name + " : " + ((u.heal ?? 0) > 0 ? "+" + u.heal + " pv " : "") + ((u.mana ?? 0) > 0 ? "+" + u.mana + " mana " : "") + ((u.buffMs ?? 0) > 0 ? "effet " + Math.round((u.buffMs ?? 0) / 1000) + " s " : "") + (u.townPortal ? "retour au camp" : "").trim());
        if (r.levelSwitch && win.__d2?.refreshStatus)
          win.__d2.refreshStatus();
      }
    } catch (e) {
      note(e instanceof Error ? e.message : String(e));
    }
    state.sel = null;
    refreshOpen();
  }
  function tryBeltKey(n) {
    const g = game();
    if (!g || !g.useBeltKey)
      return;
    try {
      const r = parseOr(g.useBeltKey(n), {});
      if (r.use)
        note(r.use.name + " bue (touche " + n + ").");
      else
        note("Colonne " + n + " : vide.");
    } catch (e) {
      note(e instanceof Error ? e.message : String(e));
    }
    refreshOpen();
  }
  function renderInv(root) {
    root.textContent = "";
    const items = invItems();
    const laid = layoutOf(items);
    loadSizes();
    const doll = el("div", "pdoll");
    doll.style.width = 9 * CELL + "px";
    for (const s of SLOTS) {
      const d = el("div", "pslot");
      d.style.left = s[4] * CELL + "px";
      d.style.top = s[5] * CELL + "px";
      d.style.width = s[2] * CELL - 2 + "px";
      d.style.height = s[3] * CELL - 2 + "px";
      d.title = s[1] + " — équipement non branché au moteur (silhouette)";
      d.appendChild(el("span", "pslotname", s[1]));
      doll.appendChild(d);
    }
    root.appendChild(doll);
    const gridWrap = el("div", "pgridwrap");
    const grid = gridEl(INV_W, INV_H, CELL);
    grid.style.height = INV_H * CELL + "px";
    for (let y = 0;y < INV_H; y++) {
      for (let x = 0;x < INV_W; x++) {
        const c = el("div", "pcell");
        c.style.left = x * CELL + "px";
        c.style.top = y * CELL + "px";
        grid.appendChild(c);
      }
    }
    grid.addEventListener("click", () => {
      if (!state.sel)
        return;
      const selKey = itemKey(state.sel);
      const rest = laid.filter((l) => itemKey(l.item) !== selKey).map((l) => l.box);
      const s = sizeOf(state.sel.code);
      const spot = firstFitPure(s[0], s[1], INV_W, INV_H, rest);
      if (!spot) {
        note("Aucune place libre pour " + state.sel.name + " (" + s[0] + "×" + s[1] + ").");
        return;
      }
      state.layout[selKey] = { x: spot.x, y: spot.y, w: s[0], h: s[1] };
      note(state.sel.name + " posé en (" + spot.x + ", " + spot.y + ").");
      state.sel = null;
      refreshOpen();
    });
    for (const l of laid) {
      grid.appendChild(itemBlock(l, (it) => {
        if (state.sel && itemKey(state.sel) === itemKey(it))
          state.sel = null;
        else
          state.sel = it;
        refreshOpen();
      }, (it) => useConsumable(it)));
    }
    const over = laid.filter((l) => l.box.overflow);
    gridWrap.appendChild(grid);
    root.appendChild(gridWrap);
    if (over.length) {
      const ov = el("div", "pover");
      for (const l of over) {
        const chip = itemBlock({ item: l.item, box: { x: 0, y: 0, w: l.box.w, h: l.box.h } }, null, null);
        chip.classList.add("chip");
        chip.style.position = "static";
        ov.appendChild(chip);
      }
      root.appendChild(el("div", "foot", over.length + " objet(s) sans place visible (grille visuelle pleine)."));
      root.appendChild(ov);
    }
    const g = game();
    const belt = parseOr(g && g.beltInfo ? g.beltInfo() : "{}", {});
    const beltSec = el("div", "pbelthead");
    beltSec.appendChild(el("h4", undefined, "Ceinture (1-4)"));
    const bg = gridEl(BELT_W, BELT_H, CELL);
    bg.style.height = BELT_H * CELL + "px";
    for (let by = 0;by < BELT_H; by++) {
      for (let bx = 0;bx < BELT_W; bx++) {
        const bc = el("div", "pcell");
        bc.style.left = bx * CELL + "px";
        bc.style.top = by * CELL + "px";
        bg.appendChild(bc);
      }
    }
    const beltArr = belt && belt.belt || [];
    for (let col = 0;col < BELT_W; col++) {
      const cells = beltArr[col] ?? [];
      for (let row = 0;row < BELT_H; row++) {
        const cell = cells[row];
        if (!cell)
          continue;
        const cnum = col;
        const potion = cell.kind === "Potion" || cell.kind === "Misc";
        bg.appendChild(itemBlock({
          item: { code: cell.code, name: cell.name, kind: cell.kind, amount: 0, slot: -1 },
          box: { x: cnum, y: row, w: 1, h: 1 }
        }, null, potion ? () => tryBeltKey(cnum + 1) : null));
      }
    }
    beltSec.appendChild(bg);
    root.appendChild(beltSec);
    root.appendChild(el("div", "foot", "Miroir visuel : le moteur range chaque objet en 1×1 (écart documenté) — ici les tailles réelles " + "invwidth/invheight (patch_*.txt) sont empilées en première place libre ; l'index moteur est conservé pour les actions."));
  }
  function renderStash(root) {
    root.textContent = "";
    const g = game();
    const s = parseOr(g && g.stashInfo ? g.stashInfo() : "{}", {});
    if (!s.w) {
      root.appendChild(el("div", "foot", "Moteur non initialisé."));
      return;
    }
    const head = el("div", "phead");
    head.appendChild(el("h3", undefined, "STASH — " + fmt(s.gold ?? 0) + " / " + fmt(s.goldCap ?? 0) + " or"));
    head.appendChild(el("div", "foot", "Difficulté " + (s.difficulty ?? "normal") + " · bourse : " + fmt(stats().gold ?? 0) + " / " + fmt(s.carryCap ?? 0) + " or (10 000 × niveau)"));
    root.appendChild(head);
    function goldBtn(label, kind) {
      const b = el("button", undefined, label);
      b.addEventListener("click", () => {
        const gg = game();
        if (!gg || !gg.stashDeposit || !gg.stashWithdraw)
          return;
        const info = parseOr(gg.stashInfo ? gg.stashInfo() : "{}", {});
        const carried = stats().gold ?? 0;
        const dep = typeof kind === "number" && kind > 0 || kind === "dep";
        let amt;
        if (kind === "dep")
          amt = Math.min(carried, Math.max(0, (info.goldCap ?? 0) - (info.gold ?? 0)));
        else if (kind === "wd")
          amt = info.gold ?? 0;
        else
          amt = Math.abs(kind);
        if (amt <= 0) {
          note(dep ? "Dépôt refusé : " + (carried <= 0 ? "bourse vide." : "stash plein.") : "Retrait refusé : stash vide.");
          return;
        }
        try {
          const v = parseOr(dep ? gg.stashDeposit(amt) : gg.stashWithdraw(amt), {});
          note((dep ? "Déposé " : "Retiré ") + fmt(amt) + " or (stash " + fmt(v.gold ?? 0) + ").");
        } catch (e) {
          note((e instanceof Error ? e.message : String(e)).replace(/^Error: /, ""));
        }
        refreshOpen();
      });
      return b;
    }
    const row = el("div", "pgoldrow");
    row.appendChild(goldBtn("Déposer 100", 100));
    row.appendChild(goldBtn("Déposer 1 000", 1000));
    row.appendChild(goldBtn("Déposer tout", "dep"));
    row.appendChild(goldBtn("Retirer 100", -100));
    row.appendChild(goldBtn("Retirer 1 000", -1000));
    row.appendChild(goldBtn("Retirer tout", "wd"));
    root.appendChild(row);
    const grid = gridEl(s.w ?? 6, s.h ?? 4, CELL);
    grid.style.height = (s.h ?? 4) * CELL + "px";
    for (let y = 0;y < (s.h ?? 4); y++) {
      for (let x = 0;x < (s.w ?? 6); x++) {
        const c = el("div", "pcell");
        c.style.left = x * CELL + "px";
        c.style.top = y * CELL + "px";
        grid.appendChild(c);
      }
    }
    for (const it of s.items ?? []) {
      const colors = KIND_COLORS3.Misc;
      const d = el("div", "pitem");
      d.style.left = it.x * CELL + "px";
      d.style.top = it.y * CELL + "px";
      d.style.width = it.w * CELL - 2 + "px";
      d.style.height = it.h * CELL - 2 + "px";
      d.style.background = colors[0];
      d.style.borderColor = colors[1];
      d.title = it.name + " (" + it.w + "×" + it.h + ") — taille réelle du moteur";
      d.appendChild(el("span", "pitemname", it.name));
      grid.appendChild(d);
    }
    root.appendChild(grid);
    root.appendChild(el("div", "foot", "Dépôt/retrait : branchement economy.rs RÉEL (bourse débitée/créditée, plafonds " + "300k/500k/800k par difficulté, porté 10 000 × niveau) — save additive `gold_stash`."));
  }
  function refreshOpen() {
    for (const id of Object.keys(state.panels)) {
      const p = state.panels[id];
      if (p.open)
        render(id);
    }
  }
  function render(id) {
    const p = state.panels[id];
    if (!p)
      return;
    if (id === "char")
      renderChar(p.body);
    else if (id === "inv")
      renderInv(p.body);
    else if (id === "stash")
      renderStash(p.body);
  }
  function buildPanel(id, title) {
    const root = el("div", "d2panel");
    root.id = "d2panel-" + id;
    const bar = el("div", "pbar");
    bar.appendChild(el("span", "ptitle", title));
    const close = el("button", "pclose", "×");
    close.title = "Fermer (Échap)";
    close.addEventListener("click", () => togglePanel(id, false));
    bar.appendChild(close);
    root.appendChild(bar);
    const body = el("div", "pbody");
    root.appendChild(body);
    document.body.appendChild(root);
    const rec = { root, body, open: false };
    state.panels[id] = rec;
    return rec;
  }
  function togglePanel(id, force) {
    const p = state.panels[id] ?? buildPanel(id, id === "char" ? "PERSONNAGE" : id === "inv" ? "INVENTAIRE" : "STASH");
    p.open = force !== undefined ? force : !p.open;
    p.root.style.display = p.open ? "block" : "none";
    if (p.open) {
      render(id);
      for (const other of Object.keys(state.panels)) {
        if (other !== id && state.panels[other].open)
          togglePanel(other, false);
      }
    }
  }
  function injectCss() {
    const css = el("style");
    css.textContent = ".d2panel{position:fixed;top:64px;z-index:60;display:none;width:auto;max-width:94vw;" + "background:#141019f5;border:1px solid #5a4a2a;border-radius:4px;padding:10px;" + "color:#e8dcbf;font:11px/1.35 system-ui,sans-serif;box-shadow:0 4px 18px #000d}" + "#d2panel-char{left:16px}#d2panel-inv{left:50%;transform:translateX(-50%)}#d2panel-stash{right:16px}" + ".d2panel .pbar{display:flex;justify-content:space-between;align-items:center;margin-bottom:6px}" + ".d2panel .ptitle{color:#c7a45a;font-weight:700;letter-spacing:.1em;font-size:12px}" + ".d2panel .pclose{background:#241c10;color:#c7a45a;border:1px solid #4a3c22;border-radius:3px;" + "width:20px;height:20px;cursor:pointer;font-weight:700}" + ".d2panel .pclose:hover{border-color:#c7a45a;color:#ffe9ad}" + ".d2panel h3{margin:0 0 4px;color:#c7a45a;font-size:12px;letter-spacing:.08em}" + ".d2panel h4{margin:8px 0 3px;color:#a89670;font-size:10px;text-transform:uppercase;letter-spacing:.09em;" + "border-bottom:1px solid #2e2718;padding-bottom:2px}" + ".d2panel .phead{margin-bottom:6px}.d2panel .phead .foot{margin-top:2px}" + ".d2panel .pcols{display:flex;gap:18px}.d2panel .pcol{min-width:172px}" + ".d2panel .prow{display:flex;justify-content:space-between;gap:12px;padding:1px 0}" + ".d2panel .plab{color:#b8ad98}.d2panel .pval{color:#f0e6d2;font-weight:600}" + ".d2panel .prow.gold .pval{color:#d9b96a}.d2panel .prow.dim .pval{color:#8f8676;font-weight:400}" + ".d2panel .prow.neg .pval{color:#d8503c}" + ".d2panel .xpbar{height:7px;background:#16120ccc;border:1px solid #000000aa;border-radius:3px;" + "margin:3px 0 2px;overflow:hidden}" + ".d2panel .xpfill{height:100%;background:linear-gradient(#d8b23a,#7a5c14);border-radius:2px}" + ".d2panel .foot{font:9px system-ui;color:#8f8676;margin-top:6px;max-width:420px}" + ".pgridwrap{position:relative}.pgrid{position:relative;display:grid;margin:4px 0;" + "background:#100d14;border:1px solid #3a3226;border-radius:2px}" + ".pcell{position:absolute;background:#1d1712;border:1px solid #3a3226;" + "box-shadow:inset 0 1px 0 #ffffff08}" + ".pitem{position:absolute;box-sizing:border-box;border:1px solid;border-radius:2px;" + "display:flex;align-items:flex-end;justify-content:center;overflow:hidden;cursor:pointer;" + "box-shadow:inset 0 1px 0 #ffffff22,0 1px 3px #000a;z-index:2}" + ".pitem:hover{outline:1px solid #ffe9ad;filter:brightness(1.15)}" + ".pitem.sel{outline:2px solid #d9b96a;box-shadow:0 0 8px #d9b96a88}" + ".pitemname{font:8px system-ui;color:#ffe9b0;text-shadow:0 1px 2px #000;padding:0 2px;" + "max-height:100%;overflow:hidden;word-break:break-word;text-align:center}" + ".pitem.chip{position:static;display:inline-flex;min-width:52px;min-height:20px;padding:2px 6px;margin:2px}" + ".pdoll{position:relative;height:" + 5 * CELL + "px;margin-bottom:8px;" + "background:#100d14;border:1px solid #3a3226;border-radius:2px}" + ".pslot{position:absolute;box-sizing:border-box;background:repeating-linear-gradient(45deg,#181318,#181318 4px,#1d1712 4px,#1d1712 8px);" + "border:1px dashed #3a3226;border-radius:2px;display:flex;align-items:center;justify-content:center}" + ".pslotname{font:8px system-ui;color:#5f5648;letter-spacing:.05em}" + ".pbelthead h4{margin-top:10px}" + ".pgoldrow{display:flex;flex-wrap:wrap;gap:4px;margin-bottom:6px}" + ".pgoldrow button{padding:3px 7px;font-size:11px;background:#241c10;color:#d9b96a;" + "border:1px solid #4a3c22;border-radius:3px;cursor:pointer}" + ".pgoldrow button:hover{border-color:#c7a45a;color:#ffe9ad}";
    document.head.appendChild(css);
  }
  function injectButtons() {
    const toolbar = document.getElementById("toolbar");
    if (!toolbar)
      return;
    const defs = [
      ["Perso", "Panneau personnage (C)", "char"],
      ["Inventaire", "Inventaire (I)", "inv"],
      ["Stash", "Stash 6×4 + or (B)", "stash"]
    ];
    for (const [label, title, id] of defs) {
      const b = el("button", undefined, label);
      b.id = "btn-panel-" + id;
      b.title = title;
      b.addEventListener("click", () => togglePanel(id));
      toolbar.appendChild(b);
    }
  }
  function onKey(e) {
    if (e.ctrlKey || e.altKey || e.metaKey)
      return;
    const k = (e.key || "").toLowerCase();
    if (k === "i") {
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel("inv");
    } else if (k === "c") {
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel("char");
    } else if (k === "b") {
      e.preventDefault();
      e.stopImmediatePropagation();
      togglePanel("stash");
    } else if (e.key === "Escape") {
      for (const id of Object.keys(state.panels)) {
        if (state.panels[id].open)
          togglePanel(id, false);
      }
    }
  }
  function init() {
    if (state.inited)
      return;
    state.inited = true;
    try {
      state.layout = parseOr(localStorage.getItem(PANELS_KEY) || "{}", {});
    } catch {
      state.layout = {};
    }
    injectCss();
    injectButtons();
    window.addEventListener("keydown", onKey, true);
  }
  function initLive() {
    if (state.live)
      return;
    state.live = true;
    setInterval(refreshOpen, 1500);
  }
  init();
  (function whenReady() {
    const g = game();
    if (g && g.isFinalized && g.isFinalized())
      initLive();
    else
      setTimeout(whenReady, 400);
  })();
  win.D2Panels = {
    toggle: togglePanel,
    open: (id) => togglePanel(id, true),
    refresh: refreshOpen,
    state
  };
}
function installPanels(win) {
  if (win.D2Panels) {
    registerModule("panels-ui", "adopt");
    return;
  }
  installTsPanels(win);
}
if (typeof window !== "undefined") {
  await warmLegacy("panels-ui.js");
  installPanels(window);
}

// src/main.ts
if (typeof window !== "undefined") {
  window.__d2Ts = {
    version: "25a",
    step: "migration TypeScript",
    installed: TS25A_REGISTRY.installed,
    adopted: TS25A_REGISTRY.adopted
  };
}
