/* L'historique. */
"use strict";

/* =======================================================================
   HISTORIQUE
   =======================================================================
   TOUS LES ENVOIS ET TOUS LES RETOURS SE LISENT ICI, en un seul endroit.
   La page Engagements est
   l'endroit où l'on FAIT le mois ; l'Historique, celui où l'on VOIT ce qu'il a
   produit, envoyé et reçu.

   Une ligne par mois (`etat`, `historique`) : son titre, sa pastille, ce que
   la route en dit, et le geste qui le reprend s'il reste à faire
   (`reprendre`). Elle se déplie sur la vue technique du mois, que le
   moteur rend (`historique-mois`) : ses mots (`ecran`), ses documents, chacun
   ouvrable ; ses gestes ; ses envois, tests de ce logiciel compris (repliés) ;
   et sous chacun ses retours dans l'ordre d'arrivée, LUS par le moteur —
   l'organisme, le statut, le verdict, les anomalies — avec la pièce archivée
   à ouvrir en dernier recours. La page n'y décide rien : elle affiche ce que
   les routes rendent, mots et gestes compris. */
function dessinerHistorique() {
  dessinerReprise();
  dessinerExercices();
  dessinerRegularisation();
  const cible = document.getElementById("historique");
  if (!ETAT.historique.length) {
    cible.innerHTML = `<p class="discret">${echapper(ETAT.historique_dit)}</p>`;
    return;
  }
  // L'état de la DSN, en mots et en couleur : c'est ce qu'on cherche en premier dans un mois passé (`pastille`).
  cible.innerHTML = ETAT.historique.map((m) => `
    <details class="mois-hist" data-mois="${echapper(m.mois)}">
      <summary class="rang mois">
        <span class="rang-nom">${echapper(m.titre)}</span>
        <span class="rang-etat"><span class="pastille ${CLASSE_DE_COULEUR[m.pastille.couleur]}">${echapper(m.pastille.mots)}${
          m.depose_dit ? " " + echapper(m.depose_dit) : ""}</span>${
          m.clos_dit ? ` <span class="pastille">${echapper(m.clos_dit)}</span>` : ""}</span>
        <span class="rang-infos">${m.documents_dit ? echapper(m.documents_dit) : ""}</span>
        <span class="rang-actions">
        </span>
      </summary>
      <div class="mois-vue"><p class="discret">Lecture…</p></div>
    </details>`).join("");
  cible.querySelectorAll("details.mois-hist").forEach((d) => {
    d.addEventListener("toggle", () => { if (d.open) dessinerMoisHistorique(d); });
  });
}

/** Le mois `mois` déplié dans l'Historique : depuis l'étape d'un mois (« voir les envois et leurs retours ») ou le
 *  bandeau. */
async function ouvrirMoisHistorique(mois) {
  document.querySelector('.onglets button[data-vue="historique"]').click();
  const d = document.querySelector(`details.mois-hist[data-mois="${CSS.escape(mois)}"]`);
  if (!d) return;
  d.open = true;
  d.scrollIntoView({ block: "start" });
}


/** La vue technique d'un mois : le geste qui le reprend, ses documents et ses gestes, ses envois et leurs retours, tels
 *  que les routes les rendent. */
async function dessinerMoisHistorique(d) {
  const mois = d.dataset.mois;
  const m = ETAT.historique.find((x) => x.mois === mois);
  const vue = d.querySelector(".mois-vue");
  let h;
  try {
    h = await api("historique-mois", { mois });
  } catch (e) {
    vue.innerHTML = `<p class="discret">${echapper(e.message)}</p>`;
    return;
  }
  const r = m.reprendre;
  const paie = r ? `<p><button class="secondaire" data-reprendre>${echapper(r.libelle)}</button>${
    m.reste_dit ? ` <span class="discret">— ${echapper(m.reste_dit)}</span>` : ""}</p>` : "";

  // Les documents, rangés sous leur sorte, dans l'ordre que le moteur donne.
  const sortes = [];
  h.documents.forEach((doc, i) => {
    let s = sortes.find((x) => x.nom === doc.sorte);
    if (!s) sortes.push(s = { nom: doc.sorte, docs: [] });
    s.docs.push(i);
  });
  const documents = sortes.length ? sortes.map((s) => `
    <div class="docs-sorte"><b>${echapper(s.nom)}</b>${s.docs.map((i) =>
      `<button class="lien" data-document="${i}" title="${echapper(h.documents[i].chemin)}">${echapper(h.documents[i].nom)}</button>`).join("")}
    </div>`).join("") : `<p class="discret">${echapper(h.documents_dit)}</p>`;

  const envois = h.envois.length ? h.envois.map((e) => dessinerEnvoi(e, h.ecran)).join("")
    : `<p class="discret">${echapper(h.envois_dit)}</p>`;

  vue.innerHTML = `${paie}
    <h3>${echapper(h.ecran.documents)}</h3>${documents}
    <div data-gestes>${boutonsRanges(h.gestes)}</div>
    <h3>${echapper(h.ecran.envois)}</h3>${envois}`;

  // Un document s'ouvre ; un document parti dont le fichier manque passe d'abord par son geste (`refaire`).
  vue.querySelectorAll("[data-document]").forEach((b) => {
    const x = h.documents[+b.dataset.document];
    b.onclick = () => (x.refaire ? ouvrirGeste(x.refaire, mois) : ouvrirEtDire(x.chemin));
  });
  vue.querySelectorAll("[data-piece]").forEach((b) => {
    b.onclick = () => ouvrirEtDire(b.dataset.piece);
  });
  brancherLesGestes(vue.querySelector("[data-gestes]"), h.gestes, { titre: m.titre }, async () => {});
  const reprendre = vue.querySelector("[data-reprendre]");
  if (reprendre) reprendre.onclick = async () => {
    // Quitter d'abord ce qui porte une saisie non enregistrée : l'onglet Engagements, ensuite, ne redemande rien.
    if (!(await quitterLEcran())) return;
    document.querySelector('.onglets button[data-vue="paie"]').click();
    await chargerMois(mois);
    // Le geste que la route rend : le mois, déjà ouvert (`etat-du-mois`), ou la boîte du dépôt de sa DSN en retard.
    ouvrirGeste(r, mois);
  };
}

