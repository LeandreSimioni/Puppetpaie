/* Les rappels de l'ouverture, « pour demarrer », l'echeancier. */
"use strict";

/* =======================================================================
   LES RAPPELS DE L'OUVERTURE, ET L'EXERCICE
   =======================================================================
   « Quand le mois est fini, le logiciel s'ouvre sur le mois suivant, et là
   il faut un pop-up : il est temps d'envoyer la DSN du mois précédent. Avez-
   vous bien fini les paies du mois précédent ? Et un bouton vérifier et
   envoyer. ». Puis, en janvier : clôturer l'exercice, exporter
   les chiffres pour la compta, saisir les numéros d'objet de l'année.

   La page ne décide de rien : /api/rappels rend la liste, déjà dans l'ordre,
   et chaque rappel dit où en est le mois (à saisir, à écrire, à déposer, à
   corriger). « Vérifier et envoyer » ne fait qu'ouvrir le mois et enchaîner
   les boîtes qui existent déjà — écrire le mois, puis le récapitulatif de
   dépôt avec son mot de passe. Rien ne part d'ici. « Plus tard » ferme la
   boîte ; elle reviendra à la prochaine ouverture, tant que le journal des
   dépôts ne dit pas que c'est fait. */
/* `lu(route)` : la lecture de l'ouverture (js/demarrage.js : tout arrive d'un
 * coup, et la boîte s'ouvre avec la page) — elle rend le résultat ou lève le
 * refus, comme `api`. Sans elle, on demande. */
let RAPPELS_MONTRES = null;   // ce que la boîte de l'ouverture a montré : `{liste, ech, manques}`, ou null
async function montrerRappels(lu = api) {
  // Une lecture refusée se dit ; l'échéancier ou la liste « Pour démarrer » refusés, la boîte montre le reste.
  let r, ech = null, manques = [];
  try { r = await lu("rappels"); } catch (e) { dire(e.message, "erreur"); return; }
  try { ech = await lu("echeancier"); } catch (e) { dire(e.message, "erreur"); }
  try { manques = await lireDemarrage(lu); } catch (e) { dire(e.message, "erreur"); }
  const liste = r.rappels;
  // L'ÉCHÉANCIER : « un pop-up au démarrage : on est le…,
  // bienvenue sur PuppetPaie, n'oublie pas de faire la DSN ». Elle n'apparaît que s'il y a quelque chose à montrer, et
  // ce sont les rappels qui le disent (`ouvrir_a_l_ouverture`).
  if (!r.ouvrir_a_l_ouverture) return;
  // Ce qu'une DSN pourrait donner compte dans ce que la boîte a montré : qu'il change, et elle se rouvre.
  RAPPELS_MONTRES = { liste, ech, manques, dsn: DEMARRAGE.dsn };
  if (dejaMontres(RAPPELS_MONTRES)) return;
  boiteRappels(liste, ech, manques, r.fermer_dit);
}

/* UNE FOIS, PUIS PLUS : la boîte s'ouvre à l'ouverture du logiciel ; la page
 * rechargée pendant qu'il tourne ne la rouvre pas tant qu'elle dirait la même
 * chose (le même jour, les mêmes choses à faire). La mémoire est celle de la
 * fenêtre (`sessionStorage`) : le logiciel rouvert la montre de nouveau. */
function dejaMontres(montres) {
  // La clé nomme la compagnie quand l'échéancier la dit (`null` sinon : une seule mémoire).
  const cle = "rappels-montres:" + JSON.stringify(montres.ech ? montres.ech.compagnie : null);
  const empreinte = JSON.stringify(montres);
  try {
    if (sessionStorage.getItem(cle) === empreinte) return true;
    sessionStorage.setItem(cle, empreinte);
  } catch (e) {
    // Le navigateur refuse sa mémoire : la boîte s'ouvre, et le refus se dit.
    dire(e.message, "erreur");
  }
  return false;
}

