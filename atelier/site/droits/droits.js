// LA PAGE DES DROITS D'UN INTERMITTENT (puppetpaie.fr/droits/). Elle s'ouvre tout de suite ; le moteur se charge
// derrière, dans le worker de la démo (/demo/moteur.js : les deux mêmes fichiers, onglet.wasm.gz et demo.tar.gz).
// Elle ne décide rien : elle demande la route `droits-des-pieces` et pose son écran, bloc par bloc dans l'ordre où la
// route le rend (`ecran` : les cases, dessinées depuis la forme que la route leur donne, le résumé, le tableau des
// paies, le dépôt, les pièces). Chaque changement (un bulletin déposé ou retiré, une case) redemande la route. Les
// bulletins déposés vont dans le dossier que le dépôt nomme (`dossier`), sur le disque en mémoire du moteur. Elle ne
// garde rien : ni stockage, ni cache ; fermer l'onglet efface tout.
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

  // Les rangées d'une case, telles qu'elles partent : chacune ses parties remplies ; une rangée vide ne part pas.
  function rangeesEnvoyees(v) {
    return v.map(function (r) {
      var o = {};
      Object.keys(r).forEach(function (k) { if (r[k] !== "" && r[k] !== null && r[k] !== false) o[k] = r[k]; });
      return o;
    }).filter(function (o) { return Object.keys(o).length > 0; });
  }

  // La charge : ce qui est saisi dans les cases que la route a rendues, et rien d'autre ; une case vide n'y est pas.
  function charge() {
    var c = {};
    rendues.forEach(function (cle) {
      var v = saisie[cle];
      if (Array.isArray(v)) {
        var rangees = rangeesEnvoyees(v);
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

  // Le bouton qui retire la rangée `n` d'une case (son libellé, celui de la route ; son nom, avec le rang).
  function boutonRetirer(c, n) {
    var b = el("button", "lien", c.retirer);
    b.type = "button";
    b.setAttribute("aria-label", c.retirer + " " + (n + 1));
    b.addEventListener("click", function () {
      saisie[c.cle].splice(n, 1);
      redessiner();
      calculer();
    });
    return b;
  }

  // Le bouton qui ajoute une rangée vide à une case (son libellé, celui de la route).
  function boutonAjouter(c) {
    var b = el("button", null, c.ajouter);
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
      bloc.append(champ(c.forme, c.choix, saisie[c.cle], function (v) { saisie[c.cle] = v; calculer(); }, id));
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
      if (c.retirer) ligne.append(boutonRetirer(c, n));
      bloc.append(ligne);
    });
    if (c.ajouter) bloc.append(boutonAjouter(c));
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

  // LE RÉSUMÉ de la personne : son nom, son bilan, sa fenêtre, ce qui manque et le jour butoir, le compte des heures,
  // l'allocation estimée et visée (chacune son détail replié), les notes.
  function resume(p) {
    var r = el("section", "bloc");
    r.append(el("h2", null, p.nom), el("p", "phrase", p.bilan), el("p", "aide", p.fenetre));
    if (p.butoir) {
      var b = el("div", "butoir");
      b.append(el("p", null, p.butoir.phrase), el("p", null, p.butoir.pourquoi), el("p", "reference", p.butoir.reference));
      r.append(b);
    } else {
      r.append(el("p", "phrase", p.phrase));
    }
    r.append(paires(p.lignes));
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
    if (p.notes.length) r.append(liste(p.notes));
    return r;
  }

  // LE TABLEAU DES PAIES, et sous lui la case des lignes prévues.
  function paies(b) {
    var s = el("section", "bloc");
    if (b.tableau.titre) s.append(el("h3", null, b.tableau.titre));
    if (b.tableau.rangees.length) s.append(tableau(b.tableau));
    s.append(dessinerCase(b.lignes));
    return s;
  }

  // LE DÉPÔT : la zone où l'on glisse ou choisit ses bulletins (son libellé et son aide, ceux de la route), puis les
  // fichiers déposés, chacun avec « Retirer ».
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
    var ul = el("ul", "fichiers");
    fichiers.forEach(function (f) {
      var li = el("li");
      li.append(el("span", null, f.nom));
      var x = el("button", "lien", "Retirer");
      x.type = "button";
      x.setAttribute("aria-label", "Retirer " + f.nom);
      x.addEventListener("click", function () { retirerFichier(f.nom); });
      li.append(x);
      ul.append(li);
    });
    s.append(ul);
    return s;
  }

  // LES PIÈCES DÉPOSÉES : ce que la route en lit.
  function pieces(b) {
    var s = el("section", "bloc");
    s.append(liste(b.phrases));
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

  // L'ÉCRAN : les blocs que la route rend, dans son ordre. Une case que la route ne rend plus perd ce qui y était saisi
  // (rendue de nouveau, elle repart de ce que la route dit) ; ce qui est saisi dans les autres reste.
  function montrer(d) {
    dernier = d;
    $("refus").classList.add("cache");
    annoncer(d.phrase);
    var cases = [];
    d.ecran.forEach(function (b) {
      if (b.est === "case") cases.push(b);
      if (b.est === "paies") cases.push(b.lignes);
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
      else if (b.est === "resume") ecran.append(resume(b));
      else if (b.est === "paies") ecran.append(paies(b));
      else if (b.est === "depot") ecran.append(depot(b));
      else if (b.est === "pieces") ecran.append(pieces(b));
      else throw new Error("bloc inconnu de la route : " + b.est);
    });
    if (ici && $(ici)) {
      if (tape !== null) $(ici).value = tape;
      $(ici).focus();
    }
  }

  demarrer();
})();
