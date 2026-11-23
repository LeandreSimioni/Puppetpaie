// LE MOTEUR DE LA DÉMO : un worker que la page garde vivant (demo.js). Il télécharge le moteur (onglet.wasm.gz) et
// l'installation de démo (demo.tar.gz), déballe l'installation sur un disque en mémoire, démarre le moteur, puis sert
// chaque demande que le relais (relais.js) lui passe par un MessagePort : la demande entre telle quelle dans
// `onglet_traiter`, la réponse du moteur en sort telle quelle. Rien ne s'écrit ailleurs qu'en mémoire : fermer
// l'onglet efface tout.
//
// L'échange avec le moteur est celui de la crate `onglet` (atelier/sources/onglet/src/lib.rs) : `onglet_entree(n)`
// rend l'adresse où écrire la demande, `onglet_demarrer` et `onglet_traiter` rendent la longueur de leur réponse,
// `onglet_sortie` son adresse. La mémoire du module peut grandir à chaque appel : une vue sur elle se refait après
// chaque appel au module, jamais avant.

import WASI from "./wasi-shim/wasi.js";
import { File, Directory, OpenFile, PreopenDirectory, ConsoleStdout } from "./wasi-shim/fs_mem.js";
import { Filestat } from "./wasi-shim/wasi_defs.js";

const te = new TextEncoder();
const td = new TextDecoder();

// LES DATES DES FICHIERS. Le disque en mémoire du shim ne date aucun fichier (toute date vaut 0, soit 1970), et il
// range les dates aux octets 38, 46 et 52 de la structure `filestat` au lieu de 40, 48 et 56 (WASI preview1). Or le
// moteur reconnaît qu'un fichier a changé à sa date et à sa taille (l'empreinte des états, routes/src/etat_garde.rs ;
// le code figé, socle/src/fichiers.rs ; les journaux d'envoi, envois/src/journal.rs) : sans date, une réécriture de
// même taille passerait inaperçue et l'écran garderait l'ancien calcul. Chaque fichier prend donc la date de sa
// dernière écriture (celle de l'archive pour ce qui en vient), strictement croissante, et la structure s'écrit aux
// bons octets.
let derniereDate = 0n;
function maintenant() {
  let t = BigInt(Date.now()) * 1000000n;
  if (t <= derniereDate) t = derniereDate + 1n;
  derniereDate = t;
  return t;
}
const DEBUT = maintenant();
Object.defineProperty(File.prototype, "data", {
  configurable: true,
  get() { return this.octets; },
  set(v) { this.octets = v; this.date = maintenant(); },
});
for (const nom of ["fd_write", "fd_pwrite"]) {
  const ecrire = OpenFile.prototype[nom];
  OpenFile.prototype[nom] = function (...a) {
    const r = ecrire.apply(this, a);
    this.file.date = maintenant();
    return r;
  };
}
const statFichier = File.prototype.stat;
File.prototype.stat = function () {
  const s = statFichier.call(this);
  s.atim = s.mtim = s.ctim = this.date;
  return s;
};
const statDossier = Directory.prototype.stat;
Directory.prototype.stat = function () {
  const s = statDossier.call(this);
  s.atim = s.mtim = s.ctim = this.date ?? DEBUT;
  return s;
};
Filestat.prototype.write_bytes = function (v, p) {
  v.setBigUint64(p, this.dev, true);
  v.setBigUint64(p + 8, this.ino, true);
  v.setUint8(p + 16, this.filetype);
  v.setBigUint64(p + 24, this.nlink, true);
  v.setBigUint64(p + 32, this.size, true);
  v.setBigUint64(p + 40, this.atim, true);
  v.setBigUint64(p + 48, this.mtim, true);
  v.setBigUint64(p + 56, this.ctim, true);
};

// CE QUE LE WORKER DIT À LA PAGE : l'avancement, puis « prêt », un refus du moteur ou sa chute.
const dire = (m) => self.postMessage(m);

// LES MESURES (lues par les essais : `self.mesures`) : pour chaque demande servie, le temps passé dans
// `onglet_traiter` et le temps total du worker, puis le temps total vu par le relais ; les cent dernières.
self.mesures = [];
self.mesuresDuDemarrage = null;
// Et chaque envoi au filet (l'adresse et son issue, jamais le corps) : les cent derniers.
self.envois = [];

