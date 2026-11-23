/* Le mois : le choisir, le lire, ses cartes (`js/carte.js`), ce que l'aperçu en dit, ses engagements importés. */
"use strict";

/* UNE ERREUR DU MOTEUR, LISIBLE : la première ligne du message dit quoi faire ; le reste — la règle, la commande — se
 * déplie si on le veut. */
function erreurEnClair(e) {
  const lignes = String(e.message || e).split("\n").map((l) => l.trim()).filter(Boolean);
  const tete = lignes.shift() || "Erreur";
  return `${echapper(tete)}${lignes.length ? `<details class="discret"><summary>en savoir plus</summary>
    ${lignes.map((l) => `<div>${echapper(l)}</div>`).join("")}</details>` : ""}`;
}

/** Un net payé : vide tant que le prélèvement à la source ne l'est pas (la paie n'est pas encore validée). */
function netPaye(v) {
  return v == null ? "—" : euros(v);
}

/* LE MOIS, EN DEUX NOMBRES : ce qu'on vire (le net) et ce que le mois coûte à la compagnie, puis une ligne par salarié,
 * son net, le coût et le brut au survol. Le moteur les somme (`total_net`, `total_cout`, `par_salarie` de l'aperçu). */
function vueApercu(a) {
  const groupes = a.par_salarie;
  // Ce que les chiffres ne comptent pas (un engagement à compléter, ou aucun), le moteur le dit (`phrase_du_mois`).
  const phrase = a.phrase_du_mois ? `<p class="discret">${echapper(a.phrase_du_mois)}</p>` : "";
  if (!groupes.length) return phrase;
  return `
    <div class="mois-chiffres">
      <div><span>${echapper(a.total_net_dit)}</span><b>${netPaye(a.total_net)}</b></div>
      <div><span>${echapper(a.total_cout_dit)}</span><b>${euros(a.total_cout)}</b></div>
    </div>
    <ul class="mois-salaries">${groupes.map((g) => `
      <li title="${echapper(g.survol_dit)}">
        <span>${echapper(g.nom)}
          <span class="discret">· ${echapper(g.engagements_dit)}</span></span>
        <b>${netPaye(g.net_paye)}</b>
      </li>`).join("")}
    </ul>${phrase}`;
}

/** L'APERÇU DU MOIS (`apercu`), et la paie de chaque carte (`poserLesPaies`). `lu` : l'aperçu tel que la lecture du
 *  mois le porte (la saisie enregistrée, sans modification en cours) ; sans lui, on le demande au moteur. */
