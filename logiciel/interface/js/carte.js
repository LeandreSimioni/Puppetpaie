/* LA CARTE D'UN ENGAGEMENT, dessinée par le dessinateur seul (`js/avance.js`), depuis ce que les routes rendent.
 *
 * Ce que la carte montre et fait saisir : les cases de sa nature (`natures[].carte` de la route etat), chacune sa forme et
 * son libellé, la valeur que la carte écrit (la saisie du mois, route `saisie` ; puis le brouillon que rend chaque
 * écriture, route `enregistrer-saisie`). Chaque case est à la place que la route dit : obligatoire, en haut ; ouverte par
 * un fait que l'aperçu nomme (le contrat écrit, `contrat.ecrits` ; un contrat au mois, `faits_avances` ; la fin du
 * contrat, `contrat.rupture`), dans sa branche ; les autres sous « Avancé ». Sous les lignes, ce que l'aperçu dit du
 * contrat ; puis sa paie, le montant qu'on vise (route `ajuster`), son état (`etat-par-engagement`) et ses gestes.
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

/* ------------------------------------------------ ce que les cases offrent */

/** CE QUE LES CASES DE LA CARTE QUI SE CHOISISSENT OFFRENT, sous la clé de leur case : `choix` de la route etat (les
 *  fiches, les métiers, les établissements) ; et, tant que la route ne les rend pas sous `choix`, les listes qu'elle rend à
 *  côté, sous un autre nom que leur case (la seule table de la page qui nomme des cases : elle disparaît quand la route
 *  les rend sous `choix`) : les familles des métiers (`emplois`, `familles_emplois`), les motifs de recours et de
 *  rupture d'une nature (`natures[]`, la ligne vide de la fin dite par `contrat.rupture.motif_vide_dit`), la transaction
 *  (`transaction`), les modèles de contrat (`contrat.modeles` de l'aperçu), la nature d'une carte neuve
 *  (`nouvelle_carte` de la route saisie). */
function choixDeLaCarte(div) {
  const n = natureDe(div._carte.nature);
  const contrat = div._paie && div._paie.contrat;
  const rupture = contrat && contrat.rupture;
  const familles = ETAT.familles_emplois;
  const famille = (code) => { const m = ETAT.emplois.find((x) => x.code === code); return m ? m.famille : null; };
  const rang = (c) => { const k = familles.indexOf(famille(c.code)); return k < 0 ? familles.length : k; };
  const choix = {
    ...ETAT.choix,
    metier: ETAT.choix.metier.map((c) => ({ ...c, groupe: famille(c.code) })).sort((a, b) => rang(a) - rang(b)),
    motif_recours: n.motifs_recours.map((m) => ({ code: m.code, libelle: m.libelle })),
    motif_rupture: [...(rupture ? [{ code: "", libelle: rupture.motif_vide_dit }] : []),
      ...n.motifs_rupture.map((m) => ({ code: m.code, libelle: m.libelle, dit: m.dit }))],
    transaction: ETAT.transaction.map((t) => ({ code: t.code, libelle: t.libelle })),
  };
  if (contrat && contrat.modeles && contrat.modeles.length) {
    choix.modele_contrat = contrat.modeles.map((m) => ({ code: m.id == null ? "" : m.id, libelle: m.nom }));
  }
  if (NOUVELLE_CARTE.nature_au_choix) {
    choix.nature = ETAT.natures.filter((x) => NOUVELLE_CARTE.natures.includes(x.cle) || x.cle === div._carte.nature)
      .map((x) => ({ code: x.cle, libelle: x.libelle }));
  }
  return choix;
}

/** Ce que les choix d'une ligne de paie offrent à toutes les lignes de la carte : les rubriques que la carte propose
 *  (`propose` de sa paie dans l'aperçu ; avant, de la carte neuve de sa nature), et celles que ses lignes portent déjà. */
function choixDesLignes(div, lignes) {
  const codes = [...new Set([...proposeDe(div).rubriques, ...lignes.map((l) => l.rubrique).filter(Boolean)])];
  return { rubrique: codes.map((code) => ({ code, libelle: rubriqueDe(code) ? rubriqueDe(code).libelle : code })) };
}

/** Ce que la carte propose de rubriques (`propose` : de sa paie dans l'aperçu ; avant, de la carte neuve de sa nature). */
function proposeDe(div) {
  return (div._paie && div._paie.propose) || NOUVELLE_CARTE.propose[div._carte.nature] || { rubriques: [], par_defaut: null };
}

