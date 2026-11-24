/* Le formulaire d'une fiche : l'ouvrir telle que la route la rend, lire, enregistrer, dater. */
"use strict";

/* Ce que l'écran dit de chaque valeur d'une liste de choix : les `libelles` du champ, tels que la description des fiches
 * les rend (`donnees/champs-base.json`) ; `""` est la ligne vide, quand elle vaut le cas ordinaire. */
function libellesDe(cle) {
  const d = ETAT.configuration.description;
  for (const nom of ["entreprise", "salarie", "spectacle"]) {
    const c = d[nom].find((x) => x.cle === cle);
    // Un champ sans libellés (`libelles` absent) montre ses valeurs telles quelles.
    if (c) return c.libelles ? c.libelles : {};
  }
  return {};
}

/* Ce qu'une case du formulaire montre : la valeur de la fiche, et rien quand la fiche ne la dit pas (un champ facultatif
 * absent). C'est la seule règle d'affichage d'une absence, ici. */
const texteDeLaCase = (v) => (v == null ? "" : String(v));

/* LES CASES D'UNE STRUCTURE, telles que le moteur les décrit (`champs` de la description) : `{champs: [[clé, libellé,
 * aide, genre]]}`, ou null. Une case déclarée qui se déduit d'une autre (`ecran: false`) ne se montre pas ; le genre
 * dit les libellés d'un choix. */
function sousChamps(c) {
  // Une structure dit ses cases (`champs`) ; un champ seul n'en a pas.
  if (!c || !c.champs || !c.champs.length) return null;
  const genre = (s) => {
    // Une case de choix dit ses libellés quand elle en a (`libelles`) ; la ligne vide se montre « — » sans eux.
    const lib = s.libelles ? s.libelles : {};
    const g = {};
    if (s.choix && s.choix.length) {
      const vide = "" in lib || !s.choix.includes("") ? [["", "" in lib ? lib[""] : "—"]] : [];
      g.choix = Object.fromEntries([...vide, ...s.choix.filter((v) => v !== "").map((v) => [v, lib[v] ?? v])]);
    }
    return g;
  };
  return { champs: c.champs.filter((s) => s.ecran !== false).map((s) => [s.cle, s.libelle, s.aide, genre(s)]) };
}

/* La structure `cle` de la fiche `nom` (par défaut, celle qu'on montre), case par case (`sousChamps`). */
function structureDecrite(cle, nom = FICHE_ACTIVE) {
  return sousChamps(ETAT.configuration.description[nom].find((c) => c.cle === cle));
}

/* Une fiche s'ouvre telle que la route la rend (`ouvrirLaFicheRendue`), quelle qu'elle soit. */
function dessinerFormulaire(fiche) {
  if (!fiche) return;
  return ouvrirLaFicheRendue(fiche);
}

/* LA FICHE QUE LA ROUTE REND (`fiche`), dessinée par le dessinateur commun (`js/avance.js`, `poserLaFiche`) : son en-tête,
 * tel que la route le dit (titre, ce qui le suit, sa note, où lire le coût de l'année, si « Annuler » se montre ; pour un
 * salarié, ses documents, ses imprévus et ses droits, ou la demande qui fait remplir une fiche neuve), l'encart de ses
 * erreurs, ses cases ouvertes, « Annuler » et « Enregistrer ». Ce qu'on tape repart à la même route, 400 ms après la
 * dernière frappe ou à chaque changement ; sa réponse dit les cases, leurs fautes, l'encart, et ce qu'elle a récrit.
 * L'écriture est celle de toute fiche (`ecrireFiche`). `fiche` : la fiche de la liste (sa `cle`), ou une neuve. */
let FICHE_SANS_LISTE = false;        // la fiche ouverte n'a pas de liste où revenir (la route : pas d'« Annuler »)
let NOTE_APRES_OUVERTURE = null;     // ce que la fiche qui se rouvre dit sous « Enregistrer » (l'heure de l'écriture)