/* CE QUE LE MOTEUR A POSÉ DE LUI-MÊME (`pour-demarrer`, `poses`) : dit avec
 * ce qui manque, en information, avec le lien vers sa case. */
let POSES_DEMARRAGE = [];
/** La case où mène un lien de manque, quand la ligne en nomme une. */
const attributChamp = (champ) => (champ ? ` data-manque-champ="${echapper(champ)}"` : "");
function htmlPoses() {
  if (!DEMARRAGE.poses_titre) return "";
  return `<div class="rappel demarrage-poses">
    <h3>${echapper(DEMARRAGE.poses_titre)}</h3>
    <ul class="rappel-liste">${POSES_DEMARRAGE.map((p) => `
      <li>${pastilleDeLEtat(p.pastille)}
        <button type="button" class="lien"${attributChamp(p.champ)}
          data-manque-onglet="${echapper(p.onglet)}"><b>${echapper(p.libelle)}</b></button>
        <div class="note">${echapper(p.texte)}</div></li>`).join("")}</ul>
  </div>`;
}

/* POUR DÉMARRER. Le moteur (`pour-demarrer`) rend d'un coup ce que la
 * compagnie peut remplir d'avance : chaque ligne, sa case (`case`), où elle
 * vit (`ou`), et pour qui (`qui`, une ligne d'un salarié ou d'un spectacle). Chaque case se
 * remplit sur place, dessinée comme dans sa fiche, telle que la route la rend, par le
 * dessinateur commun (`caseDeRoute`) ; un seul
 * « Enregistrer » écrit tout ce qui a changé (`enregistrer-fiche`, une
 * écriture par fiche) et joue les gestes que le moteur lui donne
 * (`a_enregistrer`), et la liste se relit. La page ne fait que la mise en
 * page. */
const DEMARRAGE_VIDE = { titre: "", note: "", manques: [], dsn: null, poses_titre: null, trouver_dit: "", enregistrer: null, configuration_dit: "" };
let DEMARRAGE = DEMARRAGE_VIDE;

/* LA CASE D'UNE LIGNE, la route la rend (`case_rendue`, sa valeur celle que le moteur propose, sinon celle de la fiche)
 * avec ce qui l'enregistre (`enregistrer`) : le dessinateur commun la dessine, comme dans sa fiche (`caseDeRoute`,
 * `modifiable`). Sans elle (un organisme, le barème, le taux AT/MP, un accès), la ligne mène à sa place. */
const seRemplitIci = (m) => !!(m.case_rendue && m.enregistrer);

