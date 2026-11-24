/* CE QUE LES DEUX PAGES MONTRENT PAREIL : la page d'une compagnie (`index.html`) et l'accueil des compagnies
 * (`accueil.html`, servi par le moteur). Avant `js/avance.js`, qui en dessine les écrans que les routes rendent
 * (`poserLEcranRendu`) : l'échappement, le bouton d'un acte en cours, la pastille d'un état, l'encart. Rien ici ne
 * décide : chaque couleur et chaque mot vient de la route. */
"use strict";

/* Une valeur absente ne s'échappe pas : elle lève (`toString` d'un absent), et le filet la voit. Ce qui peut manquer se
 * teste avant d'être dessiné. */
const echapper = (s) => s.toString().replace(/[&<>"]/g,
  (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));

/** UN ACTE EN COURS SE VOIT. Le bouton cliqué s'éteint et dit ce qui se passe, là où l'on
 *  regarde — dans la boîte, que le bandeau du bas ne traverse pas. Rend
 *  la fonction qui le rallume (une erreur : on peut réessayer). */
function occupe(bouton, texte) {
  if (!bouton) return () => {};
  const avant = bouton.innerHTML;
  bouton.disabled = true;
  bouton.classList.add("occupe");
  bouton.textContent = texte ?? bouton.dataset.occupe ?? bouton.textContent;
  return () => {
    if (!bouton.isConnected) return;
    bouton.disabled = false;
    bouton.classList.remove("occupe");
    bouton.innerHTML = avant;
  };
}

/* LA PASTILLE D'UN ÉTAT : le moteur dit la couleur (`vert` fait, `orange`
 * à faire, `rouge` refusé, `gris` une information) et les mots ; la page
 * les dessine, sans choisir. */
const CLASSE_DE_COULEUR = { vert: "bien", orange: "alerte", rouge: "erreur", gris: "info" };
const pastilleDeLEtat = (p) => p && p.mots
  ? `<span class="pastille ${CLASSE_DE_COULEUR[p.couleur]}">${echapper(p.mots)}</span>` : "";
/* Un encart de la même couleur : `.encart` a ses propres classes. */
const ENCART_DE_COULEUR = { vert: "bien", orange: "attention", rouge: "erreur", gris: "info" };

/* Un encart que le moteur rend (`routes::encart` : titre, phrase, couleur) ; rien s'il n'y en a pas. La page n'écrit
 * aucune phrase : ce qu'une boîte dit d'un cas lui vient ainsi de la route. */
const encart = (e) => e && (e.titre || e.phrase)
  ? `<div class="encart ${ENCART_DE_COULEUR[e.couleur]}">${e.titre ? `<b>${echapper(e.titre)}</b> ` : ""}${e.phrase ? echapper(e.phrase) : ""}</div>` : "";
const encarts = (l) => l.map(encart).join("");