/** La forme d'une case : celle que la route dit ; une case décrite « texte » dont la route rend la liste se choisit dans
 *  cette liste (la forme est à corriger dans la route). */
function formeDe(d, choix) {
  return d.forme === "texte" && choix[d.cle] ? "choix_rendu" : d.forme;
}

/* ------------------------------------------------ les cases, à leur place */

/** Les cases d'une carte : la nature quand elle se choisit (`nouvelle_carte`), celles de sa nature, puis celles que son
 *  motif de recours porte. */
function casesDeLaCarte(div) {
  const c = div._carte;
  const n = natureDe(c.nature).carte;
  const m = n.motifs_recours.find((x) => x.code === c.motif_recours);
  const nature = NOUVELLE_CARTE.nature_au_choix ? [{ cle: "nature", libelle: "Nature", exige: "obligatoire", forme: "choix_rendu" }] : [];
  return [...nature, ...n.cases, ...(m ? m.cases : [])];
}

/** LES CASES DE LA FIN DU CONTRAT, quand l'aperçu dit qu'elle a un sens ici (`contrat.rupture`) : son motif, ce que le
 *  motif choisi demande (`faits_de_rupture` de sa nature ; sans motif, `faits_sans_motif`), et, quand cette fin se
 *  signale (`signalement_par_motif`, `signalement_fctu`), les cases d'une fin (`cases_de_la_fin` de la fiche de la route
 *  qui les écrit). */
function casesDeLaFin(div, rupture) {
  const n = natureDe(div._carte.nature);
  const motif = div._carte.motif_rupture;
  const signale = motif ? !!rupture.signalement_par_motif[motif] : rupture.signalement_fctu;
  const faits = motif ? n.faits_de_rupture[motif] || [] : n.faits_sans_motif;
  const dela = signale ? CATALOGUE["enregistrer-saisie"].cases_de_la_fin.map((c) => c.nom) : ["motif_rupture"];
  return { cles: [...new Set([...dela, ...faits])], signale };
}

/** LA PLACE DE CHAQUE CASE, d'après ce que les routes disent : les lignes ; une case obligatoire, en haut ; une case
 *  qu'un fait ouvre, dans sa branche (la fin du contrat quand elle est ouverte, le contrat écrit, les faits qu'un contrat
 *  au mois appelle, une case qu'un manque désigne) ; les autres sous « Avancé ». */
function placesDesCases(div) {
  const c = div._carte;
  const p = div._paie;
  const contrat = p && p.contrat;
  const rupture = contrat && contrat.rupture;
  const fin = rupture ? casesDeLaFin(div, rupture) : { cles: [], signale: false };
  const finOuverte = !!rupture && (!!div._finOuverte || fin.cles.some((k) => dite(c[k])));
  const ecrits = contrat ? contrat.ecrits : [];
  const appeles = [...((p && p.faits_avances) || []), ...(div._demandees || [])];
  const places = { base: [], lignes: [], fin: [], contrat: [], faits: [], avance: [] };
  casesDeLaCarte(div).forEach((d) => {
    if (d.forme === "lignes") places.lignes.push(d);
    else if (d.exige === "obligatoire") places.base.push(d);
    else if (finOuverte && fin.cles.includes(d.cle)) places.fin.push(d);
    else if (ecrits.includes(d.cle)) places.contrat.push(d);
    else if (appeles.includes(d.cle)) places.faits.push(d);
    else places.avance.push(d);
  });
  return { places, rupture: finOuverte ? { ...rupture, signale: fin.signale } : null };
}

/** Une case de la carte : son libellé, l'aide que la route etat en donne (`aides`, sous la clé de la case), ce que le
 *  dessinateur en dessine, et ce que la valeur choisie dit d'elle-même (`dit` : un motif de rupture). */
