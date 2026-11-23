/* Ce qui part : les étapes du mois, et les boîtes qui déposent, envoient et font signer. */
"use strict";

/* =======================================================================
   LES ÉTAPES DU MOIS
   =======================================================================
   Le moteur rend (`etat-du-mois`) les colonnes du mois et, étape par étape
   (DPAE, Contrats, DSN, Documents), les groupes à dessiner : chacun son état
   (`fait`, `a_faire`, `en_attente`, `refuse`), les colonnes qu'il couvre, ce
   qu'il écrit (`compte_texte` en gras, `quoi`, `echeance.court`) et le geste
   qu'ouvre un clic. La page en fait des cases alignées sur les colonnes — la
   première, la grande, porte ce qui est fait ; ce qui reste sort à côté — et
   relie les cases d'une étape à la suivante par des traits droits (l'arbre).
   Elle ne choisit ni une couleur hors de l'état, ni une phrase, ni un geste.

   RIEN NE PART D'UN CLIC : une case ouvre un récapitulatif, qui demande le
   mot de passe DU LOGICIEL, jamais celui de l'organisme (le moteur va le
   chercher lui-même au coffre). C'est le moteur qui refuse un acte, et lui
   seul : un garde-fou posé dans un front se contourne par l'autre.
*/

/** Le geste d'un groupe ou d'un carré : la boîte de la route que le moteur nomme (`route`, avec sa `charge` ; `puis` :
 *  l'acte qui suit). Un dossier à ouvrir porte son `chemin`. `sur` : le rappel dont vient le geste (la fin de contrat à
 *  compléter : son `objet`). */
function ouvrirGeste(action, mois, sur) {
  if (!action) return;
  if (action.chemin) return ouvrirEtDire(action.chemin, action.phrase, true);
  const c = action.charge;
  const m = c.mois || mois;
  switch (action.route) {
    case "depot-apercu":
      return c.quoi === "dpae" && !c.engagements ? boiteDpae(m) : boiteDepot(c, { testerDabord: !!action.tester });
    case "signature-apercu":
      return action.puis === "contrat-recu" ? boiteContratsEnAttente(m) : boiteSignature({ ...c, mois: m });
    case "envoi-apercu": return boiteEnvoi(m);
    case "historique-mois": return ouvrirMoisHistorique(m);
    // Une pièce partie dont le fichier manque : la route la refait et rend son chemin, qui s'ouvre.
    case "piece-partie": return api(action.route, c).then((r) => ouvrirEtDire(r.chemin), (e) => dire(e.message, "erreur"));
    case "contrat-recu": return noterRecu(m, c);
    case "avenant-apercu": return boiteAvenant(m, c.salarie, c.debut, c.numero);
    // La carte du contrat, sur la première case qui manque (`objet.a_saisir`) ; elle s'écrit par `puis`.
    case "saisie": {
      const f = sur.objet;
      return f.a_saisir.length ? allerALaCase({ ...f, mois: f.mois || m }, f.a_saisir[0].champ) : null;
    }
    case "etat-du-mois": return null;   // le mois, déjà ouvert
    case "cloture": return boiteCloture(action);
    case "export-compta": return lancerExportCompta(c.annee);
  }
}

/** La classe d'une case : la couleur que le moteur donne au groupe (`couleur`) ; une attente se dessine en deux moitiés. */
const classeDeCase = (g) => g.coupee ? "en-attente" : g.couleur;

/** Les colonnes déjà dessinées pour un mois : une colonne qui apparaît entre en glissant. */
const COLONNES_VUES = new Map();

/** Une case : un groupe d'une étape (`k`, son rang), placé sur ses colonnes. */
function dessinerCase(et, k, g, colonnes, neuves, gestes) {
  const de = colonnes.indexOf(g.colonnes[0]) + 1;
  const a = colonnes.indexOf(g.colonnes[g.colonnes.length - 1]) + 2;
  const classe = classeDeCase(g);
  const fini = g.etat === "fait";
  const cote = g.colonnes[0] !== colonnes[0];
  const neuf = g.colonnes.some((c) => neuves.has(c));
  const ech = g.echeance && g.echeance.court;
  const titre = [g.phrase || et.nom, et.phrase].filter(Boolean).join(" — ");
  let bouton = "";
  if (!fini && g.action) {
    gestes.push(g.action);
    bouton = `data-geste="${gestes.length - 1}" tabindex="0" role="button"`;
  }
  const quoi = `<span class="quoi">${g.compte_texte
    ? `<span class="compte">${echapper(g.compte_texte)}</span> ` : ""}${echapper(g.quoi)}</span>`;
  const limite = ech ? `<span class="ech">${echapper(ech)}</span>` : "";
  const corps = g.coupee ? `
      <span class="moitie faite"><span class="etape-rond">✓</span>${cote ? "" : `<b>${echapper(et.nom)}</b>`}</span>
      <span class="moitie attend"><span class="kase-texte">${quoi}${limite}</span></span>` : `
      <span class="etape-rond">${fini ? "✓" : k + 1}</span>
      <span class="kase-texte">
        <span class="ligne1"><b>${echapper(et.nom)}</b></span>${fini && !g.quoi ? "" : quoi}${limite}
      </span>`;
  return `<div class="kase ${classe}${cote ? " cote" : ""}${neuf ? " neuf" : ""}${bouton ? " cliquable" : ""}"
      data-k="${k}" style="grid-column:${de} / ${a}" title="${echapper(titre)}" ${bouton}>${corps}</div>`;
}