// LE TÉLÉCHARGEMENT, avec son avancement réel. Le total n'est connu que si le serveur envoie le fichier sans le
// compresser en route (la longueur annoncée est alors celle des octets lus) ; sinon, seuls les octets reçus se disent.
const recus = new Map();
let dernierAvis = 0;
function avancer(nom, n, total, fini) {
  recus.set(nom, { n, total });
  const t = performance.now();
  if (!fini && t - dernierAvis < 100) return;
  dernierAvis = t;
  let somme = 0, tout = 0, connu = true;
  for (const r of recus.values()) {
    somme += r.n;
    if (r.total === null) connu = false;
    else tout += r.total;
  }
  dire({ quoi: "avance", recus: somme, total: connu && recus.size === 2 ? tout : null });
}
async function telecharger(nom) {
  const r = await fetch(new URL(nom, import.meta.url));
  if (r.status === 404) throw new Error("la démo n'est pas en ligne pour le moment (il manque " + nom + " sur le serveur).");
  if (!r.ok) throw new Error(`le fichier ${nom} ne s'est pas téléchargé (le serveur a répondu ${r.status}).`);
  const longueur = r.headers.get("Content-Length");
  const total = r.headers.get("Content-Encoding") === null && longueur !== null ? Number(longueur) : null;
  let n = 0;
  avancer(nom, 0, total, false);
  return r.body.pipeThrough(new TransformStream({
    transform(morceau, suite) { n += morceau.byteLength; avancer(nom, n, total, false); suite.enqueue(morceau); },
    flush() { avancer(nom, n, total, true); },
  }));
}

// LE LECTEUR DE TAR (ustar et GNU, ce que `tar czf` écrit) : des dossiers et des fichiers ordinaires, rien d'autre ;
// une entrée d'une autre sorte se refuse en se nommant.
function deballer(o, racine) {
  const texte = (a, b) => { const s = td.decode(o.subarray(a, b)); const i = s.indexOf("\0"); return i < 0 ? s : s.slice(0, i); };
  const nombre = (a, n) => parseInt(texte(a, a + n).trim() || "0", 8);
  const enfant = (dossier, nom, inode) => { inode.parent = dossier; dossier.contents.set(nom, inode); return inode; };
  const dossierDe = (parties) => {
    let d = racine;
    for (const p of parties) d = d.contents.get(p) ?? enfant(d, p, new Directory(new Map()));
    return d;
  };
  let p = 0, nomLong = null, fichiers = 0;
  while (p + 512 <= o.length && o[p] !== 0) {
    const taille = nombre(p + 124, 12);
    const date = BigInt(nombre(p + 136, 12)) * 1000000000n;
    const type = o[p + 156] === 0 ? "0" : String.fromCharCode(o[p + 156]);
    const prefixe = texte(p + 257, p + 263) === "ustar" ? texte(p + 345, p + 500) : "";
    const nom = nomLong ?? (prefixe ? prefixe + "/" : "") + texte(p, p + 100);
    nomLong = null;
    const corps = o.subarray(p + 512, p + 512 + taille);
    p += 512 + Math.ceil(taille / 512) * 512;
    if (type === "L") { nomLong = td.decode(corps).replace(/\0+$/, ""); continue; }
    if (type === "x") {
      const chemin = td.decode(corps).split("\n").map((l) => l.replace(/^\d+ /, "")).find((l) => l.startsWith("path="));
      if (chemin !== undefined) nomLong = chemin.slice(5);
      continue;
    }
    if (type === "g") continue;
    const parties = nom.split("/").filter((x) => x !== "" && x !== ".");
    if (type === "5") { if (parties.length) dossierDe(parties).date = date; continue; }
    if (type !== "0") throw new Error(`L'installation de démo contient une entrée que la page ne sait pas déballer (type « ${type} ») : ${nom}`);
    const f = enfant(dossierDe(parties.slice(0, -1)), parties[parties.length - 1], new File(corps));
    f.date = date;
    fichiers++;
  }
  // Une archive entière finit par un bloc vide : sinon elle est coupée.
  if (p + 512 > o.length) throw new Error("l'installation de démo est incomplète (son archive est coupée).");
  return fichiers;
}

async function lireTout(flux) {
  const morceaux = [];
  let n = 0;
  for await (const m of flux) { morceaux.push(m); n += m.byteLength; }
  const o = new Uint8Array(n);
  let i = 0;
  for (const m of morceaux) { o.set(m, i); i += m.byteLength; }
  return o;
}

// LE CORPS DES DEMANDES ET DES RÉPONSES passe en base64 dans le JSON de l'échange (`DemandeDeLOnglet`).
function enB64(u8) {
  let s = "";
  for (let i = 0; i < u8.length; i += 0x8000) s += String.fromCharCode.apply(null, u8.subarray(i, i + 0x8000));
  return btoa(s);
}
function deB64(t) {
  const s = atob(t);
  const o = new Uint8Array(s.length);
  for (let i = 0; i < s.length; i++) o[i] = s.charCodeAt(i);
  return o;
}

