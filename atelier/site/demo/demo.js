// LA PAGE DE LA DÉMO : elle vérifie que le navigateur peut, installe le relais (relais.js), garde vivant le worker du
// moteur (moteur.js), montre l'avancement en mots simples, puis tient la page du logiciel dans un cadre plein écran.
// Elle ne garde rien : ni IndexedDB, ni localStorage, ni cache.
"use strict";

const $ = (id) => document.getElementById(id);
const Mo = (n) => (n / 1048576).toLocaleString("fr-FR", { maximumFractionDigits: 1, minimumFractionDigits: 1 }) + " Mo";

let worker = null;
let enMarche = false;
let remplacee = false;

function etape(nom) {
  const ordre = ["telecharger", "preparer", "demarrer"];
  const i = ordre.indexOf(nom);
  ordre.forEach((n, j) => {
    const li = $("e-" + n);
    li.classList.toggle("fait", j < i);
    li.classList.toggle("encours", j === i);
  });
  performance.mark("demo:" + nom);
}

function impossible(phrase) {
  $("etapes").hidden = true;
  $("impossible-phrase").textContent = phrase;
  $("impossible").hidden = false;
}

function arret(phrase) {
  enMarche = false;
  fermerLaMontre();
  const cadre = $("cadre");
  if (cadre) cadre.remove();
  if (worker) worker.terminate();
  worker = null;
  $("arret-phrase").textContent = phrase;
  $("arret").hidden = false;
  $("relancer").focus();
}

// CE QUE LE LOGICIEL « OUVRE » (un PDF, un fichier, un dossier) : sur un poste, un programme du poste le montre ; ici,
// le moteur le passe à la page (moteur.js, `montrer`), qui le montre par-dessus le logiciel. Un PDF s'affiche ; tout
// fichier s'ouvre dans un autre onglet ou se télécharge ; un dossier liste ce qu'il contient.
const TYPES = { pdf: "application/pdf", txt: "text/plain;charset=utf-8", csv: "text/csv;charset=utf-8", json: "application/json", xml: "text/xml", html: "text/html;charset=utf-8", png: "image/png", jpg: "image/jpeg", svg: "image/svg+xml" };
let adresseMontree = null;
let dossierOuvert = null;

function fermerLaMontre() {
  $("montre").hidden = true;
  $("montre-corps").replaceChildren();
  $("montre-liens").replaceChildren();
  if (adresseMontree) URL.revokeObjectURL(adresseMontree);
  adresseMontree = null;
  const cadre = $("cadre");
  if (cadre) cadre.focus();
}

function montrer(m) {
  fermerLaMontre();
  const nom = m.chemin.split("/").pop();
  $("montre-nom").textContent = nom;
  const corps = $("montre-corps");
  if (m.noms) {
    if (!m.noms.length) {
      const p = document.createElement("p");
      p.textContent = "Ce dossier est vide.";
      corps.append(p);
    }
    const ul = document.createElement("ul");
    const ligne = (texte, chemin) => {
      const b = document.createElement("button");
      b.type = "button";
      b.textContent = texte;
      b.addEventListener("click", () => worker.postMessage({ quoi: "montrer", chemin }));
      const li = document.createElement("li");
      li.append(b);
      ul.append(li);
    };
    // Dans un dossier du dossier ouvert par le logiciel, on remonte ; pas plus haut que lui.
    if (dossierOuvert !== null && m.chemin !== dossierOuvert && m.chemin.startsWith(dossierOuvert + "/")) {
      ligne("‹ " + m.chemin.split("/").slice(-2)[0], m.chemin.split("/").slice(0, -1).join("/"));
    } else {
      dossierOuvert = m.chemin;
    }
    for (const x of m.noms) ligne(x.nom + (x.dossier ? "/" : ""), m.chemin + "/" + x.nom);
    corps.append(ul);
  } else {
    const ext = nom.includes(".") ? nom.split(".").pop().toLowerCase() : "";
    // Un type que le navigateur n'exécute pas : une page HTML écrite par le logiciel se lit en texte.
    const type = ext === "html" ? "text/plain;charset=utf-8" : TYPES[ext] || "application/octet-stream";
    adresseMontree = URL.createObjectURL(new Blob([m.octets], { type }));
    const lien = (texte, pose) => {
      const a = document.createElement("a");
      a.href = adresseMontree;
      a.textContent = texte;
      pose(a);
      $("montre-liens").append(a);
    };
    lien("Ouvrir dans un autre onglet", (a) => { a.target = "_blank"; a.rel = "noopener"; });
    lien("Télécharger", (a) => { a.download = nom; });
    if (ext === "pdf") {
      const f = document.createElement("iframe");
      f.title = nom;
      f.src = adresseMontree;
      corps.append(f);
    } else {
      const p = document.createElement("p");
      p.textContent = "Ce fichier ne s'affiche pas ici : ouvrez-le dans un autre onglet, ou téléchargez-le.";
      corps.append(p);
    }
  }
  $("montre").hidden = false;
  $("montre-fermer").focus();
}

