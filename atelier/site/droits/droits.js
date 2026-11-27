// LA PAGE DES DROITS D'UN INTERMITTENT (puppetpaie.fr/droits/). Elle s'ouvre tout de suite ; le moteur se charge
// derrière, dans le worker de la démo (/demo/moteur.js : les deux mêmes fichiers, onglet.wasm.gz et demo.tar.gz).
// Elle ne décide rien : elle demande la route `droits-des-pieces` et pose son écran, bloc par bloc dans l'ordre où la
// route le rend (`ecran` : la réponse, l'allocation visée, les cases, dessinées depuis la forme que la route leur
// donne, le tableau des paies, le dépôt, les pièces, le détail du calcul). Avant que le moteur soit prêt, elle pose ce que la route rend sans charge
// (`premier-ecran.json`) ; le moteur prêt, « Calculer » (ou un bulletin déposé) fait le premier calcul. Ensuite, chaque
// changement (un bulletin déposé ou retiré, une case) redemande la route. Les
// bulletins déposés vont dans le dossier que le dépôt nomme (`dossier`), sur le disque en mémoire du moteur. Ce qui est
// saisi et déposé se garde dans l'onglet seul (`sessionStorage`) : recharger la page le reprend, fermer l'onglet
// l'efface ; rien ne part ailleurs.
"use strict";

// « UN PROBLÈME, UNE IDÉE ? » : la boîte du logiciel elle-même (logiciel/js/filet.js et ce qu'elle prend, servis sous
// logiciel/ par le déploiement depuis demo.tar.gz : droits/LISEZ-MOI.md). Ce qu'elle attend de la page qui la porte,
// la page le lui donne comme l'accueil des compagnies : le catalogue (aucune route automatique ici), l'état d'une
// compagnie (aucune), `api` (une route du moteur : son résultat, ou son refus levé) et `dire` (une phrase à montrer).
// `api` se pose quand le moteur est prêt, avec le bouton.
var CATALOGUE = {};
var ETAT = null;
var api = null;
var dire = null;

