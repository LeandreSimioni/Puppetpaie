/* LA CARTE D'UN ENGAGEMENT, dessinée par le dessinateur seul (`js/avance.js`), depuis ce que les routes rendent.
 *
 * Ce que la carte montre et fait saisir : les cases de sa nature (`natures[].carte` de la route etat), chacune sa forme et
 * son libellé, la valeur que la carte écrit (la saisie du mois, route `saisie` ; puis le brouillon que rend chaque
 * écriture, route `enregistrer-saisie`). Une case obligatoire se montre ; une case facultative se montre remplie, ou
 * quand le contrat écrit de cet engagement l'appelle (`contrat.ecrits` de l'aperçu) ; les autres sont sous « Plus ».
 * Sous les cases, ce que l'aperçu dit de sa paie ; puis son état (`etat-par-engagement`) et ses gestes.
 *
 * Chaque case changée part seule à la route (`enregistrer-saisie`, `changements`) ; la carte se redessine de ce que la
 * route rend. Un engagement validé se montre en une ligne ; ouvert, ses changements partent à « Modifier l'engagement »
 * (`modifier-engagement`). La carte ne calcule, ne compte ni ne décide rien : la route le fait. */
"use strict";

/** La nature de clé `cle`, telle que la route etat la décrit (`natures`). */
function natureDe(cle) {
  return ETAT.natures.find((n) => n.cle === cle);
}

/** Une valeur dite : ni absente, ni vide. */
const dite = (v) => v != null && v !== "" && !(Array.isArray(v) && !v.length);

/** Les cases d'une carte : celles de sa nature, puis celles que son motif de recours porte. */
function casesDeLaCarte(c) {
  const n = natureDe(c.nature).carte;
  const m = n.motifs_recours.find((x) => x.code === c.motif_recours);
  return [...n.cases, ...(m ? m.cases : [])];
}

/** Ce que la carte propose de rubriques (`propose` : de sa paie dans l'aperçu ; avant, de la carte neuve de sa nature). */
function proposeDe(div) {
  return (div._paie && div._paie.propose) || NOUVELLE_CARTE.propose[div._carte.nature] || { rubriques: [], par_defaut: null };
}

/** Une case de la carte : son libellé et ce que le dessinateur en dessine. */
function caseDeLaCarte(div, d) {
  const v = div._carte[d.cle];
  if (d.forme === "lignes") {
    const lignes = v || [];
    const propose = proposeDe(div);
    // Ce que les cases d'une ligne de frais offrent, le moteur le dit (`cases` de sa paie, ligne par ligne).
    const cases = (i) => (div._paie && div._paie.cases
      ? { cases: div._paie.cases.filter((x) => x.ligne === i).map((x) => ({ code: x.case, libelle: x.libelle, aide: x.aide })) } : {});
    const offertes = [...new Set([...propose.rubriques, ...lignes.map((l) => l.rubrique).filter(Boolean)])];
    return `<div class="case case-lignes" data-cle="${d.cle}"><span class="case-libelle">${echapper(d.libelle)}</span>
      ${lignesDePaie(lignes, offertes, cases)}
      <button type="button" class="lien ajouter-ligne">+ une ligne</button></div>`;
  }
  return `<label class="case" data-cle="${d.cle}"><span class="case-libelle">${echapper(d.libelle)}</span>
    ${caseDeForme(d.forme, v, { choix: ETAT.choix[d.cle] })}</label>`;
}

/** Une case se montre-t-elle hors de « Plus » ? Obligatoire, remplie, ou appelée par le contrat écrit. */
function caseMontree(div, d) {
  const appelees = div._paie && div._paie.contrat ? div._paie.contrat.ecrits : [];
  return d.exige === "obligatoire" || dite(div._carte[d.cle]) || appelees.includes(d.cle);
}