function htmlDemarrage(manques) {
  if (!manques.length && !DEMARRAGE.dsn) return htmlPoses();
  const n = ++DEMARRAGE_LISTES;
  // CHAQUE CHOSE MÈNE À SA PLACE : `ou` est un lien qui y conduit (la fiche,
  // l'onglet), sous la case ; la fiche d'un salarié ou d'un spectacle, quand la ligne en nomme un.
  const qui = (m) => (m.salarie ? ` data-manque-fiche="salarie" data-manque-cle="${echapper(m.salarie)}"`
    : m.spectacle ? ` data-manque-fiche="spectacle" data-manque-cle="${echapper(m.spectacle)}"` : "");
  const lien = (m) => `<button type="button" class="lien"${attributChamp(m.champ)}
        data-manque-onglet="${echapper(m.onglet)}"${qui(m)}>${echapper(m.ou)}</button>`;
  const pour = (m) => (m.pour ? `<div class="note">${echapper(m.pour)}</div>` : "");
  // LES GESTES QUE « ENREGISTRER » JOUE (`a_enregistrer`, du moteur) : chacun sa phrase et ses cases, pré-remplies,
  // que la fiche de sa route décrit (`casesDuGeste`, d'`avance.js`).
  const aEnregistrer = (m, i) => (m.a_enregistrer || []).map((g, k) => `<div data-demarrage-geste="${i}" data-demarrage-rang="${k}">
      ${casesDuGeste(g)}</div>`).join("");
  const ligne = (m) => {
    const i = manques.indexOf(m);
    if (!seRemplitIci(m)) {
      return `<li class="demarrage-ligne">
        <b>${echapper(m.libelle)}</b> ${lien(m)}
        ${pour(m)}
        ${aEnregistrer(m, i)}
        ${m.trouver ? `<details><summary>${echapper(DEMARRAGE.trouver_dit)}</summary><p class="note">${echapper(m.trouver)}</p></details>` : ""}
      </li>`;
    }
    return `<li class="demarrage-ligne" data-demarrage="${i}">
      <div class="demarrage-case">${caseDeRoute(modifiable(m.case_rendue))}</div>
      ${pour(m)}
      ${aEnregistrer(m, i)}
      <div class="actions">${lien(m)}</div>
    </li>`;
  };
  const entreprise = manques.filter((m) => !m.qui);
  const lesQui = [...new Set(manques.filter((m) => m.qui).map((m) => m.qui))];
  // CE QU'UNE DSN DONNERAIT (`dsn`, du moteur) : sa phrase, et chaque ligne, qui mène à sa place.
  const dsn = DEMARRAGE.dsn ? `<div class="encart attention"><p>${echapper(DEMARRAGE.dsn.phrase)}</p>
      <ul class="rappel-liste">${DEMARRAGE.dsn.manques.map((m) => `<li class="demarrage-ligne">
        <b>${echapper(m.libelle)}</b>${m.qui ? ` <span class="discret">— ${echapper(m.qui)}</span>` : ""} ${lien(m)}</li>`).join("")}</ul>
    </div>` : "";
  return `
    <div class="rappel demarrage demarrage-liste" data-demarrage-liste="${n}">
      ${DEMARRAGE.titre ? `<h3>${echapper(DEMARRAGE.titre)}</h3>` : ""}
      ${dsn}
      ${DEMARRAGE.note ? `<p class="note">${echapper(DEMARRAGE.note)}</p>` : ""}
      ${entreprise.length ? `<ul class="rappel-liste">${entreprise.map(ligne).join("")}</ul>` : ""}
      ${lesQui.map((q) => `<h4>${echapper(q)}</h4>
        <ul class="rappel-liste">${manques.filter((m) => m.qui === q).map(ligne).join("")}</ul>`).join("")}
      <div class="anomalie" data-demarrage-dit hidden></div>
      <div class="actions">
        ${DEMARRAGE.enregistrer ? `<button class="primaire" data-demarrage-enregistrer>${echapper(DEMARRAGE.enregistrer)}</button>` : ""}
        <button class="petit" data-aller-configuration>${echapper(DEMARRAGE.configuration_dit)}</button>
      </div>
    </div>${htmlPoses()}`;
}
let DEMARRAGE_LISTES = 0;   // chaque liste peinte a son numéro : ses cases se lisent chez elle
const DEPART_DES_LIGNES = new WeakMap();   // ce que dit chaque ligne à l'ouverture de sa liste (`casesDuBloc`)

/* Les cases dessinées d'une liste : ce qu'une case de route fait seule (un calendrier, une rangée). Chaque liste est un
 * écran retenu (`retenirLEcran`) : la quitter avec une saisie non enregistrée se demande ; une valeur que le moteur
 * propose remplit sa case dès l'ouverture, elle n'est donc pas un changement. Jeter relit la liste. */
function brancherDemarrage(conteneur) {
  conteneur.querySelectorAll(".demarrage-liste [data-demarrage]").forEach((li) => {
    brancherLesCases(li);
    DEPART_DES_LIGNES.set(li, casesDuBloc(li));
  });
  conteneur.querySelectorAll(".demarrage-liste").forEach((liste) => {
    retenirLEcran(liste, { enregistrer: () => enregistrerDemarrage(liste), jeter: repeindreDemarrage });
  });
}