async function ouvrirLaFicheRendue(fiche) {
  FICHE_COURANTE = fiche;
  const note = NOTE_APRES_OUVERTURE;
  NOTE_APRES_OUVERTURE = null;
  const form = document.getElementById("config-formulaire");
  const charge = fiche.cle ? { nom: FICHE_ACTIVE, cle: fiche.cle } : { nom: FICHE_ACTIVE };
  let r;
  // Une fiche qui ne s'ouvre pas ne laisse pas celle d'avant à l'écran sous son nom.
  try { r = await api("fiche", charge); } catch (e) { if (FICHE_COURANTE === fiche) form.innerHTML = ""; dire(e.message, "erreur"); return; }
  // Une autre fiche s'est ouverte pendant que la route répondait : c'est elle qui se dessine.
  if (FICHE_COURANTE !== fiche) return;
  FICHE_SANS_LISTE = !r.annuler;
  dessinerSuppression();
  const encart = (x) => (x.erreurs_titre ? `<div class="encart erreur"><b>${echapper(x.erreurs_titre)}</b> — ${echapper(x.erreurs_note)}</div>` : "");
  const s = r.salarie;
  const inv = r.invitation;
  form.innerHTML = `
    <div class="carte" style="margin-top:16px">
      <div class="entete-fiche">
        ${r.titre ? `<h2>${echapper(r.titre)}${r.sous_titre ? ` <span class="discret">— ${echapper(r.sous_titre)}</span>` : ""}</h2>` : ""}
        ${s ? `<button type="button" class="secondaire petit" data-exporter>${echapper(s.exporter)}</button>` : ""}
      </div>
      ${r.cout ? `<div class="cout-annee" data-cote="${echapper(r.cout.cote)}" data-cout-de="${echapper(r.cout.de)}"></div>` : ""}
      ${s ? `<div class="fiche-imprevus" id="fiche-imprevus"></div><div class="fiche-conseils" id="fiche-conseils"></div>` : ""}
      ${inv ? `<div class="encart invitation">
        <h3>${echapper(inv.titre)}</h3>
        <p class="aide">${echapper(inv.aide)}</p>
        ${inv.geste ? `<div data-invitation>${casesDuGeste(inv.geste)}</div>
          <div class="actions"><button type="button" class="primaire" data-inviter>${echapper(inv.geste.libelle)}</button></div>`
          : `<p class="note">${echapper(inv.sans_boite)}</p>`}
      </div>` : ""}
      ${r.note ? `<p class="note">${echapper(r.note)}</p>` : ""}
      <div data-fiche-erreurs>${encart(r)}</div>
      <div data-fiche-cases></div>
      <div class="actions">
        <button class="secondaire" id="config-annuler"${r.annuler ? "" : " hidden"}>${aideDe("fiche_annuler")}</button>
        <button class="primaire" id="config-enregistrer">${aideDe("fiche_enregistrer")}</button>
        <span class="note" id="config-modifie"${note ? "" : " hidden"}>${note ? echapper(note) : aideDe("fiche_modifiee")}</span>
      </div>
    </div>`;
  const cases = form.querySelector("[data-fiche-cases]");
  poserLaFiche(cases, r);
  let tour = 0;
  let minuterie = null;
  // Les cases qui viennent de changer, de la plus ancienne à la plus récente, chacune au tour de sa dernière frappe :
  // elles partent à chaque relecture, et s'oublient quand une réponse qui les portait se dessine.
  const changees = new Map();
  const relire = async () => {
    const n = ++tour;
    let r2;
    try {
      r2 = await api("fiche", { ...charge, ...casesDeLaCharge(lireLesCases(cases)), changees: [...changees.keys()] });
    } catch (e) { dire(e.message, "erreur"); return; }
    if (n !== tour || !cases.isConnected) return;
    for (const [k, t] of [...changees]) if (t <= n) changees.delete(k);
    form.querySelector("[data-fiche-erreurs]").innerHTML = encart(r2);
    poserLaFiche(cases, r2);
  };
  const changer = (e) => {
    const c = e.target.closest && e.target.closest(".case[data-cle]");
    if (c) { changees.delete(c.dataset.cle); changees.set(c.dataset.cle, tour + 1); }
    clearTimeout(minuterie);
    minuterie = setTimeout(relire, 400);
  };
  cases.addEventListener("input", changer);
  cases.addEventListener("change", changer);
  document.getElementById("config-enregistrer").onclick = () => enregistrerFiche(fiche);
  document.getElementById("config-annuler").onclick = annulerFiche;
  if (s) {
    form.querySelector("[data-exporter]").onclick = () => exporterSalarie(s.cle);
    dessinerImprevusDuSalarie(s.cle);
    dessinerConseilsDuSalarie(s.cle);
  }
  if (inv && inv.geste) {
    const bloc = form.querySelector("[data-invitation]");
    brancherLesCases(bloc);
    form.querySelector("[data-inviter]").onclick = (ev) => demanderFiche(ev.currentTarget, inv, bloc);
  }
  peindreCoutAnnee();
  // Quitter la fiche avec une saisie non enregistrée se demande (`retenirLEcran`) : jeter la reprend telle qu'elle est
  // enregistrée ; enregistrer peut être annulé dans la boîte de ce qui change, et l'on reste.
  retenirLEcran(cases, { enregistrer: () => enregistrerFiche(fiche), jeter: () => dessinerFormulaire(fiche), quandChange: montrerModifie });
}

