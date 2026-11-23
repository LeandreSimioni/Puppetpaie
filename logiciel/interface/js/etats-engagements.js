/* L'état des engagements du mois (`etat-par-engagement`) : ses trois carrés, ses pièces, ses versions, le non actif, et
   les boîtes qu'il ouvre (la DPAE, l'avertissement d'une modification). La carte qui les montre est `js/carte.js`.

   L'état est celui des engagements enregistrés : un engagement qu'on vient d'ajouter n'en a pas encore, et sa carte n'en
   montre pas. Les carrés, leurs phrases et leurs gestes viennent du moteur ; un clic ouvre l'envoi que le moteur nomme,
   rien ne part d'un clic sans sa boîte. */
"use strict";

let ETATS_ENGAGEMENTS = null;      // les lignes d'`etat-par-engagement`, par identité d'engagement ; null = pas lu

/* LES TROIS CARRÉS DE LA LIGNE VALIDÉE : DPAE, Contrat, Documents. Le moteur
   rend chacun (`etat-par-engagement`, `carres`) : son nom, sa couleur, ce qui
   reste en un mot, sa phrase et le geste qu'ouvre un clic. La page les
   dessine ; un carré qui a un geste est un bouton, sauf sur la carte d'un
   engagement validé, qui s'ouvre toute entière sur son détail. */

/** Le bandeau d'une ligne : un carré par acte. */
function bandeauEtats(ligne, gestes = true) {
  return `<div class="etats-actes">` + ligne.carres.map((c, i) => {
    const clic = gestes && !!c.action;
    const classe = c.coupee ? "en-attente" : c.couleur;
    return `<span class="etat-acte ${classe}${clic ? " cliquable" : ""}"
        ${clic ? `data-carre="${i}" tabindex="0" role="button"` : ""}
        title="${echapper(c.phrase + (clic ? " — " + c.action.libelle : ""))}">
      <i class="carre"></i>${echapper(c.nom)}${c.reste ? ` <span class="reste">${echapper(c.reste)}</span>` : ""}</span>`;
  }).join("") + `</div>`;
}

/** Le clic sur un carré : le geste que le moteur lui donne. Le contrat ou l'avenant parti
 *  et pas revenu se note signé d'un clic, et le message permet de revenir. */
function brancherRecu(div) {
  div.querySelectorAll(".etat-acte.cliquable").forEach((s) => {
    const agir = (ev) => {
      ev.preventDefault();
      ev.stopPropagation();
      const ligne = (s.closest(".engagement") || div)._ligne;
      const c = ligne && ligne.carres[+s.dataset.carre];
      if (!c || !c.action) return;
      const mois = document.getElementById("mois").value;
      ouvrirGeste(c.action, mois);
    };
    s.onclick = agir;
    s.onkeydown = (ev) => { if (ev.key === "Enter" || ev.key === " ") agir(ev); };
  });
}

/** « Reçu signé » (`contrat-recu`) du contrat, ou de l'avenant que la charge
 *  nomme (`avenant`), ou son annulation. Le message est la phrase du moteur ;
 *  une note posée porte « Annuler », qui rappelle la charge que le moteur
 *  rend pour la défaire (`annuler`). */
async function noterRecu(mois, charge) {
  let r;
  try {
    r = await api("contrat-recu", charge);
  } catch (e) { dire(e.message, "erreur"); return; }
  await dessinerEtatsDesEngagements(mois);
  await rafraichirActions();
  dire(r.phrase);
  if (!r.annuler) return;
  const el = document.getElementById("message");
  const annuler = document.createElement("button");
  annuler.className = "lien message-annuler";
  annuler.textContent = r.annuler.libelle;
  annuler.onclick = () => { el.classList.add("cachee"); noterRecu(mois, r.annuler.charge); };
  el.append(" — ", annuler);
}