/* LES GESTES QUE « ENREGISTRER » JOUE (`a_enregistrer`, du moteur), tels que leurs cases les disent : `{route,
 * charge}` ; un geste dont les cases sont vides ne part pas. */
function gestesDemarrage(liste) {
  return [...liste.querySelectorAll("[data-demarrage-geste]")].flatMap((bloc) => {
    const m = DEMARRAGE.manques[+bloc.dataset.demarrageGeste];
    const g = m && m.a_enregistrer && m.a_enregistrer[+bloc.dataset.demarrageRang];
    const charge = g && chargeDuGeste(bloc, g);
    return charge ? [{ route: g.route, charge }] : [];
  });
}

/* CE QUE LA LISTE PORTE, ÉCRITURE PAR ÉCRITURE : les lignes que la même route enregistre avec la même charge (une
 * fiche : l'entreprise, un salarié, un spectacle) partent ensemble, chacune sa case dans les `changements` de la charge
 * (`casesDeLaCharge`, comme la fiche), telle qu'elle la dit ; la route garde ce qui ne change pas. Deux lignes des
 * parties d'une même case (une structure) en font une : la seconde pose ses parties sur ce que la première a dit. */
function changementsDemarrage(liste) {
  const parEcriture = new Map();
  liste.querySelectorAll("[data-demarrage]").forEach((li) => {
    const m = DEMARRAGE.manques[+li.dataset.demarrage];
    if (!m || !seRemplitIci(m)) return;
    // NE PART QUE CE QUI A CHANGÉ depuis l'ouverture de la liste, et ce que le moteur propose (`propose` : la fiche ne
    // l'a pas encore, « Enregistrer » le valide).
    if (m.propose == null && casesDuBloc(li) === DEPART_DES_LIGNES.get(li)) return;
    const cle = JSON.stringify(m.enregistrer);
    const e = parEcriture.get(cle) || { route: m.enregistrer.route, charge: m.enregistrer.charge, donnees: {} };
    lesCasesLues(li.querySelector(".demarrage-case")).forEach(([c, v]) => {
      const x = li.querySelector(`.demarrage-case .case[data-cle="${CSS.escape(c)}"] > [data-case]`);
      const deja = e.donnees[c];
      e.donnees[c] = deja != null && x.dataset.forme === "parties" ? lireLesParties(x, deja) : v;
    });
    parEcriture.set(cle, e);
  });
  return [...parEcriture.values()].map((e) => ({ route: e.route, charge: { ...e.charge, ...casesDeLaCharge(e.donnees) } }));
}
/* ENREGISTRER « POUR DÉMARRER » : tout ce que la liste porte, une écriture
 * par fiche (`enregistrer-fiche`), puis chaque geste que le moteur dit de
 * jouer (`a_enregistrer`), puis la liste se relit. Le premier refus arrête
 * la suite et se dit. Rend vrai quand tout est écrit. */
