// Corps au sol (J1) — demo/corpse-ui.js (NOUVEAU fichier, chargé AVANT game.js).
//
// Rôle (mission CORPS AU SOL J1) :
//  * À la mort (event CorpseDropped) : overlay « Vous êtes mort… » avec le
//    bouton « Retourner au camp » — le respawn au camp est AUTOMATIQUE côté
//    moteur (~2 s, RESPAWN_MS) ; le bouton masque simplement l'overlay.
//  * Tant qu'un corps gît au sol : flare pulsant dessiné sur le canvas de
//    jeu (autour du corps) + hint texte (tuile, distance, objets restants).
//    Rappel D2 : le corps porte l'équipement ; l'or porté est tombé à côté
//    (péage officiel déjà déduit par le moteur — cf. engine corpse.rs).
//  * Clic sur le corps (routé par game.js → {"type":"recoverCorpse"}) :
//    ré-équipe tout à portée, sinon le héros marche et grab à l'arrivée.
//
// Branché sur : game.corpseInfo() — JSON {"x","y","sx","sy","items"} ou null
// (sx/sy = projection écran du corps pour la caméra courante).
window.D2CorpseUI = (function () {
  'use strict';

  let overlay = null;   // panneau de mort (DOM)
  let hint = null;      // barre d'aide sous le corps (DOM)
  let hintUntil = 0;    // message transitoire de récupération
  let dead = false;     // overlay de mort visible ?

  function ensureDom() {
    if (overlay) return;
    const style = document.createElement('style');
    style.textContent = `
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
    document.head.appendChild(style);

    overlay = document.createElement('div');
    overlay.id = 'corpse-death';
    overlay.innerHTML = `
      <div class="panel">
        <h2>Vous êtes mort…</h2>
        <p id="corpse-death-text">Votre corps reste là où vous êtes tombé.</p>
        <p>Réapparition au camp dans un instant.</p>
        <button id="corpse-return-btn">Retourner au camp</button>
      </div>`;
    document.body.appendChild(overlay);
    document.getElementById('corpse-return-btn').addEventListener('click', () => {
      // Le respawn est automatique côté moteur (RESPAWN_MS ≈ 2 s) : le
      // bouton n'a qu'à rendre la vue au joueur.
      overlay.classList.remove('show');
    });

    hint = document.createElement('div');
    hint.id = 'corpse-hint';
    document.body.appendChild(hint);
  }

  // Event CorpseDropped {x, y, gold} — pose l'overlay de mort.
  function onDeath(info) {
    ensureDom();
    dead = true;
    const gold = (info && info.gold) || 0;
    const where = info ? ` (${info.x}, ${info.y})` : '';
    const t = document.getElementById('corpse-death-text');
    if (t) {
      t.textContent = gold > 0
        ? `Votre corps garde votre équipement${where} ; ${gold} or sont tombés à côté.`
        : `Votre corps garde votre équipement${where}.`;
    }
    overlay.classList.add('show');
  }

  // Event HeroRespawned — masque l'overlay (le héros est nu au camp).
  function onRespawned() {
    dead = false;
    if (overlay) overlay.classList.remove('show');
  }

  // Event CorpseRecovered — petit message au-dessus du corps.
  function onRecovered() {
    ensureDom();
    hintUntil = performance.now() + 1800;
  }

  // Appelé chaque frame par game.js : flare pulsant autour du corps (dessiné
  // directement sur le canvas de jeu) + hint texte. Polle corpseInfo().
  function frame(canvas, game) {
    ensureDom();
    let info = null;
    try { info = JSON.parse(game.corpseInfo() || 'null'); } catch { /* avant finalize */ }
    const ctx = canvas.getContext('2d');
    const rect = canvas.getBoundingClientRect();
    const scale = canvas.width / Math.max(rect.width, 1);

    if (!info) {
      hint.style.display = 'none';
      return;
    }

    const sx = info.sx * scale;
    const sy = info.sy * scale;
    const now = performance.now();
    const pulse = 0.5 + 0.5 * Math.sin(now / 180); // 0..1 pulsation ~5 Hz

    // Flare : halo rouge pulsant + croix « ☠ » au sol, style marqueur D2.
    ctx.save();
    ctx.strokeStyle = `rgba(230, 60, 40, ${0.35 + 0.45 * pulse})`;
    ctx.lineWidth = 2;
    ctx.beginPath();
    ctx.ellipse(sx, sy, 16 + 5 * pulse, 8 + 2.5 * pulse, 0, 0, Math.PI * 2);
    ctx.stroke();
    ctx.fillStyle = `rgba(255, 210, 160, ${0.8 + 0.2 * pulse})`;
    ctx.font = '13px monospace';
    ctx.textAlign = 'center';
    ctx.fillText('☠', sx, sy - 10 - 2 * pulse);
    ctx.restore();

    // Hint au-dessus du corps (ou message « récupéré » bref).
    hint.style.display = 'block';
    hint.style.left = `${rect.left + window.scrollX + sx - hint.offsetWidth / 2}px`;
    hint.style.top = `${rect.top + window.scrollY + sy - 34}px`;
    if (now < hintUntil) {
      hint.textContent = 'Équipement récupéré !';
      hint.style.color = '#b8e0a0';
    } else {
      hint.style.color = '#ffd9a0';
      hint.textContent = `☠ Votre corps${info.items ? ` — ${info.items} objet${info.items > 1 ? 's' : ''}` : ' (vide)'} — cliquez dessus pour récupérer`;
    }
    // L'overlay de mort reste prioritaire visuellement (z-index 90 > 40).
    if (dead) hint.style.display = 'none';
  }

  return { init: ensureDom, onDeath, onRespawned, onRecovered, frame };
})();
