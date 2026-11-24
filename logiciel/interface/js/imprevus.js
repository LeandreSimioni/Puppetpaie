/* Les imprévus : un arrêt de travail, une absence, ce qu'ils font naître, et la fin d'un contrat qui se signale. */
"use strict";

/* ------------------------------------------------ LES IMPRÉVUS
   Simple pour qui a besoin de simple, compliqué pour qui a besoin de compliqué.
   Quelqu'un qui fait des paies
   apprend qu'un salarié est malade : il va sur la ligne de ce salarié, pas
   dans une section du mois qui dirait « Arrêts de travail » tous les mois
   pour rien. Les imprévus sont rares, et rien ici ne doit laisser croire
   qu'il y en a tout le temps.

   LA FORME, EN TROIS ENDROITS, ET PAS UN DE PLUS :

   - LA PORTE est sur la carte de l'engagement : « Imprévu… » (et « … »
     sur un engagement validé). Elle propose un arrêt de travail, une absence
     (congé parental, sans solde, sabbatique… : une suspension du contrat,
     bloc 65), et, quand la carte sait déjà dire une fin, la rupture avant
     terme — qui n'est pas une boîte de plus : elle ouvre la branche de la
     fin, sur la carte (`ouvrirLaFin`, js/carte.js). Le salarié et le contrat
     sont ceux de la carte : on ne les redemande pas. Sous la carte, une ligne
     par imprévu qui la touche ce mois-ci, avec ses gestes (la reprise, les
     indemnités journalières, retirer) et les jours payés pendant l'imprévu, à
     retirer un par un.
   - LA COLONNE DU MOIS ne garde que CE QUI EST DÛ : une ligne par
     obligation à faire ou en retard, sa date butoir, son état, et le bouton
     qui fait l'acte. Vide quand il n'y a rien, titre compris. La phrase et
     les textes d'une obligation ne se lisent qu'à la demande (« pourquoi »).
   - LE DOSSIER DU SALARIÉ (Configuration › Salariés) garde l'histoire : ses
     imprévus passés et en cours, ce qui en est parti, et la même porte.

   LE VOCABULAIRE, tenu partout : un IMPRÉVU (le mot de la porte), un ARRÊT DE
   TRAVAIL, une ABSENCE (ce que la DSN appelle suspension), la REPRISE, le
   MAINTIEN de salaire, CE QUI EST DÛ. Les congés payés d'un permanent ne
   sont pas des imprévus : ils restent dans la saisie du mois.

   RIEN NE SE DÉCIDE ICI (règle 2) : le moteur dit ce que chaque fait ouvre
   (`imprevus` : ses `gestes`, `a_dire`, `cachets`, `obligations` ; la boîte
   d'un imprévu pas encore écrit, `arret-obligations` et `suspension-saisie`),
   et la page ne montre un bouton ou un champ que parce qu'il le dit. */

let IMPREVUS = null;        // la dernière lecture `imprevus` du mois affiché
let FINS = null;            // la dernière lecture `fins-de-contrat` du mois affiché
let NOMENCLATURES_IMPREVUS = null;   // la porte (`porte`) : celle de la dernière lecture

/** Le nom d'un salarié, tel que la saisie le montre ; sa clé à défaut. */
function nomDuSalarie(cle) {
  // La saisie écrit parfois le nom tel qu'on l'a tapé (« Audinot ») là où la fiche a sa clé (« AUDINOT ») : même personne.
  const s = ETAT.salaries.find((x) => String(x.cle).toUpperCase() === String(cle).toUpperCase());
  return s ? s.libelle : String(cle);
}

/* ------------------------------------------------------ lire, et peindre */

