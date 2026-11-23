/* Les corrections des mois déposés. */
"use strict";

/* Un engagement d'un mois déposé, modifié puis revalidé après la veille de
   l'échéance, ne refait plus la DSN de son mois : sa correction part dans la
   DSN d'un mois suivant, datée du mois d'origine (cahier technique 2026.1
   §2.2.2.5). Cet écran, dans l'Historique, montre pour chaque contrat la paie
   déclarée et la paie validée : celles à porter, puis celles déjà portées.
   Corriger, c'est modifier l'engagement et le revalider. */
async function dessinerRegularisation() {
  const cible = document.getElementById("regularisation");
  let r;
  try { r = await api("regularisations"); }
  catch (e) { cible.innerHTML = `<p class="discret">${echapper(e.message)}</p>`; return; }
  // Le titre, et celui de chaque mois, sont ceux que la route rend (aucun titre quand il n'y a aucune correction).
  cible.innerHTML = (r.titre ? `<h2>${echapper(r.titre)}</h2>` : "") + r.corrections.map((m) => `
      <div class="corrections-mois">
        <b>${echapper(m.titre)}</b>
        ${m.corrections.map((c) => `
          <div class="rang">
            <span class="rang-nom">${echapper(c.nom)}</span>
            ${pastilleDeLEtat(c.pastille)}
            <span class="rang-infos">${echapper(c.infos_dit)}
              ${c.dates ? `<span class="discret">${echapper(c.dates)}</span>` : ""}
              · ${echapper(c.portee_dit)}</span>
          </div>`).join("")}
      </div>`).join("");
}
