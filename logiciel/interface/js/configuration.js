/* La configuration : la liste, l'importation, metiers, AT/MP, sante ; les écrans qu'une route rend (`js/avance.js`). */
"use strict";

/* =======================================================================
   CONFIGURATION
   ===================================================================== */
let FICHE_ACTIVE = "salarie";
let CLE_ACTIVE = null;
/* Une fiche neuve est ouverte : `dessinerConfig` ne l'efface pas. */
let FICHE_NEUVE = false;

document.querySelectorAll(".sous-onglets button").forEach((b) => {
  b.onclick = async () => {
    if (!(await quitterLaFiche())) return;
    document.querySelectorAll(".sous-onglets button").forEach((x) => x.classList.remove("actif"));
    b.classList.add("actif");
    if (FICHE_ACTIVE !== b.dataset.fiche) FICHE_NEUVE = false;
    FICHE_ACTIVE = b.dataset.fiche;
    CLE_ACTIVE = null;
    dessinerConfig();
  };
});

const ARCHIVES_VISIBLES = { salarie: false, spectacle: false };
/* Ce qu'on cherche dans chaque liste ; gardé quand la liste se redessine. */
const RECHERCHE = { salarie: "", spectacle: "" };

function fichesDe(nom) {
  const c = ETAT.configuration;
  if (nom === "entreprise") return [c.entreprise];
  return nom === "salarie" ? c.salaries : c.spectacles;
}

/* L'ordre de deux cases facultatives : une case vide vient avant une case remplie. */
function comparerCases(x, y, ...options) {
  if (x == null || y == null) return (y == null) - (x == null);
  return String(x).localeCompare(String(y), ...options);
}

