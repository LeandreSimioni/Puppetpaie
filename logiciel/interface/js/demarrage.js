/* Le demarrage : demarrer, finir le chargement ; la fenetre PuppetPaie, le bouton Accueil. Charge en dernier. */
"use strict";

/* =======================================================================
   DEMARRAGE
   =======================================================================
   L'OUVERTURE D'UN COUP : la page entière d'un seul geste, jamais petit à
   petit ni derrière une roue qui tourne sans dire pourquoi. UNE demande,
   `demarrage`, que la fenêtre sert sans processus
   (`etat_garde::FileDesEtats::demarrage`) : tout ce que la page lit en
   s'ouvrant, juste — l'accueil, le mois à ouvrir et son écran,
   l'échéancier, les rappels, ce qui manque pour démarrer, les conseils, la
   veille, la mise à jour, les signalements. La page se peint d'un coup
   avec, et le voile s'en va. L'écran du mois peut arriver périmé
   (`ecran.perime`) : l'état frais le remplace en place (`verifierLeMois`).

   Ce qui se refait (compagnie neuve, une écriture depuis), le moteur le
   fait et dit ce qu'il fait (`phrase`) : le voile l'affiche, et la page
   redemande. */

/** Une lecture de l'ouverture : son résultat, ou son refus levé (comme `api`). */
function lectureDe(o, route) {
  return structuredClone(lireReponse(o.lectures[route]));
}

/** L'état lu (`etat`), et le bandeau qui le dit : le seul endroit où la page
 *  le pose, pour que le bandeau ne parle jamais d'un état d'avant. */
function poserEtat(etat) {
  ETAT = etat;
  // Une compagnie fictive : ses boutons qui envoient disent « simulé » (interface.css, `.envoi`).
  document.body.classList.toggle("fictive", !!(etat && etat.simulation));
  peindreEtatGlobal();
  return ETAT;
}
async function relireEtat() {
  return poserEtat(await api("etat"));
}

/** Le bandeau : les erreurs de configuration, ce qui manque pour payer aujourd'hui, et les compteurs. */
function peindreEtatGlobal() {
  // Une option VIDE en tete, et c'est elle le defaut. Un deroulant montre
  // toujours quelque chose : sans elle, le premier spectacle de la liste a
  // l'air choisi alors que personne ne l'a choisi -- et il porte le numero
  // d'objet qui part en DSN. Un mois porte d'ailleurs plusieurs spectacles :
  // c'est l'engagement qui dit le sien, pas le mois.
  const c = ETAT.configuration;
  // Les erreurs de configuration, comptées par le moteur (`configuration`, `erreurs`).
  const html = `
    ${pastilleDeLEtat(c.erreurs)}
    ${c.pour_payer ? `<div>${echapper(c.pour_payer)}</div>` : ""}
    ${c.compteurs_dit ? `<div>${echapper(c.compteurs_dit)}</div>` : ""}`;
  const el = document.getElementById("etat-global");
  if (el.dataset.peint !== html) { el.dataset.peint = html; el.innerHTML = html; }
}

/** Les conseils de l'ouverture, lus : `{jour, resume}` (un refus se dit, et ce conseil ne se montre pas). */
function conseilsDe(o) {
  const lu = (r) => { try { return lectureDe(o, r); } catch (e) { dire(e.message, "erreur"); return null; } };
  return { jour: lu("conseil-du-jour"), resume: lu("conseils-resume") };
}

