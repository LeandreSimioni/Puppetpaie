// LE RELAIS DE LA DÉMO : un service worker qui ne fait que passer les demandes de la page du logiciel au moteur (le
// worker de moteur.js, par un MessagePort que la page de la démo lui confie) et rendre ses réponses. Il ne met rien en
// cache et ne garde rien.
//
// POURQUOI SA PORTÉE EST LA RACINE DU SITE : la page du logiciel demande des chemins qui partent de la racine (`/js/…`,
// `/api/…`) et revient à l'accueil par `location.replace("/")` (moteur/src/interface/pages/accueil.html, garder.html ;
// logiciel/interface/js/configuration.js). Un service worker voit toutes les demandes d'une page qu'il tient, mais une
// navigation ne passe que par celui dont la portée couvre l'adresse visée : avec une portée `/demo/…`, ouvrir une
// compagnie ferait sortir le cadre vers l'accueil de puppetpaie.fr. Le relais a donc la portée `/` (le serveur l'admet
// par `Service-Worker-Allowed: /` sur ce fichier), et il ne relaie que ce qui vient du cadre du logiciel : une
// navigation de cadre, une demande d'une page tenue dans un cadre, ou un lien du logiciel ouvert dans un autre onglet
// (il porte le jeton de la fenêtre). Les pages du site (toutes au premier niveau : sa politique interdit les cadres)
// et les fichiers de la démo elle-même passent au réseau. Et il ne reste pas : sans page de démo ouverte, il se
// désinscrit (`partirSansDemo`).

// LA POLITIQUE DES PAGES QUE LE RELAIS FABRIQUE : une réponse fabriquée ici ne porte pas les en-têtes du serveur. Comme
// le serveur du logiciel sur le poste (moteur/src/interface/http.rs), le relais pose une politique quand le moteur n'en
// donne pas (il en donne une à ses PDF) ; celle-ci part de celle du site et n'ajoute que ce que la page du logiciel
// exige : ses scripts et ses attributs `on…` en ligne (`'unsafe-inline'`), ses feuilles de style et ses styles en
// ligne, ses images (le cachet), ses appels à `/api/…`, ses cadres (les PDF des courriels joués et de l'aperçu du
// bulletin), et `frame-ancestors 'self'` pour que la page de la démo la tienne dans son cadre
// (atelier/site/demo/LISEZ-MOI.md dit ce que chacun casse sans lui).
const POLITIQUE = "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; img-src 'self'; connect-src 'self'; frame-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'self'";

const DEMO = new URL("./", self.location).pathname;

let port = null;
let attentes = [];
let demandePosee = false;
let prochain = 1;
const enCours = new Map();
const cadres = new Map();

self.addEventListener("install", () => self.skipWaiting());

// UN PORT DE LA PAGE DE LA DÉMO. Une page neuve (un autre onglet, ou la démo relancée) remplace la précédente, qui en
// est avertie ; une demande partie vers l'ancien moteur reçoit une réponse qui le dit.
self.addEventListener("message", (e) => {
  if (!e.data || e.data.quoi !== "port") return;
  const neuf = e.ports[0];
  // Toute autre page de démo s'arrête (le relais a pu redémarrer entre-temps et ne plus savoir laquelle tournait) :
  // sinon son cadre parlerait au moteur de celle-ci.
  const source = e.source.id;
  lesDemos().then((demos) => {
    for (const c of demos) if (c.id !== source) c.postMessage({ quoi: "remplacee" });
  });
  for (const [id, x] of enCours) {
    if (x.port !== neuf) {
      enCours.delete(id);
      x.rendre({ code: 503, type: "text/plain; charset=utf-8", entetes: [], corps: new TextEncoder().encode("La démo a redémarré : cette demande est restée sans réponse du moteur.").buffer });
    }
  }
  port = neuf;
  port.onmessage = (m) => {
    const x = enCours.get(m.data.id);
    if (!x) return;
    enCours.delete(m.data.id);
    x.rendre(m.data);
  };
  demandePosee = false;
  const a = attentes;
  attentes = [];
  for (const r of a) r(port);
  e.source.postMessage({ quoi: "port pose" });
});

// LES PAGES DE LA DÉMO ouvertes dans ce navigateur.
function lesDemos() {
  return self.clients.matchAll({ type: "window", includeUncontrolled: true })
    .then((tous) => tous.filter((c) => c.frameType === "top-level" && new URL(c.url).pathname.startsWith(DEMO)));
}

// SANS PAGE DE DÉMO, LE RELAIS S'EN VA : il se désinscrit, pour ne pas rester posé sur tout le site après une visite
// (la démo ne garde rien). La prochaine visite de la démo l'inscrit de nouveau.
function partirSansDemo() {
  lesDemos().then((demos) => { if (!demos.length) self.registration.unregister(); });
}