function dessinerConfig() {
  const liste = document.getElementById("config-liste");
  dessinerSuppression();
  // Avancé : la place du sous-onglet ouvert (`data-place`), ses routes sans écran dessiné.
  const avance = document.getElementById("config-avance");
  avance.dataset.auto = document.querySelector(`.sous-onglets button[data-fiche="${FICHE_ACTIVE}"]`).dataset.place;
  dessinerAvance(avance);
  // Un sous-onglet dont une route rend l'écran (sa fiche : `affichage: "rendu"`, sa place) : le dessinateur le pose.
  const rendu = routeQuiRend(avance.dataset.auto);
  if (rendu) {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerLEcranRendu(liste, rendu);
    return;
  }
  if (FICHE_ACTIVE === "importer") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerImportation(liste);
    return;
  }
  if (FICHE_ACTIVE === "coffre") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerCoffre(liste);
    return;
  }
  if (FICHE_ACTIVE === "metiers") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerMetiers(liste);
    return;
  }
  // Les modèles de contrat et les clauses particulières : `modeles-contrat.js`.
  if (FICHE_ACTIVE === "contrats") {
    document.getElementById("config-formulaire").innerHTML = "";
    dessinerModelesContrat(liste);
    return;
  }
  const fiches = fichesDe(FICHE_ACTIVE);
  if (FICHE_ACTIVE === "entreprise") {
    liste.innerHTML = "";
    peindreDemarrage(liste);
    CLE_ACTIVE = null;
    dessinerFormulaire(fiches[0]);
    return;
  }
  // Les archives en dernier, jamais melangees aux courantes. Elles restent
  // trouvables : une fiche qu'on ne voit plus passe pour perdue, et on la
  // ressaisit -- avec un deuxieme code salarie, ce qui serait pire.
  const courantes = fiches.filter((f) => f.donnees.actif === true);
  // Les spectacles par titre.
  if (FICHE_ACTIVE === "spectacle") {
    courantes.sort((a, b) => comparerCases(a.donnees.titre, b.donnees.titre, "fr", { sensitivity: "base" }));
  }
  // Les numéros d'objet archivés, du plus récent au plus ancien.
  const archivees = fiches.filter((f) => f.donnees.actif === false)
    .sort((a, b) => comparerCases(b.donnees.numero_objet, a.donnees.numero_objet));
  const rangees = [...courantes, ...archivees];
  const salarie = FICHE_ACTIVE === "salarie";
  // Les mots de la liste, tels que le moteur les dit pour cette sorte de fiche (`configuration.ecran`).
  const ecran = ETAT.configuration.ecran;
  const mots = ecran[FICHE_ACTIVE];

  // UN SPECTACLE, PAS UNE FICHE PAR NUMÉRO. Une fiche archivée du
  // même titre qu'un spectacle actif (un ancien numéro) se montre sous lui,
  // pas dans les archives. Sans titre, la fiche n'a pas de titre à rapprocher (`null`).
  const titreDe = (d) => d.titre == null ? null : d.titre.normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "").toLowerCase().replace(/\s+/g, " ").trim();
  const sansTitre = (f) => !!f.sans_titre;
  const passees = {};
  const sousUnActif = new Set();
  if (!salarie) {
    const titresActifs = new Set(courantes.map((f) => titreDe(f.donnees)));
    archivees.forEach((f) => {
      const t = titreDe(f.donnees);
      if (sansTitre(f) || !titresActifs.has(t)) return;
      (passees[t] ||= []).push(f);
      sousUnActif.add(f.cle);
    });
  }
  // Les archives qui restent : les spectacles qui ne se jouent plus, par
  // titre ; puis les numéros sans titre, groupés -- à nommer si on le sait.
  const archiveesSeules = archivees.filter((f) => !sousUnActif.has(f.cle))
    .sort((a, b) => (sansTitre(a) - sansTitre(b))
      || comparerCases(titreDe(a.donnees), titreDe(b.donnees))
      || comparerCases(b.donnees.numero_objet, a.donnees.numero_objet));

  // UNE LIGNE PAR FICHE : le nom, ce qui
  // l'identifie, son état, puis les gestes -- « Archiver » dit en toutes
  // lettres, là où le clic sur toute la carte archivait sans prévenir.
  const rangHtml = (f, trouve) => {
    const d = f.donnees;
    const archive = d.actif === false;
    const nom = d.nom_affichage || d.nom;
    const titre = (salarie ? [nom == null ? null : nom.toUpperCase(), d.prenom_affichage || d.prenom] : [d.titre])
      .filter((x) => x != null).join(" ").trim();
    // Un numéro d'objet importé sans titre : le moteur le dit (`sans_titre`),
    // la ligne le dit en minuscules.
    const aCompleter = !salarie && sansTitre(f);
    const anciens = !salarie && !archive && !trouve ? passees[titreDe(d)] : undefined;
    const autresNumeros = anciens ? anciens.slice()
      .sort((a, b) => comparerCases(b.donnees.numero_objet, a.donnees.numero_objet)) : [];
    const etat = pastilleDeLEtat(f.pastille);
    return `<div class="rang fiche${archive ? " archivee" : ""}${f.cle === CLE_ACTIVE ? " selectionnee" : ""}"
        data-cle="${echapper(f.cle)}">
      <div class="rang-infos">
        <b class="rang-nom">${aCompleter ? `<span class="a-completer">titre à compléter</span>`
          : echapper(titre || f.cle)}</b>
        ${echapper(f.quoi_dit)}
        ${trouve ? `<span class="trouve">${echapper(trouve.ou)} : ${echapper(trouve.quoi)}${
          trouve.ancienne ? ` <span class="discret">${echapper(ecran.ancienne)}</span>` : ""}</span>` : ""}
        ${autresNumeros.length ? `<span class="annees"><span>anciens numéros</span>
          ${autresNumeros.map((p) => `<button class="lien" data-ouvrir="${echapper(p.cle)}">${
            echapper(p.donnees.numero_objet || p.cle)}</button>`).join(" · ")}</span>` : ""}
      </div>
      <div class="rang-etat">${etat}</div>
      <div class="rang-actions">
        <button class="lien" data-activer="${archive ? "oui" : "non"}">${echapper(f.geste_dit)}</button>
      </div>
    </div>`;
  };

  // CACHÉES PAR DÉFAUT : un spectacle archivé ne
  // se joue plus. Une case les montre ; elle reste
  // cochée tant qu'on ne la décoche pas, et une fiche archivée ouverte les
  // montre d'office.
  const voirArchives = ARCHIVES_VISIBLES[FICHE_ACTIVE]
    || archivees.some((f) => f.cle === CLE_ACTIVE);
  // UNE BARRE, DEUX GESTES : chercher, ajouter. Le titre qui répétait le
  // sous-onglet (« SALARIÉS » sous Salariés) est parti.
  liste.innerHTML = `<div class="barre-liste">
      <input type="search" id="config-recherche" class="recherche" autocomplete="off"
        value="${echapper(RECHERCHE[FICHE_ACTIVE])}"
        placeholder="Rechercher">
      <button class="secondaire" id="config-nouvelle">${echapper(mots.nouvelle)}</button>
    </div>
    <div id="config-rangs"></div>`
    + (salarie ? fichesAttenduesHtml() : "");

  // LA RECHERCHE DANS TOUS LES CHAMPS. Chaque valeur
  // de la fiche, les anciennes valeurs datées comprises (un courriel changé
  // retrouve encore la personne), sans accents, sans majuscules, sans
  // espaces (« 2 93 08 » trouve le NIR) ; une date se cherche aussi comme
  // on l'écrit (23/07/1990). Plusieurs mots : chacun doit se trouver
  // quelque part dans la fiche. Elle fouille les archivées aussi -- c'est
  // souvent là qu'on cherche.
  const libelles = {};
  ETAT.configuration.description[FICHE_ACTIVE].forEach((c) => { libelles[c.cle] = c.libelle; });
  // Les cases du nom que la ligne lit déjà : une trouvaille dedans ne se redit pas.
  const CASES_DU_NOM = new Set(["nom", "nom_affichage", "prenom", "prenom_affichage", "titre"]);
  // Une case sans libellé n'a pas d'endroit à dire (`ou` absent) : la ligne ne cite pas où l'on a trouvé.
  const libelleDe = (cle) => libelles[cle] || libelles[cle.replace(/_affichage$/, "")];
  const plat = (t) => String(t).normalize("NFD").replace(/[̀-ͯ]/g, "").toLowerCase();
  const valeurs = (f) => {
    const out = [];
    const ajouter = (ou, v, champ, ancienne = false) => {
      if (v === null || v === undefined || v === "" || typeof v === "boolean") return;
      if (typeof v === "object") {
        Object.entries(v).forEach(([k, x]) => { if (!k.startsWith("_")) ajouter(ou, x, champ, ancienne); });
        return;
      }
      const t = String(v);
      out.push({ ou, champ, quoi: t, ancienne });
      if (/^\d{4}-\d{2}-\d{2}$/.test(t)) out.push({ ou, champ, quoi: jjmmaaaa(t), ancienne });
    };
    // La forme lisible d'abord (« Avallon »), la forme déclarée ensuite
    // (« AVALLON ») : c'est la première trouvée que la ligne cite.
    Object.entries(f.donnees)
      .sort(([a], [b]) => b.endsWith("_affichage") - a.endsWith("_affichage"))
      .forEach(([k, v]) => {
        if (k.startsWith("_") || k === "historique" || k === "actif") return;
        ajouter(libelleDe(k), v, k);
      });
    const autresDates = (h) => h.valeurs.filter((e) => !e.en_vigueur)
      .forEach((e) => ajouter(libelleDe(h.champ), e.valeur, h.champ, true));
    f.donnees.historique.forEach((h) => {
      autresDates(h);
      h.lignes.forEach((l) => l.cases.forEach((c) => autresDates({ ...c, champ: h.champ })));
    });
    return out.map((x) => ({ ...x, cle: plat(x.quoi), serre: plat(x.quoi).replace(/\s+/g, "") }));
  };
  const chercher = (f, mots) => {
    const vs = valeurs(f);
    let premier = null;
    for (const m of mots) {
      const serre = m.replace(/\s+/g, "");
      const v = vs.find((x) => x.cle.includes(m) || x.serre.includes(serre));
      if (!v) return null;
      premier = premier || v;
    }
    return premier;
  };

  // Les numéros sans titre (importés des DSN, « A COMPLETER ») en un groupe,
  // sous une phrase qui dit d'où ils viennent et ce qu'on peut en faire.
  const archivesHtml = () => {
    const nommees = archiveesSeules.filter((f) => salarie || !sansTitre(f));
    const anonymes = salarie ? [] : archiveesSeules.filter((f) => sansTitre(f));
    return `${nommees.length ? `<div class="rangs">${nommees.map((f) => rangHtml(f)).join("")}</div>` : ""}
      ${anonymes.length ? `<p class="note">${echapper(ETAT.configuration.numeros_sans_titre)}</p>
        <div class="rangs">${anonymes.map((f) => rangHtml(f)).join("")}</div>` : ""}`;
  };

  const peindre = () => {
    const q = plat(RECHERCHE[FICHE_ACTIVE]).trim();
    const cible = document.getElementById("config-rangs");
    if (q) {
      const mots = q.split(/\s+/);
      const trouvees = rangees.map((f) => ({ f, t: chercher(f, mots) })).filter((x) => x.t);
      // Où l'on a trouvé, sauf si c'est le nom même qu'on lit sur la ligne.
      cible.innerHTML = trouvees.length
        ? `<div class="rangs">${trouvees.map(({ f, t }) =>
            rangHtml(f, t.ou && !CASES_DU_NOM.has(t.champ) ? t : null)).join("")}</div>`
        : `<p class="discret">Aucune fiche trouvée</p>`;
    } else {
      cible.innerHTML = `${courantes.length
          ? `<div class="rangs">${courantes.map((f) => rangHtml(f)).join("")}</div>`
          : `<p class="discret">${echapper(mots.aucun)}</p>`}
        ${archiveesSeules.length ? `<label class="coche montrer-archives">
            <input type="checkbox" id="config-archives"${voirArchives ? " checked" : ""}>
            ${echapper(mots.archives)}
            <span class="discret">(${archiveesSeules.length})</span></label>
          ${voirArchives ? archivesHtml() : ""}` : ""}`;
      const caseArchives = document.getElementById("config-archives");
      if (caseArchives) caseArchives.onchange = () => {
        ARCHIVES_VISIBLES[FICHE_ACTIVE] = caseArchives.checked;
        dessinerConfig();
      };
    }
    cible.querySelectorAll(".rang.fiche").forEach((r) => {
      const cle = r.dataset.cle;
      r.querySelector("[data-activer]").onclick = async (e) => {
        const activer = e.currentTarget.dataset.activer === "oui";
        try {
          const rep = await api("activer-fiche", { nom: FICHE_ACTIVE, cle, actif: activer });
          poserEtat(rep.etat);
          dire(rep.phrase);
          dessinerConfig();
        } catch (err) { dire(err.message, "erreur"); }
      };
      const ouvrir = async (c) => {
        if (c !== CLE_ACTIVE && !(await quitterLaFiche())) return;
        CLE_ACTIVE = c;
        FICHE_NEUVE = false;
        dessinerConfig();
        dessinerFormulaire(rangees.find((f) => f.cle === c));
        document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
      };
      // LA LIGNE S'OUVRE D'UN CLIC SUR ELLE.
      rendreOuvrable(r, () => ouvrir(cle), "Ouvrir la fiche");
      r.querySelectorAll("[data-ouvrir]").forEach((b) => { b.onclick = () => ouvrir(b.dataset.ouvrir); });
    });
  };
  peindre();
  const recherche = document.getElementById("config-recherche");
  recherche.oninput = () => { RECHERCHE[FICHE_ACTIVE] = recherche.value; peindre(); };
  document.getElementById("config-nouvelle").onclick = () => nouvelleFiche(FICHE_ACTIVE);
  brancherFichesAttendues();
  if (!CLE_ACTIVE && !FICHE_NEUVE) document.getElementById("config-formulaire").innerHTML = "";
}

