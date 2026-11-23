/* Configuration → Modèles de contrat. */
"use strict";

/* LES MODÈLES DE CONTRAT DE LA COMPAGNIE.

   Chaque nature de contrat a son modèle de base, livré, contrat et avenant ; la
   compagnie le copie en modèles spéciaux, en choisit un par défaut, et un
   engagement peut en choisir un autre (Avancé). Un modèle s'ouvre en
   paragraphes, son contrat ou son avenant : la base se lit, un spécial s'écrit
   paragraphe par paragraphe.

   La page ne décide rien (règle 2) et n'écrit aucune phrase : `modeles-contrat`
   dit les modèles de chaque nature, le geste qui ouvre chacun (`ouvrir`) et ceux
   qu'on peut y faire (`gestes`, comme ceux de la nature : copier sa base) ;
   `modele-contrat` dit une partie d'un modèle, ses paragraphes et, pour chacun,
   le geste qui l'enregistre (`ecrire`) et ceux qu'on peut y faire, les gestes
   qui lisent ses parties, ceux du modèle et celui de l'aperçu. Chaque geste est
   posé tel que la route le rend (`boutonsDesGestes`, `brancherLesGestes`,
   `js/avance.js`) : sa route, sa charge, son libellé, sa boîte.

   `ouvert` : la charge du geste qui a ouvert l'éditeur d'une partie d'un modèle
   ({nature, id, partie}), redessinée après chaque geste. Chaque geste, et ouvrir
   ou fermer un modèle, quitte d'abord les paragraphes retenus de l'éditeur
   (`quitterLEcran`) : une saisie non enregistrée se demande. */
