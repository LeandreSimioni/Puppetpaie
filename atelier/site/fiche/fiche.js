// La page « Remplir ma fiche » (puppetpaie.fr/fiche/). Elle ne connaît aucune question à l'avance : le lien du
// courriel dit, dans son fragment (que le navigateur n'envoie jamais au serveur), l'empreinte du formulaire (f), les
// questions à poser (q), le code de la demande (code), l'adresse de la compagnie (a) et l'objet de la réponse (s).
// Elle charge le formulaire que le moteur publie (formulaires/<f>.js, qui appelle ficheFormulaire), dessine les
// questions que le lien nomme, vérifie ce que leur forme dit (une politesse : le logiciel revérifie tout à la relève),
// puis compose la réponse selon la grammaire du formulaire et ouvre la messagerie du salarié. Elle ne garde rien :
// ni stockage, ni requête.
"use strict";

(function () {
  var $ = function (id) { return document.getElementById(id); };

  function avis(texte) {
    var a = $("avis");
    a.textContent = texte;
    a.classList.remove("cache");
  }

  var REPONDRE_AU_COURRIEL = "Réponds directement au courriel que tu as reçu : recopie les lignes qu'il donne, chacune suivie de ta réponse.";

  // Le fragment du lien : f, q, code, a, s.
  function lireLeLien() {
    var parties = {};
    var fragment = location.hash.replace(/^#/, "");
    if (!fragment) return null;
    var morceaux = fragment.split("&");
    for (var i = 0; i < morceaux.length; i++) {
      var k = morceaux[i].indexOf("=");
      if (k < 0) return null;
      try {
        parties[morceaux[i].slice(0, k)] = decodeURIComponent(morceaux[i].slice(k + 1));
      } catch (e) {
        return null;
      }
    }
    if (!/^[0-9a-f]{16}$/.test(parties.f || "")) return null;
    if (!/^FIC-[A-Z0-9]{6}$/.test(parties.code || "")) return null;
    if (!/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(parties.a || "")) return null;
    if (typeof parties.q !== "string" || typeof parties.s !== "string") return null;
    return { formulaire: parties.f, questions: parties.q.split(",").filter(Boolean), code: parties.code, adresse: parties.a, objet: parties.s };
  }

  // ---------------------------------------------------------------- ce que la page vérifie

  function chiffres(t) { return t.replace(/[\s.\-]/g, ""); }

  // Le reste par 97 d'un nombre écrit en chiffres, de toute longueur.
  function reste97(t) {
    var r = 0;
    for (var i = 0; i < t.length; i++) r = (r * 10 + (t.charCodeAt(i) - 48)) % 97;
    return r;
  }

  // JJ/MM/AAAA, ou AAAA-MM-JJ, que le remplissage automatique du navigateur peut écrire : les deux que la relève lit.
  function dateJuste(t) {
    var m = /^\s*(\d{1,2})[\/.\- ](\d{1,2})[\/.\- ](\d{4})\s*$/.exec(t);
    var iso = /^\s*(\d{4})-(\d{1,2})-(\d{1,2})\s*$/.exec(t);
    if (!m && !iso) return false;
    var j = m ? +m[1] : +iso[3], mo = m ? +m[2] : +iso[2], a = m ? +m[3] : +iso[1];
    var d = new Date(a, mo - 1, j);
    return d.getFullYear() === a && d.getMonth() === mo - 1 && d.getDate() === j;
  }

  // La clé du NIR : 97 moins le reste du NIR par 97 (la Corse, 2A et 2B, se compte 19 et 18). Même règle que la relève.
  function nirJuste(t) {
    var n = chiffres(t).toUpperCase();
    if (!/^\d{5}(\d{2}|2A|2B)\d{8}$/.test(n)) return false;
    var treize = n.slice(0, 13).replace("2A", "19").replace("2B", "18");
    return 97 - reste97(treize) === +n.slice(13);
  }

  // La clé d'un IBAN : les quatre premiers signes à la fin, chaque lettre en nombre (A = 10), le reste par 97 vaut 1.
  function ibanJuste(t) {
    var i = t.replace(/\s/g, "").toUpperCase();
    if (!/^[A-Z]{2}\d{2}[A-Z0-9]{11,30}$/.test(i)) return false;
    var tourne = (i.slice(4) + i.slice(0, 4)).replace(/[A-Z]/g, function (c) { return String(c.charCodeAt(0) - 55); });
    return reste97(tourne) === 1;
  }

  var VERIFIER = {
    date: function (t) { return dateJuste(t) ? null : "Écris-la JJ/MM/AAAA, par exemple 14/07/1990."; },
    nir: function (t) { return nirJuste(t) ? null : "Ce numéro ne semble pas juste : vérifie ses 15 chiffres."; },
    code_postal: function (t) { return /^\d{5}$/.test(t.replace(/\s/g, "")) ? null : "Cinq chiffres."; },
    departement: function (t) { return /^(0[1-9]|[1-9]\d|2A|2B)$/i.test(t.trim()) ? null : "Deux caractères, comme 75 ou 2A."; },
    iban: function (t) { return ibanJuste(t) ? null : "Cet IBAN ne semble pas juste : vérifie-le sur ton RIB."; },
    bic: function (t) { return /^[A-Z]{6}[A-Z0-9]{2}([A-Z0-9]{3})?$/i.test(t.replace(/\s/g, "")) ? null : "8 ou 11 caractères, sur ton RIB."; }
  };

  // ---------------------------------------------------------------- le dessin

  var lien = null;
  var formulaire = null;
  // Par clé : { question, bloc, champ, aucun, choix, faute }. Sans prototype : une clé du lien comme « constructor »
  // n'y trouve rien.
  var cases = Object.create(null);

  function element(nom, classe, texte) {
    var e = document.createElement(nom);
    if (classe) e.className = classe;
    if (texte) e.textContent = texte;
    return e;
  }

  // Le lien qui ouvre la messagerie vers l'adresse de la réponse.
  function versLAdresse() {
    var a = element("a", null, lien.adresse);
    a.href = "mailto:" + encodeURIComponent(lien.adresse).replace(/%40/g, "@");
    return a;
  }

  // Une phrase où l'adresse de la réponse est un lien : `avant`, l'adresse, `apres`.
  function phraseAvecLAdresse(nom, avant, apres) {
    var e = element(nom, null, avant);
    e.appendChild(versLAdresse());
    e.appendChild(document.createTextNode(apres));
    return e;
  }

  function dessinerQuestion(q) {
    var id = "q-" + q.cle;
    var bloc = element(q.forme === "choix" ? "fieldset" : "div", "question");
    var titre = element(q.forme === "choix" ? "legend" : "label", null, q.libelle);
    if (q.forme !== "choix") titre.htmlFor = id;
    if (q.facultative) titre.appendChild(element("span", "facultatif", " (facultatif)"));
    bloc.appendChild(titre);
    var c = { question: q, bloc: bloc, champ: null, aucun: null, choix: [], faute: element("p", "faute cache") };
    if (q.forme === "choix") {
      var liste = element("div", "choix");
      (q.choix || []).forEach(function (ch) {
        var l = element("label");
        var r = document.createElement("input");
        r.type = "radio";
        r.name = id;
        r.value = ch.valeur;
        l.appendChild(r);
        l.appendChild(element("span", null, ch.libelle));
        liste.appendChild(l);
        c.choix.push(r);
      });
      bloc.appendChild(liste);
    } else if (q.forme === "liste") {
      // Une longue liste (les pays) : une liste déroulante, ses choix tels que le formulaire les déclare.
      var menu = document.createElement("select");
      menu.id = id;
      menu.autocomplete = q.autocomplete || "off";
      (q.choix || []).forEach(function (ch) {
        var o = document.createElement("option");
        o.value = ch.valeur;
        o.textContent = ch.libelle;
        menu.appendChild(o);
      });
      menu.addEventListener("change", function () { c.faute.classList.add("cache"); });
      bloc.appendChild(menu);
      c.champ = menu;
    } else {
      var champ = document.createElement("input");
      champ.type = "text";
      champ.id = id;
      // Le jeton que la question déclare : le navigateur propose ce qu'il sait ; sans jeton, rien.
      champ.autocomplete = q.autocomplete || "off";
      champ.spellcheck = false;
      if (q.forme === "date") { champ.inputMode = "numeric"; champ.placeholder = "JJ/MM/AAAA"; }
      if (q.forme === "nir" || q.forme === "code_postal") champ.inputMode = "numeric";
      champ.addEventListener("blur", function () { verifier(c); });
      champ.addEventListener("input", function () { c.faute.classList.add("cache"); });
      bloc.appendChild(champ);
      c.champ = champ;
    }
    if (q.aide) bloc.appendChild(element("p", "aide", q.aide));
    // L'explication longue que le formulaire déclare : derrière « En savoir plus », sans script.
    if (q.explication) {
      var plus = element("details", "plus");
      plus.appendChild(element("summary", null, "En savoir plus"));
      plus.appendChild(element("p", null, q.explication));
      // L'exemple chiffré que le moteur a calculé : une ligne par chiffre, telles qu'il les compose.
      if (q.exemple && q.exemple.length) {
        var exemple = element("div", "exemple");
        q.exemple.forEach(function (ligne) { exemple.appendChild(element("p", null, ligne)); });
        plus.appendChild(exemple);
      }
      bloc.appendChild(plus);
    }
    if (q.aucun) {
      var l = element("label", "aucun");
      var coche = document.createElement("input");
      coche.type = "checkbox";
      l.appendChild(coche);
      l.appendChild(element("span", null, q.aucun));
      coche.addEventListener("change", function () { basculer(c); montrerLesConditions(); });
      bloc.appendChild(l);
      c.aucun = coche;
      // Cochée d'avance quand le formulaire le déclare (c'est l'usage) : comme si la personne l'avait cochée.
      if (q.aucun_coche) { coche.checked = true; basculer(c); }
    }
    bloc.appendChild(c.faute);
    return c;
  }

  // « Je n'en ai pas » coché : la case se vide et se ferme.
  function basculer(c) {
    var ferme = c.aucun && c.aucun.checked;
    if (c.champ) { c.champ.disabled = ferme; if (ferme) c.champ.value = ""; }
    c.choix.forEach(function (r) { r.disabled = ferme; if (ferme) r.checked = false; });
    if (ferme) c.faute.classList.add("cache");
  }

  // Une question qui dépend d'une autre ne se montre que si cette autre a pour réponse « aucun ».
  function montrerLesConditions() {
    Object.keys(cases).forEach(function (cle) {
      var c = cases[cle];
      var p = c.question.si_aucun;
      if (!p) return;
      var parent = cases[p];
      var ouverte = !!(parent && parent.aucun && parent.aucun.checked && montree(parent));
      c.bloc.classList.toggle("cache", !ouverte);
    });
  }

  function montree(c) { return !c.bloc.classList.contains("cache"); }

  function valeur(c) {
    if (c.aucun && c.aucun.checked) return formulaire.reponse.aucun;
    if (c.champ) return c.champ.value.trim();
    for (var i = 0; i < c.choix.length; i++) if (c.choix[i].checked) return c.choix[i].value;
    return "";
  }

  function verifier(c) {
    var v = valeur(c);
    var f = VERIFIER[c.question.forme];
    var faute = (!v || v === formulaire.reponse.aucun || !f) ? null : f(v);
    c.faute.textContent = faute || "";
    c.faute.classList.toggle("cache", !faute);
    return !faute;
  }

  // ---------------------------------------------------------------- la réponse

  function composer() {
    var g = formulaire.reponse;
    var lignes = [];
    lien.questions.forEach(function (cle) {
      var c = cases[cle];
      if (!c || !montree(c)) return;
      var v = valeur(c);
      if (!v) return;
      if (c.question.forme === "nir" || c.question.forme === "iban" || c.question.forme === "bic" || c.question.forme === "code_postal") {
        if (v !== g.aucun) v = v.replace(/\s/g, "").toUpperCase();
      }
      lignes.push(cle + g.separateur + v);
    });
    lignes.push("");
    lignes.push(g.code + g.separateur + lien.code);
    return lignes.join("\n");
  }

  // Ce qui reste sans réponse, et ce qui ne semble pas juste : la page le dit, et n'empêche rien.
  function bilan() {
    var vides = [], fausses = [];
    lien.questions.forEach(function (cle) {
      var c = cases[cle];
      if (!c || !montree(c)) return;
      if (!valeur(c)) { if (!c.question.facultative) vides.push(c.question.libelle); }
      else if (!verifier(c)) fausses.push(c.question.libelle);
    });
    var m = $("manque");
    var phrases = [];
    if (vides.length) phrases.push("Sans réponse : " + vides.join(", ") + ".");
    if (fausses.length) phrases.push("À vérifier : " + fausses.join(", ") + ".");
    m.textContent = phrases.length ? phrases.join(" ") + " Tu peux envoyer quand même : ton employeur te redemandera ce qui manque." : "";
    m.classList.toggle("cache", !phrases.length);
  }

  function envoyer() {
    bilan();
    var corps = composer();
    location.href = "mailto:" + encodeURIComponent(lien.adresse).replace(/%40/g, "@") +
      "?subject=" + encodeURIComponent(lien.objet) + "&body=" + encodeURIComponent(corps);
  }

  function copier() {
    bilan();
    var corps = composer();
    var boite = $("copie");
    boite.textContent = "";
    var fait = function (ok) {
      boite.appendChild(phraseAvecLAdresse("p", (ok ? "Ta réponse est copiée. " : "Copie ta réponse ci-dessous. ") +
        "Envoie-la par courriel à ", ", avec pour objet « " + lien.objet + " »."));
      boite.appendChild(element("pre", null, corps));
      boite.classList.remove("cache");
    };
    if (navigator.clipboard && navigator.clipboard.writeText) {
      navigator.clipboard.writeText(corps).then(function () { fait(true); }, function () { fait(false); });
    } else {
      fait(false);
    }
  }

  // ---------------------------------------------------------------- le formulaire publié

  // Appelée par formulaires/<empreinte>.js quand il se charge.
  window.ficheFormulaire = function (f) {
    formulaire = f;
    var parCle = Object.create(null);
    f.questions.forEach(function (q) { parCle[q.cle] = q; });
    var inconnues = lien.questions.filter(function (cle) { return !parCle[cle]; });
    if (inconnues.length || !lien.questions.length) {
      avis("Ce lien ne correspond pas au formulaire. " + REPONDRE_AU_COURRIEL);
      return;
    }
    var zone = $("questions");
    // Dans l'ordre du formulaire.
    f.questions.forEach(function (q) {
      if (lien.questions.indexOf(q.cle) < 0) return;
      var c = dessinerQuestion(q);
      cases[q.cle] = c;
      zone.appendChild(c.bloc);
    });
    montrerLesConditions();
    var chapeau = $("chapeau");
    chapeau.textContent = "";
    chapeau.appendChild(document.createTextNode("Ta réponse ira à "));
    chapeau.appendChild(versLAdresse());
    chapeau.appendChild(document.createTextNode(". Cette page ne garde rien : elle prépare le courriel, que tu envoies toi-même, de ta messagerie."));
    $("formulaire").classList.remove("cache");
    // L'adresse où part la réponse se dit sur les deux boutons : un lien forgé vers une autre adresse se voit.
    $("envoyer").textContent = "Envoyer à " + lien.adresse;
    $("copier").textContent = "Copier ma réponse, à envoyer à " + lien.adresse;
    // La touche Entrée dans une case ne soumet rien : seuls les boutons agissent.
    $("formulaire").addEventListener("submit", function (e) { e.preventDefault(); });
    $("envoyer").addEventListener("click", envoyer);
    $("copier").addEventListener("click", copier);
  };

  lien = lireLeLien();
  if (!lien) {
    avis("Ce lien est incomplet. " + REPONDRE_AU_COURRIEL);
    return;
  }
  var s = document.createElement("script");
  s.src = "formulaires/" + lien.formulaire + ".js";
  s.onerror = function () { avis("Le formulaire de ce lien n'est pas encore en ligne. " + REPONDRE_AU_COURRIEL); };
  document.body.appendChild(s);
})();