function caseDeLaCarte(div, d, choix) {
  const v = div._carte[d.cle];
  if (d.forme === "lignes") {
    return `<div class="case case-lignes" data-cle="${d.cle}"><span class="case-libelle">${echapper(d.libelle)}</span>
      ${lignesDePaie(v || [], lignesChoixDe(div), { choix: choixDesLignes(div, v || []), mois: MOIS_AFFICHE, salarie: salarieDuBloc(div) || null })}
      <button type="button" class="lien ajouter-ligne">+ une ligne</button>
      <p class="e-montre discret"></p></div>`;
  }
  const aide = ETAT.aides[d.cle];
  const offerts = choix[d.cle] || [];
  const choisi = offerts.find((o) => o.code !== "" && o.code === String(v));
  return `<label class="case" data-cle="${d.cle}"${aide ? ` title="${echapper(aide)}"` : ""}><span class="case-libelle">${echapper(d.libelle)}</span>
    ${caseDeForme(formeDe(d, choix), v, { choix: choix[d.cle] })}${choisi && choisi.dit ? `<small class="case-dit">${echapper(choisi.dit)}</small>` : ""}</label>`;
}

/** Ce que les cases d'une ligne de frais offrent, le moteur le dit (`cases` de sa paie, ligne par ligne) ; sans aperçu,
 *  rien ne s'offre et rien ne se coche. */
function lignesChoixDe(div) {
  return (i) => (div._paie && div._paie.cases
    ? { cases: div._paie.cases.filter((x) => x.ligne === i).map((x) => ({ code: x.case, libelle: x.libelle, aide: x.aide })) } : {});
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

/** DESSINE LA CARTE `div` depuis ce qu'elle sait : sa carte, sa paie, son état. Une carte qui ne se dessine pas dit
 *  pourquoi, garde ses gestes, et ne ferme pas les autres. */
function dessinerCarte(div) {
  try {
    dessinerLaCarte(div);
  } catch (e) {
    div.innerHTML = `<div class="encart erreur">${erreurEnClair(e)}</div><div class="e-imprevus"></div><div class="etats"></div>`;
    try { peindreLesEtats(div); } catch (x) { dire(x.message, "erreur"); }
  }
}

function dessinerLaCarte(div) {
  div.classList.toggle("validee", !!(div._ligne && div._ligne.valide));
  div.classList.toggle("en-modification", !!div._modif);
  if (carteFermee(div)) { dessinerLaLigne(div); return; }
  const choix = choixDeLaCarte(div);
  const { places, rupture } = placesDesCases(div);
  const cases = (liste) => liste.map((d) => caseDeLaCarte(div, d, choix)).join("");
  const branche = (liste) => (liste.length ? `<div class="branche"><div class="e-cases">${cases(liste)}</div></div>` : "");
  // « Avancé » s'ouvre de lui-même quand une de ses cases est remplie : rien de saisi ne se cache.
  const avanceOuvert = div._avance != null ? div._avance : places.avance.some((d) => dite(div._carte[d.cle]));
  div.innerHTML = `
    <div class="e-cases">${cases(places.base)}</div>
    ${cases(places.lignes)}
    ${branche(places.contrat)}
    ${branche(places.faits)}
    ${rupture ? `<div class="branche e-fin">
      <h3>${echapper(rupture.porte)}</h3>
      <p class="discret">${echapper(ETAT.aides[rupture.signale ? "fin_signalee" : "fin_derogatoire"])}</p>
      <div class="e-cases">${cases(places.fin)}</div></div>` : ""}
    ${places.avance.length ? `<details class="e-avance"${avanceOuvert ? " open" : ""}><summary>Avancé</summary>
      <div class="e-cases">${cases(places.avance)}</div></details>` : ""}
    <div class="e-paie"></div>
    <div class="e-viser"></div>
    <div class="e-imprevus"></div>
    <div class="etats"></div>`;
  brancherLesCases(div, { passes: (mois, salarie) => api("contrats-passes", { mois, salarie }) });
  const avance = div.querySelector(".e-avance");
  if (avance) avance.ontoggle = () => { div._avance = avance.open; };
  div.onchange = (ev) => caseChangee(div, ev.target);
  div.querySelectorAll(".ajouter-ligne").forEach((b) => {
    b.onclick = () => {
      const bloc = b.closest(".case-lignes");
      const r = rubriqueDe(proposeDe(div).par_defaut);
      const neuve = r ? ligneDonnee({ rubrique: r.code }, r) : {};
      changer(div, [{ case: bloc.dataset.cle, valeur: [...lireLesLignes(bloc), neuve] }]);
    };
  });
  div.querySelectorAll(".retirer-ligne").forEach((b) => {
    b.onclick = () => {
      const bloc = b.closest(".case-lignes");
      const lignes = lireLesLignes(bloc).filter((_, i) => i !== +b.closest(".ligne-paie").dataset.ligne);
      changer(div, [{ case: bloc.dataset.cle, valeur: lignes }]);
    };
  });
  dessinerLeVise(div);
  peindreLaPaie(div);
  peindreLesEtats(div);
  peindreImprevusDesCartes();
}

/** Une case a changé : sa valeur, lue par sa forme, part à la route. Une case des lignes : toutes les lignes, telles que
 *  la route les lit. Une case qui n'est pas de la carte (le montant visé) n'écrit rien. */
function caseChangee(div, x) {
  const bloc = x.closest(".case-lignes");
  if (bloc) { changer(div, [{ case: bloc.dataset.cle, valeur: lireLesLignes(bloc) }]); return; }
  const c = x.closest(".case[data-cle]");
  if (!c) return;
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
  // Un engagement auquel il manque un fait s'enregistre aussi : sa carte dit ce qui manque, et la ligne d'état le rappelle.
  noteSaisie(r.enregistre_dit + (r.a_completer ? " — " + ETAT.aides.a_completer : ""));
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

/** Les chevrons d'un contrat sur plusieurs mois (`contrat.chevrons` de l'aperçu, le côté où il déborde du mois, et ce
 *  qu'il en dit) : un clic mène au mois voisin. */
function chevronsDe(p) {
  const ch = p && p.contrat && p.contrat.chevrons;
  if (!ch) return "";
  const un = (sens, pas, signe) => (ch[sens] ? `<button type="button" class="chevron-pluri ${sens}" data-pas="${pas}"
    title="${echapper(ch[sens])}" aria-label="${echapper(ch[sens])}">${signe}</button>` : "");
  return un("avant", -1, "&#x2039;") + un("apres", 1, "&#x203A;");
}

/** Pose les chevrons de la paie de `div` à son bord. */
function poserLesChevrons(div) {
  div.querySelectorAll(":scope > .chevron-pluri").forEach((c) => c.remove());
  div.insertAdjacentHTML("beforeend", chevronsDe(div._paie));
  div.querySelectorAll(":scope > .chevron-pluri").forEach((b) => {
    b.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); moisVoisin(+b.dataset.pas); };
  });
}