/* -------------------------------------------- le panneau des quatre étapes */
async function dessinerActions(mois) {
  const cible = document.getElementById("actions-mois");
  if (!mois) { cible.innerHTML = ""; return; }
  cible.innerHTML = `<p class="discret">Lecture…</p>`;
  let e;
  try {
    e = await api("etat-du-mois", { mois });
  } catch (err) {
    cible.innerHTML = `<p class="discret">${echapper(err.message)}</p>`;
    return;
  }
  const colonnes = e.colonnes.length ? e.colonnes : ["G"];
  const vues = COLONNES_VUES.get(mois);
  const neuves = new Set(vues ? colonnes.filter((c) => !vues.includes(c)) : []);
  COLONNES_VUES.set(mois, colonnes);
  const gabarit = colonnes.map((c, i) => i === 0 ? "minmax(0, 1.4fr)" : "minmax(0, 1fr)").join(" ");
  const gestes = [];
  const rangees = e.etapes.map((et, k) => {
    const cases = et.groupes.map((g) => dessinerCase(et, k, g, colonnes, neuves, gestes)).join("");
    return `<li class="rangee"><div class="kases" style="grid-template-columns:${gabarit}">${cases}</div></li>`;
  }).join("");
  cible.innerHTML = `<ol class="etapes">${rangees}<svg class="arbre" aria-hidden="true"></svg></ol>`;

  cible.querySelectorAll("[data-geste]").forEach((d) => {
    const ouvrir = (ev) => { ev.preventDefault(); ouvrirGeste(gestes[Number(d.dataset.geste)], mois); };
    d.addEventListener("click", ouvrir);
    if (d.getAttribute("role") === "button") {
      d.addEventListener("keydown", (ev) => { if (ev.key === "Enter" || ev.key === " ") ouvrir(ev); });
    }
  });
  dessinerArbre(cible.querySelector(".etapes"));
  if (!dessinerActions.observe) {
    // L'arbre suit la largeur du panneau (fenêtre redimensionnée, colonne qui réapparaît).
    dessinerActions.observe = new ResizeObserver(() => dessinerArbre(cible.querySelector(".etapes")));
    dessinerActions.observe.observe(cible);
  }
}

/* L'ARBRE : un trait droit relie une case d'une étape à une case de la suivante. Chaque case du haut a le sien vers le
 * bas si son milieu tombe dans une case ; chaque case du bas qu'aucun trait n'atteint a le sien vers le haut. Un trait
 * est orange si ses deux bouts restent à faire (le rouge compte comme l'orange, l'attente aussi), vert sinon. */
function dessinerArbre(liste) {
  const svg = liste && liste.querySelector(".arbre");
  if (!svg) return;
  const base = liste.getBoundingClientRect();
  if (!base.width) return;
  const rangs = [...new Set([...liste.querySelectorAll(".kase")].map((el) => el.dataset.k))];
  const boites = (k) => [...liste.querySelectorAll(`.kase[data-k="${k}"]`)].map((el) => {
    // Le trait d'une case coupée tombe dans sa moitié qui attend.
    const r = el.getBoundingClientRect();
    const attend = el.querySelector(".moitie.attend");
    const a = attend ? attend.getBoundingClientRect() : r;
    return { el, r, x: a.left + a.width / 2 };
  });
  const ouvert = (el) => !el.classList.contains("vert") && !el.classList.contains("gris");
  const couleur = (h, b) => ouvert(h) && ouvert(b) ? "orange" : "vert";
  const traits = [];
  for (let i = 0; i < rangs.length - 1; i++) {
    const dessus = boites(rangs[i]), dessous = boites(rangs[i + 1]);
    const dedans = (x, b) => x >= b.r.left && x <= b.r.right;
    dessous.forEach((d) => {
      const haut = dessus.find((h) => dedans(d.x, h));
      const deja = dessus.some((h) => dedans(h.x, d));
      if (haut && !deja) traits.push({ c: couleur(haut.el, d.el), x: d.x, y1: haut.r.bottom, y2: d.r.top });
    });
    dessus.forEach((h) => {
      const bas = dessous.find((d) => dedans(h.x, d));
      if (bas) traits.push({ c: couleur(h.el, bas.el), x: h.x, y1: h.r.bottom, y2: bas.r.top });
    });
  }
  const ordre = { vert: 1, orange: 2 };
  svg.innerHTML = traits.sort((a, b) => ordre[a.c] - ordre[b.c]).map((t) =>
    `<path class="${t.c}" d="M${t.x - base.left},${t.y1 - base.top} V${t.y2 - base.top}"/>`).join("");
}

/* Ce qu'il faut relire après avoir déposé ou envoyé : l'état du mois affiché, l'état de chaque engagement, et ses
 * chiffres (un acte peut refaire une paie : le taux de PAS revenu avec la DPAE), lus dans la même lecture du mois.
 * Les chiffres ne s'attendent pas : appelée pendant un enregistrement, l'aperçu prend son tour après lui. */
async function rafraichirActions() {
  const mois = document.getElementById("mois").value;
  await dessinerActions(mois);
  await dessinerEtatsDesEngagements(mois);
  if (mois && toutEstEcrit()) lireLeMois(mois).then((lu) => calculerApercu(lu.lectures.apercu)).catch((e) => dire(e.message, "erreur"));
}

/* ---------------------------------------- le récapitulatif d'un dépôt */

/** CE QUE LA DSN DÉCLARE, contrat par contrat, LU DANS LE FICHIER
 *  (ce que l'utilisateur veut voir et vérifier : que tous les contrats sont dans la DSN). Comparé aux contrats
 *  saisis du mois : un contrat absent du fichier, ou que l'engagement dit
 *  autrement, se dit en tête. */
