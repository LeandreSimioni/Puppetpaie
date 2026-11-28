# La page des droits d'un intermittent

Le site puppetpaie.fr l'héberge sous `https://puppetpaie.fr/droits/` : son déploiement prend ce dossier sur `main`
(sans ce fichier). Aucun lien de l'accueil n'y mène pour l'instant. Un intermittent y dépose ses bulletins de paie (de
tous ses employeurs) ou remplit des lignes, et voit d'abord s'il a ses heures (en vert) et son allocation, ou ce qui
lui manque et jusqu'à quand. Elle ne garde rien ailleurs que dans cet onglet : recharger la page reprend ce que vous avez saisi, fermer l'onglet efface tout.

- `index.html` (le style y est, sauf celui de la boîte « Un problème, une idée ? », `logiciel/filet.css`) et
  `droits.js` : la page. Elle ne
  décide rien : elle demande la route `droits-des-pieces` (`atelier/sources/routes/src/accueil/droits_des_pieces.rs`)
  et pose son écran bloc par bloc, dans l'ordre où la route le rend (`ecran`). Avec un calcul : la réponse (sa
  couleur, celle que la route dit), l'allocation visée (« Je voudrais … € par jour », « Réinitialiser » vide la case),
  l'année de chômage (sans aucun travail, son chiffre, puis ses calculs repliés, et la case du taux de CSG, quand la route la rend ;
  ce que rapporte un jour de travail et ses exemples, chacun son calcul, son détail replié ; l'année comme la dernière, ses
  barres dessinées à l'échelle de la plus haute, son calcul et son bilan, puis, repliés, le tableau des mois et ses
  remarques ; les `precisions` de la route ne se montrent pas), la date
  anniversaire, le tableau des paies (celles que les 12 mois comptent) où les lignes prévues se saisissent
  (leur nature dans une liste déroulante : artiste, technicien, enseignement, formation ; vide, celle de base que la
  route y montre ; une paie lue dit la sienne en texte), le dépôt, les dates d'embauche, les pièces, et « Détails du
  calcul », replié (son `resume`, puis les `lignes` : ce qui ne se dit que quand cela arrive). Sans
  calcul : la date anniversaire, le tableau vide, le dépôt.
- `premier-ecran.json` : ce que la route rend sans charge (la date anniversaire, le tableau vide, le dépôt), le dossier
  du dépôt à `null`. La page le pose dès l'ouverture, avec les mêmes fonctions que toute réponse, pendant que le moteur
  se charge : on y tape, on y dépose, on y ajoute des lignes. Un test de structure le tient égal à la route
  (`moteur/tests/fichiers/structure.rs`) : la route changée, il se recopie. Le moteur prêt, le bouton « Calculer »
  apparaît ; son appui fait le premier calcul, et ensuite tout se recalcule seul.
- Le moteur : le worker de la démo (`/demo/moteur.js`), avec les deux mêmes fichiers (`/demo/onglet.wasm.gz`,
  `/demo/demo.tar.gz`), sans relais ni cadre : la page tient elle-même le bout du port où elle demande
  `/api/droits-des-pieces`, et fait poser les bulletins déposés dans le dossier que le dépôt nomme (`dossier`, le
  message `poser` du worker). Elle s'ouvre tout de suite ; le moteur se charge derrière, et le premier calcul part
  à l'appui sur « Calculer ». Un envoi au filet, s'il y en a, est celui de la démo (l'import `poster` du worker).
- « Un problème, une idée ? » (le bouton du bas, qui arrive avec le moteur) : **la boîte du logiciel elle-même**,
  `logiciel/interface/js/filet.js`, avec ce qu'elle prend comme sur l'accueil des compagnies (`montrer.js`,
  `boites.js`, `dates.js`, `calendrier.js`, `avance.js`), son style (`filet.css`) et son outil de capture
  (`html2canvas.min.js`). Aucune copie dans Git : le déploiement les prend dans `demo.tar.gz` (l'installation que le
  moteur déballe, donc de la même publication que lui) et les pose sous `droits/logiciel/` (la liste ci-dessous). La
  page donne à la boîte ce que l'accueil lui donne : `CATALOGUE` (vide), `ETAT` (`null`), `api` (la route, par le
  port du worker), `dire` (l'avis de la page), le voile et la boîte (`#voile`, `#boite`) et leur style. Les routes
  (`signalements`, `signalement-apercu`, `signalement-envoyer`) sont servies sans compagnie ; dans l'onglet, le
  message part sur place, et la boîte ne promet ni réponse ni renvoi (le paquet soudé : `routes::filet`,
  `ecran_du_message`).

## Ce que le déploiement pose sous `/droits/logiciel/`

Pris dans `demo.tar.gz` (la Release, comme `/demo/`), sous `./logiciel/interface/`, posés tels quels :

```
./logiciel/interface/filet.css               -> droits/logiciel/filet.css
./logiciel/interface/html2canvas.min.js      -> droits/logiciel/html2canvas.min.js
./logiciel/interface/js/montrer.js           -> droits/logiciel/js/montrer.js
./logiciel/interface/js/boites.js            -> droits/logiciel/js/boites.js
./logiciel/interface/js/dates.js             -> droits/logiciel/js/dates.js
./logiciel/interface/js/calendrier.js        -> droits/logiciel/js/calendrier.js
./logiciel/interface/js/avance.js            -> droits/logiciel/js/avance.js
./logiciel/interface/js/filet.js             -> droits/logiciel/js/filet.js
```

## Ce que le serveur doit servir sous `/droits/`

La politique de sécurité du chemin `/droits/` : celle du site, plus `'wasm-unsafe-eval'` et `connect-src`, pour le
worker du moteur qu'elle lance (un navigateur qui donne au worker la politique de la page qui le crée en a besoin ici ;
un autre lui donne celle de `/demo/moteur.js`, qui les a déjà). Pour la boîte « Un problème, une idée ? » :
`style-src 'self'` (sa feuille, `logiciel/filet.css`) et `img-src data:` (les vignettes des captures, que la boîte
montre en `data:`). Pas de cadre : ni `frame-src`, ni relais.

```
default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; font-src 'self'; img-src 'self' data:; connect-src 'self' https://filet.puppetpaie.fr; base-uri 'none'; form-action 'none'; frame-ancestors 'none'
```

Le cache : comme le site (`expires -1`). Un `add_header` posé dans un `location /droits/` de nginx annule ceux du
`server` : ce bloc redit les en-têtes de sécurité du site, comme celui de `/demo/`.