async function demarrer() {
  CATALOGUE = Object.fromEntries((await appelerMoteur("routes", { avance: true })).routes.map((f) => [f.nom, f]));
  rangerLAvance();
  dessinerLesAvances();
  let o = await appelerMoteur("demarrage", { patienter: 400 });
  while (!o.pret) {
    // LA SEULE ATTENTE : ce que la page lit se refait. Le moteur dit ce qu'il fait.
    document.getElementById("chargement-phrase").textContent = o.phrase;
    o = await appelerMoteur("demarrage", { patienter: 400 });
  }
  AUJOURDHUI = o.aujourdhui;
  poserEtat(lectureDe(o, "etat"));
  if (window.poserAuteur) window.poserAuteur(ETAT.auteur);
  dessinerVeille(o.veille);
  try { dessinerPastilleReponses(lireReponse(o.signalements)); } catch (e) { dire(e.message, "erreur"); }
  if (window.miseAJourMontrer) window.miseAJourMontrer(o.maj);
  // Le conseil du jour, quand la compagnie a coché l'option : il ne retient rien.
  dessinerConseils(conseilsDe(o));

  // LE MOIS À OUVRIR, le moteur le dit (`mois-a-ouvrir`, et le mois du jour
  // sans lui) : le mois en cours, sauf un mois passé dont une étape n'est pas
  // verte. Son écran est arrivé avec l'ouverture.
  poserLeMois(o.mois, o.ecran);
  await chargerMois(o.mois);
  finirChargement();

  // UNE COMPAGNIE QUI COMMENCE PAR L'IMPORT (`commencer_par_l_import`, du
  // moteur) : la page s'ouvre sur lui et son guide ; ce qui manque pour
  // démarrer se dit après lui, sous ce qu'il a repris.
  if (ETAT.commencer_par_l_import) {
    document.querySelector('.onglets button[data-vue="configuration"]').click();
    const sous = document.querySelector('.sous-onglets button[data-fiche="importer"]');
    if (sous) sous.click();
  } else {
    // Ce qui attend : la DSN du mois d'avant, la clôture en janvier. Une
    // boîte, avec la page, tant que ce n'est pas fait.
    montrerRappels((r) => lectureDe(o, r));
  }

  // Le travail de fond : on le regarde toutes les minutes. C'est une lecture
  // d'etat en memoire cote serveur, elle ne coute rien -- surtout pas un
  // appel a Google ou a net-entreprises, qui n'ont lieu que dans le fil de
  // la veille, a son rythme a lui.
  setInterval(suivreVeille, 60000);
}

/** Le voile du demarrage (index.html, #chargement) : rien que le fond tant
 *  que la page n'est pas peinte, et la phrase du moteur s'il faut attendre ;
 *  puis la page, d'un coup. Retire aussi en cas d'echec : un voile qui
 *  resterait cacherait le message qui dit pourquoi. */
function finirChargement() {
  const v = document.getElementById("chargement");
  if (v) v.remove();
}

demarrer().catch((e) => {
  finirChargement();
  // Une valeur à saisir de nouveau : sa boîte le dit déjà (`montrerLaDemande`).
  if (e.detail && e.detail.classe === "ValeurARedire") return;
  dire(e.message, "erreur");
});

/* UNE VALEUR D'UNE FICHE QUI NE SE LIT PLUS : toute route que le moteur ne sert pas d'ici là refuse en renvoyant à la
   demande (`ValeurARedire`). La boîte montre la première valeur que la route `valeurs-illisibles` rend — sa phrase, sa
   case dessinée comme dans sa fiche (`caseDeRoute`) — et la redit par `enregistrer-fiche`, avec la charge que la route
   donne : la case saisie, ou vide exprès. Puis la suivante ; quand il n'en reste aucune, la page se recharge. La boîte
   ne se ferme pas d'un clic à côté : rien d'autre ne se fait d'ici là. */
let DEMANDE_MONTREE = false;
async function montrerLaDemande() {
  if (DEMANDE_MONTREE) return;
  DEMANDE_MONTREE = true;
  finirChargement();
  let d;
  try { d = await appelerMoteur("valeurs-illisibles"); } catch (e) { DEMANDE_MONTREE = false; dire(e.message, "erreur"); return; }
  if (!d.valeurs.length) { location.reload(); return; }
  const v = d.valeurs[0];
  ouvrirBoite(`
    <h3>${echapper(d.titre)}</h3>
    <p>${echapper(v.phrase)}</p>
    <div id="redire-case">${v.case ? caseDeRoute(v.case) : ""}</div>
    <div id="redire-dit"></div>
    <div class="actions">
      <button class="secondaire" id="redire-vide">${echapper(d.vide)}</button>
      ${v.case ? `<button class="primaire" id="redire-ok">${echapper(d.enregistrer)}</button>` : ""}
    </div>`);
  CLIC_DEHORS = () => {};
  const bloc = document.getElementById("redire-case");
  brancherLesCases(bloc);
  const dit = document.getElementById("redire-dit");
  let confirme = false;
  const redire = async (valeur) => {
    const charge = { ...v.enregistrer, changements: [{ case: v.cle, valeur }] };
    if (confirme) charge.confirme = true;
    let r;
    try { r = await appelerMoteur("enregistrer-fiche", charge); } catch (e) {
      dit.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
      return;
    }
    // Un avertissement (une phrase à confirmer) : rien n'est écrit ; « Confirmer » renvoie la même charge, confirmée.
    if (r.avertissement) {
      dit.innerHTML = r.phrases.map((p) => `<p class="note">${echapper(p)}</p>`).join("");
      confirme = true;
      const ok = document.getElementById("redire-ok");
      if (ok) ok.textContent = d.confirmer;
      return;
    }
    fermerBoite();
    DEMANDE_MONTREE = false;
    montrerLaDemande();
  };
  document.getElementById("redire-vide").onclick = () => redire(null);
  const ok = document.getElementById("redire-ok");
  if (ok) {
    ok.onclick = () => {
      const x = bloc.querySelector("[data-case]");
      // Une case à cocher dit oui ou non telle qu'elle est ; toute autre, ce que sa forme lit.
      const lu = x.dataset.forme === "oui_non" ? x.checked : lireLaCase(x);
      redire(lu === undefined ? null : lu);
    };
  }
}

