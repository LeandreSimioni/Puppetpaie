# La page des droits d'un intermittent

Le site puppetpaie.fr l'héberge sous `https://puppetpaie.fr/droits/` : son déploiement prend ce dossier sur `main`
(sans ce fichier). Aucun lien de l'accueil n'y mène pour l'instant. Un intermittent y dépose ses bulletins de paie (de
tous ses employeurs) ou remplit des lignes, et voit ses heures, ce qui lui manque et jusqu'à quand, et son allocation
estimée. Elle ne garde rien : fermer l'onglet efface tout.

- `index.html` (le style y est : la politique du site n'admet pas de feuille à part) et `droits.js` : la page. Elle ne
  décide rien : elle demande la route `droits-des-pieces` (`atelier/sources/routes/src/accueil/droits_des_pieces.rs`)
  et pose son écran bloc par bloc, dans l'ordre où la route le rend (`ecran` : une case, le résumé, le tableau des
  paies où les lignes prévues se saisissent, le dépôt, les pièces déposées). Sans calcul : la date anniversaire, le
  dépôt, le tableau vide ; avec un calcul : le résumé en tête, puis le tableau, la date anniversaire et le dépôt, les
  dates d'embauche, les pièces.
- `premier-ecran.json` : ce que la route rend sans charge (la date anniversaire, le dépôt, le tableau vide), le dossier
  du dépôt à `null`. La page le pose dès l'ouverture, avec les mêmes fonctions que toute réponse, pendant que le moteur
  se charge : on y tape, on y dépose, on y ajoute des lignes. Un test de structure le tient égal à la route
  (`moteur/tests/fichiers/structure.rs`) : la route changée, il se recopie. Le moteur prêt, le bouton « Calculer »
  apparaît ; son appui fait le premier calcul, et ensuite tout se recalcule seul.
- Le moteur : le worker de la démo (`/demo/moteur.js`), avec les deux mêmes fichiers (`/demo/onglet.wasm.gz`,
  `/demo/demo.tar.gz`), sans relais ni cadre : la page tient elle-même le bout du port où elle demande
  `/api/droits-des-pieces`, et fait poser les bulletins déposés dans le dossier que le dépôt nomme (`dossier`, le
  message `poser` du worker). Elle s'ouvre tout de suite ; le moteur se charge derrière, et le premier calcul part
  à l'appui sur « Calculer ». Un envoi au filet, s'il y en a, est celui de la démo (l'import `poster` du worker).
- « Un problème, une idée ? » (le bouton du bas, qui arrive avec le moteur) : la boîte du logiciel
  (`logiciel/interface/js/filet.js`), par les mêmes routes, que le moteur sert sans compagnie (`signalements` pour ce
  qu'elle dit, `signalement-apercu` pour « voir ce qui part », `signalement-envoyer`) ; dans l'onglet, le message part
  sur place, et la boîte dit où il en est. Pas de capture d'écran (le logiciel la fait par html2canvas, qu'il sert
  lui-même) : on joint des images choisies, montrées par leur nom (la politique n'admet pas d'image `data:`). Ni liste
  des messages, ni réponse : rien n'est gardé.

## Ce que le serveur doit servir sous `/droits/`

La politique de sécurité du chemin `/droits/` : celle du site, plus `'wasm-unsafe-eval'` et `connect-src`, pour le
worker du moteur qu'elle lance (un navigateur qui donne au worker la politique de la page qui le crée en a besoin ici ;
un autre lui donne celle de `/demo/moteur.js`, qui les a déjà). Pas de cadre : ni `frame-src`, ni relais.

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'unsafe-inline'; font-src 'self'; img-src 'self'; connect-src 'self' https://filet.puppetpaie.fr; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```

Le cache : comme le site (`expires -1`). Un `add_header` posé dans un `location /droits/` de nginx annule ceux du
`server` : ce bloc redit les en-têtes de sécurité du site, comme celui de `/demo/`.
