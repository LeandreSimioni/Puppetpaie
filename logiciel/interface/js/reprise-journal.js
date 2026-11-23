/* La reprise du journal des dépôts. Les arrêts de travail et les fins de contrat sont dans js/imprevus.js. */
"use strict";

/* ------------------------------------------- la reprise du journal
   Ce qui est parti sans passer par le journal des dépôts — par un autre
   logiciel, par le site d'un organisme — n'y figure pas, et l'écran dirait
   « rien n'est parti » sur des actes qui ont eu lieu.

   Ce bloc rend au journal ce que les traces de dépôt attestent, en tête de l'Historique :
   les envois et leurs retours s'y lisent). Il n'apparaît QUE s'il y a quelque chose
   à reprendre : une fois le journal à jour, il disparaît et ne revient que si
   un nouveau dépôt atteste d'un chemin qui n'est pas passé par ici.

   DEUX TEMPS, comme partout ailleurs : on montre d'abord ce qui serait écrit,
   ligne par ligne, et on n'écrit que sur un second clic. Une reprise qui part
   au premier appel est une reprise qu'on lance « pour voir » et qui laisse
   des lignes que personne n'a relues. */
async function dessinerReprise() {
  const cible = document.getElementById("reprise");
  if (!cible) return;
  let r;
  try {
    r = await api("journal-reprise", {});
  } catch (e) {
    cible.innerHTML = `<div class="encart erreur">${echapper(e.message)}</div>`;
    return;
  }
  if (!r.ajoutes.length) { cible.innerHTML = ""; return; }

  // Replié en une ligne, sauf quand la route le dit déplié (un dépôt réel accepté s'y trouve).
  cible.innerHTML = `
    <details class="reprise" ${r.ecran.deplie ? "open" : ""}>
    <summary>${echapper(r.ecran.resume)}</summary>
    <div class="encart">
      <b>${echapper(r.ecran.titre)}</b>
      ${echapper(r.ecran.explication)}
      ${r.ecran.reels ? `<br><b>${echapper(r.ecran.reels)}</b>` : ""}
    </div>
    <div class="liste-choix">
      ${r.ajoutes.map((v) => `
        <div class="choix"><span>
          <b>${echapper(v.dit)}</b>
          ${pastilleDeLEtat(v.pastille)}
          <br><span class="discret">${echapper(v.origine_dit)}</span>
        </span></div>`).join("")}
    </div>
    <p data-gestes>${boutonsDesGestes(r.gestes)}</p>
    <p class="note">${echapper(r.ecran.logiciel)}</p>
    </details>`;

  // Le geste que la route rend (la même route, qui écrit). La reprise remet des envois au journal : l'Historique, où
  // elle se montre, se redessine avec eux.
  brancherLesGestes(cible.querySelector("[data-gestes]"), r.gestes, {}, async () => {
    await rafraichirActions();
    dessinerHistorique();
  });
}

