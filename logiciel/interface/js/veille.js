/* La veille. */
"use strict";

/* =======================================================================
   LA VEILLE
   =======================================================================
   Ce que le serveur fait sans qu'on le lui demande : sauvegarder le dossier
   prive dans le Drive, relever les retours. La page ne declenche rien
   d'elle-meme au chargement -- le serveur a deja lance le premier tour a
   l'ouverture. Elle ne fait que demander ou ca en est.

   Elle ne calcule rien non plus : tout ce qui s'affiche ici est rendu par
   la fenêtre (`veille`, `interface/veille.rs`), comme le reste. */
let VEILLE = null;

/** Un passage de veille a pu changer ce que la page montre : un contrat
 *  signé d'un clic (pastille, colonne de droite, fiche), un retour rattaché
 *  à un dépôt. « Il devrait se mettre à jour tout en même temps » : quand
 *  le dernier passage n'est plus celui qu'on a vu, on repeint les états et
 *  la colonne des actes — ce sont des lectures locales, elles ne coûtent
 *  rien. */
async function rafraichirApresVeille(v) {
  const avant = VEILLE?.dernier?.quand;
  const apres = v?.dernier?.quand;
  const envoisAuBout = envoisArrives(v);
  if (!envoisAuBout && (!apres || apres === avant || v.en_cours)) return;
  if (envoisAuBout) oublierLeMois();
  const mois = document.getElementById("mois").value;
  try {
    await dessinerEtatsDesEngagements(mois);
    await rafraichirActions();
    await relireFichesAttendues();
  } catch (e) { dire(e.message, "erreur"); }
}

/* LES ENVOIS PARTIS EN FOND (`dit.envois`, routes/src/envois_en_fond.rs) : un envoi
   à la signature, des documents, un dépôt réel partent sur un fil du moteur une fois
   vérifiés ; la page reçoit « en cours d'envoi » (`en_fond`), et le bandeau dit la
   suite. Les numéros déjà vus au bout, pour ne repeindre et ne suivre qu'une fois. */
const ENVOIS_VUS_AU_BOUT = new Set();

/** Des envois sont-ils arrivés au bout depuis le dernier regard ? Un dépôt réel parti
 *  se suit ensuite comme après l'avoir attendu (`suivre`, la charge de `suivre-depot`). */
function envoisArrives(v) {
  const neufs = (v?.dit?.envois || []).filter((e) => e.etat !== "en_cours" && !ENVOIS_VUS_AU_BOUT.has(e.numero));
  neufs.forEach((e) => {
    ENVOIS_VUS_AU_BOUT.add(e.numero);
    if (e.suivre) suivreDepot(e.suivre.quoi, e.suivre.flux, e.suivre.mois);
  });
  return neufs.length > 0;
}

/* LE BANDEAU. On y cherche trois choses, dans cet ordre : est-ce que tout
   va bien ? est-ce qu'il est arrivé quelque chose ? est-ce qu'il y a quelque
   chose à FAIRE ? Une phrase calme quand tout va (« À jour — relevé à
   16:28 », le détail au survol) ; les nouvelles en clair (retours,
   signatures) ; ce qui est à faire en orange, dit en mots, avec le lien vers
   l'endroit où le faire. Jamais une commande de terminal : une fenêtre
   n'en a pas. La mise à jour n'est pas ici : l'indicateur la dit
   (js/mise-a-jour.js). */
/** Aller aux mots de passe, d'où qu'on soit. */
function allerAuxMotsDePasse() {
  document.querySelector('.onglets button[data-vue="configuration"]').click();
  document.querySelector('.sous-onglets button[data-fiche="coffre"]').click();
}