function recapitulatifDsn(r) {
  if (!r) return "";
  if (r.erreur) return `<div class="encart attention"><b>Récapitulatif illisible</b>
    ${echapper(r.erreur)}</div>`;
  // Un nombre du fichier, à la française ; une correction le signe d'un vrai
  // moins (« −184 h » : ce qui s'ajoute à ce qui a déjà été déclaré).
  const signe = (x) => String(x).replace(/^-/, "−");
  const lignes = r.salaries.map((s) => s.contrats.map((c, i) => `
    <tr class="${c.ecarts.length ? "ecart" : ""}${c.correction ? " correction" : ""}">
      <td>${i === 0 ? `<b>${echapper(s.nom)}</b> ${echapper(s.prenoms)}` : ""}</td>
      <td>${echapper(c.numero)}</td>
      <td class="dates">${echapper(c.dates_dit)}</td>
      <td>${echapper(c.emploi)}</td>
      <td class="num">${echapper(c.activite_dit)}</td>
      <td class="num">${c.remunere && c.brut != null ? echapper(signe(euros(c.brut))) : "—"}</td>
      <td class="fin">${echapper(c.fin_dit)}</td>
    </tr>
    ${c.ecarts.length ? `<tr class="ecart"><td></td><td colspan="6">
      ${pastilleDeLEtat(c.pastille_ecarts)}</td></tr>` : ""}`).join("")).join("");
  return `
    <div class="encart ${ENCART_DE_COULEUR[r.couleur]}">
      <b>${echapper(r.titre)}</b> ${echapper(r.phrase)}
    </div>
    ${r.absents.length ? `<ul class="absents">${r.absents.map((x) =>
      `<li>${pastilleDeLEtat(x.pastille)} ${echapper(x.dit)}</li>`).join("")}</ul>` : ""}
    <table class="recap-dsn">
      <tr><th>Salarié</th><th>Contrat</th><th>Dates</th><th>Emploi</th>
        <th class="num">Activité</th><th class="num">Brut</th><th>Fin</th></tr>
      ${lignes}
    </table>`;
}

function corpsDsn(a) {
  const v = a.ordres;
  return `
    <p class="discret">${echapper(a.description)}</p>
    ${recapitulatifDsn(a.recapitulatif)}
    <details class="replie">
      <summary>${echapper(a.ordres_titre)}</summary>
      <div class="encart ${ENCART_DE_COULEUR[a.couleur_ordres]}">
      ${a.ordres_erreur ? `<p><b>${echapper(a.ordres_erreur)}</b></p>` : ""}
      ${v.length
        ? `<table class="ordres">
             <tr><th>À qui</th><th>Pour quoi</th><th class="nombre">Montant</th><th>Prélevé</th></tr>
             ${v.map((x) => `<tr>
               <td><b>${echapper(x.organisme)}</b></td>
               <td>${echapper(x.objet)}</td>
               <td class="nombre"><b>${echapper(euros(x.montant))}</b></td>
               <td title="${echapper(x.source)}">${echapper(x.quand)}</td></tr>
             ${x.credit ? `<tr class="credit"><td></td><td colspan="3">
               <div class="encart" title="${echapper(x.credit.source)}">
               <b>${echapper(x.credit.titre)}</b>
               ${echapper(x.credit.que_faire)}</div></td></tr>` : ""}`).join("")}
           </table>` : ""}
      ${echapper(a.ordres_phrase)}
      </div>
    </details>`;
}

function corpsDpae(a) {
  return `
    <p class="discret">${echapper(a.description)}</p>
    <table>
      <tr><th>Salarié</th><th>Contrat</th><th>Du</th><th>Au</th>
        <th>Trace d'envoi</th></tr>
      ${a.embauches.map((x) => `
        <tr>
          <td>${echapper(x.nom)}</td>
          <td>${x.type_contrat ? echapper(x.type_contrat) : ""}</td>
          <td>${echapper(jjmmaaaa(x.contrat.du))}</td>
          <td>${x.contrat.au ? echapper(jjmmaaaa(x.contrat.au))
                      : `<span class="discret">sans terme</span>`}</td>
          <td>${pastilleDeLEtat(x.pastille)}</td>
        </tr>`).join("")}
    </table>`;
}

/** LA DÉCLARATION SE DÉSIGNE, ELLE NE SE NOMME PAS PAR SON FICHIER : `charge`
 *  est ce que le moteur rend pour elle (`{quoi, mois, etablissement?,
 *  engagements?}`, ou le signalement écrit). Le moteur l'écrit ou la reprend
 *  (`depot-apercu`), et ce qu'il en dit se lit sous la barre. */