// UN PORT NEUF ENTRE LE RELAIS ET LE MOTEUR : un bout au worker, l'autre au service worker.
function confierUnPort(versLeRelais) {
  const c = new MessageChannel();
  worker.postMessage({ quoi: "port" }, [c.port1]);
  versLeRelais.postMessage({ quoi: "port" }, [c.port2]);
}

function ouvrirLeCadre() {
  const cadre = document.createElement("iframe");
  cadre.id = "cadre";
  cadre.title = "PuppetPaie (démo)";
  cadre.src = "/";
  cadre.hidden = true;
  cadre.addEventListener("load", () => {
    // Le cadre n'est le logiciel que si le relais l'a servi ; sinon (le navigateur a passé le relais, par exemple à
    // un rechargement forcé), c'est une autre page du site : on ne la montre pas.
    let tenu = false;
    try {
      tenu = !!cadre.contentWindow.navigator.serviceWorker.controller;
    } catch (e) {
      tenu = false;
    }
    if (!tenu) {
      cadre.remove();
      impossible("La démo ne s'est pas mise en place dans cette page. Rechargez la page, sans forcer le rechargement.");
      return;
    }
    if (!cadre.hidden) return;
    performance.mark("demo:affiche");
    etape("fin");
    cadre.hidden = false;
    $("accueil").hidden = true;
    cadre.focus();
  }, { once: false });
  document.body.appendChild(cadre);
}

function lancerLeMoteur() {
  $("arret").hidden = true;
  $("accueil").hidden = false;
  $("etapes").hidden = false;
  etape("telecharger");
  worker = new Worker("moteur.js", { type: "module" });
  worker.onerror = (e) => {
    console.error("Le worker du moteur :", e.message);
    arret(enMarche ? "Le logiciel s'est arrêté sur une erreur. C'est un défaut du logiciel, pas une erreur de votre part." : "La démo n'a pas pu démarrer : le logiciel ne s'est pas chargé.");
  };
  worker.onmessage = (e) => {
    const m = e.data;
    if (m.quoi === "avance") {
      const barre = $("barre");
      if (m.total) {
        $("e-taille").textContent = " : " + Mo(m.recus) + " sur " + Mo(m.total);
        barre.classList.remove("sans-total");
        barre.firstElementChild.style.width = Math.min(100, (100 * m.recus) / m.total).toFixed(1) + "%";
      } else {
        $("e-taille").textContent = " : " + Mo(m.recus) + " reçus";
        barre.classList.add("sans-total");
      }
    } else if (m.quoi === "etape") {
      etape(m.etape);
    } else if (m.quoi === "pret") {
      enMarche = true;
      // Le relais du moment : il a pu être remplacé depuis l'ouverture de la page.
      navigator.serviceWorker.ready.then((r) => { if (enMarche) confierUnPort(r.active); });
    } else if (m.quoi === "montrer") {
      montrer(m);
    } else if (m.quoi === "refus") {
      arret("Le logiciel refuse de démarrer : " + m.phrase);
    } else if (m.quoi === "echec") {
      arret("La démo n'a pas pu démarrer : " + m.phrase);
    } else if (m.quoi === "tombe") {
      arret("Le logiciel s'est arrêté sur une erreur. C'est un défaut du logiciel, pas une erreur de votre part.");
    }
  };
  worker.postMessage({ quoi: "demarrer" });
}

async function lancer() {
  if (typeof WebAssembly !== "object" || !("serviceWorker" in navigator) || typeof DecompressionStream !== "function") {
    // Il manque WebAssembly, les service workers ou la décompression en flux.
    impossible("Ce navigateur ne peut pas faire tourner la démo. Essayez un navigateur à jour, dans une fenêtre ordinaire (pas en navigation privée), ou téléchargez la démo.");
    return;
  }
  navigator.serviceWorker.addEventListener("message", (e) => {
    const m = e.data;
    if (m.quoi === "port?") {
      if (enMarche && !remplacee) confierUnPort(e.source);
    } else if (m.quoi === "port pose") {
      if (enMarche && !$("cadre")) ouvrirLeCadre();
    } else if (m.quoi === "remplacee") {
      remplacee = true;
      arret("La démo s'est ouverte dans un autre onglet : celle-ci s'est arrêtée.");
    }
  });
  try {
    await navigator.serviceWorker.register("relais.js", { scope: "/" });
  } catch (e) {
    console.error("Le relais de la démo ne s'installe pas :", e);
    impossible("La démo ne peut pas s'installer dans cette page (en navigation privée, certains navigateurs la refusent). Essayez une fenêtre ordinaire, ou téléchargez la démo.");
    return;
  }
  await navigator.serviceWorker.ready;
  lancerLeMoteur();
}

$("montre-fermer").addEventListener("click", () => { dossierOuvert = null; fermerLaMontre(); });

$("relancer").addEventListener("click", () => {
  remplacee = false;
  lancerLeMoteur();
});

lancer();