function dessinerVeille(v) {
  VEILLE = v;
  const zone = document.getElementById("veille");
  const texte = document.getElementById("veille-texte");
  const envois = v?.dit?.envois || [];
  if (!v || (!v.minutes && !v.dernier?.quand && !v.en_cours && !envois.length)) { zone.hidden = true; return; }
  zone.hidden = false;
  zone.classList.remove("souci", "neuf");
  // LES ENVOIS EN COURS, dits par le moteur, comme l'étape d'un relevé ; le bandeau les suit jusqu'au bout.
  const enCours = envois.filter((e) => e.etat === "en_cours")
    .map((e) => `<span class="veille-etape">${echapper(e.phrase)}</span>`);
  // LE LOGICIEL SE FERME dès que l'envoi est parti (la croix, le bouton, la mise à jour) : le moteur le dit.
  if (v?.dit?.fermeture) enCours.push(`<b>${echapper(v.dit.fermeture)}</b>`);
  if (enCours.length) suivreLesEtapes();

  if (v.en_cours) {
    // L'ÉTAPE, DITE PAR LE MOTEUR : qui il appelle, puis qui d'autre, puis la sauvegarde.
    texte.innerHTML = [`<span class="veille-etape">${echapper(v.etape || v.dit.en_cours)}</span>`, ...enCours].join(" — ");
    suivreLesEtapes();
    return;
  }
  // Le dernier passage : `null` tant qu'aucun n'a eu lieu.
  const d = v.dernier;

  // 1. L'ÉTAT, en une phrase. Le détail des relevés passe au survol : on
  //    le cherche rarement, et il reste à portée.
  const detail = d ? [...d.injoignables, d.sauvegarde, d.retours, d.signatures, d.fiches].filter(Boolean).join("\n") : "";
  if (v.sauvegarde_en_cours) suivreLesEtapes();
  // « HORS CONNEXION, C'EST QUAND IL N'Y A PAS INTERNET, PAS QUAND ON ATTEND » : l'attente
  // dit l'heure du prochain relevé ; un service qui n'a pas répondu se nomme (au survol), et le relevé qui est
  // passé reste dit. « Pas de réponse » seulement quand le relevé lui-même n'est pas passé.
  const k = v.dit;
  const sansReponse = d ? d.injoignables.join("\n") : "";
  const morceaux = [`<span class="veille-etat" title="${echapper(detail)}">${echapper(k.etat)}${v.sauvegarde_en_cours
        ? ` · <span class="veille-etape">${echapper(v.etape || k.sauvegarde)}</span>` : ""}</span>${
    k.muets ? ` — <span class="veille-sauvegarde" title="${echapper(sansReponse)}">${echapper(k.muets)}</span>` : ""}${k.fictive
    ? ` · <button class="lien" data-fictive-explique="coulisses">${echapper(k.fictive)}</button>` : ""}`];
  // LA SAUVEGARDE QUI N'ARRIVE PAS AU BOUT, à sa place : un dossier que
  // Google refuse une fois repart au passage suivant, sans bruit ; ce qui dure, le moteur le dit
  // en une phrase (`sauvegarde::souci`), et elle se lit ici, à côté de l'état, pas parmi les retours.
  if (d && k.sauvegarde_souci && !d.hors_ligne) {
    morceaux.push(`<span class="veille-sauvegarde"${d.sauvegarde ? ` title="${echapper(d.sauvegarde)}"` : ""}>${echapper(k.sauvegarde_souci)}</span>`);
  }

  // 2. LES NOUVELLES : ce qui est arrivé depuis le dernier regard.
  const nouvelles = [];
  // Les retours se LISENT dans l'Historique : le bandeau les annonce et y mène.
  const retours = k.retours ? ` <button class="lien" data-vers-historique>${echapper(k.retours)}</button>` : "";
  if (d) d.signatures_recues.forEach((x) => nouvelles.push(x.phrase));
  envois.filter((e) => e.etat === "parti").forEach((e) => nouvelles.push(e.phrase));
  if (enCours.length) morceaux.push(...enCours);
  // L'HORLOGE DE LA COMPAGNIE FICTIVE : ce que l'administrateur fictif a fait
  // pendant l'absence du visiteur, chaque geste à sa date limite.
  const horloge = k.horloge ? ` <button class="lien" id="fictive-horloge">${echapper(k.horloge)}</button>` : "";
  if (retours) morceaux.push(retours);
  if (nouvelles.length) morceaux.push(`<b>${nouvelles.map(echapper).join(" · ")}</b>`);
  if (horloge) morceaux.push(horloge);

  // 3. CE QUI EST À FAIRE : en mots, avec le chemin. Le premier se lit, les
  //    suivants se déplient.
  // Ce qui manque est celui du coffre AU MOMENT où la fenêtre répond (`dit.a_poser`), pas celui du dernier relevé.
  const afaire = k.a_poser.map((a) =>
    `${echapper(a.phrase)} <button class="lien" data-aller>${echapper(a.geste)}</button>`);
  // UN ENVOI QUI N'EST PAS ALLÉ AU BOUT : ce qui a échoué, et le geste qui le refait (la boîte d'où il est parti).
  envois.forEach((e, i) => {
    if (e.etat === "echoue") {
      afaire.push(`${echapper(e.phrase)}${e.refaire
        ? ` <button class="lien" data-refaire="${i}">${echapper(e.refaire.libelle)}</button>` : ""}`);
    }
    // CE QUE L'ENVOI AVERTIT (un salarié sans courriel, la boîte qui envoie), que le moteur dit avec lui.
    e.avertissements.forEach((a) => afaire.push(echapper(a)));
  });
  if (d) [...d.manquants, ...d.incidents].forEach((x) => afaire.push(echapper(x)));
  if (v.erreur) afaire.push(echapper(v.erreur));
  if (afaire.length) {
    zone.classList.add("souci");
    morceaux.push(afaire.length === 1 ? afaire[0]
      : `<details class="ennuis"><summary>${afaire[0]}
           <b>Voir la suite</b></summary>
           ${afaire.slice(1).map((x) => `<div>${x}</div>`).join("")}</details>`);
  } else if (nouvelles.length) {
    zone.classList.add("neuf");
  }
  texte.innerHTML = morceaux.join(" — ");
  texte.title = texte.textContent.replace(/\s+/g, " ").trim();   // une ligne : ce qui dépasse se lit au survol
  texte.querySelectorAll("[data-vers-historique]").forEach((b) => {
    b.onclick = () => document.querySelector('.onglets button[data-vue="historique"]').click();
  });
  texte.querySelectorAll("[data-aller]").forEach((b) => {
    b.onclick = () => allerAuxMotsDePasse();
  });
  texte.querySelectorAll("[data-refaire]").forEach((b) => {
    b.onclick = (ev) => { ev.preventDefault(); ouvrirGeste(envois[+b.dataset.refaire].refaire); };
  });
  const bh = texte.querySelector("#fictive-horloge");
  if (bh) bh.onclick = () => ouvrirBoite(`
    <div class="fictive-fenetre">
      <h3>Compagnie fictive — ${echapper(k.horloge_titre)}</h3>
      <p class="intro">${echapper(k.horloge_intro)}</p>
      <ul>${d.horloge.map((x) => `<li>${echapper(x)}</li>`).join("")}</ul>
      <div class="actions"><button class="primaire" onclick="fermerBoite()">Compris</button></div>
    </div>`);
  texte.querySelectorAll("[data-fictive-explique]").forEach((b) => {
    b.onclick = (ev) => { ev.preventDefault(); fenetreExplicationFictive(b.dataset.fictiveExplique); };
  });
}