/** Les imprévus et les fins du mois, lus ensemble : la colonne et les cartes en dépendent. */
async function dessinerImprevus(mois) {
  if (!mois) return;
  let imprevus, fins;
  // Une lecture refusée se dit ; ce qu'elle aurait montré ne se montre pas.
  try { imprevus = await api("imprevus", { mois }); }
  catch (e) { imprevus = null; dire(e.message, "erreur"); }
  try { fins = await api("fins-de-contrat", { mois }); }
  catch (e) { fins = null; dire(e.message, "erreur"); }
  if (MOIS_AFFICHE && MOIS_AFFICHE !== mois) return;      // on a changé de mois entre-temps
  IMPREVUS = imprevus;
  FINS = fins;
  if (imprevus) NOMENCLATURES_IMPREVUS = imprevus;
  peindreDus();
  peindreImprevusDesCartes();
}

/** Après une écriture : relire, et refaire l'aperçu — un imprévu change la paie (la ligne retirée, le maintien, les avis). */
async function rafraichirImprevus() {
  await dessinerImprevus(MOIS_AFFICHE);
  if (MOIS_AFFICHE) calculerApercu();
}

/** La porte (`porte`) : celle de la dernière lecture, sinon une lecture. */
async function nomenclaturesImprevus() {
  if (NOMENCLATURES_IMPREVUS) return NOMENCLATURES_IMPREVUS;
  NOMENCLATURES_IMPREVUS = await api("imprevus", MOIS_AFFICHE ? { mois: MOIS_AFFICHE } : {});
  return NOMENCLATURES_IMPREVUS;
}

/* ------------------------------------------- la colonne : ce qui est dû
   Une ligne par obligation due. Un arrêt en fait naître une ou plusieurs
   (le signalement, la déclaration d'accident, le signalement de reprise) ;
   une fin de contrat qui se signale, une. Ce qui est fait, ou pas encore dû,
   ne paraît pas : un bloc qui dirait « rien à faire » tous les mois finirait
   par ne plus se lire. Les deux routes disent ce qui est dû (`dus`), chacune
   dans son rang (`rang`, un texte qui se range : une carte à corriger, puis le
   plus pressé) ; la page les met ensemble dans ce rang. Chaque chose due porte
   ses gestes (signaler, la dire faite, le jour où on l'a su, les papiers de
   fin) : la route les rend, le dessinateur les dessine. */
function peindreDus() {
  const cible = document.getElementById("dus");
  if (!cible) return;
  const dus = [...(IMPREVUS ? IMPREVUS.dus : []), ...(FINS ? FINS.dus : [])];
  if (!dus.length) { cible.innerHTML = ""; return; }
  dus.sort((x, y) => x.rang.localeCompare(y.rang));
  const titre = (IMPREVUS && IMPREVUS.dus_titre) || FINS.dus_titre;
  // Trois formes : une obligation d'un arrêt (`obligation`), une fin lue, une carte à corriger (`lecture`).
  cible.innerHTML = `<h2>${echapper(titre)}</h2>
    ${dus.map((x, k) => (x.lecture === "a_corriger" ? rangACorriger(x, k) : x.lecture ? rangFin(x, k) : rangObligation(x, k))).join("")}`;
  dus.forEach((x, k) => {
    const r = cible.querySelector(`[data-du="${k}"]`);
    if (x.lecture === "a_corriger") return;
    const titreDeLaBoite = x.lecture ? nomDuSalarie(x.salarie) : x.qui;
    brancherLesGestes(r.querySelector(".du-gestes"), x.gestes, { titre: titreDeLaBoite }, async (rendu) => {
      await rafraichirImprevus();
      // Des papiers écrits se remettent en main propre : leur dossier s'ouvre (`dossier`, que la route rend).
      if (rendu && rendu.dossier) await ouvrirEtDire(rendu.dossier, rendu.phrase);
    });
    if (x.lecture) brancherFin(r, x);
  });
}

/** L'état d'une obligation : la pastille et son mot. */
function pastilleDu(o) {
  return pastilleDeLEtat(o && o.pastille);
}