/** Sous les lignes : ce que l'aperçu a compris du contrat (`contrat_dit`), ses jours travaillés (`jours`), et, pour un
 *  mois sans paie, ce qu'il en dit (`contrat.sans_paie`). */
function peindreLeContrat(div) {
  const cible = div.querySelector(".e-montre");
  if (!cible) return;
  const p = div._paie;
  if (!p || p.a_completer) { cible.innerHTML = ""; return; }
  cible.innerHTML = `<span${p.numero_contrat ? ` title="${echapper(ETAT.aides.numero_contrat)}"` : ""}>${echapper(p.contrat_dit)}</span>
    · <b>${echapper(String(p.jours))}</b> jours travaillés${p.contrat.sans_paie ? ` · ${aideDe("sans_cachet")}` : ""}`;
}

/** Sous les cases : ce qui manque à l'engagement, ou ce que le moteur a compris de lui, son net, son brut et son coût, et
 *  ce qu'il en dit (`apercu`, la paie de son identité). */
function peindreLaPaie(div) {
  poserLesChevrons(div);
  peindreLeContrat(div);
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
      ${p.net_paye != null ? `<span class="e-cout">Net payé <b>${echapper(euros(p.net_paye))}</b></span>` : ""}
      <span class="e-cout">Brut <b>${echapper(euros(p.brut))}</b></span>
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
    // Ce que la carte propose, ce qui ouvre ses branches et ce qu'elle offre de choisir viennent de sa paie : la carte se
    // redessine quand ils changent, sinon seule sa paie se repeint (une case qu'on tape ne se redessine pas sous les
    // doigts).
    const forme = (x) => JSON.stringify(x ? [x.propose, x.contrat, x.cases, x.faits_avances, !x.a_completer] : null);
    try {
      if (forme(avant) !== forme(div._paie) && !div.contains(document.activeElement) && !CALENDRIER_OUVERT) dessinerCarte(div);
      else if (carteFermee(div)) dessinerLaLigne(div);
      else peindreLaPaie(div);
    } catch (e) {
      dessinerCarte(div);
    }
  });
}

/* ------------------------------------------------ viser un montant */

