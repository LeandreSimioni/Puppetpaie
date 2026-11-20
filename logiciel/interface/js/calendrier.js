/* LE CALENDRIER, UN SEUL. « On dit au moteur quand c'est une date, c'est un calendrier. » (Léandre, 20/11/2026.)
 *
 * Il se pose tout seul là où la forme d'une case dit un jour (`date`) ou des jours (`jours`) : le dessinateur
 * (`js/avance.js`) écrit la case, `poserLesCalendriers` la branche ; il n'est branché nulle part à la main.
 *
 * Toujours la même boîte, de la même taille : en tête, ‹ mois › et ‹ année ›, chacun entre ses deux flèches ; un clic sur
 * le mois montre les douze mois, un clic sur l'année montre des années, on choisit et on revient aux jours. Il s'ouvre
 * sous sa case et ne sort jamais de l'écran. Pour des jours : un clic coche un jour, cliquer et glisser du lundi au
 * vendredi en coche cinq. Une date se tape aussi dans sa case (JJ/MM/AAAA). */
"use strict";

let CALENDRIER_OUVERT = null;

/** Ferme le calendrier ouvert, s'il y en a un. */
function fermerLesCalendriers() {
  if (!CALENDRIER_OUVERT) return;
  const cal = CALENDRIER_OUVERT;
  CALENDRIER_OUVERT = null;
  cal.remove();
  if (cal._fermer) cal._fermer();
}

/** OUVRE LE CALENDRIER sous `ancre`. `o.vue` : le mois montré d'abord, AAAA-MM ; `o.pris(iso)` : le jour est-il pris ?
 *  Un jour : `o.choisir(iso)`, et il se ferme. Des jours : `o.coches` (les jours cochés, AAAA-MM-JJ), `o.changer(coches)`
 *  à chaque jour coché ou décoché, et `o.fermer()` quand il se ferme. `o.lecture(mois)` : ce mois se lit, rien ne s'y
 *  coche ; `o.marque(iso)` : ce qu'un jour d'un mois qui se lit montre (`{classe, titre}`) ; `o.dessous(mois)` : ce qui
 *  se lit sous la grille de ce mois. `o.redessiner` reçoit de quoi redessiner le calendrier ouvert (ce qui se lit est
 *  arrivé). */