/** UNE CARTE NEUVE OU LUE : son élément, sa carte (`c`, ce qu'elle écrit), son identité (`id`, absente : jamais écrite). */
function carteDEngagement(c, id) {
  const div = document.createElement("div");
  div.className = "engagement";
  div._carte = c;
  if (id) div.dataset.engagement = JSON.stringify(id);
  dessinerCarte(div);
  return div;
}

/** Un engagement validé, montré en une ligne (pas ouvert pour être modifié). */
function carteFermee(div) {
  return !!(div._ligne && div._ligne.valide && !div._modif);
}

/** DESSINE LA CARTE `div` depuis ce qu'elle sait : sa carte, sa paie, son état. */
function dessinerCarte(div) {
  div.classList.toggle("validee", !!(div._ligne && div._ligne.valide));
  div.classList.toggle("en-modification", !!div._modif);
  if (carteFermee(div)) { dessinerLaLigne(div); return; }
  const cases = casesDeLaCarte(div._carte);
  const montrees = cases.filter((d) => caseMontree(div, d));
  const autres = cases.filter((d) => !caseMontree(div, d));
  // La nature ne se choisit que là où le mois en propose plusieurs (`nouvelle_carte` de la route saisie).
  const natures = NOUVELLE_CARTE.nature_au_choix
    ? `<label class="case" data-cle="nature"><span class="case-libelle">Nature</span>
        ${caseDeForme("choix", div._carte.nature, { choix: ETAT.natures.filter((n) => NOUVELLE_CARTE.natures.includes(n.cle)
          || n.cle === div._carte.nature).map((n) => ({ code: n.cle, libelle: n.libelle })) })}</label>` : "";
  div.innerHTML = `
    <div class="e-cases">${natures}${montrees.map((d) => caseDeLaCarte(div, d)).join("")}</div>
    ${autres.length ? `<details class="e-plus"${div._plus ? " open" : ""}><summary>Plus</summary>
      <div class="e-cases">${autres.map((d) => caseDeLaCarte(div, d)).join("")}</div></details>` : ""}
    <div class="e-paie"></div>
    <div class="e-imprevus"></div>
    <div class="etats"></div>`;
  brancherLesCases(div);
  const plus = div.querySelector(".e-plus");
  if (plus) plus.ontoggle = () => { div._plus = plus.open; };
  div.onchange = (ev) => caseChangee(div, ev.target);
  div.querySelectorAll(".ajouter-ligne").forEach((b) => {
    b.onclick = () => {
      const r = rubriqueDe(proposeDe(div).par_defaut);
      const neuve = r ? ligneDonnee({ rubrique: r.code }, r) : {};
      changer(div, [{ case: "lignes", valeur: [...lireLesLignes(b.closest(".case")), neuve] }]);
    };
  });
  div.querySelectorAll(".retirer-ligne").forEach((b) => {
    b.onclick = () => {
      const bloc = b.closest(".case");
      const lignes = lireLesLignes(bloc).filter((_, i) => i !== +b.closest(".ligne-paie").dataset.ligne);
      changer(div, [{ case: "lignes", valeur: lignes }]);
    };
  });
  peindreLaPaie(div);
  peindreLesEtats(div);
  peindreImprevusDesCartes();
}

/** Une case a changé : sa valeur, lue par sa forme, part à la route. */
function caseChangee(div, x) {
  const c = x.closest("[data-cle]:not([data-case])") || x.closest(".case");
  if (!c) return;
  const lignes = x.closest(".case-lignes");
  if (lignes) { changer(div, [{ case: "lignes", valeur: lireLesLignes(lignes) }]); return; }
  const v = lireLaCase(c.querySelector("[data-case]"));
  if (v === undefined) return;
  changer(div, [{ case: c.dataset.cle, valeur: v }]);
}

/** LES CHANGEMENTS D'UNE CARTE, ÉCRITS. Une carte en saisie : la route écrit son brouillon (`enregistrer-saisie`), et la
 *  carte se redessine de ce qu'elle rend. Un engagement validé ouvert : ils s'ajoutent à ceux que « Modifier
 *  l'engagement » écrira, et l'aperçu les calcule avant. */
