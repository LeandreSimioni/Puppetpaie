# La démo du logiciel dans le navigateur

Le site l'héberge sous `https://puppetpaie.fr/demo/` (son déploiement prend ce dossier sur `main`, sans ce fichier).
N'importe qui y essaie le logiciel sans rien installer : le même moteur, bridé (`logiciel/EDITION.json` : « démo »),
compilé en WebAssembly, tourne dans l'onglet avec une compagnie fictive ; rien ne tourne sur le serveur. **La démo ne
garde rien** : l'installation est déballée sur un disque en mémoire à chaque ouverture, et fermer l'onglet efface tout
(ni IndexedDB, ni `localStorage`, ni cache écrit par la page).

## Comment elle tient

```
index.html + demo.js   la page : l'écran de chargement, puis la page du logiciel dans un cadre plein écran
   │  garde vivant
   ▼
moteur.js              le worker du moteur : télécharge onglet.wasm et demo.tar.gz, déballe l'installation sur le
   ▲                   disque en mémoire (wasi-shim/), démarre le moteur (`onglet_demarrer`), puis sert chaque
   │ MessagePort       demande par `onglet_traiter` ; il garde le jeton de la fenêtre et le joint à chaque demande
   ▼
relais.js              le service worker : passe chaque demande du cadre au worker, telle quelle, et rend sa réponse
   ▲
   │ fetch
cadre « / »            la page du logiciel, inchangée : `/`, `/js/…`, `/api/<route>`, `/lanceur/…`
```

- **La portée du relais est la racine du site** (`/`), et non `/demo/` : la page du logiciel revient à l'accueil des
  compagnies par `location.replace("/")` (ouvrir une compagnie, la quitter), et une navigation ne passe que par le
  service worker dont la portée couvre l'adresse visée. Le relais ne prend que ce qui vient d'un cadre (une navigation
  de cadre, ou une demande d'une page tenue dans un cadre) : les pages du site sont toutes au premier niveau (sa
  politique interdit les cadres) et passent au réseau sans qu'il y touche, comme les fichiers de la démo elle-même.
- **Le relais perd son port quand Chrome l'arrête** (une trentaine de secondes d'inactivité) : revenu, il le redemande
  à la page de la démo, et les demandes attendent la réponse. Une seule démo vit à la fois : un second onglet la prend,
  le premier le dit et propose de la relancer.
- **Si le moteur tombe** (l'appel au module lève une exception), le worker le dit, la page l'affiche et propose de
  relancer la démo, qui repart de zéro.
- **Le seul envoi qui sort** est celui des incidents vers le filet (l'import `poster`, une requête synchrone du
  worker) ; le moteur ne laisse partir que les adresses du filet (`socle::garde_reseau`).

## Les fichiers

- `index.html` (le style y est), `demo.js` : la page de la démo.
- `moteur.js` : le worker du moteur. Il corrige au passage deux manques du disque en mémoire du shim : les dates des
  fichiers (aucune, et rangées aux mauvais octets de `filestat`) et son journal de débogage, allumé par défaut.
- `relais.js` : le service worker.
- `wasi-shim/` : `@bjorn3/browser_wasi_shim` **0.4.2** (MIT ou Apache-2.0, `LICENSE-MIT`, `LICENSE-APACHE`), les
  fichiers de `dist/` du paquet npm dont la démo a besoin (`wasi.js`, `wasi_defs.js`, `fd.js`, `fs_mem.js`,
  `debug.js`), tels quels : le disque en mémoire et les appels WASI.
- `onglet.wasm` et `demo.tar.gz` **ne sont pas dans Git** : « Publier » les fabrique et les pose à côté de la page,
  sous ces deux noms.
  - `onglet.wasm` : le moteur, `cargo build --locked --profile navigateur --target wasm32-wasip1 -p onglet` (dans
    `atelier/sources`), le fichier `wasm32-wasip1/navigateur/onglet.wasm` de la cible.
  - `demo.tar.gz` : l'installation de démo, `tar czf demo.tar.gz -C <arbre> .`, où l'arbre porte
    `logiciel/donnees/`, `logiciel/interface/` et `logiciel/VERSION` (copies de `logiciel/`),
    `logiciel/EDITION.json` (`{"edition": "bride"}`), `PORTABLE.json` (le contenu de
    `compagnie::coffre::CONTENU_PORTABLE`) et `fictives/<nom>/` (chaque fictive livrée par `scenario livrer`). Le
    lecteur de tar de `moteur.js` ne prend que des dossiers et des fichiers ordinaires (ustar, GNU, pax).

## Ce que le serveur doit servir sous `/demo/`

La politique de sécurité du chemin `/demo/` (et d'elle seule) ; elle part de celle du site et ajoute
`'wasm-unsafe-eval'` (sans lui, le worker ne compile pas le moteur), `connect-src` (sans lui, le worker ne télécharge ni
le moteur ni l'installation, et les incidents ne partent pas) et `frame-src 'self'` (sans lui, le cadre du logiciel est
refusé) :

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src 'self'; img-src 'self'; connect-src 'self' https://filet.puppetpaie.fr; frame-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```

- `worker-src` n'est pas nécessaire : il retombe sur `script-src 'self'`.
- `relais.js` : l'en-tête `Service-Worker-Allowed: /` (sans lui, la portée `/` est refusée à l'enregistrement).
- `onglet.wasm` : `Content-Type: application/wasm`. La page compile le flux quel que soit le type annoncé, mais c'est
  son type.
- Le cache : `Cache-Control: no-cache` (`expires -1`, comme le site) avec un `ETag` ; un rechargement revalide et ne
  retélécharge rien qui n'ait changé.
- La compression : `onglet.wasm` (24,3 Mo) se compresse bien (10,2 Mo au niveau 1 de gzip, celui de nginx par défaut ;
  9,2 Mo au niveau 6) ; `demo.tar.gz` est déjà compressé. Compressé en route, le serveur n'annonce plus la longueur :
  l'écran dit alors les mégaoctets reçus, sans le total.
- Un `add_header` posé dans un `location /demo/` de nginx annule ceux du `server` : ce bloc redit les en-têtes de
  sécurité du site (`Strict-Transport-Security`, `X-Content-Type-Options`, `Referrer-Policy`, `X-Frame-Options`).

Les pages que le relais fabrique ne portent pas les en-têtes du serveur : le relais leur pose sa propre politique
(`relais.js`), sauf quand le moteur en donne une (ses PDF), comme le serveur du logiciel sur le poste. Elle ajoute à
celle du site ce que la page du logiciel exige : ses scripts et attributs `on…` en ligne (`'unsafe-inline'`), ses
feuilles de style (`style-src 'self'`) et ses styles en ligne, ses images (`img-src 'self'` : le cachet), ses appels
(`connect-src 'self'`), ses cadres (`frame-src 'self'` : les PDF des courriels joués et de l'aperçu du bulletin), et
`frame-ancestors 'self'` ; chacun casse quelque chose quand on le retire.

## La jouer en local

Il faut un serveur qui serve `atelier/site/` à la racine, avec les en-têtes ci-dessus, et `onglet.wasm` et
`demo.tar.gz` sous `/demo/`. Le service worker exige un contexte sûr : `http://127.0.0.1` en est un. Chromium joue la
démo par Playwright (`require('playwright')`) ; dans un essai, le navigateur se lance sans réseau
(`--host-resolver-rules=MAP * ~NOTFOUND, EXCLUDE 127.0.0.1`, `--no-proxy-server`) : l'envoi au filet rend alors une
panne, et rien ne sort.
