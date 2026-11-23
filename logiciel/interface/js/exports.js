/* Les exports : etats, dossier du salarie, CASC, livre, cloture, export compta. */
"use strict";

/* =======================================================================
   EXPORTS
   -----------------------------------------------------------------------
   L'endroit des exports comptables. Il ne porte que quatre questions :
   QUOI (l'état), QUAND (un mois, une année, toutes les années), COMMENT
   (le regroupement et le format), et un APERÇU avant d'écrire.

   Tout ce qui s'affiche ici vient du moteur : les états, les formats, les
   regroupements et les périodes sont rendus par `exports-periodes`. Un état
   ajouté au moteur (`routes/archives.rs`) apparaît dans ce menu sans
   qu'on touche à ce fichier — c'est la règle 2 vue depuis la page : elle
   n'affiche pas ce qu'elle croit savoir, elle affiche ce que le moteur dit.

   Rien ne part d'ici. Pas de mot de passe, donc, et pas de confirmation :
   un export lit, et le pire qu'il puisse faire est d'écrire un fichier de
   plus dans `exports/`.
   ===================================================================== */
let EXPORTS = null;          // ce que `exports-periodes` a rendu

async function dessinerExports() {
  brancherDroits();
  const cible = document.getElementById("exports-formulaire");
  try { EXPORTS = await api("exports-periodes"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const p = EXPORTS;
  if (p.ecran.vide) {
    cible.innerHTML = `<p class="discret">${echapper(p.ecran.vide)}</p>`;
    document.getElementById("exports-resultat").innerHTML = "";
    return;
  }
  const opt = (v, t, sel) =>
    `<option value="${echapper(v)}"${sel ? " selected" : ""}>${echapper(t)}</option>`;
  // D'ABORD LA FAMILLE, PUIS L'ETAT, PUIS SES QUESTIONS : rien n'est choisi d'office, et chaque choix deroule le
  // suivant. Une famille d'un seul etat le choisit d'elle-meme.
  const fams = p.familles;
  const choisir = `<option value="" selected disabled>${echapper(p.ecran.choisir)}</option>`;
  cible.innerHTML = `
    <div class="champs">
      <div class="champ">
        <label for="x-famille">${echapper(p.ecran.quoi)}</label>
        <select id="x-famille">
          ${choisir}
          ${fams.map((f, i) => opt(String(i), f.libelle)).join("")}
        </select>
      </div>
      <div class="champ cachee" id="x-etat-champ">
        <label for="x-etat">${echapper(p.ecran.lequel)}</label>
        <select id="x-etat"></select>
      </div>
    </div>
    <div id="x-suite"></div>`;
  const choixEtat = document.getElementById("x-etat");
  document.getElementById("x-famille").onchange = () => {
    const liste = fams[Number(document.getElementById("x-famille").value)].etats;
    document.getElementById("x-etat-champ").classList.remove("cachee");
    choixEtat.innerHTML = (liste.length > 1 ? choisir : "")
      + liste.map((e) => opt(e.etat, e.quoi)).join("");
    document.getElementById("x-suite").innerHTML = "";
    document.getElementById("exports-resultat").innerHTML = "";
    if (liste.length === 1) dessinerQuestions(opt);
  };
  choixEtat.onchange = () => dessinerQuestions(opt);
  dessinerDossier();
}

/* ------------------------------------------- le dossier d'un salarié */

/** TOUS SES DOCUMENTS, EN UNE FOIS : les gestes que `dossier-formes` rend (l'aperçu, l'écrire, l'envoyer), leurs cases
 *  saisies ensemble, une fois chacune (`casesDesChamps`, depuis la fiche de leur route), et ce que la route en dit.
 *  Chaque geste ne prend que ses cases (`chargeDuGeste`) ; celui qui exige le mot de passe le fait taper, comme tout
 *  envoi. La page ne décide rien : elle coche, le moteur répond. */
let DOSSIER = null;
async function dessinerDossier() {
  const cible = document.getElementById("dossier-formulaire");
  if (!cible) return;
  try { DOSSIER = await api("dossier-formes"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  const f = DOSSIER;
  // Les cases de tous les gestes, chacune une fois, dans l'ordre où ils les nomment.
  const champs = [];
  const valeurs = {};
  const choix = {};
  for (const g of f.gestes) for (const c of champsDuGeste(g)) {
    if (champs.some((x) => x.nom === c.nom)) continue;
    champs.push(c);
    valeurs[c.nom] = g.valeurs[c.nom];
    if (g.choix[c.nom]) choix[c.nom] = g.choix[c.nom];
  }
  // VENU D'UNE FICHE SALARIÉ (« Exporter ses documents ») : ce salarié, coché.
  const venu = DOSSIER_PRECHOIX && (choix.salaries || []).some((s) => s.code === DOSSIER_PRECHOIX);
  if (venu) valeurs.salaries = [DOSSIER_PRECHOIX];
  cible.innerHTML = `
    ${f.ecran.dit.map((p) => `<p class="note">${echapper(p)}</p>`).join("")}
    <div id="d-cases">${casesDesChamps(champs, valeurs, choix)}</div>
    <div class="actions">
      ${boutonsDesGestes(f.gestes)}
      <button class="lien" id="d-ouvrir" title="${echapper(f.dossier)}">${echapper(f.ecran.dossier_dit)}</button>
    </div>`;
  const bloc = document.getElementById("d-cases");
  brancherLesCases(bloc);
  cible.querySelectorAll("[data-geste]").forEach((b) => {
    b.onclick = () => lancerDossier(f.gestes[+b.dataset.geste], bloc);
  });
  if (venu) {
    document.getElementById("carte-dossier").open = true;
    cible.scrollIntoView({ behavior: "smooth", block: "start" });
  }
  DOSSIER_PRECHOIX = null;
  document.getElementById("d-ouvrir").onclick = async () => {
    await ouvrirEtDire(DOSSIER.dossier);
  };
}

/** Le geste `g` du dossier, avec ce que ses cases disent dans `bloc` ; un geste qui exige le mot de passe le fait taper
 *  d'abord, et l'envoi montre les courriels fictifs. Le résultat : ce que la route en dit. */
async function lancerDossier(g, bloc) {
  const charge = chargeDuGeste(bloc, g) || { ...g.charge };
  if (marquerLesManquantes(bloc, champsDuGeste(g), charge)) return;
  if (g.demande) {
    const mdp = await demanderMotDePasse(g.demande, { envoi: true });
    if (!mdp) return;
    charge.motdepasse = mdp;
  }
  const cible = document.getElementById("dossier-resultat");
  cible.innerHTML = `<p class="discret">…</p>`;
  const repere = g.demande ? await repereCourrielsFictifs() : null;
  let r;
  try { r = await api(g.route, charge); }
  catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  if (r.envoi) fenetreCourrielsFictifs("documents", repere);
  cible.innerHTML = `
    <h3>${echapper(r.titre)}</h3>
    ${r.refus.map((a) => `<div class="encart alerte">${echapper(a)}</div>`).join("")}
    ${r.avertissements.map((a) => `<div class="encart">${echapper(a)}</div>`).join("")}
    ${r.lignes.map((l) => `<p>${echapper(l)}</p>`).join("")}
    ${r.ecrit_dit ? `<p class="note">${echapper(r.ecrit_dit)}</p>` : ""}
    ${r.envoi ? `<div class="encart ok">${echapper(r.phrase)}</div>` : ""}`;
}

/** LES QUESTIONS DE L'ÉTAT CHOISI, et rien d'autre. L'éditeur :
 *  « quand on arrive sur les exports on n'a qu'une case, et selon ce qu'on
 *  choisit on arrive sur ce qui est pertinent ». Quelles questions, leurs mots,
 *  ce qu'elles offrent et montrent d'abord, quel bouton, un aperçu ou non :
 *  c'est le moteur qui le dit (`exports-periodes`). */
function dessinerQuestions(opt) {
  const p = EXPORTS;
  const e = p.ecran;
  const forme = p.familles.flatMap((f) => f.etats).find((x) => x.etat === document.getElementById("x-etat").value);
  const q = new Set(forme.questions);
  document.getElementById("exports-resultat").innerHTML = "";
  document.getElementById("x-suite").innerHTML = `
    <div class="champs">
      ${q.has("periode") ? `<div class="champ pleine" id="x-periode">${caseDeRoute({ cle: "periode", libelle: e.quand,
        forme: "periode", valeur: forme.periode_proposee, choix: forme.periodes_offertes })}</div>` : ""}
      ${q.has("regroupement") ? `<div class="champ">
        <label for="x-par">${echapper(e.regroupement_libelle)}</label>
        <select id="x-par">
          ${forme.regroupements_offerts.map((r) => opt(r.code, r.libelle, r.code === forme.regroupement_propose)).join("")}
        </select>
        <div class="aide">${echapper(forme.regroupement_dit)}</div>
      </div>` : ""}
      ${q.has("logiciel") ? `<div class="champ pleine">
        <label for="x-logiciel">${echapper(e.logiciel_libelle)}</label>
        <select id="x-logiciel">
          ${p.compta.logiciels.map((l) => opt(l.logiciel, l.libelle, l.logiciel === p.compta.logiciel)).join("")}
          ${opt("", e.aucun_logiciel, !p.compta.logiciel)}
        </select>
        <div class="aide" id="x-logiciel-aide"></div>
        <label class="choix" id="x-analytique-champ">
          <input type="checkbox" id="x-analytique"${p.compta.analytique ? " checked" : ""}>
          <span>${echapper(e.analytique_libelle)}</span>
        </label>
        <div class="aide">${echapper(e.analytique)}</div>
        <div id="x-controle"></div>
      </div>` : ""}
      ${q.has("format") ? `<div class="champ" id="x-format-champ">
        <label for="x-format">${echapper(e.format_libelle)}</label>
        <select id="x-format">
          ${forme.formats_offerts.map((f) => opt(f.code, f.libelle, f.code === forme.format_propose)).join("")}
        </select>
      </div>` : ""}
      ${q.has("coordonnees") ? `<div class="champ pleine">
        <label class="coche" title="${echapper(e.coordonnees)}">
          <input type="checkbox" id="x-coordonnees">
          ${echapper(e.coordonnees_libelle)}
        </label>
      </div>` : ""}
    </div>
    <div class="actions">
      ${forme.apercu ? `<button class="secondaire" id="x-apercu">${echapper(e.apercu)}</button>` : ""}
      <button class="primaire" id="x-ecrire">${echapper(forme.bouton)}</button>
      <button class="lien" id="x-dossier" title="${echapper(e.dossier)}">${echapper(e.dossier_dit)}</button>
    </div>`;
  if (q.has("periode")) {
    const periode = document.getElementById("x-periode");
    brancherLesCases(periode);
    periode.addEventListener("change", () => { document.getElementById("exports-resultat").innerHTML = ""; });
  }
  if (q.has("logiciel")) {
    const retenir = async () => {
      document.getElementById("exports-resultat").innerHTML = "";
      // « Aucun » ne nomme aucun logiciel : la charge n'en porte pas.
      const charge = { analytique: document.getElementById("x-analytique").checked };
      const logiciel = document.getElementById("x-logiciel").value;
      if (logiciel) charge.logiciel = logiciel;
      try {
        EXPORTS.compta = await api("retenir-logiciel", charge);
      } catch (e) { dire(e.message, "erreur"); }
      montrerLogiciel();
    };
    document.getElementById("x-logiciel").onchange = retenir;
    document.getElementById("x-analytique").onchange = retenir;
    const f = document.getElementById("x-format");
    if (f) f.addEventListener("change", montrerLogiciel);
    montrerLogiciel();
  }
  if (forme.apercu) document.getElementById("x-apercu").onclick = () => lancerExport(true);
  document.getElementById("x-ecrire").onclick = () => lancerExport(false);
  document.getElementById("x-dossier").onclick = async () => {
    await ouvrirEtDire(EXPORTS.dossier);
  };
}

/** LE LOGICIEL DU COMPTABLE. Choisi une fois, retenu par le
 *  moteur (config/compta.json) : il fixe le format, et le menu des formats
 *  s'efface. La case d'analytique ne vaut que pour les formats qui la
 *  portent. Ce que le plan comptable ne tiendrait pas dans ce format est dit
 *  ici, avant d'exporter — c'est le moteur qui le dit. */
function montrerLogiciel() {
  const c = EXPORTS.compta;
  const choisi = c.logiciels.find((l) => l.logiciel === document.getElementById("x-logiciel").value);
  const fmt = document.getElementById("x-format-champ");
  if (fmt) fmt.classList.toggle("cachee", !!choisi);
  document.getElementById("x-logiciel-aide").textContent = choisi ? choisi.dit : EXPORTS.ecran.format_libre;
  // Sans logiciel, la case suit le format choisi à la main.
  const f = document.getElementById("x-format");
  const porte = choisi ? choisi.analytique
    : c.logiciels.some((l) => l.analytique && f && l.format === f.value);
  document.getElementById("x-analytique-champ").classList.toggle("cachee", !porte);
  document.getElementById("x-controle").innerHTML = c.controle.map((m) =>
    `<div class="encart alerte">${echapper(m)}</div>`).join("") + `<div id="x-courts"></div>`;
  montrerCourts(choisi ? choisi.format : (f ? f.value : ""));
}

/** LES LIBELLÉS COURTS. Ce que le format choisi ne tient pas, le moteur le
 *  dit (`libelles-courts`) : la phrase et le renvoi à la boîte, seulement s'il
 *  y a quelque chose à raccourcir. */
async function montrerCourts(format) {
  const cible = document.getElementById("x-courts");
  if (!cible) return;
  let r;
  try { r = await api("libelles-courts", { format }); }
  catch (e) { cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`; return; }
  cible.innerHTML = r.libelles.length ? `<div class="encart alerte">${echapper(r.phrase)}
    <button class="lien" id="x-courts-ouvrir">${echapper(r.ecran.raccourcir)}</button></div>` : "";
  if (r.libelles.length) document.getElementById("x-courts-ouvrir").onclick = () => ouvrirCourts(r.format);
}

/** LA BOÎTE DES LIBELLÉS COURTS : chaque libellé trop long, la place qu'il a,
 *  et ce que sa case montre (`valeur`, dit par le moteur). On la garde ou on la
 *  change ; « Enregistrer » les valide toutes. Le moteur vérifie chacune et
 *  refuse sans rien écrire. Les mots de la boîte : `ecran`. */
async function ouvrirCourts(format, phrase) {
  let r;
  try { r = await api("libelles-courts", { format }); }
  catch (e) { dire(e.message, "erreur"); return; }
  const lignes = r.libelles.map((l, i) => `<tr>
      <td>${echapper(l.zone)}</td><td>${echapper(l.long)}</td>
      <td><input type="text" class="x-court" data-i="${i}" size="${l.largeur + 2}" value="${echapper(l.valeur)}"></td>
    </tr>`).join("");
  ouvrirBoite(`<h3>${echapper(r.ecran.titre)}</h3>
    <div class="encart" id="x-courts-dit">${echapper(phrase || r.phrase)}</div>
    <table class="tableau"><tr>${r.ecran.colonnes.map((c) => `<th>${echapper(c)}</th>`).join("")}</tr>${lignes}</table>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">${echapper(r.ecran.fermer)}</button>
      <button class="primaire" id="x-courts-enregistrer">${echapper(r.ecran.enregistrer)}</button>
    </div>`);
  document.getElementById("x-courts-enregistrer").onclick = async () => {
    const libelles = [...document.querySelectorAll(".x-court")].map((c) => {
      const l = r.libelles[Number(c.dataset.i)];
      return { quoi: l.quoi, cle: l.cle, court: c.value };
    });
    try {
      const fait = await api("libelles-courts-enregistrer", { format: r.format, libelles });
      ouvrirCourts(r.format, fait.phrase);
      if (document.getElementById("x-courts")) montrerCourts(r.format);
    } catch (e) {
      const dit = document.getElementById("x-courts-dit");
      dit.className = "encart erreur";
      dit.textContent = e.message;
    }
  };
}

/** `exporter` prend encore la période en trois champs (`mois`, `annee`, `toutes_les_annees`) : le code choisi dans le
 *  calendrier (`AAAA-MM`, `AAAA`, ou celui de toutes les années) va dans le sien. */
function periodeEnTrois(code) {
  if (/^\d{4}-\d{2}$/.test(code)) return { mois: code };
  if (/^\d{4}$/.test(code)) return { annee: Number(code) };
  return { toutes_les_annees: true };
}

async function lancerExport(apercu) {
  // Une case absente ou vide n'est pas dite : elle ne part pas.
  const val = (id) => { const e = document.getElementById(id); return e && e.value !== "" ? e.value : undefined; };
  const casePeriode = document.querySelector('#x-periode .case[data-cle="periode"] > [data-case]');
  const periode = casePeriode ? lireLaCase(casePeriode) : null;
  const coord = document.getElementById("x-coordonnees");
  const charge = {
    etat: val("x-etat"),
    regroupement: val("x-par"),
    format: val("x-format"),
    coordonnees: coord ? coord.checked : false,
    apercu,
  };
  const logiciel = val("x-logiciel");
  if (logiciel !== undefined) {
    if (logiciel) charge.logiciel = logiciel;
    const champ = document.getElementById("x-analytique-champ");
    charge.analytique = !champ.classList.contains("cachee")
      && document.getElementById("x-analytique").checked;
  }
  if (periode) Object.assign(charge, periodeEnTrois(periode));
  const cible = document.getElementById("exports-resultat");
  cible.innerHTML = `<p class="discret">…</p>`;
  let r;
  try { r = await api("exporter", charge); }
  catch (e) {
    // Des libellés trop longs pour le format : le refus y renvoie.
    const courts = e.detail && e.detail.a_part && e.detail.a_part.libelles_courts;
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}${courts
      ? ` <button class="lien" id="x-courts-refus">${echapper(EXPORTS.ecran.raccourcir)}</button>` : ""}</div>`;
    if (courts) document.getElementById("x-courts-refus").onclick = () => ouvrirCourts(courts);
    return;
  }
  if (r.etat === "regularisation-casc-svp") { dessinerCasc(r); return; }
  if (r.colonnes && r.sections) { dessinerTableau(r, apercu); return; }
  dessinerLivre(r, apercu);
}

/** LA RÉGULARISATION CASC-SVP. Deux étapes, dans l'ordre où on les vit
 *  : faire les virements, puis envoyer — par la boîte
 *  de la compagnie si elle est posée, c'est le premier choix et il se voit ;
 *  sinon à la main, les fichiers sont là. Le détail des mois se déplie. */
function dessinerCasc(r) {
  const cible = document.getElementById("exports-resultat");
  const lignes = r.mois.map((m) => `<tr><td>${echapper(m.libelle)}</td>
      <td class="r">${echapper(m.masse)}</td><td class="r">${echapper(m.taux)} %</td>
      <td class="r">${echapper(m.cotisation)}</td></tr>`).join("");
  const annees = r.annees.map((a) => `<tr class="sous-total"><td><b>${echapper(a.annee)}</b></td>
      <td class="r">${echapper(a.masse)}</td><td></td><td class="r"><b>${echapper(a.cotisation)}</b></td></tr>`).join("");
  const courriel = (c, titre) => `<details><summary>${titre} — ${echapper(c.entete_dit)}</summary>
      <pre class="courriel">${echapper(c.corps)}</pre></details>`;
  const n = r.annees.length;
  cible.innerHTML = `
    <h3>Régularisation CASC-SVP — ${echapper(r.periode)} — ${echapper(euros(r.total))}</h3>
    ${r.avertissements.map((a) => `<div class="encart alerte">${echapper(a)}</div>`).join("")}
    <div class="casc-etape">
      <h4>${echapper(r.virements_titre)}</h4>
      <ul>${r.annees.map((a) => `<li><b>${echapper(euros(a.cotisation))}</b>, ${echapper(a.libelle_dit)}</li>`).join("")}</ul>
      <p class="discret">${echapper(r.ecran.virement)}</p>
    </div>
    <div class="casc-etape">
      <h4>Les deux courriels</h4>
      <div class="casc-choix">
        <div class="casc-option principale">
          <h5>Depuis la boîte</h5>
          ${r.boite_liee ? `
          <div class="e-contrat-champs">
            ${r.annees.map((a) => `<label>${echapper(a.date_libelle)}
              <input type="date" class="casc-date" data-annee="${echapper(a.annee)}"></label>`).join("")}
          </div>
          <button class="primaire" id="casc-envoyer">Envoyer les 2 courriels</button>
          <p class="discret">${echapper(r.ecran.automatique)}</p>` : `
          <p>${echapper(r.ecran.automatique)}</p>`}
        </div>
        <div class="casc-option">
          <h5>À la main</h5>
          <p>${echapper(r.ecran.a_la_main)}</p>
          ${r.dossier ? `<button class="secondaire" id="casc-dossier">Ouvrir les fichiers</button>` : ""}
          ${courriel(r.courriel_audiens, "Courriel à Audiens")}
          ${courriel(r.courriel_casc, "Courriel au CASC-SVP")}
          <div class="e-contrat-champs">
            <label>Année <select id="casc-annee">${r.annees.map((a) =>
              `<option>${echapper(a.annee)}</option>`).join("")}</select></label>
            <label>Virée le <input type="date" id="casc-vire-le"></label>
            <label>Montant <input type="text" id="casc-montant" size="8"></label>
            <button class="secondaire" id="casc-noter">Noter comme régularisé</button>
          </div>
        </div>
      </div>
    </div>
    <details><summary>Détail par mois</summary>
      <table class="livre"><thead><tr><th>mois</th><th class="r">masse</th><th class="r">taux</th>
        <th class="r">CASC</th></tr></thead><tbody>${lignes}${annees}</tbody></table>
    </details>`;
  if (r.dossier) document.getElementById("casc-dossier").onclick = async () => {
    await ouvrirEtDire(r.dossier);
  };
  const choixAnnee = document.getElementById("casc-annee");
  const montant = document.getElementById("casc-montant");
  const preremplir = () => {
    const a = r.annees.find((x) => x.annee === Number(choixAnnee.value));
    montant.value = a ? a.cotisation : "";
  };
  choixAnnee.onchange = preremplir;
  preremplir();
  document.getElementById("casc-noter").onclick = async () => {
    const vire = document.getElementById("casc-vire-le").value;
    try {
      const e = await api("casc-regularise",
        { annee: Number(choixAnnee.value), vire_le: vire, montant: montant.value });
      dire(e.phrase);
    } catch (x) { dire(x.message, "erreur"); }
  };
  const envoyer = document.getElementById("casc-envoyer");
  if (envoyer) envoyer.onclick = async () => {
    const vire_le = {};
    for (const c of document.querySelectorAll(".casc-date")) if (c.value) vire_le[c.dataset.annee] = c.value;
    const liste = Object.keys(vire_le);
    const mdp = await demanderMotDePasse(r.ecran.demande, { envoi: true });
    if (!mdp) return;
    const rendre = occupe(envoyer, "Envoi…");
    const repere = await repereCourrielsFictifs();
    try {
      const e = await api("casc-envoyer", { vire_le, motdepasse: mdp });
      dire(e.phrase, "envoye");
      cible.innerHTML = `<p>${echapper(e.phrase)}</p>`;
      fenetreCourrielsFictifs("courriel", repere);
    } catch (x) {
      rendre();
      dire(x.message, "erreur");
    }
  };
}

/** UN ÉTAT DE LA MACHINERIE COMMUNE : ses colonnes, ses
 *  sections et ses totaux viennent du moteur (`tableaux.Tableau`). La page ne
 *  sait rien de chaque état : un état neuf s'affiche sans qu'on touche ici. */
function dessinerTableau(r, apercu) {
  const cible = document.getElementById("exports-resultat");
  const droite = (c) => ["montant", "nombre", "taux"].includes(c.type);
  const cel = (c, v) => {
    const t = v === null || v === undefined ? "" : String(v);
    return `<td${droite(c) ? ' class="r"' : ""}>${echapper(droite(c) ? t.replace(".", ",") : t)}</td>`;
  };
  const rang = (d, classe) => `<tr${classe ? ` class="${classe}"` : ""}>${
    r.colonnes.map((c) => cel(c, d[c.cle])).join("")}</tr>`;
  const plusieurs = r.sections.length > 1;
  const corps = r.sections.map((s) =>
    (plusieurs && s.libelle ? `<tr class="sous-total"><td colspan="${r.colonnes.length}"><b>${
      echapper(s.libelle)}</b></td></tr>` : "")
    + s.lignes.map((l) => rang(l, "")).join("")
    + (plusieurs && s.total ? rang(s.total, "sous-total") : "")).join("");
  const n = r.sections.reduce((a, s) => a + s.lignes.length, 0);
  cible.innerHTML = `
    <h2>${echapper(r.titre)} — ${echapper(r.periode)}</h2>
    <p class="discret">${echapper(r.phrase)}</p>
    ${r.ecran.personnelles ? `<div class="encart alerte">${echapper(r.ecran.personnelles)}</div>` : ""}
    ${r.avertissements.map((a) =>
        `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr>${r.colonnes.map((c) => `<th${droite(c) ? ' class="r"' : ""}>${echapper(c.libelle)}</th>`).join("")}</tr>
      ${corps}
      ${r.total ? rang(r.total, "total-general") : ""}
    </table></div>
    ${r.sources.length ? `<details><summary>Sources</summary><ul>${
      r.sources.map((x) => `<li class="discret">${echapper(x)}</li>`).join("")}</ul></details>` : ""}
    ${r.fichier ? `<div class="actions">
      <button class="lien" id="x-dossier-2">Ouvrir le dossier</button></div>` : ""}`;
  const o = document.getElementById("x-dossier-2");
  if (o) o.onclick = async () => {
    await ouvrirEtDire(EXPORTS.dossier);
  };
  if (!apercu) dire(r.phrase);
}

/** L'état rendu, tel quel : les groupes, leurs sous-totaux, le pied. */
function dessinerLivre(r, apercu) {
  const cible = document.getElementById("exports-resultat");
  const eur = (v) => (v === null || v === undefined ? "—" : echapper(v));
  const ligne = (l) => `
    <tr>
      <td>${echapper(l.mois)}</td>
      <td>${echapper(l.salarie)}</td>
      <td class="discret">${echapper(l.spectacle || l.nature)}</td>
      <td class="r">${eur(l.brut)}</td>
      <td class="r">${eur(l.cotisations_salariales)}</td>
      <td class="r">${eur(l.cotisations_patronales)}</td>
      <td class="r">${eur(l.net_imposable)}</td>
      <td class="r">${eur(l.net_a_payer)}</td>
      <td class="r">${eur(l.pas)}</td>
      <td class="r">${eur(l.cout_employeur)}</td>
    </tr>`;
  const total = (t, titre, classe) => `
    <tr class="${classe}">
      <td></td><td><b>${echapper(titre)}</b></td>
      <td class="discret">${echapper(t.bulletins_dit)}</td>
      <td class="r"><b>${eur(t.brut)}</b></td>
      <td class="r"><b>${eur(t.cotisations_salariales)}</b></td>
      <td class="r"><b>${eur(t.cotisations_patronales)}</b></td>
      <td class="r"><b>${eur(t.net_imposable)}</b></td>
      <td class="r"><b>${eur(t.net_a_payer)}</b></td>
      <td class="r"><b>${eur(t.pas)}</b></td>
      <td class="r"><b>${eur(t.cout_employeur)}</b></td>
    </tr>`;
  const corps = r.groupes.map((g) =>
    g.lignes.map(ligne).join("") + total(g.totaux, g.total_dit, "sous-total")
  ).join("");
  cible.innerHTML = `
    <h2>${echapper(r.periode)}</h2>
    <p class="discret">${echapper(r.phrase)}</p>
    ${r.ecran.partiels ? `<div class="encart info">${echapper(r.ecran.partiels)}</div>` : ""}
    ${r.avertissements.map((a) =>
        `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr><th>Mois</th><th>Salarié</th><th>Spectacle</th><th>Brut</th>
          <th>Part salariale</th><th>Part patronale</th><th>Net imposable</th>
          <th>Net à payer</th><th>PAS</th><th>Coût employeur</th></tr>
      ${corps}
      ${total(r.totaux, "TOTAL GÉNÉRAL", "total-general")}
    </table></div>
    ${r.fichier ? `<div class="actions">
      <button class="lien" id="x-dossier-2">Ouvrir le dossier</button></div>` : ""}`;
  const o = document.getElementById("x-dossier-2");
  if (o) o.onclick = async () => {
    await ouvrirEtDire(EXPORTS.dossier);
  };
  if (!apercu) dire(r.phrase);
}

function boiteCloture(annee) {
  ouvrirBoite(`
    <h3>Clôturer l'exercice — ${echapper(annee)}</h3>
    <div class="encart">
      <b>L'année se fige</b>
      ${aideDe("cloture")}
    </div>
    <p class="note">${aideDe("cloture_refus")}</p>
    <label class="coche" title="${aideDe("cloture_forcer")}"><input type="checkbox" id="c-forcer">
      Clôturer quand même</label>
    <div class="actions">
      <button class="secondaire" onclick="fermerBoite()">Annuler</button>
      <button class="primaire" id="c-lancer">Clôturer</button>
    </div>`);
  document.getElementById("c-lancer").onclick = async () => {
    document.getElementById("c-lancer").disabled = true;
    try {
      const r = await api("cloture", { annee, forcer: document.getElementById("c-forcer").checked });
      fermerBoite();
      dire(r.phrase);
      if (document.getElementById("exercices")) await dessinerExercices();
    } catch (e) {
      document.getElementById("c-lancer").disabled = false;
      dire(e.message, "erreur");
    }
  };
}

async function lancerExportCompta(annee) {
  let r;
  try { r = await api("export-compta", { annee }); }
  catch (e) { dire(e.message, "erreur"); return; }
  const lignes = r.ecritures.map((e) => `
    <tr><td>${echapper(e.date.slice(5, 7))}</td><td>${echapper(e.compte)}</td>
        <td>${echapper(e.libelle)}</td>
        <td class="r">${e.debit !== "0.00" ? echapper(e.debit) : ""}</td>
        <td class="r">${e.credit !== "0.00" ? echapper(e.credit) : ""}</td>
        <td class="discret">${echapper(e.analytique)}</td></tr>`).join("");
  ouvrirBoite(`
    <h3>Export compta — ${echapper(r.annee)}</h3>
    <p class="discret">Mois ${r.mois.map((m) => m.slice(5)).join(", ")} — ${r.ecritures.length}
      écritures, débit ${echapper(euros(r.total_debit))} = crédit ${echapper(euros(r.total_credit))}.
      ${r.fichier ? `Écrit : <b>${echapper(r.fichier)}</b>` : ""}</p>
    ${r.avertissements.map((a) => `<div class="encart info">${echapperLiens(a)}</div>`).join("")}
    <div class="tableau-compta"><table class="compta">
      <tr><th>Mois</th><th>Compte</th><th>Libellé</th><th>Débit</th><th>Crédit</th><th>Analytique</th></tr>
      ${lignes}
    </table></div>
    <div class="actions">
      <button class="secondaire" id="x-ouvrir">Ouvrir le dossier</button>
      <button class="primaire" onclick="fermerBoite()">Fermer</button>
    </div>`);
  document.getElementById("x-ouvrir").onclick = async () => {
    await ouvrirEtDire(r.dossier);
  };
  if (document.getElementById("exercices")) dessinerExercices();
}


/* ------------------------------- les heures et cachets des intermittents */

/** LA CARTE DES DROITS : à son ouverture, ce que le moteur compte pour chaque intermittent (route `droits`) — ses
 *  lignes, la phrase qui dit où il en est, ses notes, le tableau du mois. La page met en forme, elle ne compte rien. */
function brancherDroits() {
  const carte = document.getElementById("carte-droits");
  if (!carte) return;
  carte.ontoggle = async () => {
    if (!carte.open) return;
    const cible = document.getElementById("droits");
    let d;
    try { d = await api("droits"); }
    catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
    cible.innerHTML = d.salaries.map((s) => `
      <div class="droits-salarie">
        <h3>${echapper(s.nom)} <span class="discret">— ${echapper(s.fenetre)}</span></h3>
        <table>${s.lignes.map(([quoi, valeur]) => `<tr><td>${echapper(quoi)}</td><td>${echapper(valeur)}</td></tr>`).join("")}</table>
        <p><b>${echapper(s.phrase)}</b></p>
        ${s.notes.map((n) => `<p class="discret">${echapper(n)}</p>`).join("")}
        <table><tr><th></th>${s.colonnes.titres.map((t) => `<th>${echapper(t)}</th>`).join("")}</tr>
          ${s.colonnes.lignes.map((l) => `<tr><td>${echapper(l.quoi)}</td>${l.valeurs.map((v) => `<td>${echapper(v)}</td>`).join("")}</tr>`).join("")}
        </table>
      </div>`).join("") + `<p class="discret">${echapper(d.phrase)}</p>`;
  };
}