async function changer(div, changements) {
  const mois = MOIS_AFFICHE;
  if (div._modif) {
    changements.forEach((ch) => {
      div._modif = div._modif.filter((x) => x.case !== ch.case).concat([ch]);
      div._carte = { ...div._carte, [ch.case]: ch.valeur };
    });
    dessinerCarte(div);
    calculerApercu();
    return;
  }
  noteSaisie("Enregistrement…");
  let r;
  try {
    // La carte visée se lit à son tour : une carte neuve écrite par l'appel d'avant a désormais son identité.
    r = await aSonTour(() => api("enregistrer-saisie", { mois, changements,
      carte: div.dataset.engagement ? { ecrite: JSON.parse(div.dataset.engagement) } : { neuve: div._carte.nature } }));
  } catch (e) {
    noteSaisie("Non enregistré — " + e.message);
    dire(e.message, "erreur");
    dessinerCarte(div);
    return;
  }
  div.dataset.engagement = JSON.stringify(r.engagement);
  div._carte = r.brouillon;
  div._aCompleter = r.a_completer;
  noteSaisie(r.enregistre_dit);
  dessinerCarte(div);
  await apresUneEcriture(mois);
}

/** Après une écriture : ce qui se lit sur la saisie enregistrée se relit (l'historique des mois, les étapes, les
 *  imprévus, l'aperçu). */
async function apresUneEcriture(mois) {
  oublierCouts();
  try { ETAT.historique = (await api("etat")).historique; } catch (e) { dire(e.message, "erreur"); }
  await rafraichirActions();
  await dessinerImprevus(mois);
  calculerApercu();
}

/* ------------------------------------------------ ce que l'aperçu dit de sa paie */

/** Sous les cases : ce qui manque à l'engagement, ou ce que le moteur a compris de lui, son net et son coût, et ce
 *  qu'il en dit (`apercu`, la paie de son identité). */
