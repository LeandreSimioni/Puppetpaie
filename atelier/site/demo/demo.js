// LA PAGE DE LA DÉMO : elle vérifie que le navigateur peut, installe le relais (relais.js), garde vivant le worker du
// moteur (moteur.js), montre l'avancement en mots simples, puis tient la page du logiciel dans un cadre plein écran.
// Elle ne garde rien : ni IndexedDB, ni localStorage, ni cache.
"use strict";

const $ = (id) => document.getElementById(id);
const Mo = (n) => (n / 1048576).toLocaleString("fr-FR", { maximumFractionDigits: 1, minimumFractionDigits: 1 }) + " Mo";

let worker = null;
let enMarche = false;
let remplacee = false;
let relais = null;

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
  const cadre = $("cadre");
  if (cadre) cadre.remove();
  if (worker) worker.terminate();
  worker = null;
  $("arret-phrase").textContent = phrase;
  $("arret").hidden = false;
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
  worker.onerror = (e) => arret("La démo n'a pas pu démarrer : " + (e.message || "le moteur ne s'est pas chargé") + ".");
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
      confierUnPort(relais);
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
    impossible("Ce navigateur ne peut pas faire tourner la démo. Essayez un navigateur à jour, dans une fenêtre ordinaire (pas en navigation privée), ou téléchargez le logiciel.");
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
    impossible("Ce navigateur refuse de faire tourner la démo ici (en navigation privée, certains navigateurs la refusent). Essayez une fenêtre ordinaire, ou téléchargez le logiciel.");
    return;
  }
  relais = (await navigator.serviceWorker.ready).active;
  lancerLeMoteur();
}

$("relancer").addEventListener("click", () => {
  remplacee = false;
  lancerLeMoteur();
});

lancer();