/** Le « pourquoi » d'une obligation, replié : la phrase du moteur, le délai et son point de départ, les textes. */
function pourquoiDu(o) {
  if (!o) return "";
  const presume = o.depart && o.depart.presume;
  return `<details class="pourquoi"><summary>pourquoi</summary>
      <p>${echapper(o.phrase)}</p>
      ${o.delai_dit ? `<p>${echapper(o.delai_dit)}</p>` : ""}
      ${presume ? `<p class="discret"><b>Point de départ présumé</b> ${echapper(presume)}</p>` : ""}
      ${o.destinataire ? `<p class="discret">${echapper(o.destinataire)}</p>` : ""}
      ${o.textes.length ? `<ul class="discret">${o.textes.map((t) =>
        `<li>${echapper(t.reference)}${t.dit ? ` — ${echapper(t.dit)}` : ""}</li>`).join("")}</ul>` : ""}
    </details>`;
}

/** Une obligation due d'un arrêt : pour qui, et ses gestes. */
function rangObligation(d, k) {
  const o = d.obligation;
  return rangDu(k, pastilleDu(o), o.libelle, d.qui, o.butoir_dit, boutonsDesGestes(d.gestes), "", pourquoiDu(o));
}

/** Une ligne de ce qui est dû : la pastille, ce qu'il faut faire et pour qui, la date butoir et les gestes ; dessous,
 *  ce qui manque, et le « pourquoi » replié. Pour qui et la date butoir, quand le moteur les dit. */
function rangDu(k, pastille, quoi, qui, butoir, gestes, manque, pourquoi) {
  return `<div class="du" data-du="${k}">
      ${pastille}
      <div class="du-corps">
        <div class="du-quoi"><b>${echapper(quoi)}</b> <span class="discret">${qui ? echapper(qui) : ""}</span></div>
        <div class="du-bas"><span class="du-butoir">${butoir ? echapper(butoir) : ""}</span><span class="du-gestes">${gestes}</span></div>
        ${manque}${pourquoi}
      </div>
    </div>`;
}

/* LA FIN D'UN CONTRAT QUI SE SIGNALE (nature 07), sous la même
   forme. C'est le moteur qui tranche si elle se signale
   (`Nature::signalement_fctu`, route `fins-de-contrat`), et qui ne rend le
   geste qui la signale que lorsque rien ne lui manque. Ce qui manque se tape
   dans la carte, dans sa fin : le moteur dit chaque manque par le nom de sa
   case et ce qu'on y met (`a_saisir`, `avant_signaler`), et « La dire »
   (`la_dire`) mène à la case. Une fin d'un mois passé dont le signalement
   est en retard paraît aussi (`mois`) : une échéance manquée ne se tait pas. */
function rangFin(f, k) {
  return rangDu(k, pastilleDeLEtat(f.pastille), f.obligation.libelle,
    f.qui_dit, f.obligation.butoir_dit, boutonsDesGestes(f.gestes),
    f.avant_signaler ? `<div class="du-manque"><p>${echapper(f.avant_signaler)}</p>
      ${f.a_saisir.map((m, i) => `<p><b>${echapper(m.case)}</b> : ${echapper(m.dire)}
        <button class="${classeDuGeste(f.la_dire)}" data-case="${i}">${echapper(f.la_dire.libelle)}</button></p>`).join("")}</div>` : "",
    pourquoiDu(f.obligation));
}

/** Une fin sur une carte qui ne se lit plus : ce qu'il faut corriger, et où (`qui_dit`) ; aucun geste ne s'y propose. */
function rangACorriger(f, k) {
  const m = f.a_corriger;
  return rangDu(k, pastilleDeLEtat(f.pastille), f.qui_dit, "", "", "",
    `<div class="du-manque"><p><b>${echapper(m.case)}</b> : ${echapper(m.dire)}</p>
      <p class="discret">${echapper(m.pourquoi)}</p></div>`, "");
}

/** « La dire » : la case du manque, dans la carte (le confort de la main). */
function brancherFin(r, f) {
  r.querySelectorAll("[data-case]").forEach((b) => {
    b.onclick = () => allerALaCase(f, f.a_saisir[Number(b.dataset.case)].champ);
  });
}