function peindreLaPaie(div) {
  const cible = div.querySelector(".e-paie");
  if (!cible) return;
  const p = div._paie;
  const manque = (p && p.a_completer) || div._aCompleter;
  if (manque) {
    cible.innerHTML = `<div class="encart attention"><b>À compléter</b> ${erreurEnClair(manque)}</div>`;
    return;
  }
  if (!p) { cible.innerHTML = ""; return; }
  cible.innerHTML = `
    <p class="e-resolu">${echapper(p.resolu_dit)}</p>
    <p class="e-net">Net <b>${echapper(euros(p.net_a_payer))}</b>
      <span class="e-cout">Coût employeur <b>${echapper(euros(p.cout_employeur))}</b></span></p>
    ${p.avertissements.map((a) => `<div class="encart attention">${echapper(a)}</div>`).join("")}
    ${p.informations.map((a) => `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    ${(p.a_dire || []).map((m) => `<div class="encart info">${echapper(m.trouver)}
      <button type="button" class="lien" data-manque-champ="${echapper(m.champ)}" data-manque-onglet="${echapper(m.onglet)}"
        data-manque-fiche="salarie" data-manque-cle="${echapper(m.salarie)}">${echapper(m.ou)}</button></div>`).join("")}
    ${conseilDuBloc([p])}`;
}

/** Ce que l'aperçu rend pour chaque carte : la paie de son identité (une carte jamais écrite n'en a pas). */
function poserLesPaies(a) {
  document.querySelectorAll("#engagements .engagement").forEach((div) => {
    const p = div.dataset.engagement ? a.paies.find((x) => JSON.stringify(x.engagement) === div.dataset.engagement) : null;
    const avant = div._paie;
    div._paie = p || null;
    // Ce que la carte propose et ce que son contrat appelle viennent de sa paie : la carte se redessine quand ils
    // changent, sinon seule sa paie se repeint (une case qu'on tape ne se redessine pas sous les doigts).
    const forme = (x) => JSON.stringify(x ? [x.propose, x.contrat && x.contrat.ecrits, x.cases] : null);
    if (forme(avant) !== forme(div._paie) && !div.contains(document.activeElement) && !CALENDRIER_OUVERT) dessinerCarte(div);
    else if (carteFermee(div)) dessinerLaLigne(div);
    else peindreLaPaie(div);
  });
}

/* ------------------------------------------------ l'état de l'engagement et ses gestes */

/** LA LIGNE D'UN ENGAGEMENT VALIDÉ : qui et pour quoi, ce que le moteur a compris de lui, ses trois carrés, son net.
 *  Un clic l'ouvre, toutes ses cases modifiables : rien ne s'écrit avant « Modifier l'engagement ». */
function dessinerLaLigne(div) {
  const ligne = div._ligne, p = div._paie;
  div.innerHTML = `
    <div class="e-resume">
      <div class="carte-qui"><span class="carte-nom"><b>${echapper(ligne.nom)}</b></span>
        <span class="discret">${echapper([pourQuoi(ligne), p && p.resolu_dit].filter(Boolean).join(" · "))}</span></div>
      <div class="carte-actes">${bandeauEtats(ligne, false)}</div>
      <div class="carte-net" title="net à payer">${p ? echapper(euros(p.net_a_payer)) : ""}</div>
      ${recalculDe(ligne)}${versionsDe(ligne)}
      <span class="carte-modifier"><button class="lien carte-imprevu" title="Imprévu" aria-label="Un imprévu">&hellip;</button></span>
    </div>
    <div class="e-imprevus"></div>`;
  const resume = div.querySelector(".e-resume");
  rendreOuvrable(resume, () => ouvrirEnModification(div), "Modifier l'engagement");
  const porte = div.querySelector(".carte-imprevu");
  porte.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); menuImprevu(porte, { div }); };
  const frise = div.querySelector(".carte-versions");
  if (frise) frise.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); boiteFrise(ligne); };
  peindreImprevusDesCartes();
}

/** Sous une carte ouverte : son état (`etat-par-engagement`) et ses gestes. */
function peindreLesEtats(div) {
  const cible = div.querySelector(".etats");
  if (!cible) return;
  const ligne = div._ligne;
  const imprevu = div._carte.salarie ? `<button type="button" class="lien e-imprevu-btn">Imprévu…</button>` : "";
  const retirer = `<button class="lien retirer-engagement">Retirer cet engagement</button>`;
  const gestes = div._modif
    ? `${retirer}<button class="secondaire v-annuler">Annuler</button><button class="primaire v-modifier">Modifier l'engagement</button>`
    : `${retirer}<button class="primaire v-valider">Valider l'engagement</button>`;
  const pieces = ligne && ligne.valide ? ligne.carres.filter((c) => c.lien).map((c) =>
    `<button class="lien v-pieces" data-acte="${echapper(c.cle)}">${echapper(c.lien)}</button>`).join(" · ") : "";
  cible.innerHTML = `${ligne ? quiEtPourQuoi(ligne) + bandeauEtats(ligne) : ""}
    <div class="validation">${pieces ? `<span>${pieces}</span>` : ""}<span class="boutons">${imprevu}${gestes}</span></div>`;
  brancherRecu(div);
  const sur = (s, f) => { const b = cible.querySelector(s); if (b) b.onclick = (ev) => { ev.preventDefault(); f(ev); }; };
  sur(".v-valider", () => validerLaCarte(div));
  sur(".v-modifier", () => modifierLaCarte(div));
  sur(".v-annuler", () => chargerMois(MOIS_AFFICHE, true));
  sur(".retirer-engagement", () => retirerLaCarte(div));
  sur(".e-imprevu-btn", (ev) => { ev.stopPropagation(); menuImprevu(ev.currentTarget, { div }); });
  cible.querySelectorAll(".v-pieces").forEach((b) => { b.onclick = () => boitePieces(ligne, b.dataset.acte); });
}

