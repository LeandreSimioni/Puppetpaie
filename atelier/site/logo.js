// Logo animé de PuppetPaie (12/10/2026).
// La cane P entre, se retourne, repart chercher ses trois canetons ; ils reviennent, le traînard en dernier,
// tous se retournent, puis les autres lettres tombent et le mot se forme. Une seule lecture, au chargement.
// Sans script, ou avec « réduire les animations », le mot s'affiche directement.
(() => {
  const scene = document.getElementById('scene');
  if (!scene) return;
  const spans = [...document.querySelectorAll('#mot span')];
  const CANE = 0, P1 = 2, P2 = 3, TRAINARD = 6;           // P, puis les p de « pup », puis celui de « paie »
  const MARCHEURS = [CANE, P1, P2, TRAINARD];
  const AUTRES = spans.map((_, i) => i).filter(i => !MARCHEURS.includes(i));

  // Valeurs arrêtées avec Léandre le 12/10/2026 (version réglable : artefact « Réglages du logo Puppetpaie »).
  const R = {
    arret: 0,
    pause1: 0.1,
    pause2: 0.5,
    sortie: 1.8,
    coulisse: 0.7,
    t2: 2,
    ecart: 2,
    vTrain: 0.82,
    avantRegard: 0.6,
    entreRegards: 0.06,
    attenteTrain: 0,
    pauseFin: 0.4,
    dureeTour: 0.2,
    saut: 0.1,
    dand: 6,
    pas: 0.3,
    boiteFort: 7.5,
    boiteFaible: 5,
    pasTrain: 0.48,
    cane: 1.43,
    espace: 0.08,
    trait: 21,
    avantLettres: 0.7,
    entreLettres: 0.045,
    chute: 0.6,
    ombreOpacite: 0.14,
    ombreLongueur: 0.29,
    ombreBiais: 52,
    ombreFlou: 4.5
  };

  let M = null, T = null;
  let t = 0, enCours = false, dernier = 0;

  const clamp = (x, a, b) => Math.min(b, Math.max(a, x));
  const doux = u => u * u * (3 - 2 * u);
  const rebond = u => { const n = 7.5625, d = 2.75;
    if (u < 1 / d) return n * u * u;
    if (u < 2 / d) return n * (u -= 1.5 / d) * u + .75;
    if (u < 2.5 / d) return n * (u -= 2.25 / d) * u + .9375;
    return n * (u -= 2.625 / d) * u + .984375; };
  const depasse = u => { const c = 1.7, c3 = c + 1; return 1 + c3 * Math.pow(u - 1, 3) + c * Math.pow(u - 1, 2); };

  // Les ombres : une copie de chaque lettre couchée au sol derrière elle (lumière de face, en hauteur).
  // Toutes tombent sur le même sol, celui où marchent les P : l'ombre d'une lettre posée en l'air commence donc plus loin derrière,
  // d'autant plus loin qu'elle est haute ; de même quand un P saute ou qu'une lettre tombe.
  const ombres = [];
  function preparerOmbres() {
    if (!ombres.length) {
      const calque = document.createElement('div'); calque.id = 'ombres'; scene.prepend(calque);
      spans.forEach((s, i) => {
        const o = document.createElement('div'); o.className = 'ombre';
        o.appendChild(s.querySelector('svg').cloneNode(true));
        o.style.transformOrigin = '50% 100%';   // le sol : le bas de toutes les boîtes
        calque.appendChild(o); ombres.push(o);
      });
    }
    spans.forEach((s, i) => Object.assign(ombres[i].style, {
      left: s.offsetLeft + 'px', top: s.offsetTop + 'px', width: s.offsetWidth + 'px', height: s.offsetHeight + 'px',
      filter: `blur(${R.ombreFlou}px)` }));
  }
  const couche = (dx, extra = '') => `translateX(${dx.toFixed(1)}px) skewX(${-R.ombreBiais}deg) scaleY(${R.ombreLongueur}) ${extra}`;
  function mesurer() {
    const W = scene.clientWidth;
    M = { W, L: spans.map(s => s.offsetLeft), w: spans.map(s => s.offsetWidth),
          haut: spans.map(s => s.offsetTop + s.offsetHeight),
          fs: parseFloat(getComputedStyle(spans[0]).fontSize) };
    preparerOmbres();
    construire();
  }

  // Le conducteur : chaque marcheur a ses trajets (x en px, bord gauche) et ses demi-tours (rotateY).
  // Un demi-tour va de 180° à 360° (et non à 0°) : le bec passe devant, face au public, au lieu de tourner par le fond.
  function construire() {
    const { W, L, w } = M, HORS = W + 12;
    const v1 = W / R.t2;                             // la cane marche à la même allure seule et avec ses petits
    const xArret = L[CANE] + (W - L[CANE]) * R.arret;
    const a1 = (HORS - xArret) / v1;                // 1. la cane avance
    const tour1 = a1 + R.pause1;                          //    s'arrête : un temps vide (« elle a oublié ses petits »), puis se retourne vivement
    const ret0 = tour1 + R.dureeTour + R.pause2, ret1 = ret0 + (HORS - xArret) / (v1 * R.sortie);   // retournée, un temps, puis elle repart
    const S2 = ret1 + R.coulisse;                           // 2. retour avec les petits
    const v2 = W / R.t2;
    const ecart = w[CANE] * R.ecart / v2;
    const dep = { [CANE]: S2, [P1]: S2 + ecart, [P2]: S2 + ecart + w[P1] * R.ecart / v2 };
    const vit = { [CANE]: v2, [P1]: v2, [P2]: v2, [TRAINARD]: v2 * R.vTrain };
    const arr = {}; [CANE, P1, P2].forEach(i => arr[i] = dep[i] + (HORS - L[i]) / vit[i]);
    const trois = Math.max(arr[CANE], arr[P1], arr[P2]);
    const regard = trois + R.avantRegard;                      //    les trois en place, un temps, puis ils se tournent vers le traînard
    const trajetT = (HORS - L[TRAINARD]) / vit[TRAINARD];
    arr[TRAINARD] = Math.max(regard + R.attenteTrain, dep[P2] + w[P2] * R.ecart / v2 + trajetT);   // jamais collé au précédent
    dep[TRAINARD] = arr[TRAINARD] - trajetT;         //    il part bien après, et arrive sous leurs regards
    const S3 = Math.max(arr[TRAINARD], regard + 2 * R.entreRegards + R.dureeTour) + R.pauseFin;   // s'il arrive avant, il attend que les autres se soient retournés
    const tourFin = { [CANE]: regard, [P1]: regard + R.entreRegards, [P2]: regard + 2 * R.entreRegards, [TRAINARD]: S3 };
    const lettres = tourFin[TRAINARD] + R.dureeTour + R.avantLettres;

    const m = {};
    MARCHEURS.forEach(i => m[i] = { trajets: [], tours: [], boite: i === TRAINARD });
    m[CANE].trajets.push({ t0: 0, t1: a1, x0: HORS, x1: xArret }, { t0: ret0, t1: ret1, x0: xArret, x1: HORS });
    m[CANE].tours.push({ t: tour1, d: R.dureeTour, de: 180, a: 360 }, { t: (ret1 + S2) / 2, d: 0, de: 0, a: 180 });
    MARCHEURS.forEach(i => {
      m[i].trajets.push({ t0: dep[i], t1: arr[i], x0: HORS, x1: L[i] });
      m[i].tours.push({ t: tourFin[i], d: R.dureeTour, de: 180, a: 360 });
    });
    const debut = {}; AUTRES.forEach((i, k) => debut[i] = lettres + k * R.entreLettres);
    const fin = lettres + AUTRES.length * R.entreLettres + R.chute + 0.6;
    T = { m, debut, fin };
  }

  function poseMarcheur(i, t) {
    const { m } = T, fs = M.fs, mm = m[i];
    let x = mm.trajets[0].x0, rot = 0, y = 0;
    for (const s of mm.trajets) {
      if (t < s.t0) break;
      if (t >= s.t1) { x = s.x1; continue; }
      const u = (t - s.t0) / (s.t1 - s.t0);
      x = s.x0 + (s.x1 - s.x0) * u;
      const parcouru = Math.abs(x - s.x0), reste = Math.abs(s.x1 - x);
      const pas = fs * (mm.boite ? R.pasTrain : R.pas);
      const env = clamp(Math.min(parcouru, reste) / (pas * 1.2), 0, 1);
      const sn = Math.sin(parcouru / pas * Math.PI);
      rot = (mm.boite ? (sn > 0 ? R.boiteFort * sn : R.boiteFaible * sn) : R.dand * sn) * env;   // dandinement de canard ; le traînard boite
      y = -Math.abs(sn) * fs * (mm.boite ? 0.035 : 0.05) * env;
    }
    let ry = 180;
    for (const r of mm.tours) {
      if (t < r.t) break;
      if (r.d === 0 || t >= r.t + r.d) { ry = r.a; continue; }
      const u = (t - r.t) / r.d;
      ry = r.de + (r.a - r.de) * doux(u);
      y += -Math.sin(Math.PI * u) * fs * R.saut;                          // petit sursaut en se retournant
    }
    ombres[i].style.transform = couche(x - M.L[i], `translateY(${y.toFixed(1)}px) rotate(${rot.toFixed(2)}deg) perspective(1500px) rotateY(${ry.toFixed(1)}deg)`);
    ombres[i].style.opacity = R.ombreOpacite;
    spans[i].style.transform =
      `translate(${(x - M.L[i]).toFixed(1)}px, ${y.toFixed(1)}px) rotate(${rot.toFixed(2)}deg) perspective(1500px) rotateY(${ry.toFixed(1)}deg)`;
  }

  function poseLettre(i, t) {
    const u = clamp((t - T.debut[i]) / R.chute, 0, 1);
    const s = spans[i].style;
    const o = ombres[i].style;
    if (t < T.debut[i]) { s.opacity = 0; s.transform = ''; o.opacity = 0; return; }
    const r = rebond(u);
    s.opacity = 1;
    const ty = `translateY(${(-(M.haut[i] + 10) * (1 - r)).toFixed(1)}px)`;
    s.transform = ty;
    o.transform = couche(0, ty);   // l'ombre arrive du fond et vient se caler derrière la lettre
    o.opacity = (R.ombreOpacite * r).toFixed(3);
  }

  function dessiner() {
    MARCHEURS.forEach(i => poseMarcheur(i, t));
    AUTRES.forEach(i => poseLettre(i, t));
  }

  function boucle(now) {
    if (!enCours) return;
    t = Math.min(T.fin, t + (now - dernier) / 1000);
    dernier = now;
    dessiner();
    if (t >= T.fin) { enCours = false; return; }
    requestAnimationFrame(boucle);
  }
  function jouer(depuis) {
    t = depuis; dernier = performance.now();
    if (!enCours) { enCours = true; requestAnimationFrame(boucle); }
  }

  let attente; addEventListener('resize', () => { clearTimeout(attente); attente = setTimeout(() => { mesurer(); dessiner(); }, 120); });

  const demarrer = () => {
    mesurer();
    const reduit = matchMedia('(prefers-reduced-motion: reduce)').matches;
    t = reduit ? T.fin : 0;
    dessiner();   // la première image est posée avant que le mot devienne visible : rien ne clignote
    document.documentElement.classList.add('logo-pret');
    if (!reduit) jouer(0);
  };
  (document.fonts ? document.fonts.ready : Promise.resolve()).then(demarrer);
})();