/* LA CASE D'UN MANQUE, dans la carte de l'engagement : le mois de la fin,
   la carte du salarié, rouverte à la modification si elle est validée (le
   chemin de toujours, « Modifier l'engagement… »), sa fin ouverte, la case
   que le manque nomme (`champ`, la clé de sa case) sous le curseur. */

/** Attend que `test()` rende quelque chose (au plus `ms`) ; rend ce qu'il rend, ou null. */
async function attendre(test, ms) {
  for (let t = 0; t < ms; t += 200) {
    const v = test();
    if (v) return v;
    await new Promise((r) => setTimeout(r, 200));
  }
  return null;
}

/** La carte (`salarie` : son salarié, absent quand aucun n'est choisi) est-elle à ce salarié ? */
const memeSalarie = (salarie, autre) => !!salarie && salarie.toUpperCase() === String(autre).toUpperCase();

async function allerALaCase(f, champ) {
  if (f.mois && f.mois !== MOIS_AFFICHE) allerAuMois(f.mois);
  const carte = () => {
    if (f.mois && (MOIS_AFFICHE !== f.mois || document.getElementById("engagements").classList.contains("en-lecture"))) return null;
    const siennes = [...document.querySelectorAll("#engagements .engagement")]
      .filter((d) => memeSalarie(salarieDuBloc(d), f.salarie));
    return siennes.find((d) => carteDe(d).numero === f.numero || carteDe(d).debut === f.contrat.du) || siennes[0] || null;
  };
  const div = await attendre(carte, 15000);
  if (!div) return;
  // La carte ne montre pas cette case (sa nature ne l'a pas) : la phrase du manque reste à l'écran, qui dit quoi faire.
  const m = f.a_saisir.find((x) => x.champ === champ);
  if (!ouvrirLaFin(div, champ) && m) dire(`${m.case} : ${m.dire}`, "erreur");
}

/* --------------------------------------------------------- LA PORTE
   Sur la carte d'un engagement, quand elle porte UN salarié (plusieurs
   cochés, c'est une saisie en cours : on ne sait pas encore de qui on
   parle). « Imprévu… » au pied d'une carte ouverte, « … » sur un engagement
   validé. */
function majPorteImprevu(div) {
  const b = div.querySelector(".e-imprevu-btn");
  if (b) b.hidden = !salarieDuBloc(div);
}

/** Ce que la carte sait de son contrat : le salarié, son numéro (celui que l'aperçu rend, ou qu'elle écrit), le début. */
function carteDe(div) {
  return { div, salarie: salarieDuBloc(div), numero: numeroDuBloc(div), debut: debutDuBloc(div) };
}

/** La rupture se propose quand le moteur dit qu'elle a un sens pour cet engagement (`contrat.rupture` de l'aperçu) ;
 *  elle se saisit dans une carte ouverte. */
function ruptureOuverte(div) {
  if (carteFermee(div) || div.classList.contains("validee")) return null;    // une carte fermée, un engagement validé
  const r = ruptureDuBloc(div);
  return r ? r.porte : null;
}

let MENU_IMPREVU = null;

function fermerMenuImprevu() {
  if (MENU_IMPREVU) { MENU_IMPREVU.remove(); MENU_IMPREVU = null; }
}

/** Le petit menu de la porte, ancré sous le bouton : ses choix sont ceux que `imprevus` rend (`porte`), chacun la
 *  lecture qui rend sa boîte (son `code`, `boiteDUnImprevu`). `ou` : la carte, ou `{salarie}` depuis le dossier du
 *  salarié ; sur la ligne d'un engagement, `copier` : le geste que sa ligne rend (`copierLEngagement`). */
