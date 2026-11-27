// LA PAGE DES DROITS D'UN INTERMITTENT (puppetpaie.fr/droits/). Elle s'ouvre tout de suite ; le moteur se charge
// derrière, dans le worker de la démo (/demo/moteur.js : les deux mêmes fichiers, onglet.wasm.gz et demo.tar.gz).
// Elle ne décide rien : elle demande la route `droits-des-pieces` et montre ce qu'elle rend, ses cases (dessinées
// depuis la forme que la route leur donne), ses phrases et ses chiffres. Chaque changement (un bulletin déposé ou
// retiré, une case) redemande la route. Les bulletins déposés vont dans le dossier que la route nomme (`dossier`), sur
// le disque en mémoire du moteur. Elle ne garde rien : ni stockage, ni cache ; fermer l'onglet efface tout.
"use strict";

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
        calculer();
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
  }

  // La route, sa charge : la réponse du moteur, lue ({ ok, resultat, refus }). Une réponse qui ne se lit pas est une
  // faute technique : elle remonte telle quelle (son code, son texte), sans phrase de plus.
  function route(charge) {
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
      port.postMessage({ id: id, methode: "POST", adresse: "/api/droits-des-pieces", entetes: { "content-type": "application/json" }, corps: corps }, [corps]);
    });
  }

  // Les bulletins déposés, dans le dossier que la route nomme.
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
      montrerFichiers();
      aPoser = true;
      calculer();
    });
  }

  function montrerFichiers() {
    var ul = $("fichiers");
    ul.replaceChildren();
    fichiers.forEach(function (f, i) {
      var li = el("li");
      li.append(el("span", null, f.nom));
      var b = el("button", "lien", "Retirer");
      b.type = "button";
      b.setAttribute("aria-label", "Retirer " + f.nom);
      b.addEventListener("click", function () {
        fichiers.splice(i, 1);
        montrerFichiers();
        aPoser = true;
        calculer();
      });
      li.append(b);
      ul.append(li);
    });
  }

  var depot = $("depot");
  $("choisir").addEventListener("change", function (e) {
    ajouterFichiers(e.target.files);
    e.target.value = "";
  });
  ["dragenter", "dragover"].forEach(function (t) {
    depot.addEventListener(t, function (e) { e.preventDefault(); depot.classList.add("survol"); });
  });
  ["dragleave", "drop"].forEach(function (t) {
    depot.addEventListener(t, function (e) { e.preventDefault(); depot.classList.remove("survol"); });
  });
  depot.addEventListener("drop", function (e) { if (e.dataTransfer && e.dataTransfer.files.length) ajouterFichiers(e.dataTransfer.files); });
  // Un fichier lâché hors de la zone ne fait pas quitter la page (le navigateur l'ouvrirait à sa place).
  ["dragover", "drop"].forEach(function (t) {
    document.addEventListener(t, function (e) { e.preventDefault(); });
  });

  // La charge : ce qui est saisi dans les cases que la route a rendues, et rien d'autre ; une case vide n'y est pas.
  function charge() {
    var c = {};
    rendues.forEach(function (cle) {
      var v = saisie[cle];
      if (Array.isArray(v)) {
        var rangees = v.map(function (r) {
          var o = {};
          Object.keys(r).forEach(function (k) { if (r[k] !== "" && r[k] !== null && r[k] !== false) o[k] = r[k]; });
          return o;
        }).filter(function (o) { return Object.keys(o).length > 0; });
        if (rangees.length) c[cle] = rangees;
      } else if (v !== "" && v !== null && v !== undefined) {
        c[cle] = v;
      }
    });
    return c;
  }

  // ---------------------------------------------------------------- le calcul

  var enCours = false;
  var encore = false;

  function calculer() {
    if (!pret) return;
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
    suite.then(function () { return route(charge()); }).then(function (r) {
      if (r.ok && r.resultat) {
        if (dossier === null && r.resultat.dossier) {
          dossier = r.resultat.dossier;
          if (fichiers.length) { aPoser = true; encore = true; }
        }
        montrer(r.resultat);
      } else if (r.refus) {
        montrerRefus(r.refus.phrase);
      } else {
        throw new Error("réponse du moteur sans résultat ni refus : " + JSON.stringify(r));
      }
    }).catch(function (e) {
      avis("Défaut du logiciel : " + texteDErreur(e));
    }).then(function () {
      enCours = false;
      if (encore) calculer();
    });
  }

  // ---------------------------------------------------------------- les cases que la route rend

  var signature = null;
  // Les clés des cases que la route a rendues en dernier : les seules que la charge porte.
  var rendues = [];

  // Une valeur que la route rend, telle qu'elle l'écrit ; vide, rien.
  function texteDe(v) {
    return v === null || v === undefined ? "" : String(v);
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

  // UNE CASE SIMPLE, par sa forme : une date, un montant ou un nombre tapé, oui-non coché, un choix dans une liste.
  function champ(forme, choix, valeur, changer, id) {
    var i;
    if (forme === "choix_rendu") {
      i = el("select");
      i.append(el("option", null, ""));
      (choix || []).forEach(function (x) {
        var o = el("option", null, x.libelle);
        o.value = x.code;
        i.append(o);
      });
      i.value = valeur;
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

  function dessinerCase(c) {
    var bloc = el("div", "case");
    var id = "case-" + c.cle;
    if (c.forme !== "rangees") {
      var l = el("label", null, c.libelle);
      l.htmlFor = id;
      bloc.append(l);
      bloc.append(champ(c.forme, c.choix, saisie[c.cle], function (v) { saisie[c.cle] = v; calculer(); }, id));
      if (c.aide) bloc.append(el("p", "aide", c.aide));
      return bloc;
    }
    bloc.append(el("div", "libelle", c.libelle));
    (c.dit || []).forEach(function (d) { bloc.append(el("p", "aide", d)); });
    var rangees = el("div");
    bloc.append(rangees);
    function redessiner() {
      rangees.replaceChildren();
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
        if (!c.retirer) { rangees.append(ligne); return; }
        var retirer = el("button", "lien", c.retirer);
        retirer.type = "button";
        retirer.setAttribute("aria-label", c.retirer + " " + (n + 1));
        retirer.addEventListener("click", function () {
          saisie[c.cle].splice(n, 1);
          redessiner();
          calculer();
        });
        ligne.append(retirer);
        rangees.append(ligne);
      });
    }
    redessiner();
    if (c.ajouter) {
      var b = el("button", null, c.ajouter);
      b.type = "button";
      b.addEventListener("click", function () {
        var o = {};
        c.parties.forEach(function (p) { o[p.cle] = p.forme === "oui_non" ? false : ""; });
        saisie[c.cle].push(o);
        redessiner();
      });
      bloc.append(b);
    }
    return bloc;
  }

  // Les cases ne se redessinent que si la route en change la forme (une case, un choix) : ce qui est saisi reste.
  // Une case que la route ne rend plus perd ce qui y était saisi : rendue de nouveau, elle repart de ce que la route dit.
  function dessinerCases(cases) {
    rendues = cases.map(function (c) { return c.cle; });
    Object.keys(saisie).forEach(function (cle) { if (rendues.indexOf(cle) < 0) delete saisie[cle]; });
    var forme = JSON.stringify(cases.map(function (c) { return [c.cle, c.libelle, c.forme, c.choix, c.parties, c.aide, c.dit, c.ajouter, c.retirer]; }));
    if (forme === signature) return;
    signature = forme;
    cases.forEach(function (c) { if (!(c.cle in saisie)) saisie[c.cle] = depart(c); });
    var boite = $("cases");
    boite.replaceChildren();
    cases.forEach(function (c) { boite.append(dessinerCase(c)); });
  }

  // ---------------------------------------------------------------- ce que la route dit

  // UN TABLEAU QUE LA ROUTE REND (`socle::ecran::TableauRendu`) : ses colonnes (à droite, discrète), ses rangées de
  // cellules déjà dites (vide : rien), les totaux en gras. Son titre se pose à part (`titreDe`).
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

  // La phrase du résultat, que les lecteurs d'écran annoncent (seule à l'être) : celle que la route rend.
  function annoncer(phrase) {
    $("annonce").textContent = phrase;
  }

  function montrerRefus(phrase) {
    var r = $("resultat");
    r.replaceChildren(el("p", "avis", phrase));
    annoncer(phrase);
  }

  function montrer(d) {
    annoncer(d.phrase);
    var pieces = $("pieces");
    pieces.replaceChildren();
    (d.pieces || []).forEach(function (p) { pieces.append(el("li", null, p)); });
    dessinerCases(d.cases || []);

    var r = $("resultat");
    r.replaceChildren();
    var p = d.personne;
    if (!p) {
      r.append(el("p", "phrase", d.phrase));
      return;
    }
    r.append(el("h2", null, p.nom));
    r.append(el("p", "phrase", p.bilan), el("p", "aide", p.fenetre));
    if (p.butoir) {
      var b = el("div", "butoir");
      b.append(el("p", null, p.butoir.phrase), el("p", null, p.butoir.pourquoi), el("p", "reference", p.butoir.reference));
      r.append(b);
    } else {
      r.append(el("p", "phrase", p.phrase));
    }
    r.append(paires(p.lignes));

    if (p.paies.rangees.length) {
      if (p.paies.titre) r.append(el("h3", null, p.paies.titre));
      r.append(tableau(p.paies));
    }

    // Le détail d'une estimation ou d'une visée se replie, sous le titre de son tableau.
    var replie = function (t) {
      var det = el("details");
      det.append(el("summary", null, t.titre), tableau(t));
      return det;
    };
    p.estimations.forEach(function (e) {
      var bloc = el("div", "estimation");
      bloc.append(el("p", null, e.phrase));
      if (e.paies.rangees.length) bloc.append(replie(e.paies));
      r.append(bloc);
    });

    p.visee.forEach(function (v) {
      var bloc = el("div", "estimation");
      bloc.append(el("p", null, v.phrase), el("p", "aide", v.arret));
      if (v.paies && v.paies.rangees.length) bloc.append(replie(v.paies));
      r.append(bloc);
    });

    if (d.phrases && d.phrases.length) r.append(liste(d.phrases));
  }

  demarrer();
})();
