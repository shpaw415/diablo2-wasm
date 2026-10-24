/* @ts-self-types="./d2game.d.ts" */

export class D2Game {
    static __wrap(ptr) {
        const obj = Object.create(D2Game.prototype);
        obj.__wbg_ptr = ptr;
        D2GameFinalization.register(obj, obj.__wbg_ptr, obj);
        return obj;
    }
    __destroy_into_raw() {
        const ptr = this.__wbg_ptr;
        this.__wbg_ptr = 0;
        D2GameFinalization.unregister(this);
        return ptr;
    }
    free() {
        const ptr = this.__destroy_into_raw();
        wasm.__wbg_d2game_free(ptr, 0);
    }
    /**
     * Mini-carte du niveau courant (9d) — JSON
     * `{level,name,mode,lvlType,cellW,cellH,sub,cells:[[x,y,flags,cel],…]}`
     * (voir `automap.rs` : mode `automap` / fallback polygone marchable).
     * @returns {string}
     */
    automap() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_automap(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * « Auto-ranger » : déplace les potions `autobelt=1` (hp1-5, mp1-5,
     * rvs, rvl) de l'inventaire vers la ceinture 4×4 — nombre rangé
     * (idempotent ; la page l'appelle après un ramassage).
     * @returns {number}
     */
    beltAutoStore() {
        const ret = wasm.d2game_beltAutoStore(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * État ceinture + buffs (lecture HUD) — JSON
     * `{"belt":[[4×4 cellules null|{code,name,kind}]],"buffs":[{"state",
     * "remainingMs","stats":[[id,val],…]},…]}` ; `{}` avant `finalize`.
     * @returns {string}
     */
    beltInfo() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_beltInfo(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Collision grid as 0/1 bytes (row-major), for d2rules/debug overlays.
     * @returns {Uint8Array}
     */
    collisionGrid() {
        const ret = wasm.d2game_collisionGrid(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Compositor stats — surfaced in the demo status line.
     * @returns {Uint32Array}
     */
    composeStats() {
        const ret = wasm.d2game_composeStats(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Corps au sol (J1) — JSON `{"x","y","sx","sy","items"}` ou `null`.
     * Pollé par corpse-ui.js (flare + hint) et le clic « récupérer ».
     * @returns {string}
     */
    corpseInfo() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_corpseInfo(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Current difficulty as JSON `{"difficulty":"hell","label":"Hell"}` —
     * HUD badge + E2E probe.
     * @returns {string}
     */
    difficultyInfo() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_difficultyInfo(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Compose the map + build sprites + spawn the world. Call once.
     */
    finalize() {
        const ret = wasm.d2game_finalize(this.__wbg_ptr);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {Uint32Array}
     */
    gridSize() {
        const ret = wasm.d2game_gridSize(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Objets au sol projetés (EXP-H8, labels Alt) — JSON array
     * `[{index,code,name,kind,amount,x,y,sx,sy},…]`.
     * @returns {string}
     */
    groundItems() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_groundItems(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Resolved hero class as JSON `{"code":"BA","name":"Barbare"}` — HUD
     * display + E2E probe. Valid before `finalize` too.
     * @returns {string}
     */
    heroClass() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_heroClass(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Hero feet position in viewport px — for the JS click ring / cursor.
     * @returns {Float32Array}
     */
    heroScreen() {
        const ret = wasm.d2game_heroScreen(this.__wbg_ptr);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Compétences connues du héros (EXP-H4) — JSON array
     * `[{index,code,name,level,missile},…]` pour les boutons L/R de hud.js.
     * @returns {string}
     */
    heroSkills() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_heroSkills(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Hero animation state as JSON `{"anim":"RN","frame":3,"facing":6,
     * "run":true,"moving":false}` — HUD run flag + E2E probes.
     * @returns {string}
     */
    heroState() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_heroState(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Hero HUD stats (`HeroStats`) as a JSON object string — level, life,
     * mana, xp, gold… polled by the demo HUD each frame.
     * @returns {string}
     */
    heroStats() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_heroStats(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Inventory (40 slots, `null` = empty) as a JSON array string.
     * @returns {string}
     */
    inventory() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_inventory(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Apprend/monte le skill `code` (dépense 1 point) — JSON
     * `{"ok":true,"level":2,"points":1}` ou `{"ok":false,"error":"…"}`.
     * @param {string} code
     * @returns {string}
     */
    learnSkill(code) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ptr0 = passStringToWasm0(code, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            const len0 = WASM_VECTOR_LEN;
            const ret = wasm.d2game_learnSkill(this.__wbg_ptr, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Current level as JSON `{"key":"bloodmoor","name":"Blood Moor (Acte I)"}`
     * — HUD zone name + level-switch banner. `{}` if the key is unknown.
     * @returns {string}
     */
    levelInfo() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_levelInfo(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Restore a world from `saveGame` JSON. Throws (`JsValue`) on invalid
     * save or a save from another map. Map/sprites are untouched.
     * @param {string} json
     */
    loadGame(json) {
        const ptr0 = passStringToWasm0(json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.d2game_loadGame(this.__wbg_ptr, ptr0, len0);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * @returns {Uint32Array}
     */
    mapPixelSize() {
        const ret = wasm.d2game_mapPixelSize(this.__wbg_ptr);
        var v1 = getArrayU32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Fiche du mercenaire (H) — JSON
     * `{kind,subtype,level,hp,maxHp,alive,x,y,sx,sy}` ou `null` (aucun
     * mercenaire embauché). Pollé chaque frame par merc-ui.js.
     * @returns {string}
     */
    mercInfo() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_mercInfo(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Monster codes for the current level, parallel to the tick's monster
     * indices — debug/E2E. JSON `["FA","ZM",…]`.
     * @returns {string}
     */
    monsterCodes() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_monsterCodes(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Fiche de survol du monstre `index` (EXP-H6) — JSON
     * `{code,name,kind,alive,hp,maxHp,x,y,sx,sy}` ou `null`.
     * @param {number} index
     * @returns {string}
     */
    monsterInfo(index) {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_monsterInfo(this.__wbg_ptr, index);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Alive monster index nearest to a world point within `radius`
     * sub-tiles, or -1 (JS click router: attack vs move).
     * @param {number} wx
     * @param {number} wy
     * @param {number} radius
     * @returns {number}
     */
    monsterNear(wx, wy, radius) {
        const ret = wasm.d2game_monsterNear(this.__wbg_ptr, wx, wy, radius);
        return ret;
    }
    /**
     * `manifest_json` — the raw `assets/manifest.json` text. `hero` — the
     * selected class code (`"BA"`, `"SOR"`, `"PA"`…, case-insensitive, default
     * `"BA"`): the hero's COF/DCC layers AND stats both come from
     * `manifest.heroes[code]` when present, else the legacy `hero` block.
     * Initial level: `"town"`.
     * @param {string} manifest_json
     * @param {string} hero
     */
    constructor(manifest_json, hero) {
        const ptr0 = passStringToWasm0(manifest_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(hero, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ret = wasm.d2game_new(ptr0, len0, ptr1, len1);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        this.__wbg_ptr = ret[0];
        D2GameFinalization.register(this, this.__wbg_ptr, this);
        return this;
    }
    /**
     * NPC de ville (W16, villes vivantes) — JSON array
     * `[{index,id,code,name,questCode,role,x,y,vendor,assumed,talked},…]`
     * (coordonnées cellules ; projection écran ancrée sur `heroScreen()`).
     * `[]` hors camp. Contrat UI : demo/npc-ui.js (`npc_bridge.rs`).
     * @returns {string}
     */
    npcs() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_npcs(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Feed one asset blob (manifest path + bytes), e.g. fetched files.
     * Accepted any time — including between level switches (hot provide).
     * @param {string} path
     * @param {Uint8Array} bytes
     */
    provide(path, bytes) {
        const ptr0 = passStringToWasm0(path, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passArray8ToWasm0(bytes, wasm.__wbindgen_malloc);
        const len1 = WASM_VECTOR_LEN;
        wasm.d2game_provide(this.__wbg_ptr, ptr0, len0, ptr1, len1);
    }
    /**
     * RGBA framebuffer (`viewport_width * viewport_height * 4`) for
     * `putImageData` — floor, shadows, entities already composited.
     * @returns {Uint8Array}
     */
    render() {
        const ret = wasm.d2game_render(this.__wbg_ptr);
        var v1 = getArrayU8FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 1, 1);
        return v1;
    }
    /**
     * Serialize the world (JSON v1) — demo Save button → localStorage.
     * @returns {string}
     */
    saveGame() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_saveGame(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Click (viewport px) → world sub-tile coords `[x, y]`.
     * @param {number} px
     * @param {number} py
     * @returns {Float32Array}
     */
    screenToWorld(px, py) {
        const ret = wasm.d2game_screenToWorld(this.__wbg_ptr, px, py);
        var v1 = getArrayF32FromWasm0(ret[0], ret[1]).slice();
        wasm.__wbindgen_free(ret[0], ret[1] * 4, 4);
        return v1;
    }
    /**
     * Sélectionne la compétence active (main droite D2, EXP-H4) — l'input
     * `RangedAttack` consomme ce skill. `false` si hors bornes.
     * @param {number} index
     * @returns {boolean}
     */
    selectSkill(index) {
        const ret = wasm.d2game_selectSkill(this.__wbg_ptr, index);
        return ret !== 0;
    }
    /**
     * Set the difficulty (`"normal"`, `"nm"`, `"nightmare"`, `"hell"` —
     * case-insensitive; unknown = normal). Call BEFORE `finalize` so the
     * boot level spawns difficulty-scaled monsters (MonLvl (N)/(H), MonDen/
     * MonUMin/Max (N)/(H), Res*(N)/(H) immunities). W08/K impl-difficultes.
     * @param {string} diff
     */
    setDifficulty(diff) {
        const ptr0 = passStringToWasm0(diff, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        wasm.d2game_setDifficulty(this.__wbg_ptr, ptr0, len0);
    }
    /**
     * @param {number} w
     * @param {number} h
     */
    setViewport(w, h) {
        wasm.d2game_setViewport(this.__wbg_ptr, w, h);
    }
    /**
     * Arbre complet de la classe du héros — JSON
     * `{class, hero, points, charLevel, skills:[{code,name,nameKey,level,
     * maxLevel,reqLevel,prereqs,page,row,column,icon,missile,mana,canLearn,
     * mvp},…]}` (30 skills/classe, tables réelles v1.12 embarquées).
     * @returns {string}
     */
    skillTree() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_skillTree(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Animation modes listed in the manifest but skipped (missing files),
     * as a JSON array of strings — the page logs them, the game still runs.
     * @returns {string}
     */
    spriteNotes() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_spriteNotes(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Switch level: reloads DS1/DT1/collision from the provided (or hot-
     * provided) bytes, carries the hero over (stats/inventory/xp), spawns the
     * target level's monsters, and lands the hero at `(spawn_x, spawn_y)`
     * (snapped to the nearest walkable sub-tile). Throws on unknown level or
     * missing assets.
     * @param {string} name
     * @param {number} spawn_x
     * @param {number} spawn_y
     */
    switchLevel(name, spawn_x, spawn_y) {
        const ptr0 = passStringToWasm0(name, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ret = wasm.d2game_switchLevel(this.__wbg_ptr, ptr0, len0, spawn_x, spawn_y);
        if (ret[1]) {
            throw takeFromExternrefTable0(ret[0]);
        }
    }
    /**
     * Level-switch notes (degraded hero transfers…) as a JSON array.
     * @returns {string}
     */
    switchNotes() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_switchNotes(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Clic NPC (W16) : interaction si le héros est à portée (2.5 cellules) —
     * JSON `NpcInteraction` `{name,role,dialogue,actions,healed,quest_rewards}`
     * ou `Err` descriptif (`"trop loin — approchez-vous du NPC"`). Émet
     * `NpcTalked` (flux de tick) et avance le journal de quêtes branché
     * (q3/q4).
     * @param {number} index
     * @returns {string}
     */
    talkNpc(index) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.d2game_talkNpc(this.__wbg_ptr, index);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Advance the simulation. `input_json` (nullable) mirrors the
     * `PlayerInput` contract:
     * `{"type":"move","x":123,"y":45}` · `{"type":"attack","x":..,"y":..}`
     * · `{"type":"pickup"}` · `{"type":"setRun","on":true}`
     * · `{"type":"useItem","slot":0}` · `{"type":"recoverCorpse"}` (J1).
     * Returns the tick's events as a JSON array string: the d2rules contract
     * events ({"HeroMoved":…}…) plus the engine's multi-level extras
     * `{"LevelSwitch":{"key":…,"name":…}}` / `{"LevelSwitchFailed":"…"}` —
     * entering a warp zone switches the level INSIDE the tick, so the very
     * next frame already renders the new level.
     * @param {number} dt_ms
     * @param {string | null} [input_json]
     * @returns {string}
     */
    tick(dt_ms, input_json) {
        let deferred2_0;
        let deferred2_1;
        try {
            var ptr0 = isLikeNone(input_json) ? 0 : passStringToWasm0(input_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
            var len0 = WASM_VECTOR_LEN;
            const ret = wasm.d2game_tick(this.__wbg_ptr, dt_ms, ptr0, len0);
            deferred2_0 = ret[0];
            deferred2_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Direct travel to an ACTIVATED waypoint (W05): reloads the target
     * level (seeded regen, cf. impl-drlg) and lands the hero on the target
     * stone. Returns the transition JSON `{"slot","key","name"}`; throws
     * on unknown slot, non-activated waypoint or level missing from the
     * port's manifest.
     * @param {number} slot
     * @returns {string}
     */
    travelWaypoint(slot) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.d2game_travelWaypoint(this.__wbg_ptr, slot);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Touche de ceinture `key` (1..=4) — boit le premier objet de la
     * colonne (rangée 0 d'abord), mêmes effets/JSON que `useConsumable`.
     * @param {number} key
     * @returns {string}
     */
    useBeltKey(key) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.d2game_useBeltKey(this.__wbg_ptr, key);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * Utilise le consommable du slot d'inventaire `slot` (effets réels par
     * code — potions hp1-5/mp1-5/rejuv/buffs 30 s, parchemin de porte de
     * ville, tomes à charges) — JSON
     * `{"use":{code,name,family,heal,mana,buffMs,townPortal,identified,
     * chargesLeft},"levelSwitch":{key,name}?}` ; un parchemin/tome de porte
     * de ville BASCULE vers le camp (switch_level existant). `Err` lisible
     * (`"slot vide"`, `"tome vide"`, `"rien à identifier"`…).
     * @param {number} slot
     * @returns {string}
     */
    useConsumable(slot) {
        let deferred2_0;
        let deferred2_1;
        try {
            const ret = wasm.d2game_useConsumable(this.__wbg_ptr, slot);
            var ptr1 = ret[0];
            var len1 = ret[1];
            if (ret[3]) {
                ptr1 = 0; len1 = 0;
                throw takeFromExternrefTable0(ret[2]);
            }
            deferred2_0 = ptr1;
            deferred2_1 = len1;
            return getStringFromWasm0(ptr1, len1);
        } finally {
            wasm.__wbindgen_free(deferred2_0, deferred2_1, 1);
        }
    }
    /**
     * @returns {number}
     */
    viewportHeight() {
        const ret = wasm.d2game_viewportHeight(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * @returns {number}
     */
    viewportWidth() {
        const ret = wasm.d2game_viewportWidth(this.__wbg_ptr);
        return ret >>> 0;
    }
    /**
     * Waypoints (W05) — full panel state as JSON:
     * `{"current":{"key","name","levelId","slot","activated"},"list":[…]}`
     * (see `waypoint_travel.rs` / `d2rules::waypoints` for the shape).
     * @returns {string}
     */
    waypoints() {
        let deferred1_0;
        let deferred1_1;
        try {
            const ret = wasm.d2game_waypoints(this.__wbg_ptr);
            deferred1_0 = ret[0];
            deferred1_1 = ret[1];
            return getStringFromWasm0(ret[0], ret[1]);
        } finally {
            wasm.__wbindgen_free(deferred1_0, deferred1_1, 1);
        }
    }
    /**
     * Like [`D2Game::new`] with the initial level key from the manifest's
     * `levels` block (`"town"`, `"bloodmoor"`…, case-insensitive).
     * @param {string} manifest_json
     * @param {string} hero
     * @param {string} level
     * @returns {D2Game}
     */
    static withLevel(manifest_json, hero, level) {
        const ptr0 = passStringToWasm0(manifest_json, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len0 = WASM_VECTOR_LEN;
        const ptr1 = passStringToWasm0(hero, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len1 = WASM_VECTOR_LEN;
        const ptr2 = passStringToWasm0(level, wasm.__wbindgen_malloc, wasm.__wbindgen_realloc);
        const len2 = WASM_VECTOR_LEN;
        const ret = wasm.d2game_withLevel(ptr0, len0, ptr1, len1, ptr2, len2);
        if (ret[2]) {
            throw takeFromExternrefTable0(ret[1]);
        }
        return D2Game.__wrap(ret[0]);
    }
}
if (Symbol.dispose) D2Game.prototype[Symbol.dispose] = D2Game.prototype.free;
function __wbg_get_imports() {
    const import0 = {
        __proto__: null,
        __wbg___wbindgen_throw_41e9ee4f547fc59a: function(arg0, arg1) {
            throw new Error(getStringFromWasm0(arg0, arg1));
        },
        __wbg_error_b67576c5bba2dcb1: function(arg0, arg1) {
            console.error(getStringFromWasm0(arg0, arg1));
        },
        __wbindgen_generic_0000000000000001: function(arg0, arg1) {
            // Cast intrinsic for `Ref(String) -> Externref`.
            const ret = getStringFromWasm0(arg0, arg1);
            return ret;
        },
        __wbindgen_init_externref_table: function() {
            const table = wasm.__wbindgen_externrefs;
            const offset = table.grow(4);
            table.set(0, undefined);
            table.set(offset + 0, undefined);
            table.set(offset + 1, null);
            table.set(offset + 2, true);
            table.set(offset + 3, false);
        },
    };
    return {
        __proto__: null,
        "./d2game_bg.js": import0,
    };
}

const D2GameFinalization = (typeof FinalizationRegistry === 'undefined')
    ? { register: () => {}, unregister: () => {} }
    : new FinalizationRegistry(ptr => wasm.__wbg_d2game_free(ptr, 1));

function getArrayF32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getFloat32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU32FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint32ArrayMemory0().subarray(ptr / 4, ptr / 4 + len);
}

function getArrayU8FromWasm0(ptr, len) {
    ptr = ptr >>> 0;
    return getUint8ArrayMemory0().subarray(ptr / 1, ptr / 1 + len);
}

let cachedFloat32ArrayMemory0 = null;
function getFloat32ArrayMemory0() {
    if (cachedFloat32ArrayMemory0 === null || cachedFloat32ArrayMemory0.byteLength === 0) {
        cachedFloat32ArrayMemory0 = new Float32Array(wasm.memory.buffer);
    }
    return cachedFloat32ArrayMemory0;
}

function getStringFromWasm0(ptr, len) {
    return decodeText(ptr >>> 0, len);
}

let cachedUint32ArrayMemory0 = null;
function getUint32ArrayMemory0() {
    if (cachedUint32ArrayMemory0 === null || cachedUint32ArrayMemory0.byteLength === 0) {
        cachedUint32ArrayMemory0 = new Uint32Array(wasm.memory.buffer);
    }
    return cachedUint32ArrayMemory0;
}

let cachedUint8ArrayMemory0 = null;
function getUint8ArrayMemory0() {
    if (cachedUint8ArrayMemory0 === null || cachedUint8ArrayMemory0.byteLength === 0) {
        cachedUint8ArrayMemory0 = new Uint8Array(wasm.memory.buffer);
    }
    return cachedUint8ArrayMemory0;
}

function isLikeNone(x) {
    return x === undefined || x === null;
}

function passArray8ToWasm0(arg, malloc) {
    const ptr = malloc(arg.length * 1, 1) >>> 0;
    getUint8ArrayMemory0().set(arg, ptr / 1);
    WASM_VECTOR_LEN = arg.length;
    return ptr;
}

function passStringToWasm0(arg, malloc, realloc) {
    if (realloc === undefined) {
        const buf = cachedTextEncoder.encode(arg);
        const ptr = malloc(buf.length, 1) >>> 0;
        getUint8ArrayMemory0().subarray(ptr, ptr + buf.length).set(buf);
        WASM_VECTOR_LEN = buf.length;
        return ptr;
    }

    let len = arg.length;
    let ptr = malloc(len, 1) >>> 0;

    const mem = getUint8ArrayMemory0();

    let offset = 0;

    for (; offset < len; offset++) {
        const code = arg.charCodeAt(offset);
        if (code > 0x7F) break;
        mem[ptr + offset] = code;
    }
    if (offset !== len) {
        if (offset !== 0) {
            arg = arg.slice(offset);
        }
        ptr = realloc(ptr, len, len = offset + arg.length * 3, 1) >>> 0;
        const view = getUint8ArrayMemory0().subarray(ptr + offset, ptr + len);
        const ret = cachedTextEncoder.encodeInto(arg, view);

        offset += ret.written;
        ptr = realloc(ptr, len, offset, 1) >>> 0;
    }

    WASM_VECTOR_LEN = offset;
    return ptr;
}

function takeFromExternrefTable0(idx) {
    const value = wasm.__wbindgen_externrefs.get(idx);
    wasm.__externref_table_dealloc(idx);
    return value;
}

let cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
cachedTextDecoder.decode();
const MAX_SAFARI_DECODE_BYTES = 2146435072;
let numBytesDecoded = 0;
function decodeText(ptr, len) {
    numBytesDecoded += len;
    if (numBytesDecoded >= MAX_SAFARI_DECODE_BYTES) {
        cachedTextDecoder = new TextDecoder('utf-8', { ignoreBOM: true, fatal: true });
        cachedTextDecoder.decode();
        numBytesDecoded = len;
    }
    return cachedTextDecoder.decode(getUint8ArrayMemory0().subarray(ptr, ptr + len));
}

const cachedTextEncoder = new TextEncoder();

if (!('encodeInto' in cachedTextEncoder)) {
    cachedTextEncoder.encodeInto = function (arg, view) {
        const buf = cachedTextEncoder.encode(arg);
        view.set(buf);
        return {
            read: arg.length,
            written: buf.length
        };
    };
}

let WASM_VECTOR_LEN = 0;

let wasmModule, wasmInstance, wasm;
function __wbg_finalize_init(instance, module) {
    wasmInstance = instance;
    wasm = instance.exports;
    wasmModule = module;
    cachedFloat32ArrayMemory0 = null;
    cachedUint32ArrayMemory0 = null;
    cachedUint8ArrayMemory0 = null;
    wasm.__wbindgen_start();
    return wasm;
}

async function __wbg_load(module, imports) {
    if (typeof Response === 'function' && module instanceof Response) {
        if (!module.ok) {
            throw new Error(`failed to fetch Wasm: ${module.status} ${module.statusText} fetching '${module.url}'`);
        }

        if (typeof WebAssembly.instantiateStreaming === 'function') {
            try {
                return await WebAssembly.instantiateStreaming(module, imports);
            } catch (e) {
                const validResponse = expectedResponseType(module.type);

                if (validResponse && module.headers.get('Content-Type') !== 'application/wasm') {
                    console.warn("`WebAssembly.instantiateStreaming` failed because your server does not serve Wasm with `application/wasm` MIME type. Falling back to `WebAssembly.instantiate` which is slower. Original error:\n", e);

                } else { throw e; }
            }
        }

        const bytes = await module.arrayBuffer();
        return await WebAssembly.instantiate(bytes, imports);
    } else {
        const instance = await WebAssembly.instantiate(module, imports);

        if (instance instanceof WebAssembly.Instance) {
            return { instance, module };
        } else {
            return instance;
        }
    }

    function expectedResponseType(type) {
        switch (type) {
            case 'basic': case 'cors': case 'default': return true;
        }
        return false;
    }
}

function initSync(module) {
    if (wasm !== undefined) return wasm;


    if (module !== undefined) {
        if (Object.getPrototypeOf(module) === Object.prototype) {
            ({module} = module)
        } else {
            console.warn('using deprecated parameters for `initSync()`; pass a single object instead')
        }
    }

    const imports = __wbg_get_imports();
    if (!(module instanceof WebAssembly.Module)) {
        module = new WebAssembly.Module(module);
    }
    const instance = new WebAssembly.Instance(module, imports);
    return __wbg_finalize_init(instance, module);
}

async function __wbg_init(module_or_path) {
    if (wasm !== undefined) return wasm;


    if (module_or_path !== undefined) {
        if (Object.getPrototypeOf(module_or_path) === Object.prototype) {
            ({module_or_path} = module_or_path)
        } else {
            console.warn('using deprecated parameters for the initialization function; pass a single object instead')
        }
    }

    if (module_or_path === undefined) {
        module_or_path = new URL('d2game_bg.wasm', import.meta.url);
    }
    const imports = __wbg_get_imports();

    if (typeof module_or_path === 'string' || (typeof Request === 'function' && module_or_path instanceof Request) || (typeof URL === 'function' && module_or_path instanceof URL)) {
        module_or_path = fetch(module_or_path);
    }

    const { instance, module } = await __wbg_load(await module_or_path, imports);

    return __wbg_finalize_init(instance, module);
}

export { initSync, __wbg_init as default };
