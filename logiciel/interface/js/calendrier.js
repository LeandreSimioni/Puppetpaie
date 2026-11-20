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

/** OUVRE LE CALENDRIER sous `ancre`, dans ce qui la porte (`.cal-porte` : son premier parent qui n'est pas une
 *  étiquette — dans un `<label>`, un clic sur le fond du calendrier recliquerait la case de l'étiquette). `o` dit ce
 *  qu'il montre :
 *  - `vue` : le mois montré d'abord, AAAA-MM ;
 *  - `jour(iso, vue)` : ce que la case d'un jour montre, `{ classe, lu, hors, titre }` (`lu` : il ne se clique pas ;
 *    `hors` : il est grisé) ;
 *  - `choisir(iso, vue, c)` : un clic sur un jour ; `c.redessiner()`, `c.fermer()` ;
 *  - `dessous(vue)` : ce qui s'écrit sous la grille (HTML), et `brancher(cal, vue, c)` ses boutons ;
 *  - `changer(vue)` : le mois montré change.
 *  L'année du mois montré se tape dans sa tête : on va à une année lointaine sans cliquer mois par mois. */
function ouvrirLeCalendrier(ancre, o) {
  fermerLesCalendriers();
  let porte = ancre.parentElement;
  while (porte.tagName === "LABEL") porte = porte.parentElement;
  porte.classList.add("cal-porte");
  const cal = document.createElement("div");
  cal.className = "cal";
  porte.appendChild(cal);
  // Sous l'ancre, sans déborder de ce qui le porte.
  const a = ancre.getBoundingClientRect(), p = porte.getBoundingClientRect();
  cal.style.top = (a.bottom - p.top - porte.clientTop + porte.scrollTop) + "px";
  cal.style.left = Math.max(0, Math.min(a.left - p.left - porte.clientLeft, porte.clientWidth - 272)) + "px";
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
        <span class="c-titre"><b>${echapper(MOIS_FR[mo - 1])}</b>
          <input type="text" inputmode="numeric" maxlength="4" class="c-annee" value="${an}" title="année"></span>
        <button type="button" class="c-suiv" title="mois suivant">›</button>
      </div>
      <div class="c-grille">${JOURS_SEMAINE.map((x) => `<span class="c-sem">${x}</span>`).join("")}${cases}</div>
      ${o.dessous ? o.dessous(vue) : ""}`;
    cal.querySelector(".c-prec").onclick = () => aller(moisPrecedent(vue));
    cal.querySelector(".c-suiv").onclick = () => aller(moisSuivant(vue));
    // Une année tapée (quatre chiffres) montre le même mois de cette année ; autre chose ne se lit pas comme une
    // année, et la case reprend celle qui est montrée.
    const annee = cal.querySelector(".c-annee");
    annee.onchange = () => {
      if (/^\d{4}$/.test(annee.value.trim())) aller(`${annee.value.trim()}-${vue.slice(5, 7)}`);
      else annee.value = an;
    };
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

/** LA CASE D'UN JOUR : `champ`, une case dont la valeur est AAAA-MM-JJ (vide : aucun jour), se montre en une case de
 *  texte où l'on tape le jour (JJ/MM/AAAA) et qu'un clic ouvre sur le calendrier, à côté. Un jour tapé ou cliqué se
 *  pose dans `champ`, qui dit `input` puis `change`, comme si on l'avait tapé ; ce qui ne se lit pas comme JJ/MM/AAAA
 *  part tel quel, et le moteur le refuse en nommant sa case. La valeur reste celle que le moteur lit et écrit ; une
 *  valeur posée par le code (la fin calée sur le dernier jour coché) se montre aussitôt.
 *
 *  LA CASE TIENT SON VERROU : elle ne s'ouvre et ne s'écrit que si `champ` n'est pas désactivé (un contrat en cours a
 *  ses bornes verrouillées, `verrou`), et ce qui la montre est désactivé avec lui, quoi qu'un autre code y pose. */
function calendrierDUnJour(champ) {
  if (champ.dataset.calendrierPose) return;
  champ.dataset.calendrierPose = "1";
  champ.type = "hidden";
  const saisie = document.createElement("input");
  saisie.type = "text";
  saisie.className = "cal-jour-saisie";
  saisie.autocomplete = "off";
  champ.after(saisie);
  const montrer = () => {
    saisie.value = /^\d{4}-\d{2}-\d{2}$/.test(champ.value) ? jjmmaaaa(champ.value) : champ.value;
    saisie.placeholder = champ.placeholder;
    // Assez large pour JJ/MM/AAAA, et pour ce qu'elle montre (un texte qui ne se lit pas comme un jour, ce qu'elle
    // dit vide) : rien ne se coupe.
    saisie.size = Math.max(10, saisie.value.length, saisie.placeholder.length) + 1;
    if (saisie.disabled !== champ.disabled) saisie.disabled = champ.disabled;
  };
  const natif = Object.getOwnPropertyDescriptor(HTMLInputElement.prototype, "value");
  Object.defineProperty(champ, "value", {
    configurable: true,
    get() { return natif.get.call(champ); },
    set(v) { natif.set.call(champ, v); montrer(); },
  });
  new MutationObserver(montrer).observe(champ, { attributes: true, attributeFilter: ["disabled", "placeholder"] });
  // Un autre code qui rouvre ou ferme les cases d'une carte (`fermerEdition`) ne touche pas au verrou : la case montrée
  // suit `champ`, toujours.
  new MutationObserver(() => {
    if (saisie.disabled !== champ.disabled) saisie.disabled = champ.disabled;
  }).observe(saisie, { attributes: true, attributeFilter: ["disabled"] });
  montrer();
  const poser = (v) => {
    if (champ.disabled) return;
    champ.value = v;
    champ.dispatchEvent(new Event("input", { bubbles: true }));
    champ.dispatchEvent(new Event("change", { bubbles: true }));
  };
  // Un jour tapé : JJ/MM/AAAA (un ou deux chiffres pour le jour et le mois) devient AAAA-MM-JJ ; vide, la case se vide.
  saisie.onchange = () => {
    const t = saisie.value.trim();
    const m = /^(\d{1,2})\/(\d{1,2})\/(\d{4})$/.exec(t);
    poser(m ? `${m[3]}-${m[2].padStart(2, "0")}-${m[1].padStart(2, "0")}` : t);
  };
  saisie.onclick = () => {
    if (champ.disabled) return;
    // D'abord le mois du jour posé ; sans jour, celui de l'écran de paie, sinon le jour du moteur.
    const ecran = document.getElementById("mois");
    const pose = /^\d{4}-\d{2}-\d{2}$/.test(champ.value) ? champ.value.slice(0, 7) : "";
    const vue = pose || (ecran && ecran.value) || aujourdhuiIso().slice(0, 7);
    if (!vue) throw new Error("Le calendrier n'a aucun mois à montrer : ni jour posé, ni mois de l'écran, ni jour du moteur.");
    ouvrirLeCalendrier(saisie, {
      vue,
      jour: (iso) => ({ classe: iso === champ.value ? "joue" : "" }),
      choisir: (iso, _, c) => { poser(iso); c.fermer(); },
    });
  };
}

/** La case qu'on voit et qu'on tape pour `champ` : celle du calendrier quand il est posé, sinon `champ` lui-même. Ce
 *  qu'on fait défiler jusqu'à elle, ou ce qui prend le curseur. */
function caseQuiMontre(champ) {
  return champ.dataset.calendrierPose ? champ.nextElementSibling : champ;
}

/** Branche le calendrier de chaque case de `racine` qui en demande un (`data-calendrier`, posé là où le moteur dit
 *  qu'une case est un jour). */
function poserLesCalendriers(racine) {
  racine.querySelectorAll("input[data-calendrier]").forEach(calendrierDUnJour);
}