function ouvrirLeCalendrier(ancre, o) {
  fermerLesCalendriers();
  const cal = document.createElement("div");
  cal.className = "cal";
  cal._fermer = o.fermer;
  document.body.appendChild(cal);
  CALENDRIER_OUVERT = cal;
  const coches = new Set(o.coches || []);
  const pris = (iso) => (o.coches ? coches.has(iso) : o.pris(iso));
  let annee = +o.vue.slice(0, 4), mois = +o.vue.slice(5, 7) - 1, montre = "jours", glisse = null;
  const deux = (n) => String(n).padStart(2, "0");
  const vueDe = () => `${annee}-${deux(mois + 1)}`;
  const lecture = () => !!(o.lecture && o.lecture(vueDe()));
  const fleche = (sens, titre, faire) =>
    `<button type="button" class="c-fleche" data-pas="${sens}" data-quoi="${faire}" aria-label="${titre}">${sens < 0 ? "‹" : "›"}</button>`;
  const dessiner = () => {
    let grille;
    if (montre === "mois") {
      grille = `<div class="c-grille c-douze">${MOIS_FR.map((n, m) =>
        `<button type="button" data-mois="${m}" class="${m === mois ? "pris" : ""}">${echapper(n)}</button>`).join("")}</div>`;
    } else if (montre === "annees") {
      const debut = annee - 7;
      grille = `<div class="c-grille c-douze">${Array.from({ length: 12 }, (_, k) => debut + k).map((a) =>
        `<button type="button" data-annee="${a}" class="${a === annee ? "pris" : ""}">${a}</button>`).join("")}</div>`;
    } else {
      const nb = new Date(annee, mois + 1, 0).getDate();
      const premier = (new Date(annee, mois, 1).getDay() + 6) % 7;   // lundi = 0
      const lu = lecture();
      let cases = JOURS_SEMAINE.map((x) => `<span class="c-sem">${x}</span>`).join("");
      for (let i = 0; i < premier; i++) cases += `<span></span>`;
      for (let j = 1; j <= nb; j++) {
        const iso = `${annee}-${deux(mois + 1)}-${deux(j)}`;
        const m = lu && o.marque ? o.marque(iso) : null;
        cases += lu
          ? `<span class="c-jour lu${m && m.classe ? " " + m.classe : ""}"${m && m.titre ? ` title="${echapper(m.titre)}"` : ""}>${j}</span>`
          : `<button type="button" class="c-jour${pris(iso) ? " pris" : ""}" data-jour="${iso}">${j}</button>`;
      }
      grille = `<div class="c-grille c-jours">${cases}</div>${o.dessous ? `<div class="c-dessous">${o.dessous(vueDe())}</div>` : ""}`;
    }
    cal.innerHTML = `
      <div class="c-tete">
        <span class="c-groupe">${fleche(-1, "Mois d'avant", "mois")}
          <button type="button" class="c-nom" data-montrer="mois">${echapper(MOIS_FR[mois])}</button>${fleche(1, "Mois d'après", "mois")}</span>
        <span class="c-groupe">${fleche(-1, "Année d'avant", "annee")}
          <button type="button" class="c-nom" data-montrer="annees">${annee}</button>${fleche(1, "Année d'après", "annee")}</span>
      </div>${grille}`;
    cal.querySelectorAll(".c-fleche").forEach((b) => {
      b.onclick = () => {
        if (b.dataset.quoi === "annee") annee += +b.dataset.pas;
        else { const t = annee * 12 + mois + +b.dataset.pas; annee = Math.floor(t / 12); mois = t % 12; }
        dessiner();
      };
    });
    cal.querySelectorAll("[data-montrer]").forEach((b) => {
      b.onclick = () => { montre = montre === b.dataset.montrer ? "jours" : b.dataset.montrer; dessiner(); };
    });
    cal.querySelectorAll("[data-mois]").forEach((b) => { b.onclick = () => { mois = +b.dataset.mois; montre = "jours"; dessiner(); }; });
    cal.querySelectorAll("[data-annee]").forEach((b) => { b.onclick = () => { annee = +b.dataset.annee; montre = "jours"; dessiner(); }; });
    const jours = cal.querySelector(".c-jours");
    if (!jours) return;
    // Le jour sous le pointeur (le glisser passe d'un bouton à l'autre).
    const sous = (ev) => {
      const e = document.elementFromPoint(ev.clientX, ev.clientY);
      const c = e && e.closest ? e.closest("[data-jour]") : null;
      return c && jours.contains(c) ? c.dataset.jour : null;
    };
    jours.onpointerdown = (ev) => {
      const iso = sous(ev);
      if (!iso) return;
      ev.preventDefault();
      if (!o.coches) { o.choisir(iso); fermerLesCalendriers(); return; }
      glisse = { de: iso, pose: !coches.has(iso), avant: new Set(coches) };
      etendre(iso);
    };
    jours.onpointermove = (ev) => {
      if (!glisse) return;
      const iso = sous(ev);
      if (iso) etendre(iso);
    };
  };
  // Du jour où l'on a appuyé à celui où l'on est : chacun coché (ou décoché, si l'on a appuyé sur un jour coché).
  const etendre = (jusqua) => {
    const [a, b] = glisse.de < jusqua ? [glisse.de, jusqua] : [jusqua, glisse.de];
    coches.clear();
    glisse.avant.forEach((x) => coches.add(x));
    for (let j = +a.slice(8); j <= +b.slice(8); j++) {
      const iso = a.slice(0, 8) + deux(j);
      if (glisse.pose) coches.add(iso); else coches.delete(iso);
    }
    const g = glisse;
    dessiner();
    glisse = g;
    o.changer([...coches].sort());
  };
  const lacher = () => { glisse = null; };
  document.addEventListener("pointerup", lacher);
  // SOUS SA CASE, SANS SORTIR DE L'ÉCRAN, NI À DROITE NI À GAUCHE : recalé vers la gauche si la place manque à droite,
  // jamais avant le bord gauche ; au-dessus si la place manque dessous. Sa largeur ne dépasse pas l'écran (CSS).
  const placer = () => {
    const a = ancre.getBoundingClientRect(), r = cal.getBoundingClientRect();
    const large = document.documentElement.clientWidth, haut = document.documentElement.clientHeight;
    const gauche = Math.max(8, Math.min(a.left, large - r.width - 8));
    const dessus = a.bottom + r.height + 4 > haut && a.top - r.height - 4 > 0 ? a.top - r.height - 4 : a.bottom + 4;
    cal.style.left = (gauche + window.scrollX) + "px";
    cal.style.top = (dessus + window.scrollY) + "px";
  };
  dessiner();
  placer();
  // Ce qui se lit d'un mois arrive après : le calendrier ouvert se redessine, à sa place.
  if (o.redessiner) o.redessiner(() => { if (cal.isConnected && !glisse) { dessiner(); placer(); } });
  // Fermer en cliquant ailleurs, ou à la touche Échap.
  const dehors = (ev) => {
    if (!cal.isConnected) { document.removeEventListener("pointerdown", dehors); document.removeEventListener("pointerup", lacher); return; }
    const chemin = ev.composedPath();
    if (!chemin.includes(cal) && !chemin.includes(ancre)) fermerLesCalendriers();
  };
  setTimeout(() => document.addEventListener("pointerdown", dehors), 0);
  cal.onkeydown = (ev) => { if (ev.key === "Escape") fermerLesCalendriers(); };
  return cal;
}

/** Le mois que montre d'abord le calendrier d'une case : celui de son premier jour, sinon celui de l'écran de paie, sinon
 *  celui du jour du moteur. */