/** Ce que la carte dit d'elle-même à la route du montant visé : le mois, les modifications pas encore écrites, son
 *  identité. Les autres champs de sa fiche se saisissent. */
function contexteDuVise(div) {
  return { mois: MOIS_AFFICHE, modifications: modificationsAuMoteur(), engagement: JSON.parse(div.dataset.engagement) };
}

/** VISER UN MONTANT (`ajuster`) : une branche repliée, que sa fiche nomme (`geste`) ; ses cases sont les champs de sa
 *  fiche que la carte ne dit pas ; la route rend ce qui est atteint et les taux qu'il faudrait, que la carte écrit. Une
 *  carte qui n'a pas encore d'identité ou de paie calculée n'a rien à viser. */
function dessinerLeVise(div) {
  const cible = div.querySelector(".e-viser");
  const f = CATALOGUE.ajuster;
  if (!cible || !f || !div.dataset.engagement || !div._paie || div._paie.a_completer) { if (cible) cible.innerHTML = ""; return; }
  const vise = div._vise || (div._vise = { ouvert: false, valeurs: {}, phrase: null, erreur: false });
  const champs = f.charge.filter((c) => !(c.nom in contexteDuVise(div)));
  cible.innerHTML = `<details class="e-viser-branche"${vise.ouvert ? " open" : ""}><summary>${echapper(f.geste)}</summary>
    <div class="branche">
      <div class="e-cases">${casesDesChamps(champs, vise.valeurs, {})}</div>
      <p><button type="button" class="secondaire v-viser">Calculer</button></p>
      ${vise.phrase ? `<p class="${vise.erreur ? "erreur" : "discret"}">${echapper(vise.phrase)}</p>` : ""}
    </div></details>`;
  const d = cible.querySelector("details");
  d.ontoggle = () => { vise.ouvert = d.open; };
  brancherLesCases(cible);
  // Ses cases ne sont pas celles de la carte : leur changement n'écrit rien.
  cible.onchange = (ev) => ev.stopPropagation();
  cible.querySelector(".v-viser").onclick = () => viserLeMontant(div, champs);
}

async function viserLeMontant(div, champs) {
  const vise = div._vise;
  vise.valeurs = lireLesChamps(div.querySelector(".e-viser"), champs);
  let r;
  try {
    r = await aSonTour(() => api("ajuster", { ...contexteDuVise(div), ...vise.valeurs }));
  } catch (e) {
    vise.phrase = e.message;
    vise.erreur = true;
    dessinerLeVise(div);
    return;
  }
  vise.phrase = r.phrase;
  vise.erreur = false;
  // Les taux trouvés s'écrivent sur leurs lignes (`rang`), et le salaire du mois s'il change : comme si on les tapait.
  const e = r.engagements[0];
  const changements = [];
  if (e.lignes.length) {
    const cle = caseDesLignes(div).cle;
    changements.push({ case: cle, valeur: (div._carte[cle] || []).map((l, i) => {
      const t = e.lignes.find((x) => x.rang === i);
      return t ? { ...l, taux: t.taux } : l;
    }) });
  }
  if ("salaire_mensuel" in e) changements.push({ case: "salaire_mensuel", valeur: e.salaire_mensuel });
  if (changements.length) await changer(div, changements);
  else dessinerLeVise(div);
}

/* ------------------------------------------------ l'état de l'engagement et ses gestes */

/** LA LIGNE D'UN ENGAGEMENT VALIDÉ : quand (ses jours travaillés du mois, `dates_travail`), qui et pour quoi (son
 *  spectacle, ce qu'il paie, `paye_dit`, un contrat repris), au survol son bulletin, son contrat et sa validation, ses
 *  trois carrés, son net payé. Un clic l'ouvre, toutes ses cases modifiables : rien ne s'écrit avant « Modifier
 *  l'engagement ». */