async function enregistrerDemarrage(liste) {
  const fiches = changementsDemarrage(liste);
  const gestes = gestesDemarrage(liste);
  const dit = liste.querySelector("[data-demarrage-dit]");
  const bouton = liste.querySelector("[data-demarrage-enregistrer]");
  if (!fiches.length && !gestes.length) return true;
  if (bouton) bouton.disabled = true;
  dit.hidden = true;
  let phrase = "";
  try {
    for (const { route, charge } of fiches) {
      let r = await api(route, charge);
      if (r.avertissement) {
        const rendre = mettreDeCote();
        const reponse = await demanderDateDEffet(r.avertissement);
        if (rendre) rendre();
        if (!reponse) { if (bouton) bouton.disabled = false; return false; }
        r = await api(route, { ...charge, ...reponse, confirme: true });
      }
      // La phrase dite à la fin : celle de la dernière réponse qui a écrit (une fiche dit ses cases écrites, `ecrits`).
      if (r.ecrits.length) phrase = r.phrase;
    }
    for (const g of gestes) phrase = (await api(g.route, g.charge)).phrase;
  } catch (e) {
    if (bouton) bouton.disabled = false;
    // Un refus de fiche porte ses anomalies à part (`a_part.anomalies`) ; un autre refus, sa phrase.
    const aPart = e.detail && e.detail.a_part;
    const anomalies = aPart && aPart.anomalies ? aPart.anomalies.filter((a) => a.gravite === "erreur") : [];
    dit.textContent = anomalies.length ? anomalies.map((a) => `${a.libelle || a.champ} : ${a.message}`).join(" ; ") : e.message;
    dit.hidden = false;
    await relireEtat();
    return false;
  }
  await relireEtat();
  if (phrase) dire(phrase);
  await repeindreDemarrage();
  return true;
}
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-demarrage-enregistrer]");
  if (b) enregistrerDemarrage(b.closest(".demarrage-liste"));
});

/* La liste relue, partout où elle est peinte ; la fiche Entreprise, si elle
 * est ouverte et qu'on n'y a rien tapé, montre ce qui vient de s'écrire. */
async function repeindreDemarrage() {
  try { await lireDemarrage(api); } catch (e) { dire(e.message, "erreur"); return; }
  document.querySelectorAll(".demarrage-liste").forEach((l) => {
    const neuf = document.createElement("div");
    neuf.innerHTML = htmlDemarrage(DEMARRAGE.manques);
    const pose = l.nextElementSibling && l.nextElementSibling.classList.contains("demarrage-poses") ? l.nextElementSibling : null;
    if (pose) pose.remove();
    l.replaceWith(...neuf.childNodes);
  });
  document.querySelectorAll("[data-aller-configuration]").forEach((b) => {
    if (b.closest("#config-liste, #import-demarrage")) b.remove();
  });
  brancherDemarrage(document);
  if (FICHE_ACTIVE === "entreprise" && !ecranModifie(casesDeLaFicheOuverte())
      && !document.getElementById("vue-configuration").classList.contains("cachee")) {
    dessinerFormulaire(fichesDe("entreprise")[0]);
  }
}

/* « Ouvrir la configuration », sous la liste de la boîte de l'ouverture. */
document.addEventListener("click", async (e) => {
  const b = e.target.closest && e.target.closest("[data-aller-configuration]");
  if (!b) return;
  if (!(await quitterLEcran(document.getElementById("boite")))) return;
  fermerBoite();
  document.querySelector('.onglets button[data-vue="configuration"]').click();
  const sous = document.querySelector('.sous-onglets button[data-fiche="entreprise"]');
  if (sous && !sous.classList.contains("actif")) sous.click();
});

/* `pour-demarrer`, lu : la liste et ce que le moteur a posé. Une lecture
 * refusée laisse ce qui est peint et ce qu'on en sait tels quels : la liste
 * à l'écran reste celle dont on lit les changements. */
async function lireDemarrage(lu) {
  const pd = await lu("pour-demarrer");
  DEMARRAGE = { titre: pd.titre, note: pd.note, manques: pd.manques, dsn: pd.dsn || null, poses_titre: pd.poses_titre || null,
    trouver_dit: pd.trouver_dit, enregistrer: pd.enregistrer || null, configuration_dit: pd.configuration_dit };
  POSES_DEMARRAGE = pd.poses;
  return DEMARRAGE.manques;
}

/** Aller à ce qui manque : l'onglet, puis la case (son tiroir ouvert, le
 *  curseur dedans). Les organismes ont leur onglet ; ce qui se dit sur la
 *  fiche d'un salarié ou d'un spectacle (`salarie`, `spectacle`, du moteur)
 *  ouvre sa fiche. */