/* CE QUE LA FICHE MONTRE QUAND ON Y TAPE : la mention « modifié », et « Annuler », qu'une fiche sans liste (Entreprise)
 * ne montre qu'après un changement ; revenu à ce qui était, elles s'effacent. */
function montrerModifie(change) {
  const t = document.getElementById("config-modifie");
  const annuler = document.getElementById("config-annuler");
  if (!change) {
    if (t) t.hidden = true;
    if (annuler && FICHE_SANS_LISTE) annuler.hidden = true;
    return;
  }
  if (t) { t.hidden = false; t.textContent = ETAT.aides.fiche_modifiee; }
  if (annuler) annuler.hidden = false;
}

/* OÙ UNE CASE RENVOIE (`renvoi` de la case, rendu par la route) : le sous-onglet des métiers, ou la case d'une autre
 * fiche (`allerAuManque`). */
document.addEventListener("click", (e) => {
  const b = e.target.closest && e.target.closest("[data-renvoi]");
  if (!b) return;
  const r = JSON.parse(b.dataset.renvoi);
  if (r.vers === "metiers") document.querySelector('.sous-onglets button[data-fiche="metiers"]').click();
  else allerAuManque(r.case, r.fiche);
});

/* Ce que dit la fiche ouverte, telle que la route la rend (`lireLesCases`) ; aucune fiche ouverte, rien. */
function lireLaFicheOuverte() {
  const rendue = casesDeLaFicheOuverte();
  return rendue ? lireLesCases(rendue) : {};
}

/* Le bloc des cases de la fiche ouverte ; aucune fiche ouverte, rien. */
function casesDeLaFicheOuverte() {
  return document.querySelector("#config-formulaire [data-fiche-cases]");
}

/* Le salarie a choisir dans « Le dossier d'un salarié », a l'arrivee dans
 * l'onglet Exports (`dessinerDossier` le lit, puis l'oublie). */
let DOSSIER_PRECHOIX = null;

async function exporterSalarie(cle) {
  if (!(await quitterLEcran())) return;
  DOSSIER_PRECHOIX = cle;
  // L'onglet se dessine a son arrivee, et `dessinerDossier` y pose le choix.
  document.querySelector('.onglets button[data-vue="exports"]').click();
}

/* LA DEMANDE DE LA FICHE À LA PERSONNE (`invitation` de l'en-tête d'un salarié neuf) : le geste que la route rend
 * (`fiche-demander`, ses cases dans `bloc`), après le mot de passe du logiciel que la route dit demander. */
