/* LE CALENDRIER, UN SEUL. « On ne code pas un calendrier spécifique pour la carte. On dit au moteur quand c'est une
 * date, c'est un calendrier. » (Léandre, 20/11/2026.)
 *
 * Un mois à la fois, sept colonnes, et on clique les jours. Ce qui le fait venir, c'est la forme que le moteur dit d'une
 * case (`forme` : `date` d'une case de la carte, `natures[].carte.cases` de la route etat, ou du champ d'une fiche
 * de route ; `jours` d'une clé d'une ligne de paie, `rubriques[].cles`) : la page ne décide pas, case par case, qu'ici
 * c'est un calendrier. Une case de forme `date` reçoit `data-calendrier` et `poserLesCalendriers` la branche ; les jours
 * d'une ligne de paie l'ouvrent avec leurs règles (`js/saisie-lignes.js`). */
"use strict";

/** Ferme le calendrier ouvert, s'il y en a un. */
function fermerLesCalendriers() {
  document.querySelectorAll(".cal").forEach((c) => c.remove());
}

/** OUVRE LE CALENDRIER sous `ancre`, dans son parent (qui le porte : `.cal-porte`). `o` dit ce qu'il montre :
 *  - `vue` : le mois montré d'abord, AAAA-MM ;
 *  - `jour(iso, vue)` : ce que la case d'un jour montre, `{ classe, lu, hors, titre }` (`lu` : il ne se clique pas ;
 *    `hors` : il est grisé) ;
 *  - `choisir(iso, vue, c)` : un clic sur un jour ; `c.redessiner()`, `c.fermer()` ;
 *  - `dessous(vue)` : ce qui s'écrit sous la grille (HTML), et `brancher(cal, vue, c)` ses boutons ;
 *  - `changer(vue)` : le mois montré change. */
function ouvrirLeCalendrier(ancre, o) {
  fermerLesCalendriers();
  const porte = ancre.parentElement;
  porte.classList.add("cal-porte");
  const cal = document.createElement("div");
  cal.className = "cal";
  porte.appendChild(cal);
  // Sous l'ancre, sans déborder de ce qui le porte.
  cal.style.left = Math.max(0, Math.min(ancre.offsetLeft, porte.clientWidth - 272)) + "px";
  let vue = o.vue;
  const c = {
    redessiner: () => { if (cal.isConnected) dessiner(); },
    fermer: fermerLesCalendriers,
  };
  const aller = (m) => { vue = m; if (o.changer) o.changer(vue); dessiner(); };
  const dessiner = () => {
    const an = +vue.slice(0, 4), mo = +vue.slice(5, 7);
    const nb = new Date(an, mo, 0).getDate();
    const premier = (new Date(an, mo - 1, 1).getDay() + 6) % 7;   // lundi = 0
    let cases = "";
    for (let i = 0; i < premier; i++) cases += `<span class="c-vide"></span>`;
    for (let j = 1; j <= nb; j++) {
      const iso = `${vue}-${String(j).padStart(2, "0")}`;
      const d = o.jour(iso, vue);
      const titre = d.titre ? ` title="${echapper(d.titre)}"` : "";
      cases += d.lu
        ? `<span class="c-jour lu ${d.classe || ""}"${titre}>${j}</span>`
        : `<button type="button" class="c-jour ${d.classe || ""} ${d.hors ? "hors" : ""}" data-iso="${iso}"${
            d.hors ? " disabled" : ""}${titre}>${j}</button>`;
    }
    cal.innerHTML = `
      <div class="c-tete">
        <button type="button" class="c-prec" title="mois précédent">‹</button>
        <b>${echapper(moisEnClair(vue))}</b>
        <button type="button" class="c-suiv" title="mois suivant">›</button>
      </div>
      <div class="c-grille">${JOURS_SEMAINE.map((x) => `<span class="c-sem">${x}</span>`).join("")}${cases}</div>
      ${o.dessous ? o.dessous(vue) : ""}`;
    cal.querySelector(".c-prec").onclick = () => aller(moisPrecedent(vue));
    cal.querySelector(".c-suiv").onclick = () => aller(moisSuivant(vue));
    cal.querySelectorAll("button.c-jour:not([disabled])").forEach((b) => {
      b.onclick = () => o.choisir(b.dataset.iso, vue, c);
    });
    if (o.brancher) o.brancher(cal, vue, c);
  };
  dessiner();
  // Fermer en cliquant ailleurs, ou à la touche Échap.
  setTimeout(() => {
    const dehors = (ev) => {
      if (!cal.isConnected) { document.removeEventListener("click", dehors); return; }
      // `composedPath` et non `contains` : un clic sur un jour redessine la grille, et le bouton cliqué n'est déjà plus
      // dans la page quand on arrive ici -- `contains` dirait « dehors » et refermerait tout.
      const chemin = ev.composedPath ? ev.composedPath() : [];
      if (!chemin.includes(cal) && !chemin.includes(ancre)) {
        fermerLesCalendriers(); document.removeEventListener("click", dehors);
      }
    };
    document.addEventListener("click", dehors);
  }, 0);
  cal.onkeydown = (ev) => { if (ev.key === "Escape") fermerLesCalendriers(); };
  return c;
}