async function allerAuManque(champ, onglet, nom, cle) {
  if (!document.getElementById("voile").classList.contains("cachee")) {
    if (!(await quitterLEcran(document.getElementById("boite")))) return;
    fermerBoite();
  }
  // Déjà dans la configuration : on n'y retourne pas. Recliquer l'onglet
  // redessinait la fiche et, sur une fiche en cours de saisie, ouvrait « Vous
  // avez modifié cette fiche » — suivre un manque après l'autre sans
  // enregistrer entre deux ne doit rien demander.
  if (document.getElementById("vue-configuration").classList.contains("cachee")) {
    document.querySelector('.onglets button[data-vue="configuration"]').click();
  }
  const fiche = nom || onglet || "entreprise";
  if (nom) await ouvrirFicheImportee(nom, cle);
  else {
    const sous = document.querySelector(`.sous-onglets button[data-fiche="${fiche}"]`);
    if (sous && !sous.classList.contains("actif")) sous.click();
  }
  if (fiche === "organismes" || fiche === "coffre" || !champ) return;
  const trouver = () => document.querySelector(`#config-formulaire .case[data-cle="${CSS.escape(champ)}"]`);
  let essais = 0;
  const viser = () => {
    // Une case de route porte sa clé sur son libellé : le curseur va dans sa case.
    const c = trouver()?.querySelector("[data-case]") || trouver();
    if (!c) { if (++essais < 20) setTimeout(viser, 100); return; }
    const tiroir = c.closest("details");
    if (tiroir) tiroir.open = true;
    c.scrollIntoView({ block: "center" });
    c.focus({ preventScroll: true });
  };
  // La liste « Pour démarrer » se peint au-dessus de la fiche quand le moteur
  // a répondu : viser après elle, sinon elle repousse la case hors de la vue.
  Promise.resolve(DEMARRAGE_PEINT).then(viser, viser);
}
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-manque-onglet]");
  if (!b) return;
  e.preventDefault();
  allerAuManque(b.dataset.manqueChamp, b.dataset.manqueOnglet, b.dataset.manqueFiche, b.dataset.manqueCle);
});

/** La liste « Pour démarrer », en tête de Configuration › Entreprise. */
let DEMARRAGE_PEINT = null;   // la peinture en cours de la liste, pour `allerAuManque`
function peindreDemarrage(conteneur) {
  DEMARRAGE_PEINT = peindreDemarrageDans(conteneur);
  return DEMARRAGE_PEINT;
}
async function peindreDemarrageDans(conteneur) {
  let manques;
  try { manques = await lireDemarrage(api); } catch (e) { dire(e.message, "erreur"); return; }
  conteneur.innerHTML = htmlDemarrage(manques);
  const b = conteneur.querySelector("[data-aller-configuration]");
  if (b) b.remove();
  brancherDemarrage(conteneur);
}

/* L'en-tête de la boîte : la date, la bienvenue, puis ce que la route `echeancier` en montre, tel qu'elle le rend :
 * « Ce mois-ci » (`ce_mois_titre`) et ses rangs, ou ce qu'il dit sans eux (`ce_mois_vide`) ; l'année qui vient repliée
 * (`annee_titre`, ses rangs). Chaque rang : sa pastille, son titre, ce qu'il dit, ses gestes. */