async function demanderFiche(bouton, inv, bloc) {
  const charge = chargeDuGeste(bloc, inv.geste) || inv.geste.charge;
  const mdp = await demanderMotDePasse(inv.demande, { envoi: true });
  if (!mdp) return;
  const rendre = occupe(bouton, "Envoi…");
  const repere = await repereCourrielsFictifs();
  try {
    // Un prénom non tapé part absent : le courriel dit alors « Bonjour, ».
    const r = await api(inv.geste.route, { ...charge, motdepasse: mdp });
    // La réponse ne porte plus la configuration (ni NIR ni IBAN) : la page la relit.
    ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
    oublierLEcran(casesDeLaFicheOuverte());
    CLE_ACTIVE = null;
    FICHE_NEUVE = false;
    dessinerConfig();
    dire(r.phrase, "envoye");
    fenetreCourrielsFictifs("fiche", repere);
  } catch (e) {
    rendre();
    dire(e.message, "erreur");
  }
}

/* Les demandes parties et pas revenues, sous la liste des salaries. */
function fichesAttenduesHtml() {
  const l = ETAT.configuration.fiches_attendues;
  if (!l.length) return "";
  return `<div class="encart attente-fiches">
    <b>${echapper(ETAT.configuration.fiches_attendues_titre)}</b>
    <button class="lien" id="fiches-relever">relever maintenant</button>
    <ul>${l.map((a) => `<li>${echapper(a.prenom ? a.prenom + " — " : "")}${
        echapper(a.adresse)} <span class="discret">${echapper(a.demande_dit)}</span>
      <button class="lien" data-oublier-fiche="${echapper(a.adresse)}">ne plus attendre</button></li>`).join("")}</ul>
  </div>`;
}

function brancherFichesAttendues() {
  const relever = document.getElementById("fiches-relever");
  if (relever) relever.onclick = async () => {
    relever.disabled = true;
    try {
      const r = await api("fiches-relever", {});
      ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
      dessinerConfig();
      dire(r.phrase, undefined);
    } catch (e) { relever.disabled = false; dire(e.message, "erreur"); }
  };
  document.querySelectorAll("[data-oublier-fiche]").forEach((b) => {
    b.onclick = async () => {
      b.disabled = true;
      try {
        const r = await api("fiche-oublier-demande", { adresse: b.dataset.oublierFiche });
        // La réponse ne porte plus la configuration (ni NIR ni IBAN) : la page la relit.
        ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
        dessinerConfig();
        dire(r.phrase, undefined);
      } catch (e) { b.disabled = false; dire(e.message, "erreur"); }
    };
  });
}

/* Après un passage de la veille (une réponse a pu arriver) : la liste des demandes attendues relue et repeinte en
   place, sans toucher à la fiche ouverte. */
async function relireFichesAttendues() {
  if (!ETAT) return;
  ETAT.configuration = await api("configuration", {}); peindreEtatGlobal();
  const encart = document.querySelector("#config-liste .attente-fiches");
  if (!encart) return;
  encart.outerHTML = fichesAttenduesHtml();
  brancherFichesAttendues();
}

/* UN MONTANT S'ECRIT D'UNE SEULE FACON : « 1 053,08 € ». Le
   moteur envoie des nombres (« 210.80 ») ; la page en avait trois mises en
   forme locales, et une dizaine d'endroits montraient le nombre tel quel --
   la boite de depot DSN disait « 210.80 € » a cote d'un apercu en
   « 1 053,08 € ». Ce qui n'est pas un nombre repart tel quel. */
function euros(v) {
  const c = enEuros(v, 2);
  return c === v.toString().trim() ? c : c + " €";
}

/* LES CHIFFRES D'UN MONTANT, tels que `euros` les écrit (« 1 053,08 ») : deux décimales au moins, au plus `max` (sans
   borne, aucune décimale saisie ne se perd : le montant d'une case). Ce qui n'est pas un nombre repart tel quel. */
function enEuros(v, max = 20) {
  // Un montant absent ne se met pas en forme : il lève (`toString` d'un absent) ; ce qui peut manquer se teste avant.
  const t = v.toString().trim();
  const n = typeof v === "number" ? v : Number(t);
  if (t === "" || !isFinite(n)) return t;
  return n.toLocaleString("fr-FR", { minimumFractionDigits: 2, maximumFractionDigits: max });
}