/** LA CASE D'UN JOUR : `champ`, une case dont la valeur est AAAA-MM-JJ (vide : aucun jour), devient un bouton qui dit
 *  le jour (« 14/12/2026 ») et ouvre le calendrier ; un clic sur un jour le pose, et la case dit `input` puis `change`,
 *  comme si on l'avait tapé. La valeur reste celle que le moteur lit et écrit ; une valeur posée par le code (la fin
 *  calée sur le dernier jour coché) se montre aussitôt, et la case désactivée l'est aussi. */
function calendrierDUnJour(champ) {
  if (champ.dataset.calendrierPose) return;
  champ.dataset.calendrierPose = "1";
  champ.type = "hidden";
  const btn = document.createElement("button");
  btn.type = "button";
  btn.className = "cal-jour-btn";
  champ.after(btn);
  const montrer = () => {
    // Vide : ce que la case dit d'une case vide (« la fin du contrat »), sinon qu'un jour est à choisir.
    btn.textContent = champ.value ? jjmmaaaa(champ.value) : (champ.placeholder || "— jour —");
    btn.classList.toggle("vide", !champ.value);
    btn.disabled = champ.disabled;
  };
  const natif = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
  Object.defineProperty(champ, "value", {
    configurable: true,
    get() { return natif.get.call(champ); },
    set(v) { natif.set.call(champ, v); montrer(); },
  });
  new MutationObserver(montrer).observe(champ, { attributes: true, attributeFilter: ["disabled"] });
  montrer();
  const poser = (v) => {
    champ.value = v;
    champ.dispatchEvent(new Event("input", { bubbles: true }));
    champ.dispatchEvent(new Event("change", { bubbles: true }));
  };
  btn.onclick = (ev) => {
    ev.preventDefault();
    // D'abord le mois du jour posé ; sans jour, celui de l'écran de paie, sinon le jour du moteur.
    const ecran = document.getElementById("mois");
    const vue = champ.value ? champ.value.slice(0, 7) : (ecran && ecran.value) || aujourdhuiIso().slice(0, 7);
    if (!vue) return;
    ouvrirLeCalendrier(btn, {
      vue,
      jour: (iso) => ({ classe: iso === champ.value ? "joue" : "" }),
      choisir: (iso, _, c) => { poser(iso); c.fermer(); },
      dessous: () => champ.value ? `<div class="c-pied"><span></span>
        <button type="button" class="secondaire c-effacer">Effacer</button></div>` : "",
      brancher: (cal, _, c) => {
        const effacer = cal.querySelector(".c-effacer");
        if (effacer) effacer.onclick = () => { poser(""); c.fermer(); };
      },
    });
  };
}

/** Branche le calendrier de chaque case de `racine` qui en demande un (`data-calendrier`, posé là où le moteur dit
 *  qu'une case est un jour). */
function poserLesCalendriers(racine) {
  if (!racine) return;
  racine.querySelectorAll("input[data-calendrier]").forEach(calendrierDUnJour);
}