async function menuImprevu(ancre, ou) {
  fermerMenuImprevu();
  let n;
  try { n = await nomenclaturesImprevus(); } catch (e) { dire(e.message, "erreur"); return; }
  const rupture = ou.div ? ruptureOuverte(ou.div) : null;
  const menu = document.createElement("div");
  menu.className = "menu-imprevu";
  menu.setAttribute("role", "menu");
  menu.innerHTML = `
    ${n.porte.map((c) => `<button type="button" data-choix="${echapper(c.code)}">${echapper(c.libelle)}
      <span class="discret">${echapper(c.dit)}</span></button>`).join("")}
    ${rupture ? `<button type="button" data-choix="rupture">${echapper(rupture)}</button>` : ""}
    ${ou.copier ? `<button type="button" data-choix="copier">${echapper(ou.copier.libelle)}</button>` : ""}`;
  document.body.appendChild(menu);
  const r = ancre.getBoundingClientRect();
  menu.style.top = `${window.scrollY + r.bottom + 4}px`;
  menu.style.left = `${Math.max(8, window.scrollX + r.right - menu.offsetWidth)}px`;
  MENU_IMPREVU = menu;
  n.porte.forEach((c) => {
    menu.querySelector(`[data-choix="${c.code}"]`).onclick = () => { fermerMenuImprevu(); boiteDUnImprevu(c.code, ou); };
  });
  const r2 = menu.querySelector('[data-choix="rupture"]');
  if (r2) r2.onclick = () => { fermerMenuImprevu(); ouvrirLaFin(ou.div); };
  const r3 = menu.querySelector('[data-choix="copier"]');
  if (r3) r3.onclick = () => { fermerMenuImprevu(); copierLEngagement(ou.copier); };
  setTimeout(() => {
    const dehors = (ev) => {
      if (MENU_IMPREVU && !MENU_IMPREVU.contains(ev.target)) {
        fermerMenuImprevu();
        document.removeEventListener("click", dehors);
      }
    };
    document.addEventListener("click", dehors);
  }, 0);
  menu.onkeydown = (ev) => { if (ev.key === "Escape") fermerMenuImprevu(); };
  menu.querySelector("button").focus();
}

/* -------------------------------------------- sous la carte : ses imprévus
   Une ligne par imprévu qui touche CE contrat ce mois-ci : ce que c'est, sa
   période, ses gestes. Et les jours que la saisie paie pendant l'imprévu —
   le moteur les nomme (`cachets`), la page propose de les retirer un par
   un, et c'est la ligne de paie de la carte qui change, comme si on avait
   décoché le jour au calendrier. Rien ne se retire seul. */
function peindreImprevusDesCartes() {
  const cartes = [...document.querySelectorAll("#engagements .engagement")];
  const parCarte = new Map(cartes.map((d) => [d, []]));
  if (IMPREVUS) IMPREVUS.imprevus.forEach((i) => {
    const siennes = cartes.filter((d) => memeSalarie(salarieDuBloc(d), i.salarie));
    const c = siennes.find((d) => {
      const x = carteDe(d);
      return (i.contrat && x.numero === i.contrat) || (i.contrat_debut && x.debut === i.contrat_debut);
    }) || siennes[0];
    if (c) parCarte.get(c).push(i);
  });
  cartes.forEach((d) => peindreImprevusDeLaCarte(d, parCarte.get(d)));
}

function peindreImprevusDeLaCarte(div, liste) {
  const cible = div.querySelector(".e-imprevus");
  if (!cible) return;
  if (!liste.length) { cible.innerHTML = ""; return; }
  const validee = div.classList.contains("validee");
  cible.innerHTML = liste.map((i, k) => {
    // Les jours payés de ce mois : la route ne rend qu'eux (`imprevus`, un mois).
    const jours = i.cachets;
    return `<div class="imprevu" data-imprevu="${k}">
        <span class="imprevu-quoi"><b>${echapper(i.quoi)}</b>
          <span class="discret">${echapper(i.periode)}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i)}</span>
        ${jours.length ? `<div class="imprevu-jours">${echapper(i.paye_pendant)}
          ${jours.map((c) => c.jours.map((j) => `${echapper(j.phrase)}
            ${validee ? "" : `<button class="lien" data-retirer="${echapper(j.iso)}" data-rubrique="${echapper(c.rubrique)}">retirer ce jour</button>`}`).join(" · ")).join(" · ")}
          ${validee ? `<span class="discret">— ${aideDe("imprevu_valide")}</span>` : ""}
        </div>` : ""}
      </div>`;
  }).join("");
  liste.forEach((i, k) => {
    const r = cible.querySelector(`[data-imprevu="${k}"]`);
    brancherGestesImprevu(r, i, rafraichirImprevus);
    // Le jour quitte la ligne de la carte qui le porte (`retirerDesJours`, js/carte.js), qui s'écrit.
    r.querySelectorAll("[data-retirer]").forEach((b) => {
      b.onclick = async () => {
        if (!await retirerDesJours(div, [{ rubrique: b.dataset.rubrique, iso: b.dataset.retirer }])) {
          dire(ETAT.aides.jour_hors_ligne, "erreur");
        }
      };
    });
  });
}