/* ANNULER, le pendant d'« Enregistrer » : ce
 * qu'on a tapé ne s'écrit pas. Enregistrer referme la fiche et revient à la
 * liste (ecrireFiche) ; Annuler aussi, sans rien écrire : une fiche neuve
 * n'est pas créée, une fiche existante reste telle qu'elle est enregistrée.
 * La fiche Entreprise n'a pas de liste où revenir : Annuler y reprend ce
 * qui est enregistré, et ne se montre que si l'on a changé quelque chose.
 * C'est le « jeter » de la boîte des changements non enregistrés
 * (`quitterLEcran`), demandé d'un geste : il ne redemande pas. */
function annulerFiche() {
  oublierLEcran(casesDeLaFicheOuverte());
  CLE_ACTIVE = null;
  FICHE_NEUVE = false;
  dessinerConfig();
}

/*  ENREGISTRER UNE FICHE : DATER CE QUI CHANGE
 *
 *  Ce qui arrive quand on ne le fait pas : une
 *  commune de naissance a ete corrigee depuis cet ecran ; l'avertissement
 *  disait bien « enregistrer declarera PARIS 12E », et il etait juste. Ce
 *  qu'il ne disait pas, c'est que 55 DSN etaient deja parties avec l'autre
 *  forme, et que l'identite est ce sur quoi France Travail et la DGFiP
 *  rapprochent un salarie.
 *
 *  D'ou cette etape, et les deux choses qu'elle demande :
 *
 *   - A PARTIR DE QUAND la nouvelle valeur vaut. Le moteur propose le
 *     premier mois dont rien n'est parti ; une autre date se choisit, dans
 *     le passe aussi.
 *   - POURQUOI. Une correction et un demenagement se datent pareil ; seule
 *     la note dira, dans deux ans, laquelle des deux c'etait.
 *
 *  Et elle MONTRE ce que l'ancienne valeur a deja servi a declarer, parce
 *  que c'est cette information-la qui fait decider.
 */
async function enregistrerFiche(fiche) {
  return ecrireFiche(fiche, lireLaFicheOuverte(), {});
}

/* Une valeur declaree n'est pas toujours un mot : `signataire` et `banque`
 * sont des structures. `echapper(objet)` donnait « [object Object] → [object
 * Object] », c'est-a-dire un changement qu'on ne peut ni lire ni juger --
 * alors que c'est precisement ce que cette boite demande de juger. */
function valeurLisible(v) {
  if (v === null || v === undefined) return "—";
  if (typeof v === "object") {
    return Array.isArray(v) ? v.map(valeurLisible).join(" · ")
      : Object.entries(v).map(([k, x]) => `${k} : ${valeurLisible(x)}`).join(" · ");
  }
  return String(v);
}

/* UNE STRUCTURE DITE PAR SES CASES (« Prénom : Claire · Nom : Fontaine · Fonction : Présidente ») : celles que la
 * fiche montre (`structureDecrite`), remplies, avec leur libellé et le mot d'un choix ; jamais ses clés. */
function structureLisible(champ, v) {
  const s = structureDecrite(champ);
  return (s ? s.champs : []).map(([k, libelle, , opts]) => {
    const x = k.split(".").reduce((o, p) => (o && typeof o === "object" ? o[p] : undefined), v);
    if (x === undefined || x === null || String(x).trim() === "") return "";
    const mot = opts && opts.choix ? opts.choix[String(x)] : null;
    return `${libelle} : ${mot || valeurLisible(x)}`;
  }).filter(Boolean).join(" · ");
}

/* UNE VALEUR DATÉE D'UNE CASE, lisible : une structure (`declarant_dsn`) se dit case par case, chacune par son libellé
 * de `champs-base.json` -- « Courriel : CONTACT@EXEMPLE.FR » ; une case faite de cases (« taux.t1_cadre ») se dit
 * case par case. Les clés `note` et `_…` sont des notes, pas des valeurs. */