/** Les pièces parties d'un engagement pour un acte (`dpae`, `contrat-signature` : la clé de son carré). */
async function boitePieces(ligne, acte) {
  const mois = document.getElementById("mois").value;
  let r, lu;
  try {
    r = await api("pieces-parties", { salarie: ligne.salarie, numero: ligne.numero });
    lu = await api("etat-par-engagement", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  // Le moteur rend, avec le carré (`carres[].pieces`), ce que le détail montre : la phrase sans pièce, l'encart d'un
  // contrat qui n'est plus le bon, et les gestes.
  // Le carré d'un acte qui a des pièces (la DPAE, la signature du contrat) les porte toujours.
  const sienne = lu.engagements.find((l) => l.salarie === ligne.salarie && l.bornes?.du === ligne.bornes?.du) || ligne;
  const carre = sienne.carres.find((c) => c.cle === acte);
  const montre = carre.pieces;
  const titre = carre.titre;
  const pieces = acte === "dpae" ? r.dpae : r.contrat;
  // Chaque pièce : son geste quand son fichier est là (`ouvrir`), la phrase qui dit qu'il n'y est plus sinon.
  const liste = pieces.length
    ? `<ul class="liste-pieces">${pieces.map((p, i) => `<li>
        <span><b>${echapper(p.quoi)}</b> — ${echapper(p.quand_dit)}
          ${p.absente ? `<br><span class="discret">${echapper(p.absente)}</span>` : ""}</span>
        ${p.ouvrir ? `<button class="secondaire p-ouvrir" data-i="${i}">${echapper(p.ouvrir.libelle)}</button>` : ""}
      </li>`).join("")}</ul>`
    : montre.vide ? `<p class="discret">${echapper(montre.vide)}</p>` : "";
  ouvrirBoite(`
    <h3>${echapper([titre, ligne.nom].filter(Boolean).join(" — "))}</h3>
    <p class="discret">${echapper([pourQuoi(ligne), ligne.periode_dit].filter(Boolean).join(", "))}</p>
    ${carre.phrase ? `<p>${echapper(carre.phrase)}</p>` : ""}
    ${montre.encart ? `<div class="encart attention"><b>${echapper(montre.encart.titre)}</b>
      ${echapper(montre.encart.texte)}</div>` : ""}
    ${liste}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${montre.gestes.map((g, i) => `<button class="${classeDuGeste(g)} p-geste" data-i="${i}"
        ${g.titre ? `title="${echapper(g.titre)}"` : ""}>${echapper(g.libelle)}</button>`).join("")}
    </div>`);
  document.querySelectorAll(".p-geste").forEach((b) => {
    const a = montre.gestes[+b.dataset.i].action;
    b.onclick = () => {
      fermerBoite();
      ouvrirGeste(a, mois);
    };
  });
  document.querySelectorAll(".p-ouvrir").forEach((b) => {
    // La pièce partie : le moteur rend son chemin, la fenêtre l'ouvre.
    b.onclick = () => ouvrirGeste(pieces[+b.dataset.i].ouvrir, mois);
  });
}

/** Lit `etat-par-engagement` et le pose sur les cartes (`peindreEtats`, `js/carte.js`). Un refus se dit, et les cartes
 *  restent comme elles sont. */
async function dessinerEtatsDesEngagements(mois) {
  if (!mois) { ETATS_ENGAGEMENTS = null; peindreEtats(); return; }
  try {
    const r = await api("etat-par-engagement", { mois });
    ETATS_ENGAGEMENTS = new Map();
    r.engagements.forEach((l) => { ETATS_ENGAGEMENTS.set(JSON.stringify(l.engagement), l); });
    peindreNonActifs(r.non_actifs);
  } catch (e) {
    ETATS_ENGAGEMENTS = null;
    dire(e.message, "erreur");
  }
  peindreEtats();
}

/* UN ENGAGEMENT VALIDÉ DONT LA PAIE S'EST RECALCULÉE D'ELLE-MÊME : le moteur dit quand et pourquoi (`recalcul` de
 * `etat-par-engagement`) ; la carte le montre tel quel. */
function recalculDe(ligne) {
  return ligne && ligne.recalcul ? `<div class="encart carte-recalcul">${echapper(ligne.recalcul)}</div>` : "";
}

/* UN ENGAGEMENT MODIFIÉ APRÈS UN ENVOI : la ligne que le moteur écrit (« Modifié le 12/11, après l'envoi du contrat »,
 * `versions.phrase` d'`etat-par-engagement`) ouvre la frise de ses versions. Jamais modifié après un envoi : rien. */
function versionsDe(ligne) {
  return ligne && ligne.versions
    ? `<button class="lien carte-versions">${echapper(ligne.versions.phrase)}</button>` : "";
}

/** La frise : par version, sa date, ce qui a changé depuis la précédente, et ce qui est parti sur elle. */
function boiteFrise(ligne) {
  const vs = ligne.versions.versions;
  ouvrirBoite(`
    <h3>${echapper(ligne.nom)}</h3>
    <p class="discret">${echapper([pourQuoi(ligne), ligne.periode_dit].filter(Boolean).join(", "))}</p>
    <ol class="frise">${vs.map((v) => `
      <li>
        <b>${echapper(v.titre_dit)}</b> <span class="discret">— ${echapper(v.jour_dit)}</span>
        ${v.changements.length ? `<ul class="frise-changements">${v.changements.map((c) =>
          `<li>${echapper(c)}</li>`).join("")}</ul>` : ""}
        ${v.partis_dit ? `<p class="frise-partis">${echapper(v.partis_dit)}</p>` : ""}
      </li>`).join("")}
    </ol>
    <div class="actions"><button class="secondaire" onclick="fermerBoite()">Fermer</button></div>`);
}

/** L'avertissement du moteur (`retirer-engagement`) : sa phrase, et le choix qu'il nomme : rendre l'engagement non
 *  actif, ou le garder. */
function boiteNonActif(a) {
  return new Promise((resoudre) => {
    ouvrirBoite(`
      <h3>${echapper(a.titre)}</h3>
      <div class="encart attention">${echapper(a.phrase)}</div>
      <div class="actions">
        <button class="secondaire" id="r-garder">${echapper(a.garder)}</button>
        <button class="primaire" id="r-non-actif">${echapper(a.confirmer)}</button>
      </div>`, () => resoudre(false));
    document.getElementById("r-garder").onclick = () => { fermerBoite(); resoudre(false); };
    document.getElementById("r-non-actif").onclick = () => { resoudre(true); fermerBoite(); };
  });
}

/** Les engagements non actifs du mois (`etat-par-engagement`, `non_actifs`) : une
 *  carte qui se lit, qui ne s'ouvre pas, et ce que leur désactivation rend à faire. */
function peindreNonActifs(liste) {
  const cible = document.getElementById("engagements");
  if (!cible) return;
  cible.querySelectorAll(".engagement-non-actif").forEach((d) => d.remove());
  liste.forEach((n) => {
    const d = document.createElement("div");
    d.className = "engagement-non-actif";
    d.innerHTML = `<div class="na-ligne">
        <span><b>${echapper(n.nom)}</b>
          <span class="discret">· ${echapper(n.dit)}</span></span>
        <span class="mot-etat" title="${echapper(n.non_actif_dit)}">${echapper(n.mot)}</span>
      </div>
      ${n.a_faire?.length ? `<div class="etats-actes">${n.a_faire.map((x) =>
        `<span class="etat-acte orange"><i class="carre"></i>${echapper(x)}</span>`).join("")}</div>` : ""}`;
    cible.appendChild(d);
  });
}

/** L'avertissement du moteur : chaque engagement nommé une fois, une ligne par ce que le changement entraîne, et,
 *  quand un terme change dans le mois de la carte, d'où il vaut (`jour`) avec la case du jour et ce qu'elle montre
 *  d'abord (`jour_libelle`, `jour_propose`). Rend la réponse (`{jour}`) à la confirmation, `null` à « Annuler ». */
function boiteAvertissement(a) {
  return new Promise((repondre) => {
    ouvrirBoite(`<h3>${echapper(a.titre)}</h3>
      ${a.engagements.map((e) => `<div class="encart attention"><b>${echapper(e.qui)}</b>
        ${e.consequences.length ? `<ul>${e.consequences.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul>` : ""}</div>`).join("")}
      ${a.jour ? `<p>${echapper(a.jour)}</p>
        <label>${echapper(a.jour_libelle)} <input id="av-jour" class="jour-du-mois" type="text" inputmode="numeric"
          value="${echapper(a.jour_propose)}"></label>` : ""}
      <div class="actions">
        <button class="secondaire" id="av-non">${echapper(a.annuler)}</button>
        <button class="primaire" id="av-oui">${echapper(a.confirmer)}</button>
      </div>`, () => repondre(null));
    document.getElementById("av-non").onclick = () => fermerBoite();
    document.getElementById("av-oui").onclick = () => {
      const jour = document.getElementById("av-jour");
      repondre({ jour: jour && jour.value.trim() ? jour.value : null });
      fermerBoite();
    };
  });
}

/* LA BOÎTE DPAE, comme celle des signatures : une liste déroulante.
 * Chaque contrat du mois enregistré, une case ; cochée
 * d'avance si aucune DPAE n'est partie, ou si elle a été refusée. Un contrat
 * en cours depuis un mois précédent n'a pas de DPAE ce mois-ci : il se voit,
 * grisé, avec le mois où elle a été faite.
 *
 * LES EMBAUCHES COCHÉES SE DÉSIGNENT : le récapitulatif de dépôt habituel
 * (`boiteDepot`) écrit leur DPAE, ou reprend celle qui dit la même chose,
 * à côté du fichier complet du mois, jamais par-dessus. Écrire n'envoie rien ;
 * le dépôt reste le même geste, avec la même relecture et le même mot de passe. */
async function boiteDpae(mois) {
  if (!mois) return;
  let r;
  try {
    r = await api("etat-par-engagement", { mois });
  } catch (e) {
    // Un mois sans engagement rend une liste vide (plus bas) ; un refus est un refus, et il se dit tel quel.
    dire(e.message, "erreur");
    return;
  }
  const lignes = r.engagements;
  if (!lignes.length) { dire(r.phrase); return; }
  // Le moteur dit, par engagement (`dpae_choix`), s'il se coche, s'il est coché
  // d'avance, son état en mots, sa couleur et ce qu'il ajoute coché (`part`) ; et
  // le geste qui envoie (`dpae_envoyer`), aucun quand rien ne se coche.
  const g = r.dpae_envoyer;

  ouvrirBoite(`
    <h3>${echapper(r.dpae_titre)}</h3>
    ${moisEnGros(mois)}
    <p class="note">${echapper(r.dpae_notes[0])}</p>
    <div class="liste-choix">
      ${lignes.map((l, i) => {
        const e = l.etats.dpae;
        const c = l.dpae_choix;
        const etat = `<br>${pastilleDeLEtat({ couleur: c.couleur, mots: c.phrase })}`;
        return `<label class="choix ${c.cochable ? "" : "grise"}">
          <input type="checkbox" class="d-qui" data-i="${i}" ${c.cochee ? "checked" : ""}
                 ${c.cochable ? "" : "disabled"}>
          <span><b>${echapper(l.nom)}</b> — ${echapper(l.embauche_dit)}${l.numero_dit
              ? ` <span class="discret">(${echapper(l.numero_dit)})</span>` : ""}
            ${etat}
            ${e.date_changee ? `<br><span class="note">${echapper(e.date_changee.phrase)}</span>` : ""}</span></label>`;
      }).join("")}
    </div>
    <p class="note">${echapper(r.dpae_notes[1])}</p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${g ? `<button class="primaire" id="d-ecrire">${echapper(g.libelle)}</button>` : ""}
    </div>`);
  if (!g) return;

  // Le bouton dit le geste tant qu'une embauche est cochée, la phrase du moteur sinon.
  const bouton = document.getElementById("d-ecrire");
  const parts = () => [...document.querySelectorAll(".d-qui:checked")].map((c) => lignes[+c.dataset.i].dpae_choix.part);
  const majBouton = () => {
    const n = parts().length;
    bouton.disabled = !n;
    bouton.textContent = n ? g.libelle : r.dpae_rien;
  };
  document.querySelectorAll(".d-qui").forEach((c) => { c.onchange = majBouton; });
  majBouton();
  bouton.onclick = async () => {
    const choisies = parts();
    if (!choisies.length) return;
    const rendre = occupe(bouton);
    await ouvrirGeste({ ...g, charge: { ...g.charge, engagements: choisies } }, mois);
    rendre();
  };
}