async function calculerApercu(lu) {
  const cible = document.getElementById("apercu");
  const mois = MOIS_AFFICHE;
  if (!mois) return;
  cible.classList.add("attente");
  try {
    const modifications = modificationsAuMoteur();
    const a = lu && !modifications.length ? lireReponse(lu)
      : await aSonTour(() => api("apercu", { mois, modifications }));
    if (MOIS_AFFICHE !== mois) return;
    // Un mois sans engagement d'ici : ce que la route `saisie` en dit (`vide_dit`).
    cible.innerHTML = a.paies.length ? vueApercu(a) : `<p class="discret">${echapper(VIDE_DIT)}</p>`;
    poserLesPaies(a);
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${erreurEnClair(e)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/* ------------------------------------------------------- charger un mois */
let MOIS_AFFICHE = null;        // le mois REELLEMENT charge dans l'ecran
let DEMANDE_MOIS = null;

/** Lit le mois et le dessine : ses cartes, ses importés, ses contrats repris, l'aperçu, les étapes, les imprévus, l'état
 *  de chaque engagement. `repeint` : l'état frais d'un mois déjà affiché, après une écriture. Rend la fin des lectures
 *  (le démarrage l'attend pour montrer la page peinte). */
async function chargerMois(mois, repeint = false) {
  DEMANDE_MOIS = mois;
  const cible = document.getElementById("engagements");
  let lu;
  try { lu = await lireLeMois(mois, !repeint); } catch (e) { if (DEMANDE_MOIS === mois) cible.classList.remove("en-lecture"); throw e; }
  if (DEMANDE_MOIS !== mois) return;
  if (!lu.perime) moisConfirme(mois);
  const r = await api("saisie", { mois });
  // Un mois plus récent a été demandé pendant que celui-ci arrivait : on le laisse tomber.
  if (DEMANDE_MOIS !== mois) return;
  SAISIE = r.saisie;
  IMPORTES = r.importes;
  NOUVELLE_CARTE = r.nouvelle_carte;
  VIDE_DIT = r.vide_dit;
  MOIS_AFFICHE = mois;
  if (!repeint) oublierLesBulles();
  document.getElementById("mois").value = mois;
  afficherMoisChoisi();
  const defilement = window.scrollY;
  poserLesCartes(r);
  cible.classList.remove("en-lecture");
  if (repeint) window.scrollTo(0, defilement);
  await poserContratsEnCours(mois);
  if (DEMANDE_MOIS !== mois) return;
  majAvisDuMois(r.demo, r.ferme_aux_neufs ? "" : r.phrase);
  dessinerMoisImporte(r.ferme_aux_neufs, r);
  document.getElementById("resultat").innerHTML = "";
  calculerApercu(lu.lectures.apercu);
  const peint = Promise.allSettled([dessinerActions(mois), dessinerImprevus(mois), dessinerEtatsDesEngagements(mois)]);
  // UN ÉTAT PÉRIMÉ SE VÉRIFIE DERRIÈRE : le moteur le refait déjà ; l'écran se repeint de lui-même s'il a changé.
  if (lu.perime && !repeint) verifierLeMois(mois, peint);
  return peint;
}

/** L'ÉTAT FRAIS D'UN MOIS DÉJÀ AFFICHÉ. Rien n'a changé : rien ne bouge. Sinon le mois se relit, sauf pendant une
 *  écriture : alors seul ce que le moteur en dit se repeint. */
async function repeindreLeMois(mois, ancien, frais) {
  const memes = (k) => JSON.stringify(ancien.lectures[k]) === JSON.stringify(frais.lectures[k]);
  const cles = new Set([...Object.keys(ancien.lectures), ...Object.keys(frais.lectures)]);
  if ([...cles].every(memes)) return;
  if (toutEstEcrit()) { await chargerMois(mois, true); return; }
  await Promise.allSettled([dessinerActions(mois), dessinerImprevus(mois), dessinerEtatsDesEngagements(mois)]);
}

/** Ce que l'écran dit d'un mois, au-dessus de ses engagements : la mention d'un mois saisi en démo et la phrase que le
 *  moteur rend (`saisie`), rien pour un mois importé. */
function majAvisDuMois(demo, phrase) {
  majAvisDemo(demo);
  noteSaisie(phrase);
}

/** UN MOIS SAISI EN DÉMO, dans une compagnie gardée au déblocage : la mention du moteur (route `saisie`, `demo`). */
function majAvisDemo(demo) {
  const avis = document.getElementById("avis-demo");
  avis.hidden = !demo;
  if (demo) avis.textContent = demo;
}

/* LES ENGAGEMENTS IMPORTÉS QUI TOUCHENT LE MOIS (`importes` de la route `saisie`, `import::mois::cartes`) : une carte
 * fermée par engagement, comme celle d'un engagement validé : quand, qui et pour quoi, ses états, son net ; un clic sur
 * la carte montre chaque case remplie de ce que l'import a lu, avec d'où elle vient. Ce que l'import n'a pas pu lire
 * se dit sur la carte, case par case, et pourquoi. La page n'en décide rien : elle affiche. */
let IMPORTES = [];

/* CE QU'UNE CARTE NEUVE DU MOIS PEUT ÊTRE (`nouvelle_carte` de la route `saisie`) : sa nature d'ouverture, les natures
 * que le mois propose, si elle se choisit, et l'aide d'un mois vide. Le moteur le dit, mois par mois. */
let NOUVELLE_CARTE = null;

/* CE QUE DIT UN MOIS SANS PAIE D'ICI (`vide_dit` de la route `saisie`). */
let VIDE_DIT = null;

const CASES_IMPORTEES = [
  ["brut", "Brut"], ["net_imposable", "Net imposable"], ["net_a_payer", "Net à payer"],
  ["pas", "Prélèvement à la source"], ["net_verse", "Net versé (DSN)"],
  ["cotisations_salariales", "Cotisations salariales"], ["cotisations_patronales", "Cotisations patronales"],
  ["cout_employeur", "Coût employeur"],
];

/** `ferme` : la phrase du moteur quand le mois ne reçoit pas d'engagement neuf (`ferme_aux_neufs` de la route `saisie`) ;
 *  « + un engagement » se cache, la phrase se lit à sa place. `r` : la réponse de `saisie` (son en-tête, `importes_titre`
 *  et `importes_dit`). */
function dessinerMoisImporte(ferme, r) {
  const cible = document.getElementById("mois-importe");
  document.getElementById("ajouter-engagement").hidden = !!ferme;
  if (!IMPORTES.length) { cible.hidden = true; cible.innerHTML = ""; return; }
  const moisCourt = MOIS_COURTS[parseInt(MOIS_AFFICHE.slice(5, 7), 10) - 1];
  cible.innerHTML = `
    <p class="mi-tete"><b>${echapper(r.importes_titre)}</b> <span class="discret">— ${echapper(r.importes_dit)}</span></p>
    ${IMPORTES.map((p, i) => carteImportee(p, i, moisCourt)).join("")}
    <p class="discret mi-ferme"></p>`;
  // `textContent` est nullable : un mois ouvert aux engagements neufs ne dit rien ici.
  cible.querySelector(".mi-ferme").textContent = ferme;
  ouvrirLesCartesImportees(cible);
  cible.hidden = false;
}

/** La carte s'ouvre d'un clic sur elle (`rendreOuvrable`) : ses cases se déplient, un second clic les replie. */
function ouvrirLesCartesImportees(cible) {
  cible.querySelectorAll(".carte-importee").forEach((div) => {
    const cases = div.querySelector(".ci-cases");
    rendreOuvrable(div.querySelector(".e-resume"), () => {
      cases.hidden = !cases.hidden;
      div.classList.toggle("ouverte", !cases.hidden);
    }, "Ses cases");
  });
}

function carteImportee(p, i, moisCourt) {
  // QUAND : les jours travaillés que le bulletin imprime ; sans eux, la période du contrat, telle que le moteur la dit.
  const jours = p.dates.length ? joursEnClair(p.dates) : p.periode;
  const pastille = (cle, titre) => {
    const e = p.etats[cle];
    return `<span class="etat-acte ${e.couleur}" title="${echapper(e.phrase)}">
      <i class="carre"></i>${echapper(titre)}</span>`;
  };
  // LE MONTANT : le net à payer ; sans lui (une DSN seule ne le déclare pas), le brut, et la carte le dit.
  const net = p.montants.net_a_payer ? euros(p.montants.net_a_payer)
    : p.montants.brut ? `<span class="discret">brut</span> ${euros(p.montants.brut)}` : "";
  // CE QUI MANQUE SE DIT SUR LA CARTE, en une ligne (quels champs) ; le pourquoi est sous chaque case vide.
  const manque = p.manque.length ? `
    <div class="ci-manque"><b>Non lu par l'import</b> ${p.manque.map((m) => echapper(m.libelle)).join(" · ")}
      <span class="discret">— ${aideDe("importe_manque")}</span></div>` : "";
  const dits = new Set();
  const casePlate = (libelle, valeur, source, champ) => {
    const absent = valeur ? null : p.manque.find((m) => m.champ === champ);
    const pourquoi = absent && !dits.has(champ) ? (dits.add(champ), `<span class="ci-pourquoi">${echapper(absent.phrase)}</span>`) : "";
    return `<div class="ci-case${valeur ? "" : " vide"}${absent ? " non-lu" : ""}"><span class="champ">${echapper(libelle)}</span>
      <span class="ci-valeur">${valeur ? echapper(valeur) : `<i>${echapper(absent ? absent.dit : "—")}</i>`}</span>${
      valeur && source ? `<span class="ci-source">${echapper(source)}</span>` : ""}${pourquoi}</div>`;
  };
  // Une case qu'une ligne du bulletin n'imprime pas reste vide.
  const cellule = (v) => (v ? echapper(v) : "");
  const lignes = p.lignes.length ? `
    <table class="ci-lignes"><tr><th>Ligne du bulletin</th><th class="nombre">Quantité</th><th class="nombre">Taux</th><th class="nombre">Montant</th></tr>
      ${p.lignes.map((l) => `<tr><td>${cellule(l.libelle)}</td><td class="nombre">${cellule(l.quantite)}</td>
        <td class="nombre">${cellule(l.taux)}</td><td class="nombre">${l.montant ? echapper(euros(l.montant)) : ""}</td></tr>`).join("")}
    </table>` : "";
  return `
    <div class="carte-importee${p.manque.length ? " incomplete" : ""}" data-rang="${i}">
      <div class="e-resume">
        <div class="carte-quand" title="${echapper(p.dates.length ? p.dates.map(jjmmaaaa).join(", ") : ETAT.aides.importe_dates)}">
          <b class="${jours.includes(" · ") || jours.includes(" – ") ? "epars" : ""}">${echapper(jours)}</b><span>${echapper(moisCourt)}</span></div>
        <div class="carte-qui"${p.origine_dit ? ` title="${echapper(p.origine_dit)}"` : ""}>
          <span class="carte-nom"><b>${cellule(p.nom)}</b> <span class="mot-etat">importé</span></span>
          <span class="discret">${echapper(p.quoi_dit)}</span></div>
        <div class="carte-actes"><div class="etats-actes">${pastille("dpae", "DPAE")}${pastille("signature", "Contrat")}${pastille("documents", "Documents")}</div></div>
        <div class="carte-net" title="${p.net_titre ? echapper(p.net_titre) : aideDe("importe_brut")}">${net}</div>
      </div>
      ${manque}
      <div class="ci-cases" hidden>
        <div class="ci-grille">
          ${casePlate("Salarié", p.nom, "fiche", "salarie")}
          ${casePlate("Spectacle", p.spectacle_dit, "", "spectacle")}
          ${casePlate("Contrat", p.contrat, "", "contrat")}
          ${casePlate("Début", p.debut ? jjmmaaaa(p.debut) : "", "", "debut")}
          ${casePlate("Fin", p.fin ? jjmmaaaa(p.fin) : "", "", "fin")}
          ${casePlate("Jours travaillés", p.dates.map((d) => d.slice(8, 10)).join(" · "), "bulletin", "dates")}
          ${casePlate("Cachets", p.cachets != null ? String(p.cachets) : "", p.atteste_par, "cachets")}
          ${casePlate("Heures", p.heures != null ? String(p.heures) : "", p.atteste_par, "cachets")}
          ${CASES_IMPORTEES.map(([k, mot]) => casePlate(mot, p.montants[k] ? euros(p.montants[k]) : "",
              p.sources[k], k)).join("")}
        </div>
        ${lignes}
      </div>
    </div>`;
}

/* Changer de mois : un écran qui porte une saisie non enregistrée se quitte d'abord (`quitterLEcran` : rester garde le
 * mois montré) ; les écritures en route finissent (chaque case changée part tout de suite), le nouveau mois se lit. */
document.getElementById("mois").onchange = async (e) => {
  if (!(await quitterLEcran())) {
    e.target.value = MOIS_AFFICHE;
    afficherMoisChoisi();
    document.getElementById("engagements").classList.remove("en-lecture");
    return;
  }
  fermerLesCalendriers();
  chargerMois(e.target.value);
};

/* LE CHOIX DU MOIS. « Un truc simple, qu'on puisse cliquer sur
 * toute la case pour arriver aux choix, et que le défilement soit plus
 * rapide. » Une case : ‹ Octobre 2026 ›. Un clic sur une flèche charge le mois
 * TOUT DE SUITE, sans attendre de voir si un autre clic
 * suit) ; un clic qui arrive pendant le chargement le fait abandonner
 * (`DEMANDE_MOIS`, chargerMois), et c'est le dernier mois demandé qui se
 * peint. Un clic sur le nom ouvre l'année : douze mois, un clic pour y
 * aller. Un point marque les mois qui ont une saisie.
 *
 * La valeur reste dans le champ caché #mois et passe par son événement
 * « change ». */
let ANNEE_GRILLE = null;

/** Le nom dans la case — appelé partout où la valeur change. */
function afficherMoisChoisi() {
  const x = moisLu(document.getElementById("mois").value);
  const nom = document.getElementById("mois-nom");
  nom.textContent = x ? `${MOIS_FR[x[1] - 1].replace(/^./, (c) => c.toUpperCase())} ${x[0]}` : "—";
}

/** Aller à un mois : le nom et le chargement, tout de suite. */
function allerAuMois(v) {
  const c = document.getElementById("mois");
  c.value = v;
  afficherMoisChoisi();
  // LE NOM ET LES CARTES DISENT LE MÊME MOIS : dès que le nom change,
  // les cartes de
  // l'ancien mois s'effacent ; elles reviennent si l'on revient dessus.
  document.getElementById("engagements").classList.toggle("en-lecture", v !== MOIS_AFFICHE);
  // Revenir au mois affiché pendant qu'un autre se charge, c'est aussi le
  // demander : sans quoi l'autre finirait de se peindre sous ce nom-ci.
  if (v !== MOIS_AFFICHE || DEMANDE_MOIS !== MOIS_AFFICHE) c.dispatchEvent(new Event("change"));
}

function moisVoisin(pas) {
  const x = moisLu(document.getElementById("mois").value);
  if (!x) return;
  const d = new Date(x[0], x[1] - 1 + pas, 1);
  allerAuMois(moisEnTexte(d.getFullYear(), d.getMonth() + 1));
}

function dessinerGrilleMois() {
  const grille = document.getElementById("mois-grille");
  const choisi = document.getElementById("mois").value;
  const ceMois = aujourdhuiIso().slice(0, 7);   // le mois du moteur
  const saisis = new Map(ETAT.historique.map((h) => [h.mois, h.saisi_dit]));
  grille.innerHTML = `
    <div class="choix-mois-annee">
      <button type="button" class="choix-mois-pas" data-annee="-1" aria-label="Année précédente">&lsaquo;</button>
      <b>${ANNEE_GRILLE}</b>
      <button type="button" class="choix-mois-pas" data-annee="1" aria-label="Année suivante">&rsaquo;</button>
    </div>
    <div class="choix-mois-douze">${MOIS_ABR.map((n, i) => {
      const v = moisEnTexte(ANNEE_GRILLE, i + 1);
      const classes = [v === choisi ? "choisi" : "", v === ceMois ? "maintenant" : "",
                       saisis.has(v) ? "saisi" : ""].join(" ");
      return `<button type="button" class="${classes}" data-mois="${v}"
                title="${MOIS_FR[i]} ${ANNEE_GRILLE}${saisis.has(v) ? " — " + saisis.get(v) : ""}">${n}</button>`;
    }).join("")}</div>`;
  grille.querySelectorAll("[data-annee]").forEach((b) => {
    b.onclick = (e) => { e.stopPropagation(); ANNEE_GRILLE += +b.dataset.annee; dessinerGrilleMois(); };
  });
  grille.querySelectorAll("[data-mois]").forEach((b) => {
    b.onclick = () => { fermerGrilleMois(); allerAuMois(b.dataset.mois); };
  });
}

function ouvrirGrilleMois() {
  const x = moisLu(document.getElementById("mois").value);
  ANNEE_GRILLE = x ? x[0] : anneeDuJour();
  dessinerGrilleMois();
  document.getElementById("mois-grille").hidden = false;
  document.getElementById("mois-nom").setAttribute("aria-expanded", "true");
}
function fermerGrilleMois() {
  document.getElementById("mois-grille").hidden = true;
  document.getElementById("mois-nom").setAttribute("aria-expanded", "false");
}

document.getElementById("mois-avant").onclick = () => moisVoisin(-1);
document.getElementById("mois-apres").onclick = () => moisVoisin(1);
document.getElementById("mois-nom").onclick = () =>
  (document.getElementById("mois-grille").hidden ? ouvrirGrilleMois() : fermerGrilleMois());
// Les flèches du clavier sur la case font défiler, comme les boutons.
document.getElementById("mois-nom").onkeydown = (e) => {
  if (e.key === "ArrowLeft") { e.preventDefault(); moisVoisin(-1); }
  if (e.key === "ArrowRight") { e.preventDefault(); moisVoisin(1); }
};
document.addEventListener("click", (e) => {
  if (!e.target.closest("#choix-mois")) fermerGrilleMois();
});
document.addEventListener("keydown", (e) => { if (e.key === "Escape") fermerGrilleMois(); });

/* ------------------------------------------- l'enregistrement
   Ce qui est tapé dans une case EST la saisie : chaque case changée part tout de suite à sa route
   (`js/carte.js`), et la ligne d'état dit l'heure que le moteur a écrite (« Enregistré à 14:32 »). Rien ne part pour
   autant : enregistrer, c'est écrire les engagements, et rien d'autre. */

/** La ligne d'état de la saisie. */
function noteSaisie(texte) {
  document.getElementById("note-saisie").textContent = texte;
}

/** Ce que d'autres écrans arrêtent avant d'écrire (le démarrage, les imprévus) : rien n'attend plus une seconde. */
let MINUTEUR_SAISIE = null;

/** UN APPEL AU MOTEUR À LA FOIS pour les cartes du mois : les écritures et l'aperçu se suivent. Chacun attend la fin du
 *  précédent : une carte neuve dont l'écriture est en route n'est pas écrite deux fois. */
let ENREGISTREMENT = Promise.resolve(false);
let EN_ROUTE = 0;
function aSonTour(appel) {
  EN_ROUTE += 1;
  const tour = ENREGISTREMENT.then(appel).finally(() => { EN_ROUTE -= 1; });
  // La file continue après un échec : celui-ci revient à qui a demandé ce tour (`tour`), qui le dit.
  ENREGISTREMENT = tour.catch(() => false);
  return tour;
}

/** Attend que ce qui est en route soit écrit. */
function enregistrerSaisie() {
  return ENREGISTREMENT;
}

/** Tout ce qui est à l'écran est-il écrit ? */
function toutEstEcrit() {
  return EN_ROUTE === 0;
}

/* ------------------------------------------- ce que d'autres écrans prennent ici */

/** Le catalogue des métiers, tel que le moteur le rend (route « etat »), groupé par famille dans SON ordre. */
function metierDe(code) {
  return ETAT.emplois.find((m) => m.code === code) || null;
}

/** Une fiche neuve : le formulaire complet de Configuration, avec ses vérifications. Une seule façon de créer une
 *  fiche, et c'est là. Une fiche ouverte et modifiée demande d'abord, comme toute sortie d'un écran (`quitterLEcran`). */
async function nouvelleFiche(nom) {
  if (!(await quitterLEcran())) return;
  // Les onglets redessinent APRES coup (leurs gestionnaires attendent `quitterLEcran`) : sans cette marque,
  // `dessinerConfig` effacait la fiche neuve qu'on venait d'ouvrir.
  const onglet = document.querySelector('.onglets button[data-vue="configuration"]');
  const sous = document.querySelector(`.sous-onglets button[data-fiche="${nom}"]`);
  if (!onglet.classList.contains("actif")) onglet.click();
  if (!sous.classList.contains("actif")) sous.click();
  CLE_ACTIVE = null;
  FICHE_NEUVE = true;
  dessinerConfig();
  // Une fiche neuve n'a pas de clé : la route `fiche` la rend.
  dessinerFormulaire({ nom, cle: null });
  document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
}
