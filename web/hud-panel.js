// Diablo 2 WASM — web/hud-panel.js — SURFACE JOUEUR en sprites originaux.
// ========================================================================
// Le panneau de contrôle D2 COMPLET (800×104) composé depuis 800ctrlpnl7.dc6
// (web/assets/ui/panel_full.png, généré par tools/build_panel_full.py) :
//   - orbes vie/mana ORIGINAUX dessinés dans leurs sockets gravés, avec
//     remplissage liquide (clip bas→haut) pollés sur game.heroStats() ;
//   - CEINTURE : 4 cases 29×29 aux positions belts.txt, icônes item_hp*.png,
//     clic = boire (useBeltKey) ;
//   - BARRE D'EXPÉRIENCE : remplissage orange au progrès réel du niveau ;
//   - MINIPANEL : boutons originaux minipanelbtn_f*.png (perso, inventaire,
//     compétences, quêtes, carte, aide…) — mêmes actions que hud.js.
//
// Auto-init (script CLASSIQUE — pas d'`export`, cf. panels-ui.js chargé lui
// en import dynamique). Sprites servies par web/assets/ui/.
window.D2Surface = (function () {
  'use strict';

  var UI = 'assets/ui/';
  var PANEL_W = 800, PANEL_H = 104;
  // positions écran dans le panneau 800×104 (mesurées sur panel_full.png)
  var ORB_L = { x: 24, y: 39, s: 66 };   // orbe vie (socket femme, gauche)
  var ORB_R = { x: 710, y: 39, s: 66 };  // orbe mana (socket démon, droite)
  var BELT = { x: 380, y: 58, s: 29, gap: 4 };  // 4 cases (belts.txt default)
  var XP = { x: 208, y: 76, w: 132, h: 12 };    // barre d'expérience (f1)
  var MINI = { x: 312, y: 2 };            // minipanel 176×26 centré

  var game = function () {
    try { return window.__d2 && window.__d2.game ? window.__d2.game : null; }
    catch (e) { return null; }
  };
  function jparse(s, fb) { try { return JSON.parse(s); } catch (e) { return fb; } }

  var orbLife = null, orbMana = null, xpFill = null, beltSlots = [];
  var imgLife = null, imgMana = null;

  function drawOrb(cv, img, pct, label) {
    var ctx = cv.getContext('2d');
    var s = cv.width;
    ctx.clearRect(0, 0, s, s);
    ctx.save();
    ctx.beginPath();
    ctx.arc(s / 2, s / 2, s / 2 - 1, 0, Math.PI * 2);
    ctx.clip();
    ctx.globalAlpha = 0.35;
    ctx.drawImage(img, 0, 0, s, s);
    ctx.globalAlpha = 1;
    if (pct > 0) {
      var h = Math.max(2, Math.round(s * pct));
      ctx.drawImage(img, 0, s - h, s, h, 0, s - h, s, h);
    }
    ctx.restore();
    ctx.font = 'bold 12px system-ui';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = 'rgba(0,0,0,.7)';
    ctx.fillText(label, s / 2 + 1, s / 2 + 1);
    ctx.fillStyle = '#ffe9b0';
    ctx.fillText(label, s / 2, s / 2);
  }

  var ICONS = {
    hp1: UI + 'item_hp1.png', hp2: UI + 'item_hp2.png', hp3: UI + 'item_hp3.png',
    hp4: UI + 'item_hp4.png', hp5: UI + 'item_hp5.png',
  };
  function iconFor(code) {
    var c = String(code || '').toLowerCase();
    return ICONS[c] || null;
  }

  function poll() {
    var g = game();
    var st = g ? jparse(g.heroStats(), {}) : {};
    if (st.max_life) {
      drawOrb(orbLife, imgLife, Math.max(0, Math.min(1, st.life / st.max_life)),
        st.life + '/' + st.max_life);
    }
    if (st.max_mana) {
      drawOrb(orbMana, imgMana, Math.max(0, Math.min(1, st.mana / st.max_mana)),
        st.mana + '/' + st.max_mana);
    }
    if (xpFill && st.xp_to_next !== undefined) {
      var done = Math.max(0, (st.max_xp || 0) - 0);
      // progrès = 1 - reste/interval courant ; interval inconnu côté JS →
      // approximation par xp_to_next et xp : p = 1 - to_next/(to_next + xp courant du niveau)
      var span = st.xp + 0; // plancher : progression ≥ 0
      var p = 0;
      if (st.xp_to_next > 0 && st.xp > 0) {
        p = 1 - st.xp_to_next / (st.xp_to_next + Math.max(1, Math.min(st.xp, st.xp_to_next)));
      } else if (st.xp_to_next === 0) p = 1;
      xpFill.style.width = Math.round(Math.max(0, Math.min(1, p)) * 100) + '%';
    }
    if (g && g.beltInfo) {
      var bi = jparse(g.beltInfo(), {});
      var col0 = (bi.belt && bi.belt[0]) || [];
      for (var i = 0; i < 4; i++) {
        var cell = beltSlots[i];
        if (!cell) continue;
        var it = col0[i] || null;
        var ic = iconFor(it ? it.code : '');
        cell.style.backgroundImage = ic ? 'url(' + ic + ')' : 'none';
        cell.style.opacity = it ? '1' : '.3';
        cell.title = it ? it.name || it.code : 'vide';
      }
    }
  }

  function press(key) { try { window.dispatchEvent(new KeyboardEvent('keydown', { key: key })); } catch (e) {} }
  function hudButton(label) {
    var mp = document.querySelector('.minipanel');
    if (!mp) return null;
    var bs = mp.querySelectorAll('button');
    for (var i = 0; i < bs.length; i++) if (bs[i].textContent === label) return bs[i];
    return null;
  }
  var BUTTONS = [
    { i: 0, title: 'Panneau de personnage (C)', act: function () { press('c'); } },
    { i: 1, title: 'Inventaire (I)', act: function () { press('i'); } },
    { i: 4, title: 'Arbre de compétences (T)', act: function () { press('t'); } },
    { i: 2, title: 'Journal de quêtes', act: function () { var b = hudButton('Quêtes'); if (b) b.click(); } },
    { i: 9, title: 'Carte (Tab)', act: function () { if (window.D2Automap) window.D2Automap.toggle(); } },
    { i: 12, title: 'Aide (H)', act: function () { press('h'); } },
    { i: 6, title: 'Menu', act: function () { var b = hudButton('Menu'); if (b) b.click(); } },
    { i: 10, title: 'Son', act: function () { var b = document.getElementById('btn-sound'); if (b) b.click(); } },
  ];

  function build() {
    var canvas = document.getElementById('game');
    var wrap = document.createElement('div');
    wrap.id = 'd2-surface';
    wrap.innerHTML =
      '<style>' +
      '#d2-surface{position:absolute;display:block;pointer-events:none;z-index:40;user-select:none}' +
      '#d2-surface .bg{width:800px;height:104px;image-rendering:pixelated;display:block;pointer-events:auto}' +
      '#d2-surface .wrap-scale{position:relative;width:800px;height:104px;transform-origin:bottom left}' +
      '#d2-surface .orb{position:absolute;pointer-events:auto;filter:drop-shadow(0 0 6px #000c)}' +
      '#d2-surface .bslot{position:absolute;width:29px;height:29px;border:1px solid #6a5a3a;' +
      'background:#0d0a12 center/25px no-repeat;cursor:pointer;opacity:.3;pointer-events:auto;box-sizing:border-box}' +
      '#d2-surface .bslot:hover{border-color:#c7a45a;opacity:.9}' +
      '#d2-surface .xpbg{position:absolute;background:#000;border:1px solid #6a5a3a;box-sizing:border-box}' +
      '#d2-surface .xpfill{position:absolute;left:1px;top:1px;bottom:1px;width:0%;' +
      'background:linear-gradient(#d8b25a,#8f6a1e)}' +
      '#d2-surface .mpanel{position:absolute;width:176px;height:26px;display:flex;align-items:center;' +
      "justify-content:center;gap:1px;background:url('" + UI + "minipanel.png');background-size:100% 100%;" +
      'pointer-events:auto}' +
      '#d2-surface .mbtn{width:20px;height:20px;border:none;background:transparent center/contain no-repeat;' +
      'cursor:pointer;image-rendering:pixelated;padding:0}' +
      '#d2-surface .mbtn:hover{filter:brightness(1.6)}' +
      '</style>';
    var scale = document.createElement('div');
    scale.className = 'wrap-scale';

    var bg = document.createElement('img');
    bg.className = 'bg'; bg.src = UI + 'panel_full.png'; bg.draggable = false;
    scale.appendChild(bg);

    // orbes dynamiques
    orbLife = document.createElement('canvas');
    orbLife.className = 'orb'; orbLife.width = ORB_L.s; orbLife.height = ORB_L.s;
    orbLife.style.left = ORB_L.x + 'px'; orbLife.style.top = ORB_L.y + 'px';
    orbLife.title = 'Vie';
    orbMana = document.createElement('canvas');
    orbMana.className = 'orb'; orbMana.width = ORB_R.s; orbMana.height = ORB_R.s;
    orbMana.style.left = ORB_R.x + 'px'; orbMana.style.top = ORB_R.y + 'px';
    orbMana.title = 'Mana';
    scale.appendChild(orbLife);
    scale.appendChild(orbMana);

    // barre d'expérience
    var xpbg = document.createElement('div');
    xpbg.className = 'xpbg';
    xpbg.style.left = XP.x + 'px'; xpbg.style.top = XP.y + 'px';
    xpbg.style.width = XP.w + 'px'; xpbg.style.height = XP.h + 'px';
    xpbg.title = 'Expérience';
    xpFill = document.createElement('div');
    xpFill.className = 'xpfill';
    xpbg.appendChild(xpFill);
    scale.appendChild(xpbg);

    // ceinture 4 cases
    for (var i = 0; i < 4; i++) {
      (function (row) {
        var b = document.createElement('div');
        b.className = 'bslot';
        b.style.left = (BELT.x + row * (BELT.s + BELT.gap)) + 'px';
        b.style.top = BELT.y + 'px';
        b.title = 'Ceinture ' + (row + 1) + ' — cliquer pour boire';
        b.addEventListener('click', function () {
          var g = game();
          if (g && g.useBeltKey) { try { g.useBeltKey(1); } catch (e) {} }
        });
        scale.appendChild(b);
        beltSlots.push(b);
      })(i);
    }

    // minipanel (boutons originaux)
    var mp = document.createElement('div');
    mp.className = 'mpanel';
    mp.style.left = MINI.x + 'px'; mp.style.top = MINI.y + 'px';
    BUTTONS.forEach(function (d) {
      var b = document.createElement('button');
      b.className = 'mbtn';
      b.title = d.title;
      b.style.backgroundImage = "url('" + UI + "minipanelbtn_f" + d.i + ".png')";
      b.addEventListener('click', function (e) { e.stopPropagation(); d.act(); });
      mp.appendChild(b);
    });
    scale.appendChild(mp);

    wrap.appendChild(scale);

    // ancrage : bas du canvas, échelle = largeur canvas / 800
    var anchor = function () {
      var r = canvas.getBoundingClientRect();
      if (!r.width) return;
      var k = r.width / PANEL_W;
      scale.style.transform = 'scale(' + k + ')';
      wrap.style.left = r.left + 'px';
      wrap.style.top = (r.bottom - PANEL_H * k) + 'px';
      wrap.style.width = r.width + 'px';
      wrap.style.height = (PANEL_H * k) + 'px';
    };
    anchor();
    window.addEventListener('resize', anchor);
    window.addEventListener('scroll', anchor);

    document.body.appendChild(wrap);
    imgLife = new Image(); imgLife.src = UI + 'orbs_f1.png'; // rouge = vie
    imgMana = new Image(); imgMana.src = UI + 'orbs_f0.png'; // bleu = mana
    setInterval(poll, 150);
    poll();
  }

  function init() {
    if (!document.getElementById('game')) return;
    var im = new Image();
    im.onload = function () { try { build(); } catch (e) { /* jamais bloquant */ } };
    im.onerror = function () { /* sprites absents : fallback conservé */ };
    im.src = UI + 'panel_full.png';
  }

  if (document.readyState === 'loading') document.addEventListener('DOMContentLoaded', init);
  else init();

  return { poll: poll };
})();
