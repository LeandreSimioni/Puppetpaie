# Tables des CTP Urssaf du 29/10/2026 (déposées par Léandre le 14/11/2026)

- **D'où** : les fichiers « codesTypesCsv » (les codes en vigueur) et « histoCodesTypesCsv » (leur historique) de
  l'Urssaf, téléchargés par Léandre et déposés tels que servis : encodage d'origine (Latin-1), dates en `JJ/MM/AAAA`.
  Ils ne se convertissent pas.
  - `urssaf-codes-types-personnel-20261029.csv` : les codes en vigueur au 29/10/2026.
  - `urssaf-codes-types-personnel-historique-20261029.csv` : tout l'historique.
- **Ce qu'on y lit** : la colonne « Format » porte les lettres A, B, C, D, E, F, K, R, V, Y, Z. Aucun de ces fichiers
  ne dit ce que veut dire une lettre ; le guide Urssaf de la régularisation (§ 1.3,
  `editeur-dsn/guide-urssaf-regularisation.txt`) décrit toutes les lettres sauf B et C. Le CTP 312 (« Artistes cas
  général abattement 30 % »), que le moteur déclare, est en format B : son texte reste à trouver.
- **Où le moteur s'en sert** : nulle part encore. Le moteur lit la copie livrée
  `logiciel/donnees/urssaf-codes-types-personnel.csv` (`documents::bordereau`), relevée le 13/10/2026 ; la comparer à
  celle-ci et la mettre à jour est un geste à part.

## Les mêmes tables en XML (déposées par Léandre le 14/11/2026)

- `urssaf-codes-types-personnel-20261029.xml` (« codesTypesXml ») et
  `urssaf-codes-types-personnel-historique-20261029.xml` (« histoCodesTypesXml ») : encodage ISO-8859-1, déposées
  telles que servies. Le guide Urssaf (version 5.2, `editeur-dsn/guide-urssaf-regularisation-v5.2-20261019.txt`,
  l. 387-394) y renvoie : un CTP dont `fmtco` vaut `DED` porte un montant de cotisation (23.005), les autres une
  assiette (23.004).
- **Ce que le XML dit du CTP 312** (format B dans le CSV ; ligne 3171 du fichier en vigueur, effet au 01/01/2026) :
  `fmtco="000"` (une assiette), et trois lignes de base : « ACCIDENT DU TRAVAIL » (`tpcode="A"`, le taux AT à
  renseigner), « BASE DEPLAFONNEE » (`tpcode="D"`, taux 9.38), « BASE PLAFONNEE » (`tpcode="P"`, taux 10.82) : la
  structure d'un CTP de format A. La lettre B elle-même n'y est pas définie.
- Les valeurs de `fmtco` relevées : `000` (623), `DED` (105), `EXO` (94), `CLO` (7), `BFI` (6).