function htmlEcheancier(ech) {
  if (!ech) return "";
  const rangs = (liste, ou) => liste.map((x, i) => `
    <li class="echeance" data-rang-echeance="${ou}" data-i="${i}">
      <div class="echeance-titre">${pastilleDeLEtat(x.pastille)} ${echapper(x.titre)}</div>
      ${x.dit.map((l) => `<div class="note">${echapper(l)}</div>`).join("")}
      ${x.gestes.length ? `<div class="actions">${boutonsDesGestes(x.gestes)}</div>` : ""}
    </li>`).join("");
  return `
    <div class="accueil">
      <p class="accueil-date">${echapper(ech.date_dit)}</p>
      <h2 class="accueil-titre">${echapper(ech.bienvenue)}</h2>
      ${ech.ce_mois_titre ? `<h3>${echapper(ech.ce_mois_titre)}</h3>` : ""}
      ${ech.ce_mois_rangs.length ? `<ul class="echeances">${rangs(ech.ce_mois_rangs, "ce_mois_rangs")}</ul>` : ""}
      ${ech.ce_mois_vide ? `<p class="discret">${echapper(ech.ce_mois_vide)}</p>` : ""}
      ${ech.annee_titre ? `<details class="accueil-annee"><summary>${echapper(ech.annee_titre)}</summary>
        <ul class="echeances-annee">${ech.annee_mois.map((m) => `<li>${m.mois ? `<b>${echapper(m.mois)}</b>` : ""}
          <ul>${m.echeances.map((t) => `<li>${echapper(t)}</li>`).join("")}</ul></li>`).join("")}</ul></details>` : ""}
    </div>`;
}

/* L'ÉCHÉANCIER ET LES RAPPELS, dans la boîte : chacun ses gestes, tels que la route les rend (`brancherLesGestes`, leur
 * boîte par-dessus celle-ci) ; après un geste, ils se relisent, et « Pour démarrer », au-dessus d'eux, garde ce qu'on y a
 * tapé. */
function poserLesRappels(liste, ech, fermerDit) {
  const cible = document.querySelector("#boite [data-rappels-du-jour]");
  if (!cible) return;
  cible.innerHTML = htmlEcheancier(ech) + liste.map((r, i) => `
    <div class="rappel ${r.en_retard ? "retard" : ""}" data-rappel="${i}">
      <h3>${echapper(r.titre)}${r.pastille ? ` ${pastilleDeLEtat(r.pastille)}` : ""}${r.quand
        ? ` <span class="discret">— ${echapper(r.quand)}</span>` : ""}</h3>
      ${r.question ? `<p class="rappel-question">${echapper(r.question)}</p>` : ""}
      ${r.detail ? `<p class="note">${echapper(r.detail)}</p>` : ""}
      ${r.spectacles.length ? `<ul class="rappel-liste">${r.spectacles.map((s) => `<li>${echapper(s.dit)}</li>`).join("")}</ul>` : ""}
      <div class="actions">${boutonRappel(r, i)}</div>
    </div>`).join("");
  document.getElementById("rappels-fermer").textContent = fermerDit;
  const relire = async () => {
    await relireEtat();
    let r2, ech2 = null;
    try { r2 = await api("rappels"); } catch (e) { dire(e.message, "erreur"); return; }
    try { ech2 = await api("echeancier"); } catch (e) { dire(e.message, "erreur"); }
    poserLesRappels(r2.rappels, ech2, r2.fermer_dit);
  };
  cible.querySelectorAll("[data-rang-echeance]").forEach((li) => {
    const x = ech[li.dataset.rangEcheance][+li.dataset.i];
    brancherLesGestes(li, x.gestes, { titre: x.titre, pardessus: true }, relire);
  });
  cible.querySelectorAll("[data-rappel]").forEach((bloc) => {
    const r = liste[+bloc.dataset.rappel];
    const b = document.getElementById("rappel-" + bloc.dataset.rappel);
    if (b) b.onclick = () => agirSurRappel(r);
    brancherLesGestes(bloc, r.gestes, { titre: r.titre, pardessus: true }, relire);
  });
}