let ex = null;
let jeton = null;
let tempsSysteme = 0;
let appelsSysteme = 0;
let tombe = null;
const memoire = () => new Uint8Array(ex.memory.buffer);
const sortie = (n) => { const p = ex.onglet_sortie() >>> 0; return memoire().slice(p, p + (n >>> 0)); };

// L'IMPORT DU MOTEUR : le seul envoi qui sort, celui des incidents vers le filet (le moteur ne laisse partir que les
// adresses du filet, socle::garde_reseau). Une requête synchrone, permise dans un worker ; elle n'appelle jamais le
// module (un `onglet_traiter` rappelé d'ici abandonnerait le moteur).
let gardee = new Uint8Array(0);
const puppetpaie = {
  poster(ptr, len) {
    const d = JSON.parse(td.decode(memoire().slice(ptr >>> 0, (ptr >>> 0) + (len >>> 0))));
    let r;
    try {
      const x = new XMLHttpRequest();
      x.open("POST", d.adresse, false);
      x.responseType = "arraybuffer";
      x.timeout = d.delai_ms;
      // Le navigateur ignore de lui-même les en-têtes qu'une page n'a pas le droit de poser (Accept-Encoding…).
      for (const [k, v] of d.entetes) x.setRequestHeader(k, v);
      x.send(deB64(d.corps));
      r = x.status === 0
        ? { panne: "le navigateur n'a pas joint le serveur (réseau ou CORS)" }
        : { reponse: { code: x.status, corps: enB64(new Uint8Array(x.response)) } };
    } catch (e) {
      r = { panne: String((e && e.message) || e) };
    }
    self.envois.push({ adresse: d.adresse, issue: r.panne !== undefined ? "panne : " + r.panne : "code " + r.reponse.code });
    if (self.envois.length > 100) self.envois.shift();
    gardee = te.encode(JSON.stringify(r));
    return gardee.length;
  },
  reponse(ptr) { memoire().set(gardee, ptr >>> 0); },
  // « Ouvrir » un fichier ou un dossier : le moteur n'a aucun programme à lancer (compagnie::poste, `AMontrer`) ; la
  // page de la démo le montre. Rend 1 s'il est sur le disque, 0 sinon.
  montrer(ptr, len) {
    const d = JSON.parse(td.decode(memoire().slice(ptr >>> 0, (ptr >>> 0) + (len >>> 0))));
    return montrer(d.chemin) ? 1 : 0;
  },
};

// LE DISQUE EN MÉMOIRE, par ses deux racines (`/demo`, `/tmp`), pour montrer à la page ce que le moteur y a écrit.
const disque = new Map();
function montrer(chemin) {
  const parties = chemin.split("/").filter((x) => x !== "" && x !== ".");
  let ici = disque.get(parties[0]);
  for (const p of parties.slice(1)) {
    ici = ici instanceof Directory ? ici.contents.get(p) : undefined;
  }
  if (ici instanceof Directory) {
    const noms = [...ici.contents].map(([nom, x]) => ({ nom, dossier: x instanceof Directory })).sort((a, b) => a.nom.localeCompare(b.nom, "fr"));
    dire({ quoi: "montrer", chemin: "/" + parties.join("/"), noms });
    return true;
  }
  if (ici instanceof File) {
    const octets = ici.data.slice().buffer;
    self.postMessage({ quoi: "montrer", chemin: "/" + parties.join("/"), octets }, [octets]);
    return true;
  }
  return false;
}

