// Diablo II — menu principal original : logo en flammes animé + navigation
(function () {
  'use strict';
  const M = 'assets/menu/';
  const errlog = document.getElementById('errlog');
  window.addEventListener('error', (e) => { errlog.textContent += '[err] ' + e.message + '\n'; });

  // Mise à l'échelle du stage 800x600
  function fit() {
    const s = Math.min(innerWidth / 800, innerHeight / 600);
    document.getElementById('stage').style.setProperty('--s', s);
  }
  addEventListener('resize', fit); fit();

  // Logo en flammes : 30 frames (gauche + droite), ~15 fps
  const FRAMES = 30;
  let f = 0;
  const logoL = document.getElementById('logoL');
  const logoR = document.getElementById('logoR');
  const pad = (n) => String(n).padStart(2, '0');
  setInterval(() => {
    f = (f + 1) % FRAMES;
    logoL.src = M + 'logo_fire_left_' + pad(f) + '.png';
    logoR.src = M + 'logo_fire_right_' + pad(f) + '.png';
  }, 66);

  // Boutons
  let selectedHero = null;
  const classes = document.getElementById('classes');
  const play = document.getElementById('play');
  document.querySelectorAll('#buttons .dbtn').forEach((b) => {
    b.addEventListener('click', () => {
      try { new Audio('assets/sfx/ui_click.wav').play(); } catch (e) {}
      const action = b.dataset.action;
      if (action === 'solo') classes.classList.remove('hidden');
      else if (action === 'continue') location.href = 'game.html?load=1&hero=' + (localStorage.getItem('d2hero') || 'BA');
      else if (action === 'github') open('https://github.com/shpaw415/diablo2-wasm', '_blank');
      else if (action === 'quit') { classes.classList.add('hidden'); }
    });
  });
  document.querySelectorAll('#classes .card').forEach((c) => {
    c.addEventListener('click', () => {
      document.querySelectorAll('#classes .card').forEach((x) => x.classList.remove('selected'));
      c.classList.add('selected');
      selectedHero = c.dataset.hero;
      play.disabled = false;
      try { new Audio('assets/sfx/ui_click.wav').play(); } catch (e) {}
    });
  });
  play.addEventListener('click', () => {
    if (!selectedHero) return;
    localStorage.setItem('d2hero', selectedHero);
    location.href = 'game.html?hero=' + selectedHero;
  });
  // Continuer grisé sans sauvegarde
  if (!localStorage.getItem('d2save')) document.getElementById('btnContinue').disabled = true;
})();