/** Les gestes d'un imprévu, les mêmes sur la carte et dans le dossier du salarié : ceux que le moteur rend (`gestes`),
 *  dessinés et branchés depuis la fiche de leur route (`js/avance.js`). */
function gestesImprevu(i) {
  return boutonsDesGestes(i.gestes);
}

function brancherGestesImprevu(r, i, apres) {
  brancherLesGestes(r.querySelector(".imprevu-gestes"), i.gestes,
    { titre: nomDuSalarie(i.salarie), dit: [i.quoi, i.periode].filter(Boolean).join(", ") }, apres);
}

/** D'où vient la boîte : la carte (`{div}`) ou le dossier (`{salarie}`). */
async function provenance(ou) {
  if (ou.div) {
    // Ce qui est tapé dans la carte part d'abord : les contrats se lisent sur la saisie enregistrée.
    clearTimeout(MINUTEUR_SAISIE);
    await enregistrerSaisie(true);
    const carte = carteDe(ou.div);
    return { salarie: carte.salarie, carte, apres: ou.apres || rafraichirImprevus };
  }
  return { salarie: ou.salarie, carte: null, apres: ou.apres || rafraichirImprevus };
}

/* ------------------------------------------------ la boîte d'un imprévu
   Un arrêt de travail, une absence : la lecture qui rend sa boîte (le code du choix de la porte, `arret-obligations` ou
   `suspension-saisie`) dit, pour ce qui est déjà saisi, son titre, ses cases ouvertes (leur forme, leur libellé, leurs
   choix, ce que le moteur propose), ce qu'elle dit dessous et le geste qui l'enregistre. La page la pose par le
   dessinateur commun (`poserLaFiche`, `js/avance.js`) et, à chaque case changée, la redemande avec ce qui est saisi,
   puis la redessine : ce qui est tapé reste dans sa case, et le curseur aussi. Elle ne décide rien : une case n'est là
   que parce que la route la rend (la date de l'accident, la connaissance de l'accident, le contrat à choisir). Quitter
   la boîte avec une saisie non enregistrée se demande (`retenirLEcran`) ; « Annuler » la ferme sans rien garder.
   Après l'enregistrement, ce que l'imprévu écrit appelle encore (`etapeSuite`). */

/** Ce que disent les cases de `bloc`, chacune sous sa clé ; une case vide ne dit rien. */
function saisiDans(bloc) {
  return Object.fromEntries(Object.entries(lireLesCases(bloc)).filter(([, v]) => v != null));
}