(function () {
  var $ = function (id) { return document.getElementById(id); };
  var el = function (nom, classe, texte) {
    var e = document.createElement(nom);
    if (classe) e.className = classe;
    if (texte !== undefined && texte !== null) e.textContent = texte;
    return e;
  };

  // ---------------------------------------------------------------- le moteur

  var worker = null;
  var port = null;
  var pret = false;
  var prochain = 1;
  var attentes = {};
  var poses = [];

  // L'AVIS DE LA PAGE : chaque erreur s'y ajoute (aucune n'en efface une autre), au-dessus du bouton qui recharge.
  function avis(texte) {
    var a = $("avis");
    var b = a.querySelector("button");
    if (!b) {
      b = el("button", null, "Recharger la page");
      b.type = "button";
      b.addEventListener("click", function () { location.reload(); });
      a.append(b);
    }
    a.insertBefore(el("p", null, texte), b);
    a.classList.remove("cache");
  }

  dire = function (texte) { avis(texte); };

  // Le texte d'une erreur attrapée, tel qu'elle le porte.
  function texteDErreur(e) {
    return String((e && e.message) || e);
  }

  function demarrer() {
    if (typeof WebAssembly !== "object" || typeof DecompressionStream !== "function") {
      avis("Ce navigateur ne peut pas faire tourner le calcul. Essayez un navigateur à jour.");
      return;
    }
    try {
      worker = new Worker("/demo/moteur.js", { type: "module" });
    } catch (e) {
      avis("Le calcul ne peut pas démarrer dans ce navigateur.");
      return;
    }
    worker.onerror = function () { avis("Le calcul n'a pas pu démarrer : le logiciel ne s'est pas chargé."); };
    worker.onmessage = function (e) {
      var m = e.data;
      if (m.quoi === "pret") {
        var c = new MessageChannel();
        worker.postMessage({ quoi: "port" }, [c.port1]);
        port = c.port2;
        port.onmessage = function (r) {
          var f = attentes[r.data.id];
          delete attentes[r.data.id];
          if (f) f(r.data);
        };
        pret = true;
        offrirLeCalcul();
        api = function (nom, charge) {
          return route(nom, charge || {}).then(function (r) {
            if (!r.ok) throw new Error(r.refus.phrase);
            return r.resultat;
          });
        };
        $("signaler").classList.remove("cache");
      } else if (m.quoi === "pose") {
        var f = poses.shift();
        if (f) f(m.ok);
      } else if (m.quoi === "refus") {
        avis("Le logiciel refuse de démarrer : " + m.phrase);
      } else if (m.quoi === "echec") {
        avis("Le calcul n'a pas pu démarrer : " + m.phrase);
      } else if (m.quoi === "tombe") {
        pret = false;
        avis("Le logiciel s'est arrêté sur une erreur. C'est un défaut du logiciel, pas une erreur de votre part.");
      }
    };
    worker.postMessage({ quoi: "demarrer" });
    // LE PREMIER ÉCRAN, SANS ATTENDRE LE MOTEUR (Léandre, 27/11/2026 : « Il suffit de précharger juste ça ») : le moteur
    // met quelques secondes à se charger ; en attendant, la page pose ce que la route rend sans charge (la date
    // anniversaire, le dépôt, le tableau vide), figé dans `premier-ecran.json` (un test de structure le tient égal à la
    // route : moteur/tests/fichiers/structure.rs), avec les mêmes fonctions que toute réponse. Ce qu'on y saisit ou
    // dépose attend « Calculer ».
    fetch("premier-ecran.json").then(function (r) {
      if (!r.ok) throw new Error("premier-ecran.json : code " + r.status);
      return r.json();
    }).then(function (d) {
      if (dernier === null) montrer(d, {}, true);
    }).catch(function (e) {
      avis("Défaut du logiciel : " + texteDErreur(e));
    });
  }

  // LE BOUTON « CALCULER » apparaît quand le moteur est prêt (Léandre, 27/11/2026 : « Le bouton calculer arrive avec
  // le moteur ») ; un appui fait le premier calcul, et il s'en va : ensuite tout se recalcule seul à chaque changement.
  // Un bulletin déposé vaut un appui : on attend sa lecture, pas un bouton (Léandre, 27/11/2026 : « j'aurais aimé qu'il
  // y ait un petit signal qui me dise que les trucs avaient bien été pris en compte ») ; une page rechargée après un
  // premier calcul recalcule seule.
  var lance = false;
  function lancer() {
    lance = true;
    var b = $("calculer");
    if (b) b.remove();
    garder();
  }
  function offrirLeCalcul() {
    if (lance) {
      lancer();
      calculer();
      return;
    }
    var b = $("calculer");
    b.classList.remove("cache");
    b.addEventListener("click", function () {
      lancer();
      calculer();
    });
  }

  // La route `nom`, sa charge : la réponse du moteur, lue ({ ok, resultat, refus }). Une réponse qui ne se lit pas est
  // une faute technique : elle remonte telle quelle (son code, son texte), sans phrase de plus.
  function route(nom, charge) {
    return new Promise(function (rendre, rejeter) {
      var id = prochain++;
      attentes[id] = function (r) {
        var texte = new TextDecoder().decode(r.corps);
        try {
          rendre(JSON.parse(texte));
        } catch (e) {
          rejeter(new Error("réponse illisible du moteur (code " + r.code + ") : « " + texte + " » (" + texteDErreur(e) + ")"));
        }
      };
      var corps = new TextEncoder().encode(JSON.stringify(charge)).buffer;
      port.postMessage({ id: id, methode: "POST", adresse: "/api/" + nom, entetes: { "content-type": "application/json" }, corps: corps }, [corps]);
    });
  }

  // Les bulletins déposés, dans le dossier que le dépôt nomme.
  function poser(dossier) {
    return new Promise(function (rendre) {
      poses.push(rendre);
      worker.postMessage({ quoi: "poser", chemin: dossier, fichiers: fichiers.map(function (f) { return { nom: f.nom, octets: f.octets.slice(0) }; }) });
    });
  }

  // ---------------------------------------------------------------- ce qu'on dépose et ce qu'on saisit

  var fichiers = [];
  var aPoser = false;
  // Les bulletins déposés que la route n'a pas encore lus : le dépôt les montre « en lecture » jusqu'à sa réponse.
  var enLecture = [];
  // Ce qui est saisi dans chaque case, sous sa clé : un texte, oui-non, ou des rangées (chacune ses parties).
  var saisie = {};
  var dossier = null;
  // Les clés des cases que la route a rendues en dernier : les seules que la charge porte.
  var rendues = [];

  function ajouterFichiers(liste) {
    // Un fichier qui ne se lit pas se dit dans l'avis ; les autres se posent.
    var lus = Array.prototype.map.call(liste, function (f) {
      return f.arrayBuffer().then(
        function (o) { return { nom: f.name, octets: o }; },
        function (e) { avis("Le fichier " + f.name + " ne se lit pas : " + texteDErreur(e)); return null; }
      );
    });
    Promise.all(lus).then(function (neufs) {
      neufs.filter(function (n) { return n !== null; }).forEach(function (n) {
        // Deux fichiers du même nom (deux employeurs) restent deux fichiers.
        var nom = n.nom, k = 2;
        var point = nom.lastIndexOf(".");
        var base = point > 0 ? nom.slice(0, point) : nom, ext = point > 0 ? nom.slice(point) : "";
        while (fichiers.some(function (f) { return f.nom === nom; })) nom = base + " (" + k++ + ")" + ext;
        fichiers.push({ nom: nom, octets: n.octets });
        enLecture.push(nom);
      });
      aPoser = true;
      garderFichiers();
      if (!lance) lancer();
      redessiner();
      calculer();
    });
  }

  function retirerFichier(nom) {
    fichiers = fichiers.filter(function (f) { return f.nom !== nom; });
    enLecture = enLecture.filter(function (n) { return n !== nom; });
    aPoser = true;
    garderFichiers();
    calculer();
  }

  // ---------------------------------------------------------------- ce que l'onglet garde

  // Recharger la page par mégarde (sur un téléphone, d'un geste) ne perd rien (Léandre, 27/11/2026 : « tu fais
  // actualiser sans faire exprès et hop, tu perds tout ») : la saisie et les bulletins se gardent dans l'onglet
  // (`sessionStorage`, que le navigateur efface avec lui). Des bulletins trop lourds pour lui restent tant que la page
  // est ouverte, et le dépôt le dit.
  var GARDE = "puppetpaie-droits";
  var GARDE_FICHIERS = "puppetpaie-droits-bulletins";
  var gardeRatee = null;

  function garder() {
    try {
      sessionStorage.setItem(GARDE, JSON.stringify({ saisie: saisie, lance: lance }));
    } catch (e) {
      gardeRatee = "Ce navigateur ne garde pas ce que vous saisissez : recharger la page l'effacerait (" + texteDErreur(e) + ").";
    }
  }

  function enBase64(octets) {
    var o = new Uint8Array(octets), s = "";
    for (var k = 0; k < o.length; k += 0x8000) s += String.fromCharCode.apply(null, o.subarray(k, k + 0x8000));
    return btoa(s);
  }

  function deBase64(texte) {
    var s = atob(texte), o = new Uint8Array(s.length);
    for (var k = 0; k < s.length; k++) o[k] = s.charCodeAt(k);
    return o.buffer;
  }

  function garderFichiers() {
    try {
      sessionStorage.setItem(GARDE_FICHIERS, JSON.stringify(fichiers.map(function (f) { return { nom: f.nom, octets: enBase64(f.octets) }; })));
      gardeRatee = null;
    } catch (e) {
      try { sessionStorage.removeItem(GARDE_FICHIERS); } catch (e2) { /* le message ci-dessous le dit déjà */ }
      gardeRatee = "Vos bulletins sont trop lourds pour que ce navigateur les garde : si la page se recharge, il faudra les redéposer. Vos lignes, elles, sont gardées.";
    }
  }

  // Ce que l'onglet gardait, repris au chargement de la page. Ce qui ne se relit pas se dit, et la page repart vide.
  function reprendre() {
    var g, f;
    try {
      g = sessionStorage.getItem(GARDE);
      f = sessionStorage.getItem(GARDE_FICHIERS);
    } catch (e) {
      return;
    }
    try {
      if (g) {
        var lu = JSON.parse(g);
        saisie = lu.saisie || {};
        lance = lu.lance === true;
      }
      if (f) {
        fichiers = JSON.parse(f).map(function (x) { return { nom: x.nom, octets: deBase64(x.octets) }; });
        enLecture = fichiers.map(function (x) { return x.nom; });
        aPoser = fichiers.length > 0;
      }
    } catch (e) {
      saisie = {};
      fichiers = [];
      enLecture = [];
      lance = false;
      aPoser = false;
      try { sessionStorage.removeItem(GARDE); sessionStorage.removeItem(GARDE_FICHIERS); } catch (e2) { /* rien à effacer */ }
      avis("Ce que la page gardait de votre dernière visite ne se relit pas : elle repart vide (" + texteDErreur(e) + ").");
    }
  }

  // Un fichier lâché hors de la zone de dépôt ne fait pas quitter la page (le navigateur l'ouvrirait à sa place).
  ["dragover", "drop"].forEach(function (t) {
    document.addEventListener(t, function (e) { e.preventDefault(); });
  });

  // La charge : ce qui est saisi dans les cases que la route a rendues, et rien d'autre ; une case vide n'y est pas, une
  // rangée vide non plus (chaque rangée, ses parties remplies). `envoi` : pour chaque case en rangées, la rangée saisie
  // que porte chaque rang envoyé (la route nomme une rangée par son rang dans la charge).
  function charge() {
    var c = {}, envoi = {};
    rendues.forEach(function (cle) {
      var v = saisie[cle];
      if (Array.isArray(v)) {
        var rangees = [], rangs = [];
        v.forEach(function (r, n) {
          var o = {};
          Object.keys(r).forEach(function (k) { if (r[k] !== "" && r[k] !== null && r[k] !== false) o[k] = r[k]; });
          if (Object.keys(o).length) { rangees.push(o); rangs.push(n); }
        });
        if (rangees.length) c[cle] = rangees;
        envoi[cle] = rangs;
      } else if (v !== "" && v !== null && v !== undefined) {
        c[cle] = v;
      }
    });
    return { charge: c, envoi: envoi };
  }

  // ---------------------------------------------------------------- le calcul

  var enCours = false;
  var encore = false;

  // Les bulletins de `postes` sont lus (ou refusés) : ils quittent « en lecture ».
  function lus(postes) {
    enLecture = enLecture.filter(function (n) { return postes.indexOf(n) < 0; });
  }

  function calculer() {
    garder();
    if (!pret || !lance) return;
    if (enCours) { encore = true; return; }
    enCours = true;
    encore = false;
    var suite = Promise.resolve();
    var postes = [];
    if (aPoser && dossier !== null) {
      aPoser = false;
      postes = enLecture.slice();
      suite = poser(dossier).then(function (ok) {
        if (!ok) avis("Les bulletins ne se sont pas posés sur le disque du logiciel : le dossier " + dossier + " n'est pas mes-pieces de l'installation, ou n'existe pas. C'est un défaut du logiciel.");
      });
    }
    // Une erreur, où qu'elle tombe, se dit dans l'avis et ne fige rien : le calcul suivant part.
    var envoi = null;
    suite.then(function () {
      var x = charge();
      envoi = x.envoi;
      return route("droits-des-pieces", x.charge);
    }).then(function (r) {
      lus(postes);
      if (r.ok && r.resultat) {
        montrer(r.resultat, envoi);
      } else if (r.refus) {
        redessiner();
        montrerRefus(r.refus.phrase);
      } else {
        throw new Error("réponse du moteur sans résultat ni refus : " + JSON.stringify(r));
      }
    }).catch(function (e) {
      lus(postes);
      redessiner();
      avis("Défaut du logiciel : " + texteDErreur(e));
    }).then(function () {
      enCours = false;
      // Des bulletins déposés avant que la route ait nommé leur dossier se posent maintenant.
      if (encore || (aPoser && dossier !== null)) calculer();
    });
  }

  // ---------------------------------------------------------------- les cases que la route rend

  // Une valeur que la route rend, telle qu'elle l'écrit ; vide, rien.
  function texteDe(v) {
    return v === null || v === undefined ? "" : String(v);
  }

  // Une rangée vide d'une case : chaque partie vide.
  function rangeeVide(c) {
    var o = {};
    c.parties.forEach(function (p) { o[p.cle] = p.forme === "oui_non" ? false : ""; });
    return o;
  }

  // La valeur de départ d'une case, quand rien n'est encore saisi : celle que la route y dit.
  function depart(c) {
    if (c.forme === "rangees") {
      return (c.valeur || []).map(function (r) {
        var o = {};
        c.parties.forEach(function (p) { o[p.cle] = p.forme === "oui_non" ? r[p.cle] === true : texteDe(r[p.cle]); });
        return o;
      });
    }
    return texteDe(c.valeur);
  }

  // UNE CASE SIMPLE, par sa forme : une date, un montant ou un nombre tapé, oui-non coché, un choix dans une liste
  // déroulante (une liste fermée, `choix`, ou des choix que la route rend, `choix_rendu`). `propose` : ce que la route
  // prend pour la case vide (`prerempli`), montré tant que rien n'est choisi.
  function champ(forme, choix, valeur, changer, id, propose) {
    var i;
    if (forme === "choix_rendu" || forme === "choix") {
      i = el("select");
      i.append(el("option", null, ""));
      (choix || []).forEach(function (x) {
        var o = el("option", null, x.libelle);
        o.value = x.code;
        i.append(o);
      });
      i.value = valeur === "" && propose ? propose : valeur;
      i.addEventListener("change", function () { changer(i.value); });
    } else if (forme === "oui_non") {
      i = el("input");
      i.type = "checkbox";
      i.checked = valeur === true;
      i.addEventListener("change", function () { changer(i.checked); });
    } else if (forme === "date") {
      return champDate(valeur, changer, id);
    } else {
      i = el("input");
      i.type = "text";
      if (forme === "montant" || forme === "nombre") i.inputMode = "decimal";
      i.value = valeur;
      i.addEventListener("change", function () { changer(i.value.trim()); });
    }
    if (id) i.id = id;
    return i;
  }

  // UNE DATE se lit JJ/MM/AAAA, comme en France, quelle que soit la langue du téléphone (retour R-BD9FBD : le calendrier
  // « s'affiche à l'envers ») ; la toucher ouvre le calendrier du navigateur, gardé caché derrière elle, et elle se tape
  // aussi (là où le navigateur ne sait pas ouvrir son calendrier). La valeur saisie reste celle que la route lit, AAAA-MM-JJ ;
  // un texte qui n'est pas une date part tel quel, et la route dit quoi corriger.
  function enClair(iso) {
    var m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(iso);
    return m ? m[3] + "/" + m[2] + "/" + m[1] : iso;
  }

  function enIso(texte) {
    var m = /^(\d{1,2})[\/.\- ](\d{1,2})[\/.\- ](\d{4})$/.exec(texte);
    if (!m) return texte;
    var deux = function (x) { return x.length === 1 ? "0" + x : x; };
    return m[3] + "-" + deux(m[2]) + "-" + deux(m[1]);
  }

  function champDate(valeur, changer, id) {
    var s = el("span", "date");
    var i = el("input");
    i.type = "text";
    i.placeholder = "JJ/MM/AAAA";
    i.autocomplete = "off";
    i.value = enClair(valeur);
    if (id) i.id = id;
    var cal = el("input", "natif");
    cal.type = "date";
    cal.tabIndex = -1;
    cal.setAttribute("aria-hidden", "true");
    if (typeof cal.showPicker === "function") {
      // Sur un téléphone, le calendrier, pas le clavier ; un navigateur qui ne l'ouvre pas rend le clavier.
      i.inputMode = "none";
      i.addEventListener("click", function () {
        cal.value = /^\d{4}-\d{2}-\d{2}$/.test(enIso(i.value)) ? enIso(i.value) : "";
        try {
          cal.showPicker();
        } catch (e) {
          i.inputMode = "numeric";
        }
      });
    } else {
      i.inputMode = "numeric";
    }
    cal.addEventListener("change", function () {
      i.value = enClair(cal.value);
      changer(cal.value);
    });
    i.addEventListener("change", function () { changer(enIso(i.value.trim())); });
    s.append(i, cal);
    return s;
  }

  // Le bouton qui retire la rangée `n` de la case `cle` (`libelle`, celui de la route ; son nom, avec le rang). Les rangs
  // envoyés qui la suivent reculent d'un cran, pour que chaque rangée rendue retrouve la sienne.
  function boutonRetirer(cle, libelle, n, signe) {
    var b = el("button", signe ? "croix" : "lien", signe || libelle);
    if (signe) b.title = libelle;
    b.type = "button";
    b.setAttribute("aria-label", libelle + " " + (n + 1));
    b.addEventListener("click", function () {
      saisie[cle].splice(n, 1);
      if (envoiRendu[cle]) {
        envoiRendu[cle] = envoiRendu[cle].map(function (k) { return k === n ? -1 : k > n ? k - 1 : k; });
      }
      redessiner();
      calculer();
    });
    return b;
  }

  // Le bouton qui ajoute une rangée vide à la case `c` (`libelle`, celui de la route).
  function boutonAjouter(c, libelle) {
    var b = el("button", null, libelle);
    b.type = "button";
    b.addEventListener("click", function () {
      saisie[c.cle].push(rangeeVide(c));
      redessiner();
    });
    return b;
  }

  function dessinerCase(c) {
    var bloc = el("div", "case");
    var id = "case-" + c.cle;
    if (c.forme !== "rangees") {
      var l = el("label", null, c.libelle);
      l.htmlFor = id;
      bloc.append(l);
      bloc.append(champ(c.forme, c.choix, saisie[c.cle], function (v) { saisie[c.cle] = v; calculer(); }, id, texteDe(c.prerempli)));
      if (c.aide) bloc.append(el("p", "aide", c.aide));
      (c.dit || []).forEach(function (d) { bloc.append(el("p", "aide", d)); });
      return bloc;
    }
    bloc.append(el("div", "libelle", c.libelle));
    (c.dit || []).forEach(function (d) { bloc.append(el("p", "aide", d)); });
    saisie[c.cle].forEach(function (r, n) {
      var ligne = el("div", "rangee");
      c.parties.forEach(function (p) {
        var cellule = el("div", p.forme === "oui_non" ? "oui-non" : null);
        var idp = id + "-" + n + "-" + p.cle;
        var lp = el("label", null, p.libelle);
        lp.htmlFor = idp;
        var i = champ(p.forme, p.choix, r[p.cle], function (v) { r[p.cle] = v; calculer(); }, idp);
        if (p.forme === "oui_non") { cellule.append(i, lp); } else { cellule.append(lp, i); }
        ligne.append(cellule);
      });
      if (c.retirer) ligne.append(boutonRetirer(c.cle, c.retirer, n));
      bloc.append(ligne);
    });
    if (c.ajouter) bloc.append(boutonAjouter(c, c.ajouter));
    return bloc;
  }

  // ---------------------------------------------------------------- ce que la route dit

  // UN TABLEAU QUE LA ROUTE REND (`socle::ecran::TableauRendu`) : ses colonnes (à droite, discrète), ses rangées de
  // cellules déjà dites (vide : rien), les totaux en gras. Son titre se pose à part.
  function tableau(t) {
    var table = el("table");
    var classe = function (c) { return c.a_droite ? "n" : c.discrete ? "discret" : null; };
    var tr = el("tr");
    t.colonnes.forEach(function (c) { tr.append(el("th", classe(c), c.libelle)); });
    var tete = el("thead");
    tete.append(tr);
    table.append(tete);
    var corps = el("tbody");
    t.rangees.forEach(function (g) {
      var r = el("tr", g.sorte === "ligne" ? null : "total");
      g.cellules.forEach(function (v, i) { r.append(el("td", classe(t.colonnes[i]), v)); });
      corps.append(r);
    });
    table.append(corps);
    var d = el("div", "tableau");
    d.append(table);
    return d;
  }

  // Les lignes [quoi, valeur] que la route rend (le compte des heures) : deux colonnes, sans en-tête.
  function paires(lignes) {
    var table = el("table");
    var corps = el("tbody");
    lignes.forEach(function (l) {
      var r = el("tr");
      r.append(el("td", null, l[0]), el("td", "n", l[1]));
      corps.append(r);
    });
    table.append(corps);
    var d = el("div", "tableau");
    d.append(table);
    return d;
  }

  function liste(phrases) {
    var ul = el("ul", "phrases");
    phrases.forEach(function (p) { ul.append(el("li", null, p)); });
    return ul;
  }

  // LA RÉPONSE, en tête : son nom, quand la route le dit (les pièces portent plusieurs personnes), les heures (ou ce
  // qui manque) et jusqu'à quand, l'allocation, et en petit la fenêtre ; sa couleur, celle que la route dit.
  function reponse(p) {
    var r = el("section", "reponse " + p.couleur);
    if (p.nom) r.append(el("h2", null, p.nom));
    r.append(el("p", "heures", p.heures));
    if (p.jusqu_a) r.append(el("p", "jusqu", p.jusqu_a));
    if (p.allocation) r.append(el("p", "allocation", p.allocation));
    if (p.annee) r.append(el("p", "annee", p.annee));
    r.append(el("p", "sous", p.sous));
    return r;
  }

  // L'ALLOCATION VISÉE : sa case sur une ligne (son libellé, la case, son unité), le bouton qui la vide, et la réponse
  // de la route quand elle est remplie.
  function visee(b) {
    var c = b.case, id = "case-" + c.cle;
    var s = el("section", "visee");
    var ligne = el("div", "ligne");
    var l = el("label", null, c.libelle);
    l.htmlFor = id;
    ligne.append(l, champ(c.forme, c.choix, saisie[c.cle], function (v) { saisie[c.cle] = v; calculer(); }, id), el("span", null, b.unite));
    var vider = el("button", "lien", b.reinitialiser);
    vider.type = "button";
    vider.addEventListener("click", function () { saisie[c.cle] = ""; redessiner(); calculer(); });
    ligne.append(vider);
    s.append(ligne);
    if (b.reponse) s.append(el("p", null, b.reponse));
    return s;
  }

  // UN CALCUL EN LIGNES que la route rend : chaque ligne [quoi, valeur], puis le total, en gras.
  function calcul(c) {
    var g = el("div", "calcul");
    c.lignes.forEach(function (l) { g.append(el("span", null, l[0]), el("span", "n", l[1])); });
    g.append(el("span", "tot", c.total[0]), el("span", "n tot", c.total[1]));
    return g;
  }

  // LES BARRES DES MOIS : le salaire brut (vert) sous le chômage (bleu), chaque mois. L'échelle est celle du dessin :
  // la plus haute barre touche le haut.
  function barres(liste) {
    var NS = "http://www.w3.org/2000/svg";
    var svg = document.createElementNS(NS, "svg");
    svg.setAttribute("viewBox", "0 0 640 250");
    svg.setAttribute("class", "barres");
    svg.setAttribute("role", "img");
    svg.setAttribute("aria-label", "Chômage et salaire brut, mois par mois");
    var n = function (nom, a, t) {
      var x = document.createElementNS(NS, nom);
      Object.keys(a).forEach(function (k) { x.setAttribute(k, a[k]); });
      if (t !== undefined) x.textContent = t;
      svg.append(x);
    };
    var nombre = function (v) { return v === null || v === undefined ? 0 : parseFloat(v); };
    var haut = 1;
    liste.forEach(function (m) { haut = Math.max(haut, nombre(m.chomage) + nombre(m.salaire)); });
    var G = { g: 10, d: 630, h: 10, b: 220 }, w = (G.d - G.g) / Math.max(liste.length, 1);
    var Y = function (v) { return G.b - (G.b - G.h) * v / haut; };
    n("line", { x1: G.g, x2: G.d, y1: G.b, y2: G.b, stroke: "#E6E5E2" });
    liste.forEach(function (m, i) {
      var x = G.g + i * w + w * 0.15, l = w * 0.7, s = nombre(m.salaire), c = nombre(m.chomage);
      n("rect", { x: x, y: Y(s), width: l, height: G.b - Y(s), fill: "#1E7A46" });
      n("rect", { x: x, y: Y(s + c), width: l, height: Y(s) - Y(s + c), fill: "#2350C4" });
      n("text", { x: x + l / 2, y: G.b + 20, "text-anchor": "middle", "font-size": 13, fill: "#6E6B66" }, m.mois);
    });
    return svg;
  }

  // L'ANNÉE DE CHÔMAGE : sans aucun travail (le chiffre, ses calculs, ce qui s'en dit, la case du taux de CSG), ce que
  // coûte un jour de travail et ses exemples, l'année comme la dernière (ses barres, le tableau des mois, le calcul), et
  // les notes. Tout est dit par la route.
  function annee(b) {
    var s = el("section", "bloc annee");
    s.append(el("h2", null, b.titre));
    var st = b.sans_travail;
    s.append(el("h3", null, st.titre));
    var ch = el("div", "chiffre");
    ch.append(el("b", null, st.chiffre), el("span", null, st.sous));
    s.append(ch);
    st.calculs.forEach(function (c) { s.append(calcul(c)); });
    st.aide.forEach(function (a) { s.append(el("p", "aide", a)); });
    s.append(dessinerCase(b.case));
    var co = b.cout;
    s.append(el("h3", null, co.titre));
    s.append(el("p", "aide", co.aide));
    co.exemples.forEach(function (x) {
      var e = el("div", "exemple");
      e.append(el("p", null, x.titre), calcul(x.calcul));
      if (x.aide) e.append(el("p", "aide", x.aide));
      s.append(e);
    });
    if (b.comme) {
      var cm = b.comme;
      s.append(el("h3", null, cm.titre));
      s.append(el("p", "aide", cm.aide));
      s.append(barres(cm.barres));
      var lg = el("div", "legende");
      var p1 = el("span"), p2 = el("span");
      p1.append(el("i", "pastille salaire"), "salaire brut");
      p2.append(el("i", "pastille chomage"), "chômage");
      lg.append(p1, p2);
      s.append(lg);
      s.append(tableau(cm.tableau));
      var e = el("div", "exemple");
      e.append(calcul(cm.calcul));
      cm.dit.forEach(function (d) { e.append(el("p", "aide", d)); });
      s.append(e);
    }
    if (b.notes.length) s.append(liste(b.notes));
    return s;
  }

  var detailOuvert = false;

  // LE DÉTAIL DU CALCUL, replié en bas : la fenêtre, le compte des heures, ce qui manque, l'estimation (son tableau,
  // paie par paie) et les phrases.
  function details(d) {
    var det = el("details", "details");
    // Ouvert, il le reste d'un calcul à l'autre (le confort de la main).
    det.open = detailOuvert;
    det.addEventListener("toggle", function () { detailOuvert = det.open; });
    det.append(el("summary", null, d.libelle));
    // En tête, les heures de chaque annexe, celle qui s'applique, et l'annexe de chaque paie.
    if (d.annexes.length) det.append(liste(d.annexes));
    det.append(el("p", "aide", d.fenetre));
    det.append(paires(d.compte));
    if (d.estimation && d.estimation.paies.rangees.length) {
      if (d.estimation.paies.titre) det.append(el("h3", null, d.estimation.paies.titre));
      det.append(tableau(d.estimation.paies));
    }
    if (d.lignes.length) det.append(liste(d.lignes));
    return det;
  }

  // LE TABLEAU DES PAIES : les paies lues, et les lignes prévues qui s'y saisissent (`tableau.saisie` : la case, ses
  // boutons). Une rangée qui se saisit (`saisie`, son rang dans la charge envoyée) pose, sous chaque colonne qui tient
  // des parties, leurs cases ; ailleurs, ses cellules dites. Une rangée saisie que le tableau ne place pas (vide, ou qui
  // ne se compte pas) se pose après les autres. « Ajouter une ligne » sous le tableau.
  function paies(b) {
    var t = b.tableau, c = b.lignes, cle = t.saisie.case;
    var parties = {};
    c.parties.forEach(function (p) { parties[p.cle] = p; });
    var rangs = saisie[cle], envoi = envoiRendu[cle] || [], placees = {};
    // UNE LIGNE VIDE ATTEND TOUJOURS EN BAS (Léandre, 27/11/2026 : « il n'y a pas de bouton pour ajouter une ligne. Il y
    // a déjà une ligne dans le tableau, et si on la remplit, il y a une autre ligne qui apparaît toute seule, vide »).
    // Une rangée vide ne part pas dans la charge (`charge`) : elle ne compte rien.
    var remplie = function (r) {
      return Object.keys(r).some(function (k) { return r[k] !== "" && r[k] !== null && r[k] !== false; });
    };
    if (!rangs.length || remplie(rangs[rangs.length - 1])) rangs.push(rangeeVide(c));
    // L'AFFICHAGE EST CELUI DE LA PAGE (Léandre, 27/11/2026 : « les calculs, ça doit être les mêmes, il ne doit y avoir
    // aucun calcul dans la page. Mais sinon, tout le reste, on le sort comme on veut ») : la colonne de l'allocation
    // visée est verte.
    var classe = function (col) {
      var k = [];
      if (col.a_droite) k.push("n");
      if (col.discrete) k.push("discret");
      if (!col.parties || !col.parties.length) { if (/^Pour /.test(col.libelle)) k.push("vise"); }
      return k.length ? k.join(" ") : null;
    };
    var s = el("section", "bloc");
    if (t.titre) s.append(el("h3", null, t.titre));
    (c.dit || []).forEach(function (d) { s.append(el("p", "aide", d)); });
    var table = el("table", "saisie");
    var tr = el("tr");
    t.colonnes.forEach(function (col) { tr.append(el("th", classe(col), col.libelle)); });
    tr.append(el("th"));
    var tete = el("thead");
    tete.append(tr);
    table.append(tete);
    var corps = el("tbody");
    // Sous chaque colonne qui tient des parties ouvertes, leurs cases ; ailleurs, la cellule que la route dit.
    var ligneSaisie = function (n, cellules) {
      placees[n] = true;
      var r = el("tr", "prevue");
      t.colonnes.forEach(function (col, i) {
        var td = el("td", classe(col));
        if (col.parties && col.parties.length) {
          col.parties.filter(function (k) { return parties[k].ouverte; }).forEach(function (k) {
            var p = parties[k];
            var i2 = champ(p.forme, p.choix, rangs[n][k], function (v) {
              rangs[n][k] = v;
              // La ligne vide qu'on remplit en fait paraître une autre, même avant que le moteur soit prêt.
              if (n === rangs.length - 1) redessiner();
              calculer();
            }, "case-" + cle + "-" + n + "-" + k, texteDe(p.prerempli));
            (i2.querySelector("input") || i2).setAttribute("aria-label", p.libelle + " " + (n + 1));
            td.append(i2);
            // Un montant porte « € », le dessin de sa forme.
            if (p.forme === "montant") td.append(el("span", "unite", "€"));
          });
        } else if (cellules) {
          td.textContent = cellules[i] || "";
        }
        r.append(td);
      });
      var fin = el("td");
      if (n < rangs.length - 1) fin.append(boutonRetirer(cle, t.saisie.retirer, n, "×"));
      r.append(fin);
      corps.append(r);
    };
    t.rangees.forEach(function (g) {
      var n = g.saisie === undefined ? undefined : envoi[g.saisie];
      if (n !== undefined && n >= 0 && rangs[n] && !placees[n]) {
        ligneSaisie(n, g.cellules);
        return;
      }
      if (g.saisie !== undefined) return;
      var r = el("tr", g.sorte === "ligne" ? null : "total");
      g.cellules.forEach(function (v, i) { r.append(el("td", classe(t.colonnes[i]), v)); });
      r.append(el("td"));
      corps.append(r);
    });
    rangs.forEach(function (r, n) { if (!placees[n]) ligneSaisie(n, null); });
    // Dans le tableau, la nature se dit court (« Technicien », « Artiste ») : l'annexe est dite en haut.
    var court = function (x) { return x.replace(/ \(annexe \d+\)$/, ""); };
    corps.querySelectorAll("option").forEach(function (o) { o.textContent = court(o.textContent); });
    corps.querySelectorAll("td").forEach(function (td) { if (!td.children.length) td.textContent = court(td.textContent); });
    table.append(corps);
    var d = el("div", "tableau");
    d.append(table);
    s.append(d);
    return s;
  }

  // LE DÉPÔT : la zone où l'on glisse ou choisit ses bulletins (son libellé et son aide, ceux de la route).
  function depot(b) {
    dossier = b.dossier;
    var s = el("section", "bloc");
    var zone = el("label", "depot");
    var entree = el("input");
    entree.type = "file";
    entree.id = "choisir";
    entree.multiple = true;
    entree.addEventListener("change", function () {
      ajouterFichiers(entree.files);
      entree.value = "";
    });
    zone.append(el("strong", null, b.libelle), el("span", null, b.aide), entree);
    ["dragenter", "dragover"].forEach(function (t) {
      zone.addEventListener(t, function (e) { e.preventDefault(); zone.classList.add("survol"); });
    });
    ["dragleave", "drop"].forEach(function (t) {
      zone.addEventListener(t, function (e) { e.preventDefault(); zone.classList.remove("survol"); });
    });
    zone.addEventListener("drop", function (e) { if (e.dataTransfer && e.dataTransfer.files.length) ajouterFichiers(e.dataTransfer.files); });
    s.append(zone);
    // Les bulletins déposés que la route n'a pas encore lus, tout de suite sous le dépôt : on voit qu'ils sont pris.
    if (enLecture.length) {
      var ul = el("ul", "fichiers en-lecture");
      enLecture.forEach(function (n) {
        var li = el("li");
        li.append(el("span", "nom", n), el("span", "aide", pret ? "lecture…" : "le logiciel se charge, la lecture suit…"));
        ul.append(li);
      });
      s.append(ul);
    }
    if (gardeRatee) s.append(el("p", "aide", gardeRatee));
    return s;
  }

  // LES PIÈCES DÉPOSÉES, une ligne chacune : le fichier, ce que la route en lit, et le bouton qui le retire (son
  // libellé, celui de la route ; son nom, avec le fichier).
  function pieces(b) {
    var s = el("section", "bloc");
    var ul = el("ul", "fichiers");
    b.pieces.forEach(function (p) {
      var li = el("li");
      var quoi = el("div");
      quoi.append(el("span", "nom", p.fichier));
      // Sa nature et ses employeurs, quand la route les dit.
      var qui = (p.nature ? [p.nature] : []).concat(p.employeurs);
      if (qui.length) quoi.append(el("div", "aide", qui.join(" · ")));
      if (p.dit.length) quoi.append(liste(p.dit));
      var x = el("button", "lien", b.retirer);
      x.type = "button";
      x.setAttribute("aria-label", b.retirer + " " + p.fichier);
      x.addEventListener("click", function () { retirerFichier(p.fichier); });
      li.append(quoi, x);
      ul.append(li);
    });
    s.append(ul);
    return s;
  }

  // ---------------------------------------------------------------- l'écran

  // La phrase du résultat, que les lecteurs d'écran annoncent (seule à l'être) : celle que la route rend.
  function annoncer(phrase) {
    $("annonce").textContent = phrase;
  }

  // Un refus de la route se dit au-dessus de l'écran, qui reste tel qu'il était : on corrige la case qu'il nomme.
  function montrerRefus(phrase) {
    var r = $("refus");
    r.textContent = phrase;
    r.classList.remove("cache");
    annoncer(phrase);
  }

  var dernier = null;
  // Pour le dernier écran rendu : la rangée saisie que porte chaque rang envoyé, par case (`charge`).
  var envoiRendu = {};

  // L'ÉCRAN : les blocs que la route rend, dans son ordre. Une case que la route ne rend plus perd ce qui y était saisi
  // (rendue de nouveau, elle repart de ce que la route dit) ; ce qui est saisi dans les autres reste.
  // Le premier écran (`premier`) ne retire rien de la saisie : ce que l'onglet gardait d'un calcul (une case que seul un
  // calcul rend) attend la réponse de la route. Une case gardée qui reparaît, remplie, sans être partie dans la charge,
  // fait recalculer une fois.
  function montrer(d, envoi, premier) {
    dernier = d;
    envoiRendu = envoi;
    $("refus").classList.add("cache");
    annoncer(d.phrase);
    var cases = [];
    d.ecran.forEach(function (b) {
      if (b.est === "case") cases.push(b);
      if (b.est === "visee") cases.push(b.case);
      if (b.est === "annee") cases.push(b.case);
      if (b.est === "paies") cases.push(b.lignes);
    });
    var avant = rendues;
    rendues = cases.map(function (c) { return c.cle; });
    if (!premier) {
      Object.keys(saisie).forEach(function (cle) { if (rendues.indexOf(cle) < 0) delete saisie[cle]; });
      rendues.forEach(function (cle) {
        var v = saisie[cle];
        if (avant.indexOf(cle) < 0 && v !== undefined && v !== "" && !(Array.isArray(v) && !v.length)) encore = true;
      });
    }
    cases.forEach(function (c) { if (!(c.cle in saisie)) saisie[c.cle] = depart(c); });
    redessiner();
  }

  // Redessine le dernier écran rendu, en gardant la case où l'on était et ce qu'on y tapait sans l'avoir quittée.
  function redessiner() {
    if (!dernier) return;
    var actif = document.activeElement;
    var ici = actif && actif.id;
    var tape = actif && actif.tagName === "INPUT" && actif.type !== "checkbox" && actif.type !== "file" ? actif.value : null;
    var ecran = $("ecran");
    ecran.replaceChildren();
    dernier.ecran.forEach(function (b) {
      if (b.est === "case") ecran.append(dessinerCase(b));
      else if (b.est === "reponse") ecran.append(reponse(b));
      else if (b.est === "visee") ecran.append(visee(b));
      else if (b.est === "annee") ecran.append(annee(b));
      else if (b.est === "paies") ecran.append(paies(b));
      else if (b.est === "depot") ecran.append(depot(b));
      else if (b.est === "pieces") ecran.append(pieces(b));
      else if (b.est === "details") ecran.append(details(b));
      else throw new Error("bloc inconnu de la route : " + b.est);
    });
    if (ici && $(ici)) {
      if (tape !== null) $(ici).value = tape;
      $(ici).focus();
    }
  }

  reprendre();
  demarrer();
})();
