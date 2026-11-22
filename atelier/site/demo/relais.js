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
// navigation de cadre, ou une demande d'une page tenue dans un cadre. Les pages du site (toutes au premier niveau :
// sa politique interdit les cadres) et les fichiers de la démo elle-même passent au réseau sans qu'il y touche.

// LA POLITIQUE DES PAGES QUE LE RELAIS FABRIQUE : une réponse fabriquée ici ne porte pas les en-têtes du serveur ; elle
// porte celle-ci, la même que celle du chemin `/demo/` (atelier/site/demo/LISEZ-MOI.md), sauf `frame-ancestors`, qui
// laisse la démo tenir la page du logiciel dans son cadre.
const POLITIQUE = "default-src 'none'; script-src 'self' 'unsafe-inline'; style-src 'self' 'unsafe-inline'; font-src 'self'; " +
  "img-src 'self' data: blob:; connect-src 'self'; frame-src 'self'; base-uri 'none'; form-action 'none'; " +
  "frame-ancestors 'self'";

const DEMO = new URL("./", self.location).pathname;

let port = null;
let proprietaire = null;
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
  const avant = proprietaire;
  proprietaire = e.source.id;
  if (avant !== null && avant !== proprietaire) {
    self.clients.get(avant).then((c) => { if (c) c.postMessage({ quoi: "remplacee" }); });
  }
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

// LE PORT, ou sa demande à la page : Chrome arrête un service worker inactif après une trentaine de secondes, et le
// port meurt avec lui ; quand il revient, il redemande un port à la page de la démo. Les demandes attendent la réponse
// dans `attentes` : aucune ne se perd. Si aucune page de la démo n'est ouverte, il n'y a plus de moteur : `null`.
function lePort() {
  if (port) return Promise.resolve(port);
  return new Promise((resoudre) => {
    attentes.push(resoudre);
    if (demandePosee) return;
    demandePosee = true;
    self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((tous) => {
      const demos = tous.filter((c) => c.frameType === "top-level" && new URL(c.url).pathname.startsWith(DEMO));
      if (!demos.length) {
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
  h.set("Content-Security-Policy", POLITIQUE);
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
    } else if (e.resultingClientId) {
      cadres.set(e.resultingClientId, false);
    }
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