async function demarrer() {
  const t0 = performance.now();
  const [module, archive] = await Promise.all([
    telecharger("onglet.wasm.gz").then((flux) =>
      WebAssembly.compileStreaming(new Response(flux.pipeThrough(new DecompressionStream("gzip")), { headers: { "Content-Type": "application/wasm" } }))),
    telecharger("demo.tar.gz").then((flux) => lireTout(flux.pipeThrough(new DecompressionStream("gzip")))),
  ]);
  const t1 = performance.now();
  dire({ quoi: "etape", etape: "preparer" });
  const installation = new Directory(new Map());
  const fichiers = deballer(archive, installation);
  const demo = new PreopenDirectory("/demo", installation.contents);
  const tmp = new PreopenDirectory("/tmp", new Map());
  disque.set("demo", demo.dir);
  disque.set("tmp", tmp.dir);
  const wasi = new WASI([], ["PUPPETPAIE_CODE=/demo/logiciel/donnees"], [
    new OpenFile(new File([])),
    ConsoleStdout.lineBuffered((l) => console.log("[moteur]", l)),
    ConsoleStdout.lineBuffered((l) => console.warn("[moteur]", l)),
    demo,
    tmp,
  ], { debug: false }); // sans `debug: false`, le shim journalise chaque appel au système dans la console

  // Le temps passé dans les appels au système (le disque en mémoire, l'horloge…) se compte, pour les mesures.
  const systeme = {};
  for (const [nom, f] of Object.entries(wasi.wasiImport)) {
    systeme[nom] = (...a) => { const t = performance.now(); try { return f(...a); } finally { tempsSysteme += performance.now() - t; appelsSysteme++; } };
  }
  const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: systeme, puppetpaie });
  ex = instance.exports;
  // Le module n'exporte pas `_initialize` : `initialize` ne fait alors que lui donner l'instance (sa mémoire).
  wasi.initialize(instance);
  const t2 = performance.now();
  dire({ quoi: "etape", etape: "demarrer" });
  const d = JSON.parse(td.decode(sortie(ex.onglet_demarrer())));
  const t3 = performance.now();
  if (!d.page) {
    dire({ quoi: "refus", phrase: d.refus.phrase });
    return;
  }
  jeton = new URLSearchParams(d.page.split("?")[1]).get("jeton");
  if (!jeton) throw new Error("le logiciel n'a pas donné le jeton de sa fenêtre.");
  self.mesuresDuDemarrage = { telecharger: t1 - t0, preparer: t2 - t1, demarrer: t3 - t2, fichiers };
  dire({ quoi: "pret", mesures: self.mesuresDuDemarrage });
}

// UNE DEMANDE DE LA PAGE DU LOGICIEL, telle que le relais l'a reçue ; le worker y joint le jeton de la fenêtre (le
// moteur l'accepte en `X-Jeton`), que lui seul garde : un service worker ne peut pas poser de cookie.
function repondre(d) {
  const t0 = performance.now();
  if (tombe !== null) {
    return { id: d.id, code: 503, type: "text/plain; charset=utf-8", entetes: [], corps: te.encode("Le logiciel de la démo s'est arrêté : relancez la démo.").buffer };
  }
  const entetes = Object.assign({}, d.entetes, { "x-jeton": jeton });
  const o = te.encode(JSON.stringify({ methode: d.methode, adresse: d.adresse, entetes, corps: d.corps ? enB64(new Uint8Array(d.corps)) : "" }));
  let r, dedans, corps;
  try {
    const p = ex.onglet_entree(o.length) >>> 0;
    memoire().set(o, p);
    tempsSysteme = 0;
    appelsSysteme = 0;
    const a = performance.now();
    const n = ex.onglet_traiter();
    dedans = performance.now() - a;
    r = JSON.parse(td.decode(sortie(n)));
    corps = deB64(r.corps).buffer;
  } catch (e) {
    tombe = String((e && e.stack) || e);
    console.error("[moteur] tombé sur", d.methode, d.adresse, tombe);
    dire({ quoi: "tombe", demande: d.methode + " " + d.adresse, detail: tombe });
    return { id: d.id, code: 503, type: "text/plain; charset=utf-8", entetes: [], corps: te.encode("Le logiciel de la démo s'est arrêté : relancez la démo.").buffer };
  }
  self.mesures.push({ id: d.id, methode: d.methode, adresse: d.adresse.split("?")[0], code: r.code, octets: corps.byteLength, traiter: dedans, systeme: tempsSysteme, appels: appelsSysteme, worker: performance.now() - t0, relais: null });
  if (self.mesures.length > 100) self.mesures.shift();
  return { id: d.id, code: r.code, type: r.type, entetes: r.entetes, corps };
}

// UN PORT DU RELAIS : la page en confie un à chaque fois que le service worker en demande un (il le perd quand Chrome
// l'arrête) ; les anciens restent ouverts, une réponse en route y arrive encore.
function servir(port) {
  port.onmessage = (e) => {
    const d = e.data;
    if (d.mesure !== undefined) {
      const m = self.mesures.find((x) => x.id === d.mesure);
      if (m) m.relais = d.total;
      return;
    }
    const r = repondre(d);
    port.postMessage(r, [r.corps]);
  };
}

self.onmessage = (e) => {
  if (e.data.quoi === "demarrer") {
    demarrer().catch((err) => dire({ quoi: "echec", phrase: String((err && err.message) || err) }));
  } else if (e.data.quoi === "port") {
    servir(e.ports[0]);
  } else if (e.data.quoi === "montrer") {
    // Un fichier d'un dossier montré, que la page demande.
    montrer(e.data.chemin);
  }
};