function dessinerLaLigne(div) {
  const ligne = div._ligne, p = div._paie;
  const dates = p && p.dates_travail ? p.dates_travail.filter((d) => d.startsWith(MOIS_AFFICHE)) : [];
  const jours = joursEnClair(dates);
  const moisCourt = MOIS_COURTS[parseInt(MOIS_AFFICHE.slice(5, 7), 10) - 1];
  const pourquoi = [pourQuoi(ligne), p && p.paye_dit, ligne.repris_dit].filter(Boolean).join(" · ");
  const detail = [p && p.bulletin_dit, ligne.numero_dit, ligne.validee_dit].filter(Boolean).join(" · ");
  div.innerHTML = `
    <div class="e-resume">
      <div class="carte-quand" title="${echapper(dates.map(jjmmaaaa).join(", "))}">
        <b class="${jours.includes(" · ") ? "epars" : ""}">${echapper(jours)}</b><span>${echapper(moisCourt)}</span></div>
      <div class="carte-qui" title="${echapper(detail)}"><span class="carte-nom"><b>${echapper(ligne.nom)}</b></span>
        <span class="discret">${echapper(pourquoi)}</span></div>
      <div class="carte-actes">${bandeauEtats(ligne, false)}</div>
      <div class="carte-net" title="net payé">${p ? echapper(netPaye(p.net_paye)) : ""}</div>
      ${recalculDe(ligne)}${versionsDe(ligne)}
      <span class="carte-modifier"><button class="lien carte-imprevu" title="Imprévu" aria-label="Un imprévu">&hellip;</button></span>
    </div>
    <div class="e-imprevus"></div>`;
  poserLesChevrons(div);
  const resume = div.querySelector(".e-resume");
  rendreOuvrable(resume, () => ouvrirEnModification(div), "Modifier l'engagement");
  const porte = div.querySelector(".carte-imprevu");
  porte.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); menuImprevu(porte, { div }); };
  const frise = div.querySelector(".carte-versions");
  if (frise) frise.onclick = (ev) => { ev.preventDefault(); ev.stopPropagation(); boiteFrise(ligne); };
  peindreImprevusDesCartes();
}

/** Sous une carte ouverte : son état (`etat-par-engagement`) et ses gestes ; validée, quand (`valide_dit`), ses pièces et
 *  ses versions. */
function peindreLesEtats(div) {
  const cible = div.querySelector(".etats");
  if (!cible) return;
  const ligne = div._ligne;
  const imprevu = salarieDuBloc(div) ? `<button type="button" class="lien e-imprevu-btn">Imprévu…</button>` : "";
  const retirer = `<button class="lien retirer-engagement">Retirer cet engagement</button>`;
  const gestes = div._modif
    ? `${retirer}<button class="secondaire v-annuler">Annuler</button><button class="primaire v-modifier">Modifier l'engagement</button>`
    : `${retirer}<button class="primaire v-valider">Valider l'engagement</button>`;
  const valide = !!(ligne && ligne.valide);
  const pieces = valide ? ligne.carres.filter((c) => c.lien).map((c) =>
    `<button class="lien v-pieces" data-acte="${echapper(c.cle)}">${echapper(c.lien)}</button>`).join(" · ") : "";
  const fait = valide ? [`<b>Engagement validé</b> ${ligne.valide_dit ? echapper(ligne.valide_dit) : ""}`, pieces, versionsDe(ligne)]
    .filter(Boolean).join(" · ") : "";
  cible.innerHTML = `${ligne ? quiEtPourQuoi(ligne) + bandeauEtats(ligne) : ""}
    <div class="validation${valide ? " faite" : ""}">${fait ? `<span>${fait}</span>` : ""}<span class="boutons">${imprevu}${gestes}</span></div>`;
  brancherRecu(div);
  const sur = (s, f) => { const b = cible.querySelector(s); if (b) b.onclick = (ev) => { ev.preventDefault(); f(ev); }; };
  sur(".v-valider", () => validerLaCarte(div));
  sur(".v-modifier", () => modifierLaCarte(div));
  sur(".v-annuler", () => chargerMois(MOIS_AFFICHE, true));
  sur(".retirer-engagement", () => retirerLaCarte(div));
  sur(".e-imprevu-btn", (ev) => { ev.stopPropagation(); menuImprevu(ev.currentTarget, { div }); });
  sur(".carte-versions", (ev) => { ev.stopPropagation(); boiteFrise(ligne); });
  cible.querySelectorAll(".v-pieces").forEach((b) => { b.onclick = () => boitePieces(ligne, b.dataset.acte); });
}

/** Les états des engagements lus (`ETATS_ENGAGEMENTS`), posés sur leurs cartes : une carte qui change d'état (validée,
 *  ou plus) se redessine, une autre ne repeint que son état. */
