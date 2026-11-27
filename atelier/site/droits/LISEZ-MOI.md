# La page des droits d'un intermittent

Le site puppetpaie.fr l'héberge sous `https://puppetpaie.fr/droits/` : son déploiement prend ce dossier sur `main`
(sans ce fichier). Aucun lien de l'accueil n'y mène pour l'instant. Un intermittent y dépose ses bulletins de paie (de
tous ses employeurs) ou remplit des lignes, et voit ses heures, ce qui lui manque et jusqu'à quand, et son allocation
estimée. Elle ne garde rien : fermer l'onglet efface tout.

- `index.html` (le style y est : la politique du site n'admet pas de feuille à part) et `droits.js` : la page. Elle ne
  décide rien : elle demande la route `droits-des-pieces` (`atelier/sources/routes/src/accueil/droits_des_pieces.rs`)
  et montre ce qu'elle rend, ses cases (`cases` : libellé, forme, valeur, choix, parties des rangées) comprises.
- Le moteur : le worker de la démo (`/demo/moteur.js`), avec les deux mêmes fichiers (`/demo/onglet.wasm.gz`,
  `/demo/demo.tar.gz`), sans relais ni cadre : la page tient elle-même le bout du port où elle demande
  `/api/droits-des-pieces`, et fait poser les bulletins déposés dans le dossier que la route nomme (`dossier`, le
  message `poser` du worker). Elle s'ouvre tout de suite ; le moteur se charge derrière, et le premier calcul part
  quand il est prêt. Un envoi au filet, s'il y en a, est celui de la démo (l'import `poster` du worker).

## Ce que le serveur doit servir sous `/droits/`

La politique de sécurité du chemin `/droits/` : celle du site, plus `'wasm-unsafe-eval'` et `connect-src`, pour le
worker du moteur qu'elle lance (un navigateur qui donne au worker la politique de la page qui le crée en a besoin ici ;
un autre lui donne celle de `/demo/moteur.js`, qui les a déjà). Pas de cadre : ni `frame-src`, ni relais.

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src 'self'; img-src 'self'; connect-src 'self' https://filet.puppetpaie.fr; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```

Le cache : comme le site (`expires -1`). Un `add_header` posé dans un `location /droits/` de nginx annule ceux du
`server` : ce bloc redit les en-têtes de sécurité du site, comme celui de `/demo/`.