function boiteRappels(liste, ech, manques, fermerDit) {
  ouvrirBoite(`
    <div class="boite-rappels"></div>${htmlDemarrage(manques)}
    <div data-rappels-du-jour></div>
    <div class="actions">
      <button class="secondaire" id="rappels-fermer"></button>
    </div>`, () => {
    // FERMÉE, ELLE SORT DE LA PAGE : cachée sous le voile, elle gardait
    // ses boutons `[data-manque-champ]`, périmés dès que la fiche change, et
    // qu'on retrouvait en cherchant ceux de « Pour démarrer ».
    const boite = document.getElementById("boite");
    if (boite.querySelector(".boite-rappels")) boite.innerHTML = "";
  });
  // FERMER AVEC DES CASES REMPLIES ET PAS ENREGISTRÉES demande, comme partout (`quitterLEcran`).
  const fermer = async () => { if (await quitterLEcran(document.getElementById("boite"))) fermerBoite(); };
  document.getElementById("rappels-fermer").onclick = fermer;
  CLIC_DEHORS = fermer;
  poserLesRappels(liste, ech, fermerDit);
  brancherDemarrage(document.getElementById("boite"));
}

/* CE QUI EST DÛ, ET SES GESTES : le moteur rend l'`action` de chaque rappel, une route du catalogue (sa `charge`, la
   route de l'acte qui suit, `puis`, et le mot du bouton, `libelle`) ; la boîte s'ouvre d'après la route
   (`ouvrirGeste`), comme une case du mois. Ses `gestes` (« C'est fait »…) sont ceux d'un écran rendu. */
function boutonRappel(r, i) {
  const libelle = r.action && r.action.libelle;
  return (libelle ? `<button class="primaire" id="rappel-${i}">${echapper(libelle)}</button>` : "") + boutonsDesGestes(r.gestes, undefined, "secondaire");
}

/* Le geste d'un rappel mène ailleurs : tout écran qui porte une saisie non enregistrée se quitte d'abord (la boîte, et
 * ce qui est dessous), avant d'ouvrir l'onglet Paie, dont le gestionnaire ne redemande alors rien. */
async function agirSurRappel(r) {
  if (!(await quitterLEcran())) return;
  fermerBoite();
  const mois = r.action && r.action.charge.mois;
  // Un geste qui porte sur un mois l'ouvre d'abord : la boîte s'ouvre sur lui.
  if (mois) {
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(mois);
  }
  return ouvrirGeste(r.action, mois || r.mois, r);
}

/* ---- L'exercice, dans l'Historique : par année, ce qui est fait, ce qui
 * se fait. Clôturer fige l'année et écrit le zip ; l'export compta écrit le
 * CSV du journal de paie. Les deux sont locaux — rien ne part. */
async function dessinerExercices() {
  const cible = document.getElementById("exercices");
  let r;
  try { r = await api("exercices"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  if (!r.exercices.length) { cible.innerHTML = `<p class="discret">${echapper(r.exercices_dit)}</p>`; return; }
  // Chaque année, ses actions (la boîte de leur route, `ouvrirGeste`) puis ses gestes (clore, rouvrir : chacun sa boîte,
  // `brancherLesGestes`), tels que la route les rend.
  cible.innerHTML = r.exercices.map((x, i) => `
    <div class="rang exercice" data-exercice="${i}">
      <span class="rang-nom">${echapper(x.annee)}</span>
      ${pastilleDeLEtat(x.pastille)}
      <span class="rang-infos">
        ${echapper(x.infos_dit)}
      </span>
      ${x.actions.map((a, k) => `<button class="secondaire" data-action="${k}">${echapper(a.libelle)}</button>`).join("")}
      ${boutonsRanges(x.gestes)}
    </div>`).join("") + `<p class="note"><button class="lien" id="ouvrir-exercices">${echapper(r.dossier_dit)}</button></p>`;
  cible.querySelectorAll("[data-exercice]").forEach((rang) => {
    const x = r.exercices[+rang.dataset.exercice];
    rang.querySelectorAll("[data-action]").forEach((b) => {
      b.onclick = () => ouvrirGeste(x.actions[+b.dataset.action]);
    });
    brancherLesGestes(rang, x.gestes, { titre: String(x.annee) }, dessinerExercices);
  });
  const o = document.getElementById("ouvrir-exercices");
  if (o) o.onclick = async () => {
    await ouvrirEtDire(r.dossier);
  };
}