async function boiteDepot(charge, { testerDabord = false } = {}) {
  const quoi = charge.quoi;
  let a;
  try {
    a = await api("depot-apercu", charge);
  } catch (e) { dire(e.message, "erreur"); return; }
  const dit = [a.phrase, ...a.avertissements, ...a.informations].filter(Boolean);
  if (dit.length) dire(dit.join("  ·  "));
  rafraichirActions();

  ouvrirBoite(`
    <h3>${echapper(a.titre)}</h3>
    ${moisEnGros(a.mois)}
    ${quoi === "dsn" ? corpsDsn(a) : corpsDpae(a)}
    ${encarts(a.encarts)}
    <div id="d-bilan">${encartRepetition(a.repetition)}</div>
    ${noteFictive()}
    <div id="d-depots"></div>`);

  // LES DÉPÔTS QUE LA ROUTE OUVRE (`principal`, `autres`, `sans_test`), et pourquoi le réel ne l'est pas
  // (`reel_ferme`) : chacun un geste de `deposer`, qui demande lui-même le mot de passe du réel (`faireLeGeste`). Un
  // geste qui ouvre une boîte (le renvoi, le réel sans test) l'ouvre par-dessus celle-ci.
  const poserLesDepots = (x) => {
    const el = document.getElementById("d-depots");
    if (!el) return;
    const gestes = [...x.autres, x.principal, ...(x.sans_test ? [x.sans_test] : [])];
    const boutons = (garde, classe) => boutonsDesGestes(gestes, (g) => garde(g) && !g.demande, classe)
      + boutonsDesGestes(gestes, (g) => garde(g) && !!g.demande, `${classe} envoi`);
    el.innerHTML = `
      <div class="actions">
        <button class="secondaire" onclick="fermerBoite()">Annuler</button>
        ${boutons((g) => x.autres.includes(g), "secondaire")}
        ${boutons((g) => g === x.principal, "primaire")}
      </div>
      ${x.reel_ferme ? `<p class="discret">${echapper(x.reel_ferme)}</p>` : ""}
      ${x.sans_test ? `<p class="discret">${boutons((g) => g === x.sans_test, "lien")}</p>` : ""}`;
    brancherLesGestes(el, gestes, enteteDuDepot, apresLeDepot);
  };
  // Le refus d'un réel déjà parti pour ce mois ouvre son avertissement ; le renvoi, voulu, repart avec `forcer` (le
  // mot de passe se redemande).
  const enteteDuDepot = {
    pardessus: true,
    refus: (e, g) => {
      if (!e.detail || e.detail.classe !== "DsnDejaDeposee") return false;
      boiteRenvoiDsn(e.message, () => faireLeGeste({ ...g, charge: { ...g.charge, forcer: true } }, enteteDuDepot, apresLeDepot));
      return true;
    },
  };
  const apresLeDepot = async (r) => {
    if (r.mode === "test") {
      // LE TEST GARDE LA BOÎTE OUVERTE : on attend son compte rendu, et le réel s'ouvre ici même, sans avoir à revenir.
      dire(r.phrase, "envoye");
      rafraichirActions();
      await attendreBilan();
      return;
    }
    fermerBoite();
    dire(r.phrase, "envoye");
    if (r.avis) avisApresLeDepot(r);
    await rafraichirActions();
    suivreDepot(quoi, r.flux, r.mois);
  };
  // LE COMPTE RENDU DU TEST, ATTENDU DANS LA BOÎTE. Toutes les dix secondes,
  // cinq minutes au plus : il arrive le plus souvent dans la minute. Arrivé,
  // la boîte relit les dépôts que la route ouvre (conforme : le réel ; refusé :
  // toujours le test). Passé cinq minutes, on cesse de demander (chaque tour
  // relève les retours chez l'organisme : rien ne doit tourner sans fin) ; le
  // relevé ordinaire rangera le compte rendu, et l'étape du mois ouvrira alors
  // le réel.
  // La boîte peut se fermer pendant l'attente : l'écoute continue, et le dit
  // en bas de page quand le compte rendu arrive (« après un test aussi »).
  // `el` est CETTE boîte : une autre ouverte entre-temps n'est pas touchée.
  const attendreBilan = async () => {
    const el = document.getElementById("d-bilan");
    const ici = () => el && el.isConnected;
    const ecrire = (html) => { if (ici()) el.innerHTML = html; };
    // Seul le bilan du test (le certificat, ou le bilan d'anomalies) ouvre le
    // réel ou le ferme ; l'accusé d'enregistrement dit « envoi reçu », pas
    // « conforme » (le moteur refuse le réel sans lui, DSN comme DPAE).
    suiviRepris("bilan-repetition");
    for (let tour = 0; tour < 30; tour++) {
      let r = null;
      try {
        r = await api("bilan-repetition", charge);
        suiviRepris("bilan-repetition");
      } catch (e) { direLeRefusDuSuivi("bilan-repetition", e.message); }
      if (r && (r.etat === "testee" || r.etat === "test_refuse")) {
        ecrire(encartRepetition(r));
        if (!ici()) dire(`${r.titre} · ${r.phrase}`, r.couleur);
        else {
          try { poserLesDepots(await api("depot-apercu", charge)); } catch (e) { dire(e.message, "erreur"); }
        }
        rafraichirActions();
        return;
      }
      if (r) ecrire(encartRepetition(r) + phrasesDuReleve(r.a_dire).map((p) => `<p class="discret">${echapper(p)}</p>`).join(""));
      await new Promise((ok) => setTimeout(ok, enFictive() ? 3000 : 10000));
    }
  };

  poserLesDepots(a);
  // Un test déjà fait pour ce contenu qui attend encore son compte rendu : on l'attend ici.
  const rep = a.repetition;
  if (rep && rep.etat === "en_test") attendreBilan();
  // LE MOIS QU'ON VIENT DE FERMER PART EN TEST TOUT SEUL (l'action le dit, `tester`) :
  // « on produit la DSN de test complète et on attend le retour du test,
  // ensuite on propose d'envoyer la DSN réelle ». Un test n'engage rien.
  // Seul un test part ainsi : le principal que la route rend, quand c'en est un (jamais le réel).
  else if (testerDabord && a.principal.charge.mode === "test") {
    await faireLeGeste(a.principal, enteteDuDepot, apresLeDepot);
  }
}

/** UNE DSN RÉELLE DÉJÀ PARTIE POUR CE MOIS (le refus `DsnDejaDeposee` de `deposer`, sa phrase) : « on attend le retour,
 *  et si vraiment on y tient, après un avertissement, on peut renvoyer ». Ne pas renvoyer est le choix par défaut. */