/* ---------------------------------------------------- reprendre l'historique
   « Importer » reprend d'un geste, et se rejoue ; l'aperçu (sous « Avancé »)
   dit, sans rien écrire, ce qui serait créé et ce que les DSN disaient
   autrement que les fiches. Une fiche existante reçoit seulement ce qui y est vide ou « A COMPLETER »
   (`import::completer`) ; tout champ rempli fait foi, l'écart
   est signalé, un changement se date à la main.

   LA BOÎTE AUX LETTRES. Une seule
   entrée : le dossier `import/` de la compagnie, où l'on met tout ce que
   l'ancien logiciel a laissé — DSN, AEM, bulletins de paie en PDF, archives
   zip. Le moteur trie par le contenu (`importation-apercu`) et
   rend un compte rendu par fichier : la page le montre en tableau, le sort en
   pastille, la raison en clair. Elle ne décide rien : ce qui est repris, ce
   qui ne l'est pas et pourquoi, c'est le moteur qui le dit. */

/* Le sort d'un fichier ou d'une page (`import::bulletins::SORTS`) : sa pastille,
   telle que la réponse du moteur la donne (`pastilles`). */
function pastilleSort(sort, pastilles) {
  return pastilleDeLEtat(pastilles[sort]);
}

/* « Ouvrir le dossier » n'a de sens que si la page tourne sur le poste,
   servie par sa fenêtre : c'est elle qui ouvre l'explorateur (`ouvrir`, `interface/poste.rs`). Sans elle, le bouton
   n'apparaît pas. */
// Dans la fenêtre PuppetPaie (Tauri, `cadre.js`), et non dans un navigateur : le poste peut ouvrir son dossier.
const fenetreDuPoste = () => !!window.fenetrePuppetPaie;

/* REPRENDRE L'HISTORIQUE : une
   logique simple : ils mettent tout dans le dossier d'import (`import/` de la
   compagnie, le seul que l'import lit), puis « Importer ». En haut de la
   carte, à droite du titre et hors du repli du guide, « Ouvrir le dossier
   d'import » (`guide.ouvrir`, du moteur) ; hors de la fenêtre du poste, son
   chemin en clair à la même place.
   Aucun logiciel n'est nommé. UN SEUL CHEMIN, avant comme après une
   première reprise : on ajoute au dossier et on relance l'import, qui ne
   reprend que le neuf (`guide.rejouer`, du moteur). Une fois l'historique
   repris, l'état le dit en une ligne, ce qui reste à dire se peint
   au-dessus, et le guide se replie. L'aperçu, qui n'écrit rien, est hors
   du chemin simple (`guide.avance`). */
async function dessinerImportation(cible) {
  let e;
  try { e = await api("importation-etat"); } catch (err) {
    cible.innerHTML = `<div class="encart erreur">${echapper(err.message)}</div>`;
    return;
  }
  const deja = e.reprises.length || e.contrats;
  // Le dossier d'import (`dossier` de `importation-etat`) : la fenêtre du poste l'ouvre ; ailleurs, son chemin se lit.
  const boite = fenetreDuPoste()
    ? `<button type="button" class="secondaire" id="import-ouvrir">${echapper(e.guide.ouvrir)}</button>`
    : `<code class="import-chemin">${echapper(e.dossier)}</code>`;
  // L'essentiel se lit à côté du bouton tant que rien n'est repris ; ensuite, il se replie avec le guide.
  cible.innerHTML = `
    <div class="carte">
      <div class="import-tete">
        <div><h3>Reprendre votre passé</h3><div id="import-essentiel">${deja ? "" : vueEssentielImport(e.guide)}</div></div>
        ${boite}
      </div>
      <div id="import-etat">${vueEtatImportation(e)}</div>
      <div id="import-demarrage"></div>
      <div id="import-guide">${deja ? vueGuideReplie(e.guide) : vueGuideImport(e.guide)}</div>
      <p class="note">${echapper(e.guide.rejouer)}</p>
      <p><button class="primaire" id="import-tout">Importer</button></p>
      <details><summary>${echapper(e.guide.avance)}</summary>
        <button type="button" class="lien" id="import-apercu">${echapper(e.guide.apercu)}</button></details>
      <div id="import-resultat"></div>
    </div>`;
  document.getElementById("import-tout").onclick = () => importerDepuisLeGuide();
  brancherIdentites(document.getElementById("import-etat"), e);
  // Ce qui reste à dire pour la configuration (`pour-demarrer`), dès qu'il y a un historique repris.
  if (deja) peindreDemarrage(document.getElementById("import-demarrage"));
  document.getElementById("import-apercu").onclick = () => apercuImportation();
  const ouvrir = document.getElementById("import-ouvrir");
  // La boîte import/ de la compagnie, créée si elle manque.
  if (ouvrir) ouvrir.onclick = () => ouvrirEtDire(e.dossier, "", true);
}