function peindreEtats() {
  document.querySelectorAll("#engagements .engagement").forEach((div) => {
    const ligne = ETATS_ENGAGEMENTS && div.dataset.engagement ? ETATS_ENGAGEMENTS.get(div.dataset.engagement) : undefined;
    const avant = !!(div._ligne && div._ligne.valide);
    div._ligne = ligne;
    try {
      if (avant !== !!(ligne && ligne.valide)) dessinerCarte(div);
      else if (carteFermee(div)) dessinerLaLigne(div);
      else peindreLesEtats(div);
    } catch (e) {
      dessinerCarte(div);
    }
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

/* ------------------------------------------------ ce que les imprévus font sur une carte */

/** LA FIN D'UN CONTRAT, OUVERTE DEPUIS UN IMPRÉVU (la porte de la rupture, ou la case d'un manque de fin) : la carte
 *  s'ouvre (rouverte à la modification si elle est validée), sa branche de fin aussi, et la case `champ` (sinon la
 *  première de la fin) vient sous le curseur. */
function ouvrirLaFin(div, champ) {
  if (carteFermee(div)) ouvrirEnModification(div);
  div._finOuverte = true;
  if (champ) div._demandees = [champ];
  dessinerCarte(div);
  const cas = champ ? div.querySelector(`.case[data-cle="${champ}"]`) : div.querySelector(".e-fin .case");
  if (!cas) return false;
  const repli = cas.closest("details");
  if (repli) repli.open = true;
  const x = cas.querySelector("[data-case]");
  cas.scrollIntoView({ block: "center" });
  if (x && x.focus) x.focus();
  return true;
}

/** RETIRER DES JOURS PAYÉS PENDANT UN IMPRÉVU (`jours` : `{rubrique, iso}`, ce que la route `imprevus` nomme) : chaque
 *  jour quitte la ligne de sa rubrique qui le porte, et les lignes partent à la route, comme décochées au calendrier.
 *  Rend le nombre de jours retirés : aucun quand la carte est validée et fermée, ou qu'aucune ligne ne les porte. */
async function retirerDesJours(div, jours) {
  if (carteFermee(div)) return 0;
  const lignes = (div._carte[caseDesLignes(div).cle] || []).map((l) => ({ ...l }));
  let retires = 0;
  // Les clés de jours d'une ligne : celles que sa rubrique dit de forme `jours`.
  const clesDeJours = (l) => (rubriqueDe(l.rubrique) ? rubriqueDe(l.rubrique).cles.filter((c) => c.forme === "jours").map((c) => c.cle) : []);
  jours.forEach(({ rubrique, iso }) => {
    for (const l of lignes.filter((x) => x.rubrique === rubrique)) {
      const k = clesDeJours(l).find((c) => (l[c] || []).some((d) => jourDe(d) === iso));
      if (!k) continue;
      l[k] = l[k].filter((d) => jourDe(d) !== iso);
      retires += 1;
      return;
    }
  });
  if (retires) await changer(div, [{ case: caseDesLignes(div).cle, valeur: lignes }]);
  return retires;
}

/** La case des lignes de paie de la carte : celle que sa nature décrit de forme `lignes`. */
function caseDesLignes(div) {
  return casesDeLaCarte(div).find((d) => d.forme === "lignes");
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

/** Le numéro du contrat d'une carte : celui que l'aperçu rend (`numero_contrat`), sinon celui qu'elle écrit. */
function numeroDuBloc(div) {
  return (div._paie && div._paie.numero_contrat) || div._carte.numero || undefined;
}

/** La fin du contrat d'une carte, quand l'aperçu dit qu'elle a un sens ici (`contrat.rupture`) ; aucune sinon. */
function ruptureDuBloc(div) {
  return (div._paie && div._paie.contrat && div._paie.contrat.rupture) || null;
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
      // Ce que la route rend s'écrit case par case : chaque clé qui est une case de sa nature (la nature, elle, est celle
      // de la carte neuve) ; ses lignes, telles que la route les lit.
      const e = c.engagement;
      const neuve = carteDEngagement({ nature: e.nature }, null);
      const cases = casesDeLaCarte(neuve);
      const changements = cases.filter((d) => d.cle in e).map((d) => ({ case: d.cle, valeur: d.forme === "lignes"
        ? e[d.cle].map((l) => (rubriqueDe(l.rubrique) ? ligneDonnee(l, rubriqueDe(l.rubrique)) : l)) : e[d.cle] }));
      div.replaceWith(neuve);
      await changer(neuve, changements);
    };
    cible.appendChild(div);
  });
}