/** Un envoi : ce qui est parti, quand, et sous lui ses retours. Un réel s'ouvre ; un test reste replié. */
function dessinerEnvoi(e, ecran) {
  const test = e.mode === "test";
  const id = e.identite;
  const retours = e.retours.length ? e.retours.map((r) => dessinerRetour(r, ecran)).join("")
    : e.retours_dit ? `<p class="discret retour">${echapper(e.retours_dit)}</p>` : "";
  return `
    <details class="envoi ${test ? "test" : "reel"}" ${test ? "" : "open"}>
      <summary><b>${echapper(e.acte)}</b>
        ${test ? `<span class="pastille">${echapper(e.mode)}</span>` : ""}
        ${pastilleDeLEtat(e.pastille)}
        <span class="discret">— ${echapper(e.resume)}</span></summary>
      ${id ? `<div class="envoi-id">${echapper(id)}</div>` : ""}
      ${retours}
    </details>`;
}

/** Un retour, lu : l'organisme, ce que dit sa pièce, ses anomalies ; la pièce elle-même en dernier recours. */
function dessinerRetour(r, ecran) {
  // Un retour d'organisme dit son titre, sa date et ses anomalies (`titre_dit`, `quand_dit`, `anomalies`) ; un autre,
  // son titre seul. Une case d'anomalie que la pièce ne dit pas reste vide.
  const titre = r.titre_dit || r.titre;
  const cellule = (v) => (v ? echapper(v) : "");
  const anomalies = r.anomalies && r.anomalies.length ? `
    <table class="anomalies"><tr>${ecran.anomalies.map((c) => `<th>${echapper(c)}</th>`).join("")}</tr>
    ${r.anomalies.map((a) => `<tr><td>${a.code ? `<code>${echapper(a.code)}</code>` : ""}</td>
      <td>${cellule(a.message)}${a.categorie ? ` <span class="discret">(${echapper(a.categorie)})</span>` : ""}${
        a.fois ? ` <span class="discret">${echapper(a.fois)}</span>` : ""}</td>
      <td>${cellule(a.rubrique)}</td>
      <td>${cellule(a.salarie)}</td></tr>`).join("")}</table>` : "";
  return `
    <details class="retour">
      <summary>${r.quand_dit ? `<span class="discret">${echapper(r.quand_dit)}</span>` : ""}
        ${r.organisme ? `${echapper(r.organisme)} — ` : ""}${echapper(titre)}
        ${pastilleDeLEtat(r.pastille)}</summary>
      <div class="retour-corps">
        ${r.nature_dit ? `<p class="discret">${echapper(r.nature_dit)}</p>` : ""}
        ${r.phrases.map((p) => `<p>${echapper(p)}</p>`).join("")}
        ${anomalies}
        ${r.piece ? `<p><button class="lien" data-piece="${echapper(r.piece)}">${echapper(ecran.ouvrir_piece)}</button>
          </p>` : ""}
      </div>
    </details>`;
}

/** LA LISTE DES MOIS SUIT L'ÉTAT GARDÉ. Elle se peint d'abord avec ce
 *  que la page sait ; puis le moteur rend l'état frais de la compagnie
 *  (la route `etat`), et la liste se repeint s'il a changé — un dépôt fait depuis l'ouverture s'y
 *  voit sans recharger. La page n'y décide rien : elle relit. Les mois dépliés
 *  le restent. */
async function rafraichirHistorique() {
  let etat;
  try { etat = await api("etat"); } catch (e) { dire(e.message, "erreur"); return; }
  if (JSON.stringify(etat.historique) === JSON.stringify(ETAT.historique)) return;
  ETAT.historique = etat.historique;
  ETAT.historique_dit = etat.historique_dit;
  if (document.getElementById("vue-historique").classList.contains("cachee")) return;
  const ouverts = [...document.querySelectorAll("details.mois-hist[open]")].map((d) => d.dataset.mois);
  dessinerHistorique();
  ouverts.forEach((m) => {
    const d = document.querySelector(`details.mois-hist[data-mois="${CSS.escape(m)}"]`);
    if (d) d.open = true;
  });
}