function valeurHistorique(champ, v) {
  if (v === null || typeof v !== "object" || Array.isArray(v)) {
    return valeurLisible(v);
  }
  // Une case de structure se nomme par les libellés de ses cases (`champs` de sa description), et un choix par le sien.
  const decrit = ETAT.configuration.description[FICHE_ACTIVE].find((c) => c.cle === champ);
  const libelles = Object.fromEntries(decrit && decrit.champs ? decrit.champs.map((s) => [s.cle, s.libelle]) : []);
  const s = structureDecrite(champ);
  const choix = Object.fromEntries((s ? s.champs : []).filter(([, , , g]) => g && g.choix).map(([k, , , g]) => [k, g.choix]));
  const mot = (k, x) => (choix[k] && choix[k][String(x)]) || valeurLisible(x);
  const dire = (k, x) => x && typeof x === "object" && !Array.isArray(x) && !libelles[k]
    ? Object.entries(x).map(([s2, y]) => dire(`${k}.${s2}`, y)).join(" · ")
    : `${libelles[k] || k} : ${mot(k, x)}`;
  // Une case que la valeur ne dit pas (le numéro du jour d'une fin de mois) ne se nomme pas.
  return Object.keys(v).filter((k) => !k.startsWith("_") && k !== "note" && v[k] !== null).map((k) => dire(k, v[k])).join(" · ");
}

/* RIEN DE ROUGE. « On a le droit de changer de métier, je ne sais
 * même pas pourquoi on mettrait un avertissement. » Un changement de fiche est
 * un geste normal : la boîte ne demande que la date d'effet — c'est elle qui
 * garde aux déclarations déjà faites leur ancienne valeur (fiches datées) —,
 * sur un ton neutre. Le métier se dit en UNE ligne : les cases que le métier
 * décide changent ensemble (la route les marque : `metier`), ce sont plusieurs
 * écritures d'un seul fait.
 * `usage` : la phrase que le moteur fait confirmer (le jour fixe des
 * intermittents) ; sans rien à dater, la boîte ne demande pas de date. */
function demanderDateDEffet({ changements = [], depuis: propose, pourquoi, usage }) {
  const metier = changements.filter((c) => c.metier);
  const autres = changements.filter((c) => !c.metier);
  // Une case à choix se dit par le libellé que la fiche montre (« Régisseur de théâtre »), pas par son code.
  const lisible = (c, cote) => {
    const v = c[cote + "_lisible"] ?? c[cote];
    if (v && typeof v === "object" && !Array.isArray(v) && structureDecrite(c.champ)) {
      return structureLisible(c.champ, v);
    }
    const nom = libellesDe(c.champ)[texteDeLaCase(v)];
    return nom && texteDeLaCase(v) ? nom : valeurLisible(v);
  };
  const ligne = (titre, avant, apres, detail = "") => `<div class="changement">
      <div class="changement-titre">${echapper(titre)}</div>
      <div class="changement-valeurs">
        <span class="avant">${String(avant).trim() ? echapper(avant) : `<i class="discret">${aideDe("valeur_vide")}</i>`}</span>
        <span class="fleche">→</span>
        <span class="apres">${String(apres).trim() ? echapper(apres) : `<i class="discret">${aideDe("valeur_effacee")}</i>`}</span>
      </div>${detail ? `<div class="changement-prix">${detail}</div>` : ""}
    </div>`;
  let blocs = "";
  if (metier.length) {
    const emploi = metier.find((c) => c.champ === "emploi_affichage")
      || metier.find((c) => c.champ === "emploi") || metier[0];
    const codes = metier.filter((c) => c !== emploi)
      .map((c) => `${c.libelle} : ${lisible(c, "apres")}`).join(" · ");
    blocs += ligne(ETAT.aides.dater_metier, lisible(emploi, "avant"), lisible(emploi, "apres"),
                   codes ? `<span class="discret">${echapper(codes)}</span>` : "");
  }
  blocs += autres.map((c) => ligne(c.libelle, lisible(c, "avant"), lisible(c, "apres"))).join("");
  const dater = changements.length > 0;
  return new Promise((resoudre) => {
    let repondu = false;
    const repondre = (v) => { if (!repondu) { repondu = true; resoudre(v); } };
    ouvrirBoite(`<h3>${aideDe("dater_titre")}</h3>
      ${usage ? `<p class="encart attention">${echapper(usage)}</p>` : ""}
      ${dater ? `<p>${aideDe("dater")}</p>
      <div class="changements">${blocs}</div>
      <label class="champ-boite">${aideDe("dater_depuis")}
        <input type="date" id="b-depuis"${valeurDeDepart(propose)}>
      </label>
      <p class="note">${pourquoi ? echapper(pourquoi) : ""} ${aideDe("dater_avant")}</p>
      <details class="b-note-bloc"><summary class="discret">${aideDe("dater_note")}</summary>
        <input type="text" id="b-note" placeholder="${aideDe("dater_motif")}">
      </details>` : ""}
      <div class="actions">
        <button class="secondaire" id="b-annuler">${aideDe("fiche_annuler")}</button>
        <button class="primaire" id="b-dater">${aideDe("fiche_enregistrer")}</button>
      </div>`,
      // Fermer la boite sans repondre -- clic a cote -- vaut annuler : c'est
      // ce que l'ecran montre, et rien ne doit s'ecrire en douce.
      () => repondre(null));
    document.getElementById("b-annuler").onclick = fermerBoite;
    document.getElementById("b-dater").onclick = () => {
      repondre(dater ? { depuis: document.getElementById("b-depuis").value,
                         note: document.getElementById("b-note").value } : {});
      fermerBoite();
    };
  });
}