/** Les états des engagements lus (`ETATS_ENGAGEMENTS`), posés sur leurs cartes : une carte qui change d'état (validée,
 *  ou plus) se redessine, une autre ne repeint que son état. */
function peindreEtats() {
  document.querySelectorAll("#engagements .engagement").forEach((div) => {
    const ligne = ETATS_ENGAGEMENTS && div.dataset.engagement ? ETATS_ENGAGEMENTS.get(div.dataset.engagement) : undefined;
    const avant = !!(div._ligne && div._ligne.valide);
    div._ligne = ligne;
    if (avant !== !!(ligne && ligne.valide)) dessinerCarte(div);
    else if (carteFermee(div)) dessinerLaLigne(div);
    else peindreLesEtats(div);
  });
}

/** « Valider l'engagement » : ce qui s'écrit encore s'écrit d'abord ; puis la route valide le brouillon écrit, ou dit le
 *  fait qui lui manque sans rien écrire. */
async function validerLaCarte(div) {
  const mois = MOIS_AFFICHE;
  await enregistrerSaisie(true);
  if (!div.dataset.engagement) { dire(ETAT.aides.qui_engage, "erreur"); return; }
  try {
    const r = await aSonTour(() => api("valider-engagement", { mois, engagement: JSON.parse(div.dataset.engagement) }));
    dire(r.phrase);
  } catch (e) { dire(e.message, "erreur"); return; }
  await chargerMois(mois, true);
}

/** « Retirer cet engagement » : la route décide (rien de parti, il s'efface ; un envoi parti, elle avertit, et la page
 *  propose de le rendre non actif). Une carte jamais écrite s'efface de l'écran. */
async function retirerLaCarte(div) {
  const mois = MOIS_AFFICHE;
  await enregistrerSaisie(true);
  if (!div.dataset.engagement) { div.remove(); calculerApercu(); dire(ETAT.aides.engagement_retire); return; }
  const charge = { mois, engagement: JSON.parse(div.dataset.engagement), confirme: false };
  try {
    let r = await api("retirer-engagement", charge);
    if (r.avertissement) {
      if (!await boiteNonActif(r.avertissement)) return;
      r = await api("retirer-engagement", { ...charge, confirme: true });
    }
    dire(r.phrase);
    await chargerMois(mois, true);
  } catch (e) { dire(e.message, "erreur"); }
}

/** UN CLIC SUR UN ENGAGEMENT VALIDÉ L'OUVRE, toutes ses cases modifiables ; rien ne s'écrit avant « Modifier
 *  l'engagement ». */
function ouvrirEnModification(div) {
  if (!div._ligne || !div._ligne.valide) return;
  div._modif = [];
  dessinerCarte(div);
}

/** Une carte s'ouvre : un engagement validé, pour être modifié ; une autre l'est déjà. */
function ouvrirCarte(div) {
  ouvrirEnModification(div);
}

/** « MODIFIER L'ENGAGEMENT » : ce qu'on a changé, case par case ; la route dit si rien n'a changé, écrit, ou rend son
 *  avertissement quand ce qui est parti le demande. */
async function modifierLaCarte(div) {
  const mois = MOIS_AFFICHE;
  if (!div._modif.length) { await chargerMois(mois, true); return; }
  const charge = { mois, engagement: JSON.parse(div.dataset.engagement), changements: div._modif, confirme: false, jour: null };
  try {
    let r = await api("modifier-engagement", charge);
    while (r && r.avertissement) {
      const reponse = await boiteAvertissement(r.avertissement);
      if (!reponse) return;
      r = await api("modifier-engagement", { ...charge, confirme: true, jour: reponse.jour });
    }
    if (r && r.change) dire(r.phrase);
    await chargerMois(mois, true);
  } catch (e) { dire(e.message, "erreur"); }
}

