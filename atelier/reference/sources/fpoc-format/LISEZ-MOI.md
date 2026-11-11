# Le format des fiches de paramétrage des organismes complémentaires (FPOC), versions 1.3.9 et 1.4.0

## D'où ça vient

`net-entreprises-fiche691-fiches-de-parametrage-oc-20260723.zip` : l'archive que sert la **fiche 691** de la base de
connaissances net-entreprises (`https://net-entreprises.custhelp.com/app/answers/detail_dsn/a_id/691/`, citée par la page
`https://www.net-entreprises.fr/declaration/precontrole-dsn-fpoc/`), relevée par le conteneur le 11/10/2026 ; elle
contient deux archives « R139 - maj 23 juillet 2026 » et « R140 maj 23 juillet 2026 », dépliées ici. Empreinte SHA-256
(16) de l'archive : voir le commit. Fichiers dépliés :

| fichier | ce que c'est |
|---|---|
| `fpoc-specification-r1.4.0-20260721.pdf` (+ `.txt`) | « Fiche de paramétrage DSN - Organismes Complémentaires », version **1.4.0** (onglets Règles, Versions, Données, Schéma), 15 p., datée du 21/07/2026 |
| `fpoc-specification-r1.3.9-20260721.pdf` (+ `.txt`) | la même en version **1.3.9** (celle des fiches reçues aujourd'hui : `atelier/reference/fiches-parametrage/audiens-2025-12-13-fiche139-expurgee.xml`), 14 p. |
| `fpoc-r1.4.0.xsd`, `fpoc-r1.3.9.xsd` | les schémas XML |
| `fpoc-presentation-versions-139-140-20260720.pptx` (+ `.txt`) | « Fiches de paramétrage V139 - V140, mise à jour du 20 juillet 2026 » : calendrier, conditions tarifaires, delta 139 → 140 |

Les exemples XML (`ExempleMin`, `ExempleMax`) de chaque version sont dans l'archive, non dépliés.

## Ce qu'on en lit (11/10/2026 ; rien n'est codé)

- **C'est le format que `paie/src` lit** quand il relève les fiches Audiens par l'API : la 1.3.9 est la version servie
  (le nom du fichier déposé, `fiche139`), la **1.4.0** arrive (date de la version : 16/06/2025 puis 30/06/2026 ; « purge
  du stock de fiches : non planifié »). Ce qui change en 1.4.0 (onglet Versions, l. 49-75 ; diapositives 20-22) : plafond
  et plancher facultatifs sur les bases et montants forfaitaires (contrôles OC.40, OC.41), cardinalités des critères
  salariés, seuil de 99 valeurs, deux conditions tarifaires de plus, alignement sur le CT 2026.1.0. **À porter** : le
  lecteur de fiches devra accepter les deux versions.
- **Q14 (une famille : combien de bases 79 ?) : c'est la fiche qui le dit**, par ses « conditions tarifaires » (spécification
  1.4.0, l. 504-531 ; diapositives 16-18) : tarif unique « 01 = assuré et tous ses ayants droit », « 02 = isolé », « 03 = avec
  1 conjoint », « 04 = avec 1 enfant »… ; ou composants « 90 = ouvrant droit », « 91 = conjoint », « 92 = chaque ayant droit »,
  « 94 = chaque enfant ». Les diapositives donnent, pour chaque combinaison, **le nombre de bases et montants spécifiques
  (BMS) à déclarer** et les compteurs 70.008/70.009/70.010/70.011 à renseigner : une condition « 01 » = une seule base ;
  « 90/91/94 » = trois (ouvrant droit, conjoint, par enfant). Le moteur ne doit pas choisir : il lit la condition de la fiche.
- **Option et population** (l. 279-297) : « Option [0,1], à ne renseigner que si la donnée doit » figurer en DSN ; code
  option 70.004 et son libellé, « ancien code option » en cas de renumérotation technique. Même chose pour la population
  (70.005). Confirme la FAQ des OC (`../faq-dsn-organismes-complementaires/`) et V3e.
- **D3, un morceau** : la fiche peut porter des **bornes sur la date d'affiliation** (`S21.G00.70.014` : « valeur de la
  borne inférieure / supérieure », l. citées dans les deux spécifications), c'est-à-dire qu'un contrat de la fiche ne vaut
  que pour les affiliations datées dans l'intervalle. Ce n'est pas la liste des cas d'usage de 70.014 (toujours
  introuvable), mais une raison de plus de lire 70.014 depuis la fiche salarié quand l'affiliation ne commence pas avec le
  contrat.
- Niveaux de fiche (l. 20-37) : 2A (contrats, éléments de calcul), 2B (critères salariés : statut, PCS-ESE, ancienneté),
  2C (combinaisons de contrats compatibles, « affiliations obligatoires / automatiques »).

## Où le moteur s'en sert

Nulle part : le moteur ne lit aucune fiche de paramétrage ; la DSN d'un permanent santé déclare une base 79 type 18.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `fpoc-presentation-versions-139-140-20260720.pptx` : `9a7814b81278951fc462f1e33a91da28d6ae9e76211f844f2f58ac70e4503c3d`
- `fpoc-presentation-versions-139-140-20260720.txt` : `cbb1dbcfae1da577cf30f39c997447e3089d07bdd2a04f2a4182167b32f3fdf6`
- `fpoc-r1.3.9.xsd` : `839da276fe77683f0bd3b78997b863e4619dbdd55597274326bd8038545ee760`
- `fpoc-r1.4.0.xsd` : `443f1ba7568c6b68f9bc2baa6c748b07e8df303198b3f1a42ced55074f39a558`
- `fpoc-specification-r1.3.9-20260721.pdf` : `0f7a3b357977900f14635f65ea281735dd620211263935fb57a46b2f1a8a23fe`
- `fpoc-specification-r1.3.9-20260721.txt` : `56a4bb8d5324883e275756a4c5f3f2222aee93e41e9422b97b186c2aaf74e598`
- `fpoc-specification-r1.4.0-20260721.pdf` : `2590c467d779f097e8308051b13bcfcce113263ebdedb56d91ef94e661ca7c72`
- `fpoc-specification-r1.4.0-20260721.txt` : `092e22c35774eab8906dad484623565dec73968e608c8c7c2b2ed67374f5fa3f`
- `net-entreprises-fiche691-fiches-de-parametrage-oc-20260723.zip` : `08dd2085ed62a42176113858f4a496e81c409218a1e79eca1f56e806e5981133`