/* `forcer` : l'utilisateur a passé outre un refus, dans la boîte qui le
 * lui propose. Un seul chemin d'écriture, forcé ou non : le
 * chemin forcé se contentait de redessiner, et la fiche restait « modifiée »
 * -- quitter l'onglet demandait d'enregistrer ce qui l'était déjà. */
/* LES CASES QUE LA PAGE ENVOIE à `enregistrer-fiche` et à `fiche` : une liste de changements `[{case, valeur}]` (comme
 * la carte), chaque structure entière ; une case vidée part `null`, dans une structure comme seule (le moteur refuse
 * `""`), et une liste où rien n'est coché aussi. */
function sansTexteVide(v) {
  if (v === "") return null;
  if (Array.isArray(v)) return v.length ? v.map(sansTexteVide) : null;
  if (v && typeof v === "object") {
    const o = Object.fromEntries(Object.entries(v).map(([k, x]) => [k, sansTexteVide(x)]));
    // Une structure dont aucune partie n'a de valeur (vide, ou une case non cochée) est une case vide.
    return Object.values(o).every((x) => x === null || x === false) ? null : o;
  }
  return v;
}
function casesDeLaCharge(donnees) {
  return { changements: Object.entries(donnees).map(([c, valeur]) => ({ case: c, valeur: sansTexteVide(valeur) })) };
}