// LE PORT, ou sa demande à la page : Chrome arrête un service worker inactif après une trentaine de secondes, et le
// port meurt avec lui ; quand il revient, il redemande un port à la page de la démo. Les demandes attendent la réponse
// dans `attentes` : aucune ne se perd. Si aucune page de la démo n'est ouverte, il n'y a plus de moteur : `null`.
function lePort() {
  if (port) return Promise.resolve(port);
  return new Promise((resoudre) => {
    attentes.push(resoudre);
    if (demandePosee) return;
    demandePosee = true;
    lesDemos().then((demos) => {
      if (!demos.length) {
        self.registration.unregister();
        demandePosee = false;
        const a = attentes;
        attentes = [];
        for (const r of a) r(null);
        return;
      }
      for (const c of demos) c.postMessage({ quoi: "port?" });
    });
  });
}

async function relayer(q, t0) {
  const u = new URL(q.url);
  const corps = q.method === "GET" || q.method === "HEAD" ? null : await q.arrayBuffer();
  const entetes = {};
  for (const [k, v] of q.headers) entetes[k] = v;
  const p = await lePort();
  if (p === null) {
    return new Response("La démo n'est plus ouverte : rouvrez-la depuis puppetpaie.fr.", { status: 503, headers: { "Content-Type": "text/plain; charset=utf-8", "Content-Security-Policy": POLITIQUE } });
  }
  const id = prochain++;
  const r = await new Promise((rendre) => {
    enCours.set(id, { port: p, rendre });
    p.postMessage({ id, methode: q.method, adresse: u.pathname + u.search, entetes, corps }, corps ? [corps] : []);
  });
  const reponse = enReponse(r);
  p.postMessage({ mesure: id, total: performance.now() - t0 });
  return reponse;
}

// LA RÉPONSE DU MOTEUR, telle quelle : son code, son type, ses en-têtes, son corps ; s'y ajoutent la politique des pages
// fabriquées ici et l'interdiction de garder. `Set-Cookie` ne se pose pas depuis un service worker : le jeton de la
// fenêtre, c'est le worker du moteur qui le garde et le joint à chaque demande.
function enReponse(r) {
  if (r.code >= 300 && r.code < 400) {
    const ou = r.entetes.find(([k]) => k.toLowerCase() === "location");
    if (ou) return Response.redirect(new URL(ou[1], self.location.origin).href, r.code);
  }
  const h = new Headers();
  for (const [k, v] of r.entetes) {
    if (k.toLowerCase() !== "set-cookie") h.append(k, v);
  }
  h.set("Content-Type", r.type);
  if (!h.has("Content-Security-Policy")) h.set("Content-Security-Policy", POLITIQUE);
  h.set("Cache-Control", "no-store");
  const sansCorps = [101, 103, 204, 205, 304].includes(r.code);
  return new Response(sansCorps ? null : r.corps, { status: r.code, headers: h });
}

// QUI EST LE CADRE DU LOGICIEL : une page tenue dans un cadre (`frameType` « nested ») ; le site n'en a pas d'autre.
// Ce qu'on sait d'un client se garde le temps de la vie du relais ; un client inconnu (le relais vient de revenir) se
// demande au navigateur, et ce qui n'est pas un cadre passe au réseau tel quel.
self.addEventListener("fetch", (e) => {
  const t0 = performance.now();
  const q = e.request;
  if (new URL(q.url).origin !== self.location.origin) return;
  if (q.mode === "navigate") {
    if (q.destination === "iframe") {
      if (e.resultingClientId) cadres.set(e.resultingClientId, true);
      e.respondWith(relayer(q, t0));
      return;
    }
    if (e.resultingClientId) cadres.set(e.resultingClientId, false);
    // Un lien du logiciel ouvert dans un autre onglet (« ouvrir le PDF ») : il porte le jeton de la fenêtre, qu'aucune
    // page du site ne porte. Il va au moteur, pas au serveur du site.
    if (new URL(q.url).searchParams.has("jeton")) {
      e.respondWith(relayer(q, t0));
      return;
    }
    // Une page du site : elle passe au réseau ; et sans démo ouverte, le relais n'a plus rien à faire ici.
    if (!new URL(q.url).pathname.startsWith(DEMO)) partirSansDemo();
    return;
  }
  if (!e.clientId) return;
  const connu = cadres.get(e.clientId);
  if (connu === false) return;
  if (connu === true) {
    e.respondWith(relayer(q, t0));
    return;
  }
  e.respondWith(self.clients.get(e.clientId).then((c) => {
    const cadre = !!c && c.frameType === "nested";
    cadres.set(e.clientId, cadre);
    return cadre ? relayer(q, t0) : fetch(q);
  }));
});