/* LA FENETRE PUPPETPAIE, SANS BARRE NOIRE. La
   fenetre n'a pas de bordure systeme : le bandeau EST sa barre de titre. Son
   cadre (`fenetre/src/cadre.js`, pose par la fenetre dans chaque page) donne
   `window.fenetrePuppetPaie` : les trois boutons lui parlent, le bandeau se
   tire (`fenetre-tire`) et un double-clic agrandit. Dans un navigateur, il
   n'existe pas : `body` n'a pas la classe `fenetre` et rien ne s'affiche. */
function fenetreNative() {
  const api = window.pywebview && window.pywebview.api;
  // L'ACCUEIL DES COMPAGNIES : le serveur de la fenetre le
  // propose (`pywebview.api.accueil`, la cale du moteur), dans la fenetre
  // comme dans le navigateur.
  if (api && typeof api.accueil === "function" && !document.body.classList.contains("accueil-possible")) {
    boutonAccueil(api);
  }
  const f = window.fenetrePuppetPaie;
  if (!f || document.body.classList.contains("fenetre")) return;
  document.body.classList.add("fenetre");
  document.querySelector(".bandeau").classList.add("fenetre-tire");
  document.getElementById("fenetre-reduire").onclick = () => f.reduire();
  document.getElementById("fenetre-agrandir").onclick = () => f.agrandir();
  // Fermer enregistre d'abord ce qui attend encore (la saisie s'enregistre
  // une seconde apres la derniere frappe), puis ferme.
  document.getElementById("fenetre-fermer").onclick = async () => {
    if (!(await avantDeQuitter())) return;
    f.fermer();
  };
  // Agrandie, le bouton du milieu restaure : il le montre.
  const milieu = document.getElementById("fenetre-agrandir");
  const suivre = async () => {
    const agrandie = await f.agrandie();
    milieu.innerHTML = agrandie ? "&#x2750;" : "&#x25A1;";
    milieu.title = agrandie ? milieu.dataset.titreAgrandie : milieu.dataset.titre;
  };
  window.addEventListener("resize", suivre);
  suivre();
}
/* Avant de quitter la page (revenir aux compagnies, redémarrer pour mettre à
   jour : js/mise-a-jour.js) : ce qui attend encore s'enregistre. Faux si
   l'enregistrement est refusé : on reste, le message est à l'écran, et partir
   perdrait ce qu'on vient de taper. */
async function avantDeQuitter() {
  clearTimeout(MINUTEUR_SAISIE);
  await enregistrerSaisie(false);
  return toutEstEcrit();
}
window.avantDeQuitter = avantDeQuitter;
function boutonAccueil(api) {
  document.body.classList.add("accueil-possible");
  // Revenir aux compagnies quitte la page : un écran qui porte une saisie non enregistrée se quitte d'abord.
  document.getElementById("fenetre-accueil").onclick = async () => {
    if (!(await quitterLEcran())) return;
    if (!(await avantDeQuitter())) return;
    api.accueil();
  };
}
window.addEventListener("pywebviewready", fenetreNative);
fenetreNative();