/** Les engagements validés ouverts et changés, pas encore écrits : l'aperçu les calcule (`modifications`). */
function modificationsAuMoteur() {
  return [...document.querySelectorAll("#engagements .engagement")]
    .filter((div) => div._modif && div._modif.length)
    .map((div) => ({ engagement: JSON.parse(div.dataset.engagement), changements: div._modif }));
}

/* ------------------------------------------------ ce que d'autres écrans lisent d'une carte */

/** Le salarié d'une carte, absent quand elle n'en dit pas. */
function salarieDuBloc(div) {
  return div._carte.salarie || undefined;
}

/** Le début du contrat d'une carte : celui qu'elle écrit, sinon celui que l'aperçu rend de ses jours. */
function debutDuBloc(div) {
  return div._carte.debut || (div._paie && div._paie.bornes ? div._paie.bornes.du : undefined);
}

/** Ce qu'un engagement dit de lui-même en une ligne : son spectacle, ou la nature de son contrat. */
function pourQuoi(ligne) {
  return ligne.spectacle || ligne.nature;
}

/** Qui et pour quoi, sur le bandeau des états d'une carte. */
function quiEtPourQuoi(ligne) {
  const quoi = pourQuoi(ligne);
  return `<span class="etats-qui">${echapper(texteDeLaCase(ligne.nom))}${
    quoi ? ` <span class="discret">— ${echapper(quoi)}</span>` : ""}</span>`;
}

/* ------------------------------------------------ les cartes du mois */

/** Les cartes du mois lu (`saisie`) : une par engagement, sa carte et son identité. */
function poserLesCartes(r) {
  const cible = document.getElementById("engagements");
  cible.innerHTML = "";
  r.saisie.engagements.forEach((e, i) => {
    const div = carteDEngagement(e, r.identites[i]);
    div._aCompleter = r.a_completer[i];
    cible.appendChild(div);
  });
}

/** Les listes des cartes ouvertes, refaites sur l'ETAT relu (une fiche créée depuis la configuration). */
function rafraichirReferentiels() {
  document.querySelectorAll("#engagements .engagement").forEach((div) => {
    if (!carteFermee(div) && !div.contains(document.activeElement)) dessinerCarte(div);
  });
}

/* UNE CARTE NEUVE s'ouvre dans la nature que la route dit pour le mois (`nouvelle_carte` de la route saisie) ; elle
 * s'écrit à sa première case remplie. */
document.getElementById("ajouter-engagement").onclick = () => {
  document.getElementById("engagements").appendChild(carteDEngagement({ nature: NOUVELLE_CARTE.nature }, null));
};

/* LES CONTRATS REPRIS D'UN AUTRE LOGICIEL QUI COURENT ENCORE (`contrats-en-cours`) : chacun se montre avec ce que la route
 * en dit, et « Reprendre » écrit l'engagement qu'elle rend pour lui, case par case. */
async function poserContratsEnCours(mois) {
  let r;
  try { r = await api("contrats-en-cours", { mois }); } catch (e) { dire(e.message, "erreur"); return; }
  if (MOIS_AFFICHE !== mois) return;
  const cible = document.getElementById("engagements");
  r.contrats.filter((c) => !c.deja_repris).forEach((c) => {
    const div = document.createElement("div");
    div.className = "a-reprendre";
    div.innerHTML = `<p><b>${echapper(c.nom)}</b> <span class="discret">— ${echapper(r.reprise || "")}</span></p>
      <p><button class="secondaire">Reprendre</button></p>`;
    div.querySelector("button").onclick = async () => {
      const e = c.engagement;
      const changements = Object.keys(e).filter((k) => k !== "nature").map((k) => ({ case: k, valeur: k === "lignes"
        ? e.lignes.map((l) => (rubriqueDe(l.rubrique) ? ligneDonnee(l, rubriqueDe(l.rubrique)) : l)) : e[k] }));
      const neuve = carteDEngagement({ nature: e.nature }, null);
      div.replaceWith(neuve);
      await changer(neuve, changements);
    };
    cible.appendChild(div);
  });
}