/* L'ESSENTIEL DU GUIDE (`guide.essentiel`, du moteur) : les bulletins, une DSN
   déposée, le dossier d'import. */
function vueEssentielImport(g) {
  return `<ul>${g.essentiel.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>`;
}

/* Le guide replié, une fois un historique repris : l'essentiel y rentre avec le reste. */
function vueGuideReplie(g) {
  return `<details class="relire"><summary>${echapper(g.replie)}</summary>${vueEssentielImport(g)}${vueGuideImport(g)}</details>`;
}

/* LE GUIDE DE L'IMPORT (`import::guide`, du moteur), après l'essentiel : les
   pièces essentielles, puis ce qu'on peut mettre si des documents manquent ;
   ce que le logiciel lit de chaque pièce se déplie. La page le montre tel quel. */
function vueGuideImport(g) {
  const piece = (p) => `<div class="guide-piece">
      <b>${echapper(p.quoi)}</b>
      <div class="note">${echapper(p.ou)}</div>
      ${p.lu.length ? `<details><summary>${echapper(g.ce_qui_s_en_lit)}</summary><ul>${p.lu.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul></details>` : ""}
    </div>`;
  return `<div class="guide-pieces">${g.essentielles.map(piece).join("")}</div>
    <p class="intro">${echapper(g.si_manquent)}</p>
    <div class="guide-pieces">${g.autres.map(piece).join("")}</div>
    <p class="note">${echapper(g.accepte)}</p>
    <p class="note">${echapper(g.reste)}</p>
    <p class="note">${echapper(g.rien_n_est_perdu)}</p>`;
}

/* « IMPORTER », D'UN GESTE, la première fois comme les suivantes : le moteur
   reprend le neuf (`importer`), puis la page se redessine — au-dessus, ce
   qui reste à dire (la nature des contrats repris, `importation-etat` ; ce
   qu'une DSN donnerait et ce qui se saisit, `pour-demarrer`) — et, dessous,
   ce qui a été repris. */
async function importerDepuisLeGuide() {
  const rendre = occupe(document.getElementById("import-tout"), "Import…");
  await ecrireImportation();
  rendre();
  // Rien de repris (un dossier vide, des pièces illisibles) : le guide reste, avec ce que le moteur en a dit.
  let etat;
  try { etat = await api("importation-etat"); } catch (e) { dire(e.message, "erreur"); return; }
  if (!(etat.reprises.length || etat.contrats)) return;
  const guide = document.getElementById("import-guide");
  if (guide) guide.innerHTML = vueGuideReplie(etat.guide);
  const essentiel = document.getElementById("import-essentiel");
  if (essentiel) essentiel.innerHTML = "";
  const demarrage = document.getElementById("import-demarrage");
  if (demarrage) await peindreDemarrage(demarrage);
  document.getElementById("import-etat").scrollIntoView({ block: "start" });
}

/* L'ÉTAT (`importation-etat`), en une ligne et sans nommer de logiciel :
   « Historique repris le … : N bulletins, N déclarations, N contrats, de …
   à … ». Les bulletins : les paies qu'un bulletin a écrites ou complétées ;
   les déclarations : une DSN par mois déclaré (`declarations` de la reprise
   DSN) et une AEM par contrat qu'elle atteste ; les contrats : tous ceux de
   l'historique (`contrats`). Puis ce qui attend l'éditeur (`editeur`). */
function vueEtatImportation(e) {
  const repris = e.repris_dit ? `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">${echapper(e.repris_titre)}</b>
        ${echapper(e.repris_dit)}</div>
      <div class="rang-etat">${pastilleDeLEtat(e.pastille_repris)}</div>
    </div>` : "";
  const editeur = e.editeur.map((x) => `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">${echapper(x.titre_dit)}</b>${echapper(x.resume)}</div>
      <div class="rang-etat">${pastilleDeLEtat(x.pastille)}</div>
    </div>`).join("");
  return repris + vueIdentitesADire(e.identites) + vueNaturesADire(e.a_dire)
    + (editeur ? `<h3>Ce qui attend l'éditeur</h3>${editeur}` : "");
}

/* LA PERSONNE QU'UNE DSN DÉCLARE ET QUI RESSEMBLE À UNE FICHE SANS NUMÉRO
   DE SÉCURITÉ SOCIALE (`import::identite`) : le moteur pose la question
   (`identites.questions`), la page la montre avec ses deux gestes (`gestes`,
   dessinés depuis la fiche de leur route : `js/avance.js`) ; ce qui ne s'est
   pas relu pour les poser se dit (`identites.non_relues`). */
function vueIdentitesADire(identites) {
  const liste = identites.questions;
  const non = identites.non_relues ? `<p class="note">${echapper(identites.non_relues)}</p>` : "";
  if (!liste.length) return non;
  return `<h3>Est-ce la même personne ?</h3>${non}${liste.map((q, k) => `<div class="rang">
      <div class="rang-infos"><b class="rang-nom">${echapper(q.qui)}</b>${echapper(q.question)}</div>
      <div class="rang-etat" data-identite="${k}">${boutonsDesGestes(q.gestes)}</div>
    </div>`).join("")}`;
}

/* Les gestes des questions d'identité de `e`, dessinés dans `cible` ; après la réponse, l'état se relit. */
function brancherIdentites(cible, e) {
  cible.querySelectorAll("[data-identite]").forEach((b) => {
    const q = e.identites.questions[+b.dataset.identite];
    brancherLesGestes(b, q.gestes, { titre: q.qui, dit: q.question }, async () => {
      await relireEtat();
      rafraichirReferentiels();
      await rafraichirEtatImportation();
    });
  });
}

/* LA NATURE DES CONTRATS REPRIS QU'AUCUN BULLETIN NE DIT
   (`import::nature`). Les bulletins d'un permanent ne disent pas s'il
   est en CDI ou en CDD : le moteur les nomme (`a_dire`), la page le
   demande, une fois par salarié, sans rien choisir d'avance. */
function vueNaturesADire(liste) {
  if (!liste.length) return "";
  // Ce que chaque nature demande avec elle (`demande`, du moteur) : pour un
  // contrat qui court encore, son début, sa fin s'il a un terme, son numéro.
  const CASES = {
    debut: (x) => `<label>Début du contrat <input type="date" data-nature-champ="debut"></label>`,
    fin: (x) => `<label>Fin prévue <input type="date" data-nature-champ="fin"></label>`,
    numero: (x) => `<label title="${aideDe("import_numero")}">Numéro du contrat
        <input type="text" data-nature-champ="numero" placeholder="${echapper(x.numero_placeholder)}"></label>`,
    salaire: (x) => `<label>Salaire mensuel brut <input type="text" inputmode="decimal" data-nature-champ="salaire"
        placeholder="celui du contrat signé"></label>`,
    quotite: (x) => `<label title="${aideDe("import_quotite")}">Heures par mois
        <input type="text" inputmode="decimal" data-nature-champ="quotite" placeholder="celle du contrat signé"></label>`,
  };
  // Une seule nature proposée (celle que la DSN reprise déclare) : elle est
  // choisie, et ses cases se montrent d'emblée.
  const lignes = liste.map((x) => { const seule = x.natures.length === 1 ? x.natures[0] : null;
    const montre = (c) => !!seule && seule.demande.includes(c);
    return `<div class="rang" data-nature-rang="${echapper(x.salarie)}">
      <div class="rang-infos"><b class="rang-nom">${echapper(x.nom)}</b>
        ${echapper(x.dit)}
        ${x.en_cours_dit ? `<div class="note">${echapper(x.en_cours_dit)}</div>` : ""}
        ${x.dsn_non_relue ? `<div class="note">${echapper(x.dsn_non_relue)}</div>` : ""}
        <div class="e-contrat-champs" data-nature-cases${seule && seule.demande.length ? "" : " hidden"}>${["debut", "fin", "numero", "salaire", "quotite"].map((c) =>
          `<span data-nature-case="${c}"${montre(c) ? "" : " hidden"}>${CASES[c](x)}</span>`).join("")}</div></div>
      <div class="rang-etat" style="display:flex;gap:6px;align-items:center">
        <select data-nature-choix aria-label="${echapper(x.choix_dit)}">
          ${seule ? "" : `<option value="" data-demande="">— choisir —</option>`}
          ${x.natures.map((n) => `<option value="${echapper(n.cle)}"${seule ? " selected" : ""}
             data-demande="${echapper(n.demande.join(" "))}">${echapper(n.libelle)}</option>`).join("")}
        </select>
        <button type="button" class="petit" data-nature-salarie="${echapper(x.salarie)}">Enregistrer</button>
      </div>
    </div>`; }).join("");
  return `<h3>Nature de ces contrats</h3>
    <p class="note">${aideDe("import_natures")}</p>${lignes}`;
}

/* La nature choisie montre les cases qu'elle demande, et elles seules. */
document.addEventListener("change", (ev) => {
  const sel = ev.target.closest && ev.target.closest("[data-nature-choix]");
  if (!sel) return;
  const rang = sel.closest("[data-nature-rang]");
  const opt = sel.selectedOptions[0];
  const demande = opt.dataset.demande.split(" ").filter((x) => x);
  rang.querySelectorAll("[data-nature-case]").forEach((c) => { c.hidden = !demande.includes(c.dataset.natureCase); });
  rang.querySelector("[data-nature-cases]").hidden = !demande.length;
});

document.addEventListener("click", async (ev) => {
  const ouvrir = ev.target.closest && ev.target.closest("[data-ouvrir-fiche]");
  if (ouvrir) {
    ev.preventDefault();
    ouvrirFicheImportee(ouvrir.dataset.ouvrirFiche, ouvrir.dataset.ouvrirCle);
    return;
  }
  const b = ev.target.closest && ev.target.closest("[data-nature-salarie]");
  if (!b) return;
  ev.preventDefault();
  const rang = b.closest("[data-nature-rang]");
  // Une nature non choisie part absente : c'est le moteur qui la refuse (règle 2).
  const nature = rang.querySelector("[data-nature-choix]").value || null;
  b.disabled = true;
  try {
    const charge = { salarie: b.dataset.natureSalarie, nature };
    rang.querySelectorAll("[data-nature-case]:not([hidden]) [data-nature-champ]").forEach((c) => {
      // Chaque case part telle qu'elle est tapée : la route convertit un nombre (le salaire, les heures par mois).
      charge[c.dataset.natureChamp] = c.value.trim() === "" ? null : c.value.trim();
    });
    const r = await api("importation-nature", charge);
    dire(r.phrase, r.couleur);
    // Les natures au mois s'ouvrent alors dans la saisie (`nouvelle_carte` de la route saisie).
    await relireEtat();
    rafraichirReferentiels();
    rafraichirEtatImportation();
  } catch (err) {
    dire(err.message, "erreur");
    b.disabled = false;
  }
});

/* Ouvrir, dans Configuration, la fiche que l'import vient de créer. */
async function ouvrirFicheImportee(nom, cle) {
  if (!(await quitterLaFiche())) return;
  document.querySelectorAll(".sous-onglets button").forEach((x) => x.classList.toggle("actif", x.dataset.fiche === nom));
  FICHE_ACTIVE = nom;
  CLE_ACTIVE = nom === "entreprise" ? null : cle;
  FICHE_NEUVE = false;
  dessinerConfig();
  if (nom !== "entreprise") {
    const f = fichesDe(nom).find((x) => x.cle === cle);
    if (f) dessinerFormulaire(f);
  }
  document.getElementById("config-formulaire").scrollIntoView({ behavior: "smooth" });
}

/* Une fiche que l'import crée, complète, laisse, ou sur laquelle il a à
   redire. Un écart vient d'une DSN (`dsn`, `mois`), d'un bulletin ou d'un certificat
   (`bulletin`, `fichier`, `page`) : la fiche fait foi, il est seulement
   signalé. Une case vide est complétée, et la page dit d'où. */
function fichesImportees(fiches, titre) {
  const lignes = fiches.map((f) => {
    const etat = pastilleDeLEtat(f.pastille);
    // Ce que le moteur dit de la fiche : d'où elle vient, ce qu'elle reçoit, ce qu'une pièce disait autrement.
    const luDans = f.lu_dans_dit ? `<div class="acte-dernier">${echapper(f.lu_dans_dit)}</div>` : "";
    const completes = f.completer.map((e) =>
      `<div class="acte-dernier"><b>${echapper(e.champ)}</b> ${echapper(e.dit)}</div>`).join("");
    const ecarts = f.ecarts.map((e) =>
      `<div class="acte-dernier"><b>${echapper(e.champ)}</b> — ${echapper(e.dit)}</div>`).join("");
    // UNE VALEUR FAUSSE DES PIÈCES se dit, avec le libellé de sa case ; une
    // case vide n'est pas une faute (ce qu'un acte exige, « Pour démarrer »
    // le dit après l'import).
    const fautes = f.anomalies.filter((a) => a.gravite === "erreur").map((a) =>
      `<div class="acte-dernier faute">${echapper(a.libelle || a.champ)} : ${echapper(a.message)}</div>`).join("");
    const ouvrable = f.etat === "creee" && f.cle && f.nom;
    const reste = fautes ? `<div class="acte-dernier">${ouvrable ? `<button type="button" class="petit" data-ouvrir-fiche="${echapper(f.nom)}"
            data-ouvrir-cle="${echapper(f.cle)}">Compléter la fiche</button>` : ""}</div>${fautes}` : "";
    return `<div class="rang import">
      <span class="rang-nom">${echapper(f.libelle)}</span>
      ${etat}
      <span class="rang-infos">${luDans}${completes}${ecarts}${reste}</span></div>`;
  });
  if (!lignes.length) return "";
  return `<h3>${titre}</h3>${lignes.join("")}`;
}

/* Le compte rendu, fichier par fichier (`tri.fichiers`) : le chemin dans le
   dossier, le sort, la raison. Un PDF de plusieurs pages déplie ses pages. */
function compteRenduImport(tri, pastilles) {
  const comptes = Object.entries(tri.comptes).filter(([, n]) => n > 0)
    .map(([s, n]) => `${pastilleSort(s, pastilles)} ${n}`).join(" &nbsp; ");
  const lignes = tri.fichiers.map((f) => {
    const pages = f.pages.length > 1
      ? `<details class="relire"><summary>${echapper(f.pages_dit)}</summary>${f.pages.map((p) =>
          `<div class="acte-dernier">${echapper(p.page_dit)}
             ${pastilleSort(p.sort, pastilles)} ${echapper(p.raison)}</div>`).join("")}</details>` : "";
    const de = f.de_dit ? ` <span class="discret">${echapper(f.de_dit)}</span>` : "";
    return `<tr><td>${echapper(f.chemin)}${de}</td><td style="white-space:nowrap">${pastilleSort(f.sort, pastilles)}</td>
      <td>${echapper(f.raison)}${pages}</td></tr>`;
  }).join("");
  const mises = tri.mises_de_cote.map((m) => `<div class="encart attention">${echapper(m)}</div>`).join("");
  return `<h3>Fichier par fichier</h3>
    <p class="note">${echapper(tri.dit)} &nbsp; ${comptes}</p>${mises}
    <table class="tableau-detail"><thead><tr><th>Fichier</th><th>Sort</th><th>Raison</th></tr></thead>
      <tbody>${lignes}</tbody></table>`;
}

/* Les bulletins lus : combien, combien repris, de quand à quand, et ce
   qu'ils ajoutent (`bulletins`). Pas le logiciel qui les a faits. */
function bulletinsImportes(b) {
  if (!b || !b.dit) return "";
  return `<div class="encart"><b>Bulletins de paie</b> ${echapper(b.dit)}</div>`;
}

/* Les certificats Congés Spectacles (`certificats`) ou les AEM en PDF (`aem_pdf`) lus : ce que le moteur en dit (`dit`). */
function piecesImportees(c, titre) {
  if (!c || !c.dit) return "";
  return `<div class="encart"><b>${titre}</b> ${echapper(c.dit)}</div>`;
}

/* Les écarts entre un bulletin et ce que la DSN (ou l'historique) disait
   de la même paie : le bulletin fait foi sur les montants, l'écart est
   noté (`tri.fichiers[].pages[].ecarts`). */
function ecartsBulletinDsn(tri) {
  const lignes = [];
  for (const f of tri.fichiers) {
    for (const p of f.pages) {
      for (const e of p.ecarts) {
        lignes.push(`<tr><td>${echapper(p.ou_dit)}</td>
          <td>${p.mois ? echapper(moisEnClair(p.mois)) : ""}</td>
          <td>${echapper(e.champ_dit)}</td>
          <td class="nombre">${echapper(e.valeur)}</td>
          <td class="nombre">${echapper(e.bulletin)}</td>
          <td>${pastilleDeLEtat(e.pastille)}</td></tr>`);
      }
    }
  }
  if (!lignes.length) return "";
  return `<h3>Écarts des bulletins</h3>
    <p class="note">${aideDe("import_ecarts")}</p>
    <table class="tableau-detail"><thead><tr><th>Bulletin</th><th>Mois</th><th>Champ</th>
      <th class="nombre">DSN ou historique</th><th class="nombre">Bulletin</th><th></th></tr></thead>
      <tbody>${lignes.join("")}</tbody></table>`;
}

/* Les documents qui ne sont pas repris, et pourquoi (`tri.non_repris`, du
   moteur), avec la phrase qui dit que l'import se rejoue. */
function vueNonRepris(nr, pastilles) {
  if (!nr || !nr.fichiers.length) return "";
  return `<div class="encart attention"><b>Non repris</b>
      ${nr.fichiers.map((f) => `<div class="acte-dernier">${echapper(f.chemin)} ${pastilleSort(f.sort, pastilles)}
        ${echapper(f.raison)}</div>`).join("")}
      <p>${echapper(nr.phrase)}</p>
    </div>`;
}

async function rafraichirEtatImportation() {
  const cible = document.getElementById("import-etat");
  if (!cible) return;
  try {
    const e = await api("importation-etat");
    cible.innerHTML = vueEtatImportation(e);
    brancherIdentites(cible, e);
  } catch (e) { dire(e.message, "erreur"); }
}

/* L'aperçu, ou ce qui a été repris. Aucun logiciel n'y est nommé : les
   déclarations sont des DSN et des AEM, les bulletins des bulletins. */
function vueImportation(r) {
  // La couleur de l'encart, et ce que l'import a posé de la configuration,
  // viennent du moteur (`encart`, `parametrage`).
  const avert = r.avertissements.map((a) => `<div class="encart attention">${echapper(a)}</div>`).join("");
  const couleur = r.encart ? ENCART_DE_COULEUR[r.encart.couleur] : "";
  const pose = r.parametrage.length
    ? `<p class="note">Posé dans la configuration</p><ul>${r.parametrage.map((p) => `<li>${echapper(p)}</li>`).join("")}</ul>` : "";
  // Ce que la DSN reprise déclare d'un contrat repris autrement que sa carte (`dsn_fait_foi`, du moteur, phrases comprises).
  const remplace = r.dsn_fait_foi.length
    ? `<p class="note">La DSN fait foi</p><ul>${r.dsn_fait_foi.map((x) => `<li>${echapper(x.phrase)}</li>`).join("")}</ul>` : "";
  // Les personnes mises de côté, chacune sa question (`identites`, du moteur) :
  // elles se répondent dans l'état de l'import.
  const questions = r.identites.length
    ? `<div class="encart attention"><b>Est-ce la même personne ?</b><ul>${r.identites.map((q) => `<li>${echapper(q.question)}</li>`).join("")}</ul></div>` : "";
  const bouton = r.ecrit
    ? `<div class="encart ${couleur}"><b>Repris</b> ${r.fichiers.map((f) => echapper(f)).join("<br>")}
        ${pose}${remplace}
        <p class="note">${aideDe("import_copies")}</p></div>`
    : r.rien_a_faire
      ? `${questions}<div class="encart ${couleur}">${aideDe("import_rien")}</div>`
      : `${questions}<p><button class="primaire" id="import-ecrire">Reprendre</button>
         <span class="discret">${echapper(r.reprendre)}</span>
         <span class="note">${aideDe("import_avant_clic")}</span></p>`;
  return `${vueNonRepris(r.tri.non_repris, r.pastilles)}
    ${compteRenduImport(r.tri, r.pastilles)}
    <h3>${echapper(r.titre_dit)}</h3>
    <div class="encart"><b>Déclarations</b>
      ${echapper(r.declarations_dit)}</div>
    ${bulletinsImportes(r.bulletins)}
    ${piecesImportees(r.certificats, "Certificats Congés Spectacles")}
    ${piecesImportees(r.aem_pdf, "AEM en PDF")}
    ${avert}
    ${fichesImportees(r.entreprise ? [r.entreprise] : [], "Entreprise")}
    ${fichesImportees(r.salaries, "Salariés")}
    ${fichesImportees(r.spectacles, "Spectacles")}
    ${ecartsBulletinDsn(r.tri)}
    ${bouton}`;
}

function brancherImportation() {
  const b = document.getElementById("import-ecrire");
  // « Reprendre », sous l'aperçu, est le même geste qu'« Importer ».
  if (b) b.onclick = () => importerDepuisLeGuide();
}

async function apercuImportation() {
  const cible = document.getElementById("import-resultat");
  // LE GESTE QUI ATTEND SE VOIT : le bouton dit qu'il lit, la page qu'elle travaille.
  const rendre = occupe(document.getElementById("import-apercu"), "Lecture…");
  cible.innerHTML = `<p class="discret">Lecture…</p>`;
  try {
    const r = await api("importation-apercu");
    cible.innerHTML = vueImportation(r);
    brancherImportation();
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  } finally {
    rendre();
  }
}

async function ecrireImportation() {
  const cible = document.getElementById("import-resultat");
  cible.classList.add("attente");
  occupe(document.getElementById("import-ecrire"), "Reprise…");
  try {
    const r = await api("importer");
    cible.innerHTML = vueImportation(r);
    brancherImportation();
    // Les fiches créées doivent apparaître dans Salariés et Spectacles, et
    // dans les déroulants de la saisie, sans recharger la page.
    await relireEtat();
    rafraichirReferentiels();
    // CE QUI RESTE À DIRE : la nature des contrats repris se demande en tête de la page ; on y va.
    await rafraichirEtatImportation();
    const aDire = document.querySelector("[data-nature-rang]");
    if (aDire) aDire.scrollIntoView({ block: "center" });
    dire(r.phrase, r.couleur);
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
  } finally {
    cible.classList.remove("attente");
  }
}

/* La fiche que le formulaire montre en ce moment : `quitterLaFiche` en a
 * besoin pour pouvoir l'enregistrer avant de partir. */
let FICHE_COURANTE = null;

/* CE QU'UN SPECTACLE, OU UN SALARIÉ, A COÛTÉ DANS L'ANNÉE.
 * L'éditeur : « si un administrateur veut savoir combien un de ses
 * spectacles lui a coûté dans l'année, il faut qu'il le trouve
 * facilement ». La réponse était dans Exports, un état parmi vingt-deux ;
 * elle est en tête de la fiche. Le calcul est celui du livre de
 * paie (route « couts ») : une fois par année et par séance, oublié dès
 * qu'une saisie s'enregistre. */
const COUTS = {};
function coutsDe(annee) {
  if (!COUTS[annee]) COUTS[annee] = api("couts", { annee });
  return COUTS[annee];
}
function oublierCouts() { Object.keys(COUTS).forEach((k) => delete COUTS[k]); }

async function peindreCoutAnnee() {
  const el = document.querySelector("#config-formulaire .cout-annee");
  if (!el) return;
  const an = anneeDuJour();
  const ligne = (r, annee) => {
    const d = r[el.dataset.cote][el.dataset.coutDe];
    if (!d || (!d.bulletins && !Number(d.prevu))) return "";
    return `<div><b>${annee}</b> : ${echapper(d.phrase)}</div>`;
  };
  el.innerHTML = `<span class="discret">${aideDe("cout_attente")}</span>`;
  let cette, avant;
  // Un refus se dit à la place du coût, et l'année se redemande à la prochaine fiche ouverte.
  try { [cette, avant] = await Promise.all([coutsDe(an), coutsDe(an - 1)]); } catch (e) {
    oublierCouts();
    if (el.isConnected) el.innerHTML = `<span class="discret">${echapper(e.message)}</span>`;
    return;
  }
  if (!el.isConnected) return;
  el.innerHTML = ligne(cette, an) + ligne(avant, an - 1);
}

/* L'ONGLET MÉTIERS. L'éditeur : « les métiers, c'est vraiment
 * un onglet à part : on a les métiers de base de notre convention, on peut
 * en rajouter » — et les cocher ou les décocher. Ce qui est coché se
 * propose dans la fiche d'un salarié et dans un engagement ; rien d'autre
 * ne change, aucun calcul. Par famille, repliée, avec son compte ; la
 * recherche ouvre ce qu'elle trouve. Les métiers des autres conventions ne
 * se montrent que si on le demande. Un métier qui manque se crée en en
 * copiant un, et un métier créé se modifie : ce sont les gestes que la
 * route rend (`gestes`, et `modifier` de chaque métier créé), que le
 * dessinateur commun dessine (`js/avance.js`). */
let METIERS_TOUS = false;
let METIERS_FILTRE = "";

async function dessinerMetiers(cible) {
  let r;
  try { r = await api("metiers-catalogue"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const sansAccent = (t) => String(t).normalize("NFD").replace(/[\u0300-\u036f]/g, "").toLowerCase();
  const peindre = () => {
    const filtre = sansAccent(METIERS_FILTRE.trim());
    const vus = r.metiers.filter((m) => (METIERS_TOUS || m.de_la_convention || m.retenu)
      && (!filtre || sansAccent(m.libelle).includes(filtre) || sansAccent(m.code).includes(filtre)));
    const familles = [...new Set(vus.map((m) => m.famille))];
    const retenus = r.metiers.filter((m) => m.retenu).length;
    document.getElementById("metiers-compte").innerHTML =
      `<span>Métiers cochés</span> <b>${retenus}</b>`;
    document.getElementById("metiers-liste").innerHTML = familles.length ? familles.map((f) => {
      const dedans = vus.filter((m) => m.famille === f);
      const n = dedans.filter((m) => m.retenu).length;
      return `<details class="metiers-famille" data-famille="${echapper(f)}"${filtre ? " open" : ""}>
        <summary><b>${echapper(f)}</b> <span class="discret">${n} / ${dedans.length}</span></summary>
        <div class="metiers-colonnes">${dedans.map((m) => `
          <span><label class="coche" title="${echapper(m.code)}${m.de ? " — " + echapper(m.de) : ""}">
            <input type="checkbox" data-code="${echapper(m.code)}"${m.retenu ? " checked" : ""}>
            ${echapper(m.libelle)}${m.de ? ` <span class="discret">· autre convention</span>` : ""}
          </label>${m.modifier ? ` <span data-gestes-du="${echapper(m.code)}">${boutonsDesGestes([m.modifier])}</span>` : ""}</span>`).join("")}</div>
      </details>`;
    }).join("") : `<p class="discret">Aucun métier</p>`;
    // Les gestes d'un métier créé : chacun dans sa place, branché seul.
    r.metiers.filter((m) => m.modifier).forEach((m) => {
      const place = document.querySelector(`[data-gestes-du="${CSS.escape(m.code)}"]`);
      if (place) brancherLesGestes(place, [m.modifier], { titre: m.libelle, modifie: true }, apresUnGeste);
    });
    document.querySelectorAll("#metiers-liste input[data-code]").forEach((c) => {
      c.onchange = async () => {
        try {
          const x = await api("metier-retenu", { code: c.dataset.code, retenu: c.checked });
          ETAT.emplois = x.emplois;
          ETAT.emplois_autres = x.emplois_autres;
          const m = r.metiers.find((y) => y.code === c.dataset.code);
          if (m) m.retenu = c.checked;
          const garde = [...document.querySelectorAll("#metiers-liste details[open]")].map((d) => d.dataset.famille);
          peindre();
          document.querySelectorAll("#metiers-liste details").forEach((d) => {
            if (garde.includes(d.dataset.famille)) d.open = true;
          });
        } catch (e) { c.checked = !c.checked; dire(e.message, "erreur"); }
      };
    });
  };
  cible.innerHTML = `
    <div class="carte">
      <h2>Métiers de la compagnie</h2>
      <p class="note">${echapper(r.phrase)}</p>
      <div class="metiers-outils">
        <input type="search" id="metiers-filtre" placeholder="Chercher"
               value="${echapper(METIERS_FILTRE)}">
        <label class="coche"><input type="checkbox" id="metiers-tous"${METIERS_TOUS ? " checked" : ""}>
          autres conventions aussi</label>
        <span id="metiers-compte" class="discret"></span>
      </div>
      <p id="metiers-gestes">${boutonsDesGestes(r.gestes)}</p>
      <div id="metiers-liste"></div>
    </div>`;
  // Après un geste, l'état relu (la carte propose les métiers de la route etat), puis l'onglet redessiné.
  const apresUnGeste = async () => {
    try { await relireEtat(); } catch (e) { dire(e.message, "erreur"); }
    await dessinerMetiers(cible);
  };
  brancherLesGestes(document.getElementById("metiers-gestes"), r.gestes, {}, apresUnGeste);
  document.getElementById("metiers-filtre").oninput = (e) => { METIERS_FILTRE = e.target.value; peindre(); };
  document.getElementById("metiers-tous").onchange = (e) => { METIERS_TOUS = e.target.checked; peindre(); };
  peindre();
}


/* SUPPRIMER LA COMPAGNIE , sous la fiche Entreprise. Le moteur dit si elle se supprime
   (`supprimable` de la route `etat` : jamais une fictive livrée, qui se remet à zéro) et ce que la boîte demande
   (`compagnie-supprimer`, sans `confirme`) : ce qui sera effacé ; une compagnie réelle, le rappel de la conservation des
   bulletins, la sauvegarde si un Drive est lié, puis le mot de passe du logiciel (le popup de chaque acte qui engage,
   `demanderMotDePasse`). La même route, confirmée, supprime ; la page revient à l'accueil. */
function dessinerSuppression() {
  const zone = document.getElementById("config-suppression");
  if (!zone) return;
  if (FICHE_ACTIVE !== "entreprise" || !ETAT || !ETAT.supprimable) { zone.innerHTML = ""; return; }
  zone.innerHTML = `<div class="encart attention suppression-compagnie">
      <b>Supprimer la compagnie</b>
      ${echapper(ETAT.suppression)}
      <div class="actions"><button type="button" class="secondaire alerte" id="supprimer-compagnie">Supprimer la compagnie</button></div>
    </div>`;
  document.getElementById("supprimer-compagnie").onclick = boiteSuppression;
}

async function boiteSuppression() {
  let q;
  try { q = await api("compagnie-supprimer"); }
  catch (e) { dire(e.message, "erreur"); return; }
  // Une compagnie qui ne se supprime pas : le moteur dit pourquoi.
  if (!q.genre) { dire(q.phrase, "erreur"); return; }
  const reelle = q.genre === "reelle";
  const s = q.sauvegarde;
  const sauvegarde = reelle && s && s.liee
    ? `<p>${echapper(s.phrase)}</p>
       <p><button type="button" class="secondaire" id="sup-sauvegarder">Sauvegarder maintenant</button>
       <span class="discret" id="sup-sauvegarde-etat"></span></p>`
    : "";
  ouvrirBoite(`<h3>${echapper(q.titre_dit)}</h3>
    <p>${echapper(q.avertissement)}</p>
    ${reelle ? `<p>${echapper(q.conservation)}</p>` : ""}
    ${sauvegarde}
    <p class="erreur" id="sup-erreur"></p>
    <div class="actions">
      <button type="button" class="secondaire" id="sup-annuler">Annuler</button>
      <button type="button" class="secondaire alerte" id="sup-confirmer">Supprimer définitivement</button>
    </div>`);
  document.getElementById("sup-annuler").onclick = fermerBoite;
  const bSauve = document.getElementById("sup-sauvegarder");
  if (bSauve) bSauve.onclick = async () => {
    const etat = document.getElementById("sup-sauvegarde-etat");
    bSauve.disabled = true;
    etat.textContent = "…";
    try {
      const v = await api("sauvegarde-maintenant");
      etat.textContent = v.phrase;
    } catch (e) { etat.textContent = e.message; }
    bSauve.disabled = false;
  };
  document.getElementById("sup-confirmer").onclick = async () => {
    let mdp = "";
    if (q.demande) {
      mdp = await demanderMotDePasse(q.demande);
      if (mdp === null) return;
    }
    try {
      await api("compagnie-supprimer", { confirme: true, ...(mdp ? { motdepasse: mdp } : {}) });
      location.replace("/");
    } catch (e) { document.getElementById("sup-erreur").textContent = e.message; }
  };
}