async function boiteDUnImprevu(lecture, ou) {
  const p = await provenance(ou);
  // La boîte se lit pour ce salarié, et depuis cette carte : c'est son contrat que l'imprévu suspend.
  const base = { salarie: p.salarie };
  if (p.carte && p.carte.numero) base.carte_numero = p.carte.numero;
  if (p.carte && p.carte.debut) base.carte_debut = p.carte.debut;
  let r;
  try { r = await api(lecture, base); } catch (e) { dire(e.message, "erreur"); return; }
  ouvrirBoite(`
    <h3 data-titre></h3>
    <div data-cases></div>
    <div data-dit></div>
    <div class="actions">
      <button class="secondaire" data-annuler>Annuler</button>
      <button class="primaire" data-enregistrer></button>
    </div>`, () => oublierLEcran(bloc));
  const boite = document.getElementById("boite");
  const bloc = boite.querySelector("[data-cases]");
  let vue = r;
  // Chaque réponse se pose entière : ses cases se redessinent (ce que le moteur propose peut avoir changé), ce qui est
  // tapé reste.
  const poser = (x) => {
    vue = x;
    boite.querySelector("[data-titre]").textContent = x.titre;
    delete bloc.dataset.posee;
    poserLaFiche(bloc, { cases: x.cases, historique: [], avance_ouvert: false });
    boite.querySelector("[data-dit]").innerHTML = x.dit.map((l) => `<p class="note">${echapper(l)}</p>`).join("");
    // Sans geste (`enregistrer` absent : il ne pourrait que refuser, `dit` dit pourquoi), la boîte n'a qu'« Annuler ».
    const b = boite.querySelector("[data-enregistrer]");
    b.hidden = !x.enregistrer;
    if (x.enregistrer) b.textContent = x.enregistrer.libelle;
  };
  poser(r);
  let tour = 0;
  bloc.addEventListener("change", async () => {
    const n = ++tour;
    let x;
    try { x = await api(lecture, { ...base, ...saisiDans(bloc) }); } catch (e) { dire(e.message, "erreur"); return; }
    if (n === tour && bloc.isConnected) poser(x);
  });
  const enregistrer = async () => {
    const g = vue.enregistrer;
    // Rien ne s'enregistre ici : la boîte dit pourquoi, on y reste.
    if (!g) { dire(vue.dit.join(" "), "erreur"); return false; }
    let x;
    try { x = await api(g.route, { ...g.charge, ...saisiDans(bloc) }); }
    catch (e) { dire(e.message, "erreur"); return false; }
    oublierLEcran(bloc);
    return (await etapeSuite(x, x.arret || x.suspension, p)) ? ENREGISTRE_PUIS_SUITE : true;
  };
  retenirLEcran(bloc, { enregistrer });
  boite.querySelector("[data-enregistrer]").onclick = enregistrer;
  boite.querySelector("[data-annuler]").onclick = fermerBoite;
  CLIC_DEHORS = async () => { if (await quitterLEcran(boite)) fermerBoite(); };
}

/* ------------------------------------- ce que l'imprévu écrit appelle encore
   Le moteur le dit sur l'imprévu qu'il vient d'écrire : ce qu'un arrêt
   appelle encore (`a_dire` : le geste de ses indemnités journalières, quand
   il ouvre le maintien de salaire), et les jours que la saisie paie pendant
   l'imprévu (`cachets`) ; la réponse dit s'il appelle encore quelque chose
   (`appelle_encore`), et les mots de cette page (`jours_dit`, `plus_tard`,
   `enregistrer`). Rien : la boîte se ferme. Sinon, une seconde page, où
   rien n'est obligatoire — les indemnités se connaissent au relevé de la
   caisse, et se reprennent sous la carte. */

