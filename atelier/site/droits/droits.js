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

  function avis(texte) {
    var a = $("avis");
    a.replaceChildren(el("p", null, texte));
    var b = el("button", null, "Recharger la page");
    b.type = "button";
    b.addEventListener("click", function () { location.reload(); });
    a.append(b);
    a.classList.remove("cache");
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

  // La route, sa charge : la réponse du moteur, lue ({ ok, resultat, refus }).
  function route(charge) {
    return new Promise(function (rendre) {
      var id = prochain++;
      attentes[id] = function (r) {
        var texte = new TextDecoder().decode(r.corps);
        try {
          rendre(JSON.parse(texte));
        } catch (e) {
          rendre({ ok: false, refus: { phrase: texte || "Le logiciel n'a pas répondu." } });
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
    var lus = Array.prototype.map.call(liste, function (f) {
      return f.arrayBuffer().then(function (o) { return { nom: f.name, octets: o }; });
    });
    Promise.all(lus).then(function (neufs) {
      neufs.forEach(function (n) {
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

  // La charge : ce qui est saisi, une case vide n'y étant pas.
  function charge() {
    var c = {};
    Object.keys(saisie).forEach(function (cle) {
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
        if (!ok) avis("Les bulletins ne se sont pas posés sur le disque du logiciel (son dossier " + dossier + " n'existe pas). C'est un défaut du logiciel.");
      });
    }
    suite.then(function () { return route(charge()); }).then(function (r) {
      enCours = false;
      if (r.ok && r.resultat) {
        if (dossier === null && r.resultat.dossier) {
          dossier = r.resultat.dossier;
          if (fichiers.length) { aPoser = true; encore = true; }
        }
        montrer(r.resultat);
      } else {
        montrerRefus(r.refus ? r.refus.phrase : "Le logiciel n'a pas répondu.");
      }
      if (encore) calculer();
    });
  }

  // ---------------------------------------------------------------- les cases que la route rend

  var signature = null;

  function texteDe(v, forme) {
    if (v === null || v === undefined) return "";
    if (forme === "montant" || forme === "nombre") return String(v).replace(".", ",");
    return String(v);
  }

  // La valeur de départ d'une case, quand rien n'est encore saisi : celle que la route y dit.
  function depart(c) {
    if (c.forme === "rangees") {
      return (c.valeur || []).map(function (r) {
        var o = {};
        c.parties.forEach(function (p) { o[p.cle] = p.forme === "oui_non" ? r[p.cle] === true : texteDe(r[p.cle], p.forme); });
        return o;
      });
    }
    return texteDe(c.valeur, c.forme);
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
        var retirer = el("button", "lien", "Retirer");
        retirer.type = "button";
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
  function dessinerCases(cases) {
    var forme = JSON.stringify(cases.map(function (c) { return [c.cle, c.libelle, c.forme, c.choix, c.parties, c.aide, c.dit]; }));
    if (forme === signature) return;
    signature = forme;
    cases.forEach(function (c) { if (!(c.cle in saisie)) saisie[c.cle] = depart(c); });
    var boite = $("cases");
    boite.replaceChildren();
    cases.forEach(function (c) { boite.append(dessinerCase(c)); });
  }

  // ---------------------------------------------------------------- ce que la route dit

  function tableau(entetes, lignes) {
    var t = el("table");
    var tr = el("tr");
    entetes.forEach(function (e) { tr.append(el("th", e[1] ? "n" : null, e[0])); });
    var tete = el("thead");
    tete.append(tr);
    t.append(tete);
    var corps = el("tbody");
    lignes.forEach(function (l) {
      var r = el("tr");
      l.forEach(function (v, i) { r.append(el("td", entetes[i][1] ? "n" : null, v === null || v === undefined ? "" : String(v))); });
      corps.append(r);
    });
    t.append(corps);
    var d = el("div", "tableau");
    d.append(t);
    return d;
  }

  function liste(phrases) {
    var ul = el("ul", "phrases");
    phrases.forEach(function (p) { ul.append(el("li", null, p)); });
    return ul;
  }

  function montrerRefus(phrase) {
    var r = $("resultat");
    r.replaceChildren(el("p", "avis", phrase));
  }

  function montrer(d) {
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
    r.append(el("p", "aide", p.fenetre));
    if (p.butoir) {
      var b = el("div", "butoir");
      b.append(el("p", null, p.butoir.phrase), el("p", null, p.butoir.pourquoi), el("p", "reference", p.butoir.reference));
      r.append(b);
    } else {
      r.append(el("p", "phrase", p.phrase));
    }
    r.append(tableau([["", false], ["", true]], p.lignes));

    if (p.paies.length) {
      r.append(el("h3", null, "Une ligne par paie"));
      r.append(tableau(
        [["Période", false], ["Employeur", false], ["Pièce", false], ["Cachets", true], ["Heures", true], ["Enseignement", true], ["Brut", true]],
        p.paies.map(function (x) { return [x.periode, x.employeur, x.piece, x.cachets, x.heures, x.enseignement, x.brut]; })
      ));
    }

    p.estimations.forEach(function (e) {
      var bloc = el("div", "estimation");
      bloc.append(el("p", null, e.phrase));
      var det = el("details");
      det.append(el("summary", null, "Le salaire de référence, paie par paie"));
      det.append(tableau(
        [["Période", false], ["Employeur", false], ["Brut", true], ["Plafond", true], ["Limite", true], ["Compte", true], ["", false]],
        e.paies.map(function (x) { return [x.periode, x.employeur, x.brut, x.plafond, x.limite, x.compte, x.note]; })
      ));
      bloc.append(det);
      r.append(bloc);
    });

    p.visee.forEach(function (v) {
      var bloc = el("div", "estimation");
      bloc.append(el("p", null, v.phrase), el("p", "aide", v.arret));
      if (v.paies.length) {
        bloc.append(tableau(
          [["Période", false], ["Employeur", false], ["Brut", true], ["Brut visé", true], ["Par cachet", true], ["Par heure", true]],
          v.paies.map(function (x) { return [x.periode, x.employeur, x.brut, x.brut_vise, x.par_cachet, x.par_heure]; })
        ));
      }
      r.append(bloc);
    });

    if (d.phrases && d.phrases.length) r.append(liste(d.phrases));
  }

  demarrer();
})();