function boiteRenvoiDsn(phrase, renvoyer) {
  ouvrirBoite(`
    <h3>DSN déjà partie</h3>
    ${encart({ couleur: "orange", phrase })}
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Ne pas renvoyer</button>
      <button class="secondaire envoi" id="d-renvoyer">Renvoyer quand même</button>
    </div>`);
  document.getElementById("d-renvoyer").onclick = () => { fermerBoite(); renvoyer(); };
}

/** APRÈS LE DÉPÔT RÉEL, L'AVIS QUE LE MOTEUR REND (`avis` de `deposer`) : il le rend quand il y a quelque chose à dire
 *  (les documents des salariés d'une DSN qui en a). */
function avisApresLeDepot(r) {
  ouvrirBoite(`
    <h3>DSN envoyée</h3>
    ${moisEnGros(r.mois)}
    ${encart(r.avis)}
    <div class="actions">
      <button class="primaire" onclick="fermerBoite()">Compris</button>
    </div>`);
}

/** L'ÉCOUTE DES RETOURS, JUSTE APRÈS UN DÉPÔT RÉEL. Toutes les quinze secondes,
 *  dix minutes au plus, sans attendre la veille de vingt minutes ; l'écran
 *  se repeint dès que le dépôt est tranché. */
async function suivreDepot(quoi, flux, mois) {
  if (!flux) return;
  suiviRepris("suivre-depot");
  for (let tour = 0; tour < 40; tour++) {
    await new Promise((ok) => setTimeout(ok, enFictive() ? 4000 : 15000));
    let r = null;
    try {
      r = await api("suivre-depot", { quoi, flux, mois });
      suiviRepris("suivre-depot");
    } catch (e) { direLeRefusDuSuivi("suivre-depot", e.message); continue; }
    const phrases = phrasesDuReleve(r.a_dire);
    if (r.fini) {
      dire([r.phrase, ...phrases].join(" "), r.couleur);
      await rafraichirActions();
      return;
    }
    if (phrases.length) dire(phrases.join(" "), "orange");
  }
}

/** Ce que le relevé d'un suivi a à dire (`a_dire` du moteur) : ses incidents, puis ce qu'il reste à faire. */
function phrasesDuReleve(aDire) {
  return [...aDire.incidents, ...aDire.a_faire];
}

/** Ce que le certificat de conformité (nature 41) dit de chaque personne. */
function certificatParPersonne(ligne) {
  // Le test conforme (`conforme`, sa ligne du journal) ; seul un certificat de conformité nomme des personnes (`dpae`).
  if (!ligne) return "";
  const dpae = ligne.retours.filter((x) => x.dpae).flatMap((x) => x.dpae);
  if (!dpae.length) return "";
  return `<ul class="liste-certificat">${dpae.map((x) => `
    <li>${echapper([x.nom, x.prenom].filter(Boolean).join(" "))} :
      ${pastilleDeLEtat(x.pastille)}</li>`).join("")}</ul>`;
}

/** Où en est le test de CE contenu, tel que le moteur le dit (`titre`, `phrase`, `couleur` de la répétition). */
function encartRepetition(r) {
  if (!r) return "";
  return `<div class="encart ${ENCART_DE_COULEUR[r.couleur]}"><b>${echapper(r.titre)}</b>
    ${echapper(r.phrase)}${r.etat === "testee" ? certificatParPersonne(r.conforme) : ""}</div>`;
}

/* ------------------------------------------ les documents aux salariés */
/** LES DOCUMENTS DU MOIS : la liste des messages, et la case `salaries` de leurs départs, en colonne ; on
 *  décoche. Deux gestes : Imprimer (un seul PDF, salarié par salarié : bulletin,
 *  certificat, AER) et Envoyer par courriel quand une boîte est posée. Les deux
 *  sont un départ : le moteur numérote et fige la paie. */