function moisDOuverture(iso) {
  const ecran = document.getElementById("mois");
  return iso ? iso.slice(0, 7) : (ecran && ecran.value) || aujourdhuiIso().slice(0, 7);
}

/** CE QUE LE CALENDRIER DES JOURS D'UNE CASE DIT DES MOIS AUTOUR DU SIEN (`data-mois`) : un mois d'avant se lit, avec
 *  les contrats que l'écran lit (`o.passes(mois, salarie)` : la réponse de sa route, ses `contrats`, `non_lus`,
 *  `rien_dit`), en jours grisés et une ligne chacun ; un mois d'après dit ce que cocher y fait (`aides.plusieurs_mois`).
 *  Sans mois, rien de tout cela. */
function moisAutour(c, o) {
  const sien = c.dataset.mois;
  if (!sien) return {};
  const lus = new Map();
  let redessiner = () => {};
  const lire = (m) => {
    if (lus.has(m) || !o.passes) return;
    lus.set(m, null);
    o.passes(m, c.dataset.salarie || null).then((r) => lus.set(m, r), (e) => lus.set(m, { erreur: e.message })).then(() => redessiner());
  };
  const contrats = (m) => { const r = lus.get(m); return r && !r.erreur ? r.contrats : []; };
  return {
    lecture: (m) => m < sien,
    redessiner: (f) => { redessiner = f; },
    marque: (iso) => {
      const cs = contrats(iso.slice(0, 7));
      const joues = cs.filter((x) => x.dates.includes(iso));
      const plages = cs.filter((x) => iso >= x.periode.du && (x.periode.au == null || iso <= x.periode.au));
      const dits = (joues.length ? joues : plages).map((x) => [x.nom, x.titre].filter(Boolean).join(" · "));
      return { classe: joues.length ? "joue" : plages.length ? "plage" : "", titre: dits.join(" / ") };
    },
    dessous: (m) => {
      if (m > sien) return `<p class="discret">${echapper(ETAT.aides.plusieurs_mois)}</p>`;
      if (m === sien) return "";
      lire(m);
      const r = lus.get(m);
      if (!r) return o.passes ? `<p class="discret">…</p>` : "";
      if (r.erreur) return `<p class="erreur">${echapper(r.erreur)}</p>`;
      return `${r.contrats.length ? r.contrats.map((x) => `<p class="c-passe"${x.numero_objet || x.spectacle
        ? ` title="${echapper(x.numero_objet || x.spectacle)}"` : ""}><b>${echapper([x.nom, x.titre].filter(Boolean).join(" · "))}</b>
        <span class="discret">${echapper(x.dit)}</span></p>`).join("") : `<p class="discret">${echapper(r.rien_dit)}</p>`}
        ${r.non_lus.map((x) => `<p class="discret">${echapper(x)}</p>`).join("")}`;
    },
  };
}

/** Branche le calendrier de chaque case de `racine` dont la forme le demande (`data-calendrier`, posé par le
 *  dessinateur) : un jour, qui se tape aussi ; des jours, qui se cochent. Chaque case dit `change` quand sa valeur change.
 *  `o.passes` : ce que l'écran lit des contrats d'un mois passé (`moisAutour`). */
function poserLesCalendriers(racine, o = {}) {
  racine.querySelectorAll("[data-calendrier]").forEach((x) => {
    if (x.dataset.calendrierPose) return;
    x.dataset.calendrierPose = "1";
    if (x.dataset.calendrier === "jour") {
      x.addEventListener("click", () => {
        if (x.disabled) return;
        const pose = /^\d{4}-\d{2}-\d{2}$/.test(lireUnJour(x)) ? lireUnJour(x) : null;
        ouvrirLeCalendrier(x, {
          vue: moisDOuverture(pose),
          pris: (iso) => iso === pose,
          choisir: (iso) => { x.value = jjmmaaaa(iso); x.dispatchEvent(new Event("change", { bubbles: true })); },
        });
      });
      return;
    }
    // Des jours : la case (`data-case`) tient leur valeur, le bouton ouvre le calendrier.
    const c = x.closest("[data-case]");
    x.addEventListener("click", () => {
      if (x.disabled) return;
      // Chaque jour garde ce qu'il disait (un jour qui dit son propre lieu) tant qu'il reste coché. La case dit `change`
      // quand le calendrier se ferme : les jours cochés partent ensemble.
      const avant = JSON.parse(c.dataset.valeur);
      const isos = avant.map(jourDe);
      let change = false;
      ouvrirLeCalendrier(x, {
        ...moisAutour(c, o),
        vue: c.dataset.mois || moisDOuverture(isos.slice().sort()[0]),
        coches: isos,
        changer: (coches) => {
          const v = coches.map((iso) => avant.find((d) => jourDe(d) === iso) || iso);
          c.dataset.valeur = JSON.stringify(v);
          x.textContent = joursDits(v);
          change = true;
        },
        fermer: () => { if (change) c.dispatchEvent(new Event("change", { bubbles: true })); },
      });
    });
  });
}