/* Rend vrai quand la seconde page est ouverte. */
async function etapeSuite(r, i, p) {
  const jours = i.cachets;
  const g = i.a_dire;
  if (!r.appelle_encore) {
    fermerBoite();
    dire(r.phrase);
    await p.apres();
    return false;
  }
  const div = p.carte ? p.carte.div : null;
  const ici = (c) => div && c.mois === MOIS_AFFICHE && !div.classList.contains("validee");
  ouvrirBoite(`
    <p>${echapper(r.phrase)}</p>
    <h3>${echapper(r.appelle_encore)}</h3>
    ${g ? `<div id="su-geste">${casesDuGeste(g)}</div>` : ""}
    ${jours.length ? `<div class="champ-boite">${echapper(r.jours_dit)}
      <div class="liste-jours">${jours.map((c, k) => c.jours.map((j) => ici(c)
        ? `<label class="coche"><input type="checkbox" data-jour="${k}" value="${echapper(j.iso)}">
             ${echapper(j.phrase)}</label>`
        : `<div class="discret">${echapper(j.phrase_du_mois)}</div>`).join("")).join("")}
      </div>
      ${jours.some((c) => !ici(c) && c.mois === MOIS_AFFICHE) ? `<p class="discret">${aideDe("imprevu_valide")}</p>` : ""}</div>` : ""}
    <div class="actions">
      <button class="secondaire" id="su-plus-tard">${echapper(r.plus_tard)}</button>
      <button class="primaire" id="su-ok">${echapper(r.enregistrer)}</button>
    </div>`);
  const finir = async () => { fermerBoite(); await p.apres(); };
  document.getElementById("su-plus-tard").onclick = finir;
  document.getElementById("su-ok").onclick = async () => {
    const charge = g && chargeDuGeste(document.getElementById("su-geste"), g);
    if (charge) {
      try { await api(g.route, charge); }
      catch (e) { dire(e.message, "erreur"); return; }
    }
    const retirer = [...document.querySelectorAll("#boite [data-jour]:checked")]
      .map((x) => ({ rubrique: jours[+x.dataset.jour].rubrique, iso: x.value }));
    if (retirer.length && !(div && await retirerDesJours(div, retirer))) dire(ETAT.aides.jour_hors_ligne, "erreur");
    await finir();
  };
  return true;
}


/* ------------------------------------------ LE DOSSIER DU SALARIÉ
   Dans sa fiche (Configuration › Salariés) : ses imprévus, passés et en
   cours, ce qui en est parti (les signalements, les DSN qui les ont
   portés, la déclaration d'accident dite faite), ce qui est encore dû, et
   la même porte. Rien quand il n'y en a jamais eu, sauf la porte. */
async function dessinerImprevusDuSalarie(cle) {
  const cible = document.getElementById("fiche-imprevus");
  if (!cible || !cle) return;
  let r;
  try { r = await api("imprevus", { salarie: cle }); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  NOMENCLATURES_IMPREVUS = NOMENCLATURES_IMPREVUS || r;
  if (CLE_ACTIVE !== cle) return;      // une autre fiche s'est ouverte entre-temps
  // Le plus récent d'abord, sous leur titre : la route les rend ainsi (`imprevus`, un salarié).
  const liste = r.imprevus;
  const apres = async () => { await dessinerImprevusDuSalarie(cle); if (MOIS_AFFICHE) await dessinerImprevus(MOIS_AFFICHE); };
  cible.innerHTML = `
    <div class="fiche-imprevus-tete">
      ${r.titre ? `<h3>${echapper(r.titre)}</h3>` : ""}
      <button type="button" class="lien" id="fiche-imprevu-porte">${echapper(r.porte_dit)}</button>
    </div>
    ${liste.map((i, k) => `<div class="imprevu" data-imprevu="${k}">
        <span class="imprevu-quoi"><b>${echapper(i.quoi)}</b>
          <span class="discret">${echapper(i.periode)}${i.contrat_dit ? ` · ${echapper(i.contrat_dit)}` : ""}</span></span>
        <span class="imprevu-gestes">${gestesImprevu(i)}</span>
        <div class="imprevu-parti">${ceQuiEnEstParti(i)}</div>
      </div>`).join("")}`;
  liste.forEach((i, k) => brancherGestesImprevu(cible.querySelector(`[data-imprevu="${k}"]`), i, apres));
  const porte = document.getElementById("fiche-imprevu-porte");
  porte.onclick = (ev) => { ev.stopPropagation(); menuImprevu(porte, { salarie: cle, apres }); };
}

/** Ce qui est parti d'un imprévu, et ce qui en reste dû : les morceaux que le moteur dit (`parti`), sur une ligne. */
function ceQuiEnEstParti(i) {
  return i.parti.map((p) => p.discret
    ? `<span class="discret">${echapper(p.phrase)}</span>`
    : `${p.pastille ? `${pastilleDeLEtat(p.pastille)} ` : ""}${echapper(p.phrase)}`).join(" · ");
}