/** LE PASSAGE EN COURS, ÉTAPE PAR ÉTAPE : un message dans la ligne dit ce que fait la
 *  machine. Tant qu'un passage tourne, la page lit `veille-etape` toutes les secondes : deux
 *  drapeaux et l'étape que le moteur a écrite en y entrant, sans processus ni calcul — la page ne rame pas. Le passage
 *  fini, le bandeau se repeint d'un coup (`suivreVeille`). Un seul suivi à la fois. */
let SUIVI_ETAPES = null;
let RELEVE_DEMANDE = false;     // « relever maintenant » attend sa réponse : le suivi tient jusque-là
function suivreLesEtapes() {
  if (SUIVI_ETAPES || SERVEUR_PERDU) return;
  SUIVI_ETAPES = setTimeout(async () => {
    let e = null;
    try {
      e = (await fenetre("veille_etape")).etat;
      suiviRepris("veille_etape");
    } catch (x) { direLeRefusDuSuivi("veille_etape", x.message); }
    SUIVI_ETAPES = null;
    if (!e) return;
    // La fermeture attend un envoi : le bandeau se relit une fois pour le dire.
    if (e.fermeture && !VEILLE?.dit?.fermeture) { suivreVeille(); return; }
    if (e.en_cours || e.sauvegarde_en_cours || e.envoi_en_cours || RELEVE_DEMANDE) {
      const el = document.querySelector("#veille-texte .veille-etape");
      if (el && e.etape) el.textContent = e.etape;
      suivreLesEtapes();
    } else {
      suivreVeille();
    }
  }, 1000);
}

async function suivreVeille() {
  if (SERVEUR_PERDU) return;          // le bandeau dit déjà l'essentiel
  try {
    const v = (await fenetre("veille")).etat;
    suiviRepris("veille");
    await rafraichirApresVeille(v);
    dessinerVeille(v);
    dessinerPastilleReponses();
  } catch (e) { direLeRefusDuSuivi("veille", e.message); }
}

document.getElementById("veille-maintenant").onclick = async () => {
  RELEVE_DEMANDE = true;
  // Le bouton n'est là que dans le bandeau de la veille : elle est lue.
  dessinerVeille({ ...VEILLE, en_cours: true, etape: null });
  try {
    const v = (await fenetre("veille_maintenant")).etat;
    oublierLeMois();   // un passage a pu changer les mois : ce qui a été lu part
    RELEVE_DEMANDE = false;
    // Le relevé manuel repeint TOUJOURS : c'est ce qu'on vient demander.
    VEILLE = { ...VEILLE, dernier: null };
    await rafraichirApresVeille(v);
    dessinerVeille(v);
    dire(v.dit.termine);
  } catch (e) { RELEVE_DEMANDE = false; dire(e.message, "erreur"); }
};

