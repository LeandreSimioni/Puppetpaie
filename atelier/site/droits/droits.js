// LA PAGE DES DROITS D'UN INTERMITTENT (puppetpaie.fr/droits/). Elle s'ouvre tout de suite ; le moteur se charge
// derrière, dans le worker de la démo (/demo/moteur.js : les deux mêmes fichiers, onglet.wasm.gz et demo.tar.gz).
// Elle ne décide rien : elle demande la route `droits-des-pieces` et pose son écran, bloc par bloc dans l'ordre où la
// route le rend (`ecran` : la réponse, l'allocation visée, les cases, dessinées depuis la forme que la route leur
// donne, le tableau des paies, le dépôt, les pièces, le détail du calcul). Avant que le moteur soit prêt, elle pose ce que la route rend sans charge
// (`premier-ecran.json`) ; le moteur prêt, « Calculer » fait le premier calcul. Ensuite, chaque changement (un bulletin
// déposé ou retiré, une case) redemande la route. Les
// bulletins déposés vont dans le dossier que le dépôt nomme (`dossier`), sur le disque en mémoire du moteur. Elle ne
// garde rien : ni stockage, ni cache ; fermer l'onglet efface tout.
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
      if (dernier === null) montrer(d, {});
    }).catch(function (e) {
      avis("Défaut du logiciel : " + texteDErreur(e));
    });
  }

  // LE BOUTON « CALCULER » apparaît quand le moteur est prêt (Léandre, 27/11/2026 : « Le bouton calculer arrive avec
  // le moteur ») ; un appui fait le premier calcul, et il s'en va : ensuite tout se recalcule seul à chaque changement.
  var lance = false;
  function offrirLeCalcul() {
    var b = $("calculer");
    b.classList.remove("cache");
    b.addEventListener("click", function () {
      lance = true;
      b.remove();
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
      });
      aPoser = true;
      calculer();
    });
  }

  function retirerFichier(nom) {
    fichiers = fichiers.filter(function (f) { return f.nom !== nom; });
    aPoser = true;
    calculer();
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

  function calculer() {
    if (!pret || !lance) return;
    if (enCours) { encore = true; return; }
    enCours = true;
    encore = false;
    var suite = Promise.resolve();
    if (aPoser && dossier !== null) {
      aPoser = false;
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
      if (r.ok && r.resultat) {
        montrer(r.resultat, envoi);
      } else if (r.refus) {
        montrerRefus(r.refus.phrase);
      } else {
        throw new Error("réponse du moteur sans résultat ni refus : " + JSON.stringify(r));
      }
    }).catch(function (e) {
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
    } else {
      i = el("input");
      i.type = forme === "date" ? "date" : "text";
      if (forme === "montant" || forme === "nombre") i.inputMode = "decimal";
      i.value = valeur;
      i.addEventListener("change", function () { changer(i.value.trim()); });
    }
    if (id) i.id = id;
    return i;
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

  var detailOuvert = false;
  // La sorte d'heures cochée pour chaque ligne prévue (le confort de la main, avant qu'un nombre soit tapé).
  var genres = new WeakMap();

  // LE DÉTAIL DU CALCUL, replié en bas : la fenêtre, le compte des heures, ce qui manque, l'estimation (son tableau,
  // paie par paie) et les phrases.
  function details(d) {
    var det = el("details", "details");
    // Ouvert, il le reste d'un calcul à l'autre (le confort de la main).
    det.open = detailOuvert;
    det.addEventListener("toggle", function () { detailOuvert = det.open; });
    det.append(el("summary", null, d.libelle));
    // La case de l'annexe, quand la route l'y pose (les pièces la disent).
    if (d.annexe) det.append(dessinerCase(d.annexe));
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
    // aucun calcul dans la page. Mais sinon, tout le reste, on le sort comme on veut ») : l'enseignement et la formation
    // ne sont pas des colonnes, mais deux petites cases à cocher à côté des heures (« soit c'est des heures normales,
    // soit on coche enseignement ou formation ») ; la colonne de l'allocation visée est verte.
    var genreDe = function (col) {
      var p = col.parties || [];
      return p.indexOf("enseignement") >= 0 ? "enseignement" : p.indexOf("formation") >= 0 ? "formation" : null;
    };
    var iGenre = {};
    t.colonnes.forEach(function (col, i) { var g = genreDe(col); if (g) iGenre[g] = i; });
    var iHeures = t.colonnes.findIndex(function (col) { return (col.parties || []).indexOf("heures") >= 0; });
    var montree = function (col) { return genreDe(col) === null; };
    var classe = function (col) {
      var k = [];
      if (col.a_droite) k.push("n");
      if (col.discrete) k.push("discret");
      if (!col.parties || !col.parties.length) { if (/^Pour /.test(col.libelle)) k.push("vise"); }
      return k.length ? k.join(" ") : null;
    };
    // Les deux petites colonnes « ens. » et « form. », juste après les heures : les chiffres des heures restent les uns
    // sous les autres (Léandre, 27/11/2026).
    var GENRES = [["enseignement", "ens.", "Heures d'enseignement"], ["formation", "form.", "Heures de formation"]];
    var lu = function (cellules, g) { return iGenre[g] === undefined ? "" : cellules[iGenre[g]]; };
    // Les heures d'une rangée lue : ses heures, et celles d'enseignement ou de formation qu'elle porte.
    var heuresLues = function (cellules) {
      return [cellules[iHeures] || "", lu(cellules, "enseignement"), lu(cellules, "formation")].filter(Boolean).join(" + ");
    };
    var celluleDeGenre = function (contenu, titre) {
      var td = el("td", "genre");
      td.title = titre;
      if (contenu) td.append(contenu);
      return td;
    };
    // Les heures d'une ligne prévue : une case, et les deux cases à cocher de ses petites colonnes. Ce qui est tapé part
    // sous `heures`, `enseignement` ou `formation` de la charge, selon la case cochée ; la page ne compte rien.
    var casesDHeures = function (rang, n) {
      var genre = genres.get(rang) || (rang.enseignement !== "" ? "enseignement" : rang.formation !== "" ? "formation" : "heures");
      var valeur = rang[genre];
      var i = champ("nombre", null, valeur, function (v) {
        rang.heures = ""; rang.enseignement = ""; rang.formation = "";
        rang[genre] = v;
        if (n === rangs.length - 1) redessiner();
        calculer();
      }, "case-" + cle + "-" + n + "-heures");
      i.setAttribute("aria-label", "Heures " + (n + 1));
      var cases = GENRES.map(function (g) {
        var x = el("input");
        x.type = "checkbox";
        x.id = "case-" + cle + "-" + n + "-" + g[0];
        x.checked = genre === g[0];
        x.addEventListener("change", function () {
          var v = rang[genre];
          var neuf = x.checked ? g[0] : "heures";
          rang.heures = ""; rang.enseignement = ""; rang.formation = "";
          rang[neuf] = v;
          genres.set(rang, neuf);
          redessiner();
          if (v !== "") calculer();
        });
        x.setAttribute("aria-label", g[2] + " " + (n + 1));
        return celluleDeGenre(x, g[2]);
      });
      return { heures: i, cases: cases };
    };
    var s = el("section", "bloc");
    if (t.titre) s.append(el("h3", null, t.titre));
    (c.dit || []).forEach(function (d) { s.append(el("p", "aide", d)); });
    var table = el("table", "saisie");
    var tr = el("tr");
    t.colonnes.forEach(function (col, i) {
      if (!montree(col)) return;
      tr.append(el("th", classe(col), col.libelle));
      if (i === iHeures) GENRES.forEach(function (g) { var th = el("th", "genre", g[1]); th.title = g[2]; tr.append(th); });
    });
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
        if (!montree(col)) return;
        var td = el("td", classe(col));
        if (i === iHeures) {
          var h = casesDHeures(rangs[n], n);
          td.append(h.heures);
          r.append(td);
          h.cases.forEach(function (c2) { r.append(c2); });
          return;
        } else if (col.parties && col.parties.length) {
          col.parties.filter(function (k) { return parties[k].ouverte; }).forEach(function (k) {
            var p = parties[k];
            var i2 = champ(p.forme, p.choix, rangs[n][k], function (v) {
              rangs[n][k] = v;
              // La ligne vide qu'on remplit en fait paraître une autre, même avant que le moteur soit prêt.
              if (n === rangs.length - 1) redessiner();
              calculer();
            }, "case-" + cle + "-" + n + "-" + k);
            i2.setAttribute("aria-label", p.libelle + " " + (n + 1));
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
      g.cellules.forEach(function (v, i) {
        if (!montree(t.colonnes[i])) return;
        r.append(el("td", classe(t.colonnes[i]), i === iHeures ? heuresLues(g.cellules) : v));
        if (i === iHeures) GENRES.forEach(function (gg) { r.append(celluleDeGenre(lu(g.cellules, gg[0]) ? document.createTextNode("✓") : null, gg[2])); });
      });
      r.append(el("td"));
      corps.append(r);
    });
    rangs.forEach(function (r, n) { if (!placees[n]) ligneSaisie(n, null); });
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
  function montrer(d, envoi) {
    dernier = d;
    envoiRendu = envoi;
    $("refus").classList.add("cache");
    annoncer(d.phrase);
    var cases = [];
    d.ecran.forEach(function (b) {
      if (b.est === "case") cases.push(b);
      if (b.est === "visee") cases.push(b.case);
      if (b.est === "paies") cases.push(b.lignes);
      if (b.est === "details" && b.annexe) cases.push(b.annexe);
    });
    rendues = cases.map(function (c) { return c.cle; });
    Object.keys(saisie).forEach(function (cle) { if (rendues.indexOf(cle) < 0) delete saisie[cle]; });
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

  demarrer();
})();