async function dessinerModelesContrat(cible, ouvert = null) {
  cible.innerHTML = `<p class="note">…</p>`;
  let m;
  try {
    m = await appelerMoteur("modeles-contrat", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  // La base n'a pas d'id (absent de la charge) : `==` tient deux absences pour égales.
  const estOuvert = (x) => ouvert && ouvert.nature === x.ouvrir.charge.nature && ouvert.id == x.ouvrir.charge.id;
  const redessiner = (o = ouvert) => dessinerModelesContrat(cible, o);
  const ligne = (x) => `
    <div class="modele-contrat">
      <div class="modele-entete">
        <b>${echapper(x.nom)}</b>
        ${pastilleDeLEtat(x.pastille)}
      </div>
      ${x.base_changee ? `<div class="encart">${echapper(x.base_changee)}</div>` : ""}
      ${x.manques.length ? `<div class="encart erreur">${x.manques.map(echapper).join("<br>")}</div>` : ""}
      <div class="modele-gestes">
        <button type="button" class="lien" data-ouvrir>${estOuvert(x) ? "Fermer" : echapper(x.ouvrir.libelle)}</button>
        <span data-gestes>${boutonsDesGestes(x.gestes)}</span>
      </div>
      <div class="modele-editeur"></div>
    </div>`;
  cible.innerHTML = `
    <p class="intro">${echapper(m.ecran.intro)}</p>
    <p class="discret">${echapper(m.ecran.en_francais)}</p>
    <ol class="discret">${m.ecran.comment.map((x) => `<li>${echapper(x)}</li>`).join("")}</ol>
    ${m.natures.map((n) => `
      <h3>${echapper(n.titre)} <span class="discret">— ${echapper(n.libelle)}</span></h3>
      ${n.modeles.map(ligne).join("")}
      <div class="modele-gestes" data-nature-gestes>${boutonsDesGestes(n.gestes)}</div>`).join("")}`;

  const modeles = m.natures.flatMap((n) => n.modeles);
  cible.querySelectorAll(".modele-contrat").forEach((bloc, k) => {
    const x = modeles[k];
    const ici = estOuvert(x);
    bloc.querySelector("[data-ouvrir]").onclick = async () => { if (await quitterLEcran(cible)) redessiner(ici ? null : x.ouvrir.charge); };
    brancherLesGestes(bloc.querySelector("[data-gestes]"), x.gestes, { titre: x.nom, quitter: cible }, redessiner);
    if (ici) dessinerEditeurDeModele(bloc.querySelector(".modele-editeur"), ouvert, redessiner);
  });
  cible.querySelectorAll("[data-nature-gestes]").forEach((bloc, k) => {
    const n = m.natures[k];
    brancherLesGestes(bloc, n.gestes, { titre: n.titre, quitter: cible }, redessiner);
  });
}

/* L'ÉDITEUR D'UNE PARTIE D'UN MODÈLE (son contrat ou son avenant), en
   paragraphes. Un paragraphe d'un spécial a ses cases (`cases` : son titre, ses
   alinéas en rangées, chacun sa condition et son texte), dessinées par le
   dessinateur, et le geste qui les enregistre (`ecrire`) : chacun est un écran
   retenu (`retenirLEcran`), et le quitter avec une saisie non enregistrée se
   demande. Un paragraphe de la base se lit (`alineas`). Une donnée s'insère par
   la liste (`champs` de la route), dans l'alinéa où l'on écrit. `redessiner` :
   redessine la liste, l'éditeur ouvert sur la partie qu'on lui donne. */
async function dessinerEditeurDeModele(cible, ici, redessiner) {
  let lu;
  try {
    lu = await appelerMoteur("modele-contrat", ici);
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  const ecrit = lu.articles.some((p) => p.ecrire);
  const choix = (classe, premier, si, actif) => `
    <select class="${classe}" ${actif ? "" : "disabled"}>
      <option value="">${premier}</option>
      ${lu.champs.map((c) => `<option value="${echapper(c.marqueur)}" ${c.marqueur === si ? "selected" : ""}>${echapper(c.quoi)}</option>`).join("")}
    </select>`;
  // Un alinéa de la base, qui se lit seulement.
  const alineaLu = (a) => `
    <div class="modele-alinea">
      ${choix("modele-si", "Toujours", a.si, false)}
      <textarea rows="3" readonly>${echapper(a.texte)}</textarea>
    </div>`;
  const corps = (p) => (p.ecrire
    ? `<div class="modele-entete"><b>${p.numero}</b></div>
      <div data-paragraphe-cases>${grilleDesCases(p.cases.map(caseDeRoute).join(""), "", false)}</div>`
    : `<div class="modele-entete">
        <b>${p.numero}</b>
        <input type="text" class="modele-titre" value="${echapper(p.titre)}" readonly>
      </div>
      <div class="modele-alineas">${p.alineas.map(alineaLu).join("")}</div>`);
  const paragraphe = (p) => `
    <div class="modele-paragraphe">
      ${corps(p)}
      ${p.ecrire || p.gestes.length ? `<div class="modele-gestes">
        ${p.ecrire ? `<button type="button" data-enregistrer>${echapper(p.ecrire.libelle)}</button>` : ""}
        <span data-gestes>${boutonsDesGestes(p.gestes)}</span>
      </div>` : ""}
    </div>`;
  cible.innerHTML = `
    <div class="modele-gestes">
      ${lu.parties.map((g, k) => `<button type="button" class="lien" data-partie="${k}" ${g.charge.partie === lu.partie ? "disabled" : ""}>${echapper(g.libelle)}</button>`).join("")}
    </div>
    ${lu.manques.length ? `<div class="encart erreur">${lu.manques.map(echapper).join("<br>")}</div>` : ""}
    ${lu.base_changee && lu.base_changee.length ? `<div class="encart">${lu.base_changee.map(echapper).join("<br>")}</div>` : ""}
    <p class="discret">${echapper(lu.en_francais)}</p>
    ${lu.gestes.length ? `<div class="modele-gestes" data-modele-gestes>${boutonsDesGestes(lu.gestes)}</div>` : ""}
    <div class="modele-gestes">
      <select class="modele-engagement">
        ${lu.engagements.map((e, i) => `<option value="${i}">${echapper(e.nom)}</option>`).join("")}
      </select>
      <button type="button" class="lien" data-apercu>${echapper(lu.apercu.libelle)}</button>
    </div>
    ${ecrit ? choix("modele-inserer", "Insérer une donnée", null, true) : ""}
    ${lu.articles.map(paragraphe).join("")}`;

  cible.querySelectorAll("[data-partie]").forEach((b) => {
    b.onclick = async () => { if (await quitterLEcran(cible)) redessiner(lu.parties[+b.dataset.partie].charge); };
  });
  const gestes = cible.querySelector("[data-modele-gestes]");
  if (gestes) brancherLesGestes(gestes, lu.gestes, { titre: lu.nom, quitter: cible }, () => redessiner());
  cible.querySelector("[data-apercu]").onclick = async (ev) => {
    const choisi = lu.engagements[cible.querySelector(".modele-engagement").value];
    const rallumer = occupe(ev.target);
    try {
      const r = await api(lu.apercu.route, { ...lu.apercu.charge, engagement: choisi ? choisi.engagement : null });
      await ouvrirEtDire(r.chemin, r.phrase);
    } catch (e) {
      dire(e.message, "erreur");
    } finally {
      rallumer();
    }
  };
  cible.querySelectorAll(".modele-paragraphe").forEach((bloc, k) => {
    const p = lu.articles[k];
    const siens = bloc.querySelector("[data-gestes]");
    if (siens) brancherLesGestes(siens, p.gestes, { titre: p.titre, quitter: cible }, () => redessiner());
    if (!p.ecrire) return;
    // Le geste qui enregistre le paragraphe part avec sa charge, que complète ce que ses cases disent, chacune sous sa
    // clé ; une case que la route exige, vide, le retient et le dit sur elle.
    const cases = bloc.querySelector("[data-paragraphe-cases]");
    brancherLesCases(cases);
    const champs = champsDuGeste(p.ecrire);
    const enregistrer = async () => {
      const charge = chargeDuGeste(cases, p.ecrire) || p.ecrire.charge;
      if (marquerLesManquantes(cases, champs, charge)) return false;
      try {
        dire((await api(p.ecrire.route, charge)).phrase);
        return true;
      } catch (e) {
        dire(e.message, "erreur");
        return false;
      }
    };
    retenirLEcran(cases, { enregistrer });
    bloc.querySelector("[data-enregistrer]").onclick = async () => {
      if (!(await enregistrer())) return;
      oublierLEcran(cases);
      if (await quitterLEcran(cible)) redessiner();
    };
  });
  if (!ecrit) return;
  // L'alinéa où l'on écrit en dernier reçoit la donnée choisie.
  let dernier = null;
  cible.addEventListener("focusin", (ev) => { if (ev.target.matches("[data-paragraphe-cases] textarea")) dernier = ev.target; });
  const inserer = cible.querySelector(".modele-inserer");
  inserer.onchange = () => {
    if (dernier && inserer.value) {
      const marque = `{{${inserer.value}}}`;
      const [a, b] = [dernier.selectionStart, dernier.selectionEnd];
      dernier.value = dernier.value.slice(0, a) + marque + dernier.value.slice(b);
      dernier.focus();
      dernier.selectionStart = dernier.selectionEnd = a + marque.length;
    }
    inserer.value = "";
  };
}