async function boiteEnvoi(mois) {
  let a;
  try {
    a = await api("envoi-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }

  // LES DÉPARTS QUE LA ROUTE OUVRE (`principal`, `autres`), chacun un geste dont la case `salaries` (ses choix, ce
  // qu'elle montre d'abord : `valeurs`) est la colonne cochée de la liste ; celui qui exige le mot de passe (`demande`)
  // le fait taper. La liste n'a de messages que si un départ est ouvert.
  const gestes = [...a.autres, ...(a.principal ? [a.principal] : [])];
  const coches = (cle) => a.principal.valeurs.salaries.includes(cle);
  const boutons = (garde, classe) => boutonsDesGestes(gestes, (g) => garde(g) && !g.demande, classe)
    + boutonsDesGestes(gestes, (g) => garde(g) && !!g.demande, `${classe} envoi`);
  ouvrirBoite(`
    <h3>${echapper(a.titre)}</h3>
    ${moisEnGros(mois)}
    ${a.boite_liee && a.expediteur_dit ? `<p class="discret">${echapper(a.expediteur_dit)}</p>` : ""}
    ${a.empechements.map((x) =>
      `<div class="encart attention">${echapper(x)}</div>`).join("")}
    ${a.avertissements.concat(a.informations).map((x) =>
      `<div class="encart info">${echapper(x)}</div>`).join("")}
    ${!a.messages.length ? "" : `<table class="table-envoi">
      <colgroup><col class="c-coche"><col class="c-salarie">${a.boite_liee ? `<col class="c-adresse">` : ""}<col class="c-pieces"></colgroup>
      <tr><th></th>${a.colonnes.map((c) => `<th>${echapper(c)}</th>`).join("")}</tr>
      ${a.messages.map((m) => `
        <tr>
          <td><input type="checkbox" class="e-qui" value="${echapper(m.cle)}" ${coches(m.cle) ? "checked" : ""}
                     aria-label="${echapper(m.nom)}"></td>
          <td>${echapper(m.nom)}${m.pastille ? `<br>${pastilleDeLEtat(m.pastille)}` : ""}</td>
          ${a.boite_liee ? `<td>${echapper(m.destinataire)}</td>` : ""}
          <td class="pieces">${m.sortes.map((x) => `<div>${echapper(x)}</div>`).join("")}</td>
        </tr>
        ${m.renvoi ? `<tr><td></td><td colspan="${a.colonnes.length}" class="discret">${echapper(a.changements_titre)}
          <ul>${m.changements.map((c) => `<li>${echapper(c)}</li>`).join("")}</ul></td></tr>` : ""}`).join("")}
    </table>`}
    ${a.deja ? `<div class="encart attention"><b>${echapper(a.deja.titre)}</b>
      ${echapper(a.deja.quand_dit)}</div>` : ""}
    ${gestes.some((g) => g.demande) ? noteFictive() : ""}
    <div class="actions" id="e-departs">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${boutons((g) => g !== a.principal, "secondaire")}
      ${boutons((g) => g === a.principal, "primaire")}
    </div>`);

  document.querySelectorAll("#e-departs [data-geste]").forEach((b) => {
    const g = gestes[+b.dataset.geste];
    b.onclick = async () => {
      const charge = { ...g.charge, [g.cases[0]]: [...document.querySelectorAll(".e-qui:checked")].map((c) => c.value) };
      const mdp = g.demande ? await demanderMotDePasse(g.demande, { envoi: true }) : null;
      if (g.demande && !mdp) return;
      const rendre = occupe(b);
      const repere = g.demande ? await repereCourrielsFictifs() : null;
      try {
        const r = await api(g.route, mdp ? { ...charge, motdepasse: mdp } : charge);
        fermerBoite();
        // L'impression rend le PDF, qui s'ouvre ; l'envoi par courriel, ce qui est parti.
        if (!g.demande) await ouvrirEtDire(r.chemin, r.phrase);
        else {
          dire(r.phrase, "envoye");
          afficherEnvoi(r, mois);
          fenetreCourrielsFictifs("documents", repere);
        }
        await rafraichirActions();
      } catch (e) {
        rendre();
        dire(e.message, "erreur");
      }
    };
  });
}

/* --------------------------------------------- les contrats à faire signer */

/** LES CONTRATS À FAIRE PARTIR : ceux que le moteur rend à cocher
 *  (`a_envoyer` : jamais partis, ou changés depuis), cochés comme le disent
 *  les cases de leurs départs (`valeurs` : tous, ou ceux des salariés que la
 *  charge choisit, `salaries`) ; on décoche ce qu'on ne fait pas partir.
 *  Un contrat signé qui a changé part en avenant. Deux gestes : Imprimer (un
 *  seul PDF de tout ce qui est coché), et Envoyer à la signature quand une
 *  boîte courriel est posée. `charge` : celle de `signature-apercu`. */

async function boiteSignature(charge) {
  const mois = charge.mois;
  let a;
  try {
    a = await api("signature-apercu", charge);
  } catch (e) { dire(e.message, "erreur"); return; }

  const contrats = a.a_envoyer;
  // LES DÉPARTS QUE LA ROUTE OUVRE (`principal`, `autres`), chacun un geste dont les cases (`contrats`, `avenants`) se
  // remplissent de la part de chaque contrat coché (`part`) ; celui qui exige le mot de passe (`demande`) le fait taper.
  // Un contrat est coché d'abord quand sa part est dans ce que ces cases montrent d'abord (`valeurs`, les mêmes pour
  // chaque départ) ; un contrat à cocher a toujours un départ ouvert.
  const gestes = [...a.autres, ...(a.principal ? [a.principal] : [])];
  const coche = (c) => {
    const g = a.principal;
    const montrees = g.cases.flatMap((k) => g.valeurs[k].map((x) => JSON.stringify(x)));
    return g.cases.some((k) => (c.part[k] || []).some((x) => montrees.includes(JSON.stringify(x))));
  };
  const boutons = (garde, classe) => boutonsDesGestes(gestes, (g) => garde(g) && !g.demande, classe)
    + boutonsDesGestes(gestes, (g) => garde(g) && !!g.demande, `${classe} envoi`);
  ouvrirBoite(`
    <h3>${echapper(a.titre)}</h3>
    ${moisEnGros(mois)}
    ${encart(a.signature)}
    ${a.boite_liee ? noteFictive() : ""}
    ${a.avertissements.map((x) => `<div class="encart info">${echapper(x)}</div>`).join("")}
    ${!contrats.length ? (a.rien_a_envoyer ? `<p class="discret">${echapper(a.rien_a_envoyer)}</p>` : "") : `<div class="liste-choix">
      ${contrats.map((c, i) => `
        <label class="choix">
          <input type="checkbox" class="s-qui" value="${echapper(c.salarie)}" data-i="${i}"
                 ${coche(c) ? "checked" : ""}>
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(c.periode_dit)}
            ${c.numero_dit ? `<span class="discret">(${echapper(c.numero_dit)})</span>` : ""}
            ${a.boite_liee && c.destinataire ? `<br><span class="discret">${echapper(c.destinataire)}</span>` : ""}
            ${c.courriel ? `<br><span class="s-courriel" data-i="${i}">${boutonsDesGestes([c.courriel])}</span>` : ""}
            ${c.pastille ? `<br>${pastilleDeLEtat(c.pastille)}` : ""}${c.voir_avenant ? `
              <button class="lien s-avenant" data-i="${i}">${echapper(c.voir_avenant.libelle)}</button>` : ""}</span>
        </label>`).join("")}
    </div>`}
    <div class="actions" id="s-departs">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${boutons((g) => g !== a.principal, "secondaire")}
      ${boutons((g) => g === a.principal, "primaire")}
    </div>`);

  // UN DÉPART : la charge du geste, chacune de ses cases remplie de la part des contrats cochés ; l'impression ouvre le
  // PDF de tous les contrats cochés, l'envoi à la signature dit ce qui est parti.
  document.querySelectorAll("#s-departs [data-geste]").forEach((b) => {
    const g = gestes[+b.dataset.geste];
    b.onclick = async () => {
      const parts = [...document.querySelectorAll(".s-qui:checked")].map((x) => contrats[+x.dataset.i].part);
      const charge = { ...g.charge, ...Object.fromEntries(g.cases.map((k) => [k, parts.flatMap((p) => p[k] || [])])) };
      const mdp = g.demande ? await demanderMotDePasse(g.demande, { envoi: true }) : null;
      if (g.demande && !mdp) return;
      const rendre = occupe(b);
      const repere = g.demande ? await repereCourrielsFictifs() : null;
      try {
        const r = await api(g.route, mdp ? { ...charge, motdepasse: mdp } : charge);
        fermerBoite();
        if (!g.demande) await ouvrirEtDire(r.chemin, r.phrase);
        else {
          dire(r.phrase, "envoye");
          afficherEnvoi(r, mois);
          if (enFictive()) fenetreCourrielsFictifs("signature", repere);
        }
        await rafraichirActions();
      } catch (e) {
        rendre();
        dire(e.message, "erreur");
      }
    };
  });
  // LE COURRIEL QUI MANQUE : le geste que la route rend sur le contrat (`courriel`, `poser-courriel`), sa boîte par-dessus
  // celle-ci ; il s'enregistre dans la fiche du salarié, et la boîte se rouvre avec lui, les mêmes salariés cochés.
  document.querySelectorAll(".s-courriel").forEach((el) => {
    const c = contrats[+el.dataset.i];
    el.onclick = (ev) => ev.preventDefault();   // dans un <label> : ne pas cocher la case
    brancherLesGestes(el, [c.courriel], { titre: c.nom, pardessus: true }, async (pose) => {
      await relireEtat();
      const gardes = [...document.querySelectorAll(".s-qui:checked")].map((x) => x.value);
      fermerBoite();
      dire(pose.phrase, "envoye");
      boiteSignature({ mois, salaries: gardes });
    });
  });
  // L'aperçu de son avenant, tel que la route le nomme (`voir_avenant`).
  document.querySelectorAll(".s-avenant").forEach((b) => {
    b.onclick = (ev) => {
      ev.preventDefault();                 // le bouton est dans un <label>
      fermerBoite();
      ouvrirGeste(contrats[+b.dataset.i].voir_avenant, mois);
    };
  });
}

/** LES CONTRATS EN ATTENTE : un contrat ou un avenant parti, pas revenu
 *  signé. Le moteur dit, pour chacun, où il en est (`pastille`),
 *  et ses gestes, route et charge : « Reçu signé » (`recu`, pour tous : ce
 *  qui part par courriel peut revenir signé sur papier) et « Relancer » ou
 *  « Envoyer par courriel » (`relancer`, boîte posée : parti par courriel, ou
 *  imprimé). Une note se défait : le
 *  moteur rend de quoi l'annuler (`annuler`) et la phrase qui la dit. */
async function boiteContratsEnAttente(mois, fait) {
  let a;
  try {
    a = await api("signature-apercu", { mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const contrats = a.en_attente;
  ouvrirBoite(`
    <h3>${echapper(a.titre_en_attente)}</h3>
    ${moisEnGros(mois)}
    ${fait ? `<div class="encart info" id="a-fait">${echapper(fait.texte)}
      <button class="lien" id="a-annuler">${echapper(fait.annuler.libelle)}</button></div>` : ""}
    ${!contrats.length ? `<p class="discret">${echapper(a.rien_en_attente)}</p>` : `<div class="liste-choix">
      ${contrats.map((c, i) => `
        <div class="choix">
          <span><b>${echapper(c.nom)}</b> — ${echapper(c.spectacle)},
            ${echapper(c.periode_dit)}
            ${c.numero_dit ? `<span class="discret">(${echapper(c.numero_dit)})</span>` : ""}
            <br>${pastilleDeLEtat(c.pastille)}</span>
          <span class="gestes-contrat">
            <button class="primaire petit a-recu" data-i="${i}">${echapper(c.recu.libelle)}</button>
            ${c.relancer ? `<button class="secondaire petit a-relancer" data-i="${i}"
              >${echapper(c.relancer.bouton)}</button>` : ""}
          </span>
        </div>`).join("")}
    </div>`}
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
    </div>`);

  const annuler = document.getElementById("a-annuler");
  if (annuler) annuler.onclick = async () => {
    const rendre = occupe(annuler, "…");
    try {
      await api(fait.annuler.route, fait.annuler.charge);
      await rafraichirActions();
      boiteContratsEnAttente(mois);
    } catch (e) { rendre(); dire(e.message, "erreur"); }
  };
  document.querySelectorAll(".a-recu").forEach((b) => {
    b.onclick = async () => {
      const c = contrats[Number(b.dataset.i)];
      const rendre = occupe(b, "…");
      try {
        const r = await api(c.recu.route, c.recu.charge);
        await rafraichirActions();
        boiteContratsEnAttente(mois, { texte: r.phrase, annuler: r.annuler });
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
  document.querySelectorAll(".a-relancer").forEach((b) => {
    b.onclick = async () => {
      const c = contrats[Number(b.dataset.i)];
      const mdp = await demanderMotDePasse(c.relancer.demande, { envoi: true });
      if (!mdp) return;
      const rendre = occupe(b, "…");
      try {
        const r = await api(c.relancer.route, { ...c.relancer.charge, motdepasse: mdp });
        fermerBoite();
        dire(r.phrase, "envoye");
        await rafraichirActions();
      } catch (e) { rendre(); dire(e.message, "erreur"); }
    };
  });
}

/** L'AVENANT. Le contrat parti se lit (son PDF), l'engagement
 *  d'aujourd'hui s'y compare, article par article : ce qui a changé se
 *  montre, l'ancien texte à côté du nouveau. L'avenant part à la signature
 *  par le même circuit « Je signe » que le contrat. Un contrat qui n'a pas
 *  commencé peut aussi être refait en entier (« annule et remplace »). */
async function boiteAvenant(mois, salarie, debut, numero) {
  let a;
  try {
    // Le numéro et le mois disent QUEL contrat, si le salarié en a deux qui commencent le même jour (retours Windows, B).
    a = await api("avenant-apercu", { salarie, debut, numero, mois });
  } catch (e) { dire(e.message, "erreur"); return; }
  const mods = a.modifications;
  const cite = (textes) => textes.map((x) => `<p class="citation">« ${echapper(x)} »</p>`).join("");
  ouvrirBoite(`
    <h3>${echapper(a.titre)}</h3>
    <p class="discret">${echapper(a.situation)}</p>
    ${a.avenants.map((x) => `<p class="discret">${echapper(x.phrase)}</p>`).join("")}
    ${encarts(a.encarts)}
    ${mods.length ? `
      ${mods.map((m) => m.phrase ? `<div class="modif-avenant">
        <b>${echapper(m.article)}</b>
        <p>${echapper(m.phrase)}</p>
      </div>` : `<div class="modif-avenant">
        <b>${echapper(m.titre_dit)}</b>
        <div class="discret">Avant</div>${cite(m.avant)}
        <div class="discret">Désormais</div>${cite(m.apres)}
      </div>`).join("")}
      <label>Date d'effet de l'avenant
        <input type="date" id="av-effet" value="${echapper(a.effet)}"></label>` : ""}
    ${a.contrat_signe ? `<div class="encart info">${echapper(a.contrat_signe.phrase)}
      <button class="lien" id="av-dossier">Ouvrir son dossier</button></div>` : ""}
    <p class="discret"><a href="#" id="av-pieces">Ce qui est parti</a></p>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Fermer</button>
      ${mods.length && a.remplacable && a.boite_liee ? `<button class="secondaire envoi" id="av-remplace">Annule et remplace</button>` : ""}
      ${mods.length && a.boite_liee ? `<button class="primaire envoi" id="av-envoyer">Envoyer l'avenant</button>` : ""}
    </div>`);
  // LE CONTRAT SIGNÉ HORS DU LOGICIEL : la route rend le chemin de son dossier, la fenêtre l'ouvre (et le crée).
  const dossier = document.getElementById("av-dossier");
  if (dossier) dossier.onclick = (ev) => {
    ev.preventDefault();
    ouvrirEtDire(a.contrat_signe.chemin, "", true);
  };
  document.getElementById("av-pieces").onclick = (ev) => {
    ev.preventDefault();
    fermerBoite();
    boitePieces({ salarie, numero, nom: a.nom, bornes: a.bornes, spectacle: a.spectacle,
                }, "contrat-signature");
  };
  const envoyer = document.getElementById("av-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const effet = document.getElementById("av-effet").value || a.effet;
    const mdp = await demanderMotDePasse(a.demande_envoyer, { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi…");
    try {
      const r = await api("demander-signature", { mois, avenants: [{ salarie, debut, numero, mois, effet }], motdepasse: mdp });
      fermerBoite();
      dire(r.phrase, "envoye");
      afficherEnvoi(r, a.mois);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
  const remplace = document.getElementById("av-remplace");
  if (remplace) remplace.onclick = async () => {
    const mdp = await demanderMotDePasse(a.demande_remplacer, { envoi: true });
    if (!mdp) return;
    const rendre = occupe(remplace, "Envoi…");
    try {
      const r = await api("demander-signature", { mois: a.mois, contrats: [{ salarie, debut, numero: numero || a.contrat }],
                                                  motdepasse: mdp, remplace: true });
      fermerBoite();
      dire(r.phrase, "envoye");
      afficherEnvoi(r, a.mois);
      await rafraichirActions();
    } catch (e) {
      rendre();
      dire(e.message, "erreur");
    }
  };
}

/** CE QUI EST PARTI, tel que la route d'envoi le rend (`r` : son titre, sa phrase, ses avertissements), sous le mois. */
function afficherEnvoi(r, mois) {
  document.getElementById("resultat").innerHTML = `
    <div class="carte" style="margin-top:20px">
      <h2>${echapper(r.titre)} — ${echapper(moisEnClair(mois))}</h2>
      <div class="encart bien">${echapper(r.phrase)}</div>
      ${r.avertissements.length
        ? `<p class="note">${echapper(r.avertissements.join(" · "))}</p>` : ""}
    </div>`;
}