async function ecrireFiche(fiche, donnees, daté, forcer = false) {
  try {
    const r = await api("enregistrer-fiche",
      { nom: FICHE_ACTIVE, cle: fiche.cle, ...casesDeLaCharge(donnees), ...daté, ...(forcer ? { forcer: true } : {}) });
    // Ce qui change demande sa date d'effet : le moteur l'a dit, rien n'est écrit ; la même demande, datée et confirmée.
    if (r.avertissement) {
      const réponse = await demanderDateDEffet(r.avertissement);
      // Annulé, ou la boîte fermée d'un clic à côté : la fiche garde ce qu'on y a tapé, et la page le dit.
      if (!réponse) {
        dire(ETAT.aides.rien_enregistre);
        return false;
      }
      return ecrireFiche(fiche, donnees, { ...réponse, confirme: true }, forcer);
    }
    await relireEtat();
    // Une fiche neuve : le moteur a fabriqué sa clé, c'est elle qu'on ouvre.
    if (!fiche.cle && r.cle) { CLE_ACTIVE = r.cle; FICHE_NEUVE = false; }
    dire(r.phrase);
    oublierLEcran(casesDeLaFicheOuverte());
    // ENREGISTRER REFERME LA FICHE. Elle se redessinait ouverte,
    // identique : rien ne distinguait une fiche enregistree d'une fiche qui
    // ne l'etait pas, et l'editeur attendait qu'elle se ferme. Un salarie ou un
    // spectacle revient a la liste, sa ligne marquee un instant ; la fiche
    // fiche sans liste (la route le dit : pas d'« Annuler »), reste et dit l'heure du moteur (`enregistre_dit`), pas
    // celle du navigateur : au banc, le temps est simulé.
    const enregistree = CLE_ACTIVE;
    const resteOuverte = FICHE_SANS_LISTE;
    if (!resteOuverte) { CLE_ACTIVE = null; FICHE_NEUVE = false; }
    NOTE_APRES_OUVERTURE = resteOuverte ? r.enregistre_dit : null;
    dessinerConfig();
    if (!resteOuverte && enregistree) {
      const rang = document.querySelector(`#config-liste .rang.fiche[data-cle="${CSS.escape(enregistree)}"]`);
      if (rang) {
        rang.classList.add("vient-d-etre-enregistree");
        rang.scrollIntoView({ behavior: "smooth", block: "nearest" });
        setTimeout(() => rang.classList.remove("vient-d-etre-enregistree"), 2500);
      }
    }
    if (!fiche.cle) {
      // Une fiche neuve : les listes des blocs de paie doivent la proposer
      // sans qu'on recharge le mois.
      rafraichirReferentiels();
    }
    return true;
  } catch (e) {
    if (e.detail?.classe === "FicheRefusee") {
      // On propose de forcer, mais on ne force jamais tout seul : une regle
      // peut se tromper, l'utilisateur en sait parfois plus qu'elle.
      // Le libellé du champ, pas sa clé technique (« numero_objet »).
      const libelles = {};
      ETAT.configuration.description[FICHE_ACTIVE].forEach((c) => { libelles[c.cle] = c.libelle; });
      const erreurs = e.detail.a_part.anomalies.filter((a) => a.gravite === "erreur");
      // LE MOTEUR DIT CE QUI SE FORCE : une erreur de forme (`forcable`), oui ;
      // une case qu'une structure ne décrit pas, non.
      const forcable = erreurs.every((a) => a.forcable !== false);
      // Ce que le moteur dit de ces erreurs (`pourquoi`) : celles qui bloquent si l'une bloque, sinon celles qui se forcent.
      const pourquoi = [...new Set(erreurs.filter((a) => forcable || a.forcable === false).map((a) => a.pourquoi).filter(Boolean))];
      // Le libellé et son message sur une ligne : `.encart b` est un titre.
      ouvrirBoite(`<h3>${aideDe("refus_titre")}</h3>
        <div class="encart erreur">${e.detail.a_part.erreurs_dit ? `<b>${echapper(e.detail.a_part.erreurs_dit)}</b>` : ""}
          ${erreurs.map((a) => `<div class="anomalie"><b>${echapper(libelles[a.champ] || a.champ)}</b> : ${echapper(a.message)}</div>`).join("")}</div>
        ${pourquoi.map((p) => `<p class="note">${echapper(p)}</p>`).join("")}
        <div class="actions">
          <button class="${forcable ? "secondaire" : "primaire"}" onclick="fermerBoite()">${aideDe("refus_corriger")}</button>
          ${forcable ? `<button class="primaire" id="b-forcer">${aideDe("refus_forcer")}</button>` : ""}
        </div>`);
      // La date d'effet deja saisie voyage avec : passer outre une erreur
      // ne doit pas faire perdre au passage ce qu'on venait de dater.
      const forcer = document.getElementById("b-forcer");
      if (forcer) forcer.onclick = () => { fermerBoite(); ecrireFiche(fiche, donnees, daté, true); };
    } else { dire(e.message, "erreur"); }
    return false;
  }
}
