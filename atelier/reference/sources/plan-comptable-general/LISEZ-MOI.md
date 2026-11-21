# Le plan comptable général (ANC, règlement n° 2014-03) — le plan de comptes

Relevé le 11/10/2026 (séance « restes », lot R3 : l'export comptable n'écrivait jamais le prélèvement à la source).

## Le texte déposé

- `anc-plan-de-comptes-20260101.pdf` : « Plan de comptes prévu à l'article 1121-1 du règlement ANC n° 2014-03
  relatif au plan comptable général », **version au 1er janvier 2026**, Autorité des normes comptables, tel que servi
  (18 pages, SHA-256 `e5d85249d348809bf5f4585d37ecdb9b7c37badd7bf86b371da27eef4bb8dd4a`).
- `anc-plan-de-comptes-20260101.txt` : sa transcription (`pdftotext -layout`).
- D'où : la page « Plan comptable général » de l'ANC, <https://www.anc.gouv.fr/plan-comptable-general-0>, lien
  « Plan de comptes 2026 » :
  <https://www.anc.gouv.fr/files/anc/files/1_Normes_fran%C3%A7aises/Plans%20comptables/2026/Plan-de-comptes-2026.pdf>.
- `anc-plan-comptable-general-20260101.pdf` : le recueil complet du règlement ANC n° 2014-03, **version au
  1er janvier 2026**, tel que servi le 21/11/2026 (183 pages, SHA-256
  `002bbc6a5eca1079b895ba6c4b670f01db9820459cf834c375cfe021ca17db18`), lien « PCG » de la même page :
  <https://www.anc.gouv.fr/files/anc/files/1_Normes_fran%C3%A7aises/Plans%20comptables/2026/PCG--1er-janvier-2026.pdf>.
- `anc-plan-comptable-general-20260101.txt` : sa transcription (`pdftotext -layout`).

## L'avantage en nature : ce que le recueil dit, et ce qu'il tait

Le recueil ne contient nulle part les mots « avantage en nature » (cherché dans la transcription). Le seul article
sur les charges de personnel est l'art. 1221-64 (ligne 9042 de la transcription) : « Sont inscrites au compte 641
« Rémunérations du personnel » toutes les rémunérations du personnel ». Qu'un avantage en nature, qui est une
rémunération (il entre dans le brut soumis), aille au 641 avec le brut est donc une **inférence** sur cet article ;
le texte ne nomme ni sous-compte ni contrepartie pour la part qui ne se paie pas en argent. Aucune règle du moteur
ne s'en réclame encore.

## Ce qu'on en lit

En tête : « Le plan de comptes présenté ci-après, est commun au système de base et au système abrégé. Le plan de
comptes minimal comprend l'ensemble des comptes imprimés en caractères normaux. Les comptes imprimés en caractères
italiques sont d'utilisation facultative. » (La transcription perd l'italique.)

Les comptes que le plan livré du moteur (`logiciel/donnees/plan-comptable-base.json`) emploie, et ceux qu'il
pourrait employer (lignes 391-418 et 733-749 de la transcription) :

| compte | libellé du plan de comptes | où le moteur s'en sert |
|---|---|---|
| 421 | Personnel - Rémunérations dues | `net` (421000) : le net payé à chaque salarié |
| 425 | Personnel - Avances et acomptes et autres comptes débiteurs | `retenues.acompte` (425000) : l'acompte retenu solde l'avance |
| 427 | Personnel - Oppositions | nommé dans `_retenues` et dans le refus ; aucune rubrique livrée ne s'en sert |
| 4282 | Dettes provisionnées pour congés à payer | `provision_conges.dette` (428200) |
| 431 | Sécurité sociale | tiers 431000 (Urssaf) |
| 437 | Autres organismes sociaux | tiers 437100 à 437400 |
| 4382 | Charges sociales sur congés à payer | `provision_conges.dette_charges` (438200) |
| 442 | Contributions, impôts et taxes recouvrés pour le compte de l'État | — |
| **4421** | **Prélèvements à la source (Impôt sur le revenu)** | **`pas` (442100)** : le PAS retenu sur le net |
| 6411 | Salaires, appointements | `brut` (641100) |
| 6412 | Congés payés | `provision_conges.conges` (641200) |
| 6414 | Indemnités et avantages divers | `frais` (641400) |
| 6451 à 6458 | Cotisations à l'Urssaf, aux mutuelles, aux caisses de retraites, à Pôle emploi, aux autres organismes sociaux | les charges des `lignes` (645100 à 645800) |

## Où le moteur s'en sert

- `atelier/sources/paie/src/exercice_ecrit.rs`, `ecritures_du_mois` : depuis le lot R3, l'écriture de paie du mois
  porte au crédit du compte `pas` du plan (442100 par défaut, le 4421 ci-dessus) le prélèvement à la source retenu
  sur les bulletins du mois, et le 421 reçoit le net **après** impôt (le « MONTANT NET A PAYER » du bulletin).
  Le montant est celui de `pas::des_paies`, le même calcul que le bulletin, le livre de paie et l'état
  « prélèvement à la source ».
- Même fonction, depuis le lot R3 aussi : une ligne hors assiette de nature `retenue` (la rubrique livrée `acompte`)
  n'est plus portée avec les indemnités non soumises au 641400 (où elle diminuait les charges de personnel) mais au
  compte que le plan lui donne sous `retenues` (425000 pour l'acompte), au crédit, au nom du salarié ; une retenue que
  le plan ne nomme pas fait refuser l'export.
- `logiciel/donnees/plan-comptable-base.json` : les numéros par défaut (`pas` y était déjà, 442100, sans que rien ne
  l'écrive ; `retenues` y est ajouté), et les notes `_pas` et `_retenues`.

## Ce qui reste, et ce qui est inférence

- **Acompte au 425, saisie au 427** : le plan de comptes nomme les deux comptes, il ne dit pas lequel reçoit quelle
  retenue ; ranger l'acompte retenu au 425 (« avances et acomptes ») et une saisie au 427 (« oppositions ») est une
  **inférence** sur leurs libellés.
- Le plan livré écrit 645500 pour les Congés Spectacles : le plan de comptes ne nomme pas de 6455 (il s'arrête à
  6451, 6452, 6453, 6454, 6458). Un sous-compte à six chiffres d'un compte du plan est l'usage ; un 6455 n'y figure
  pas. Constat, pas corrigé : c'est une donnée que le comptable de la compagnie remplace.
- Le passage « 4421 → 442100 » (compléter de zéros à six chiffres) est l'usage des logiciels de comptabilité, pas
  une phrase du plan de comptes : **inférence**.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `anc-plan-comptable-general-20260101.pdf` : `002bbc6a5eca1079b895ba6c4b670f01db9820459cf834c375cfe021ca17db18`
- `anc-plan-comptable-general-20260101.txt` : `da090a5f264daa44c6f0a2ce410959d859e39642ddb06cfacec617eb2b539ff9`
- `anc-plan-de-comptes-20260101.pdf` : `e5d85249d348809bf5f4585d37ecdb9b7c37badd7bf86b371da27eef4bb8dd4a`
- `anc-plan-de-comptes-20260101.txt` : `c09e9a67c62b65f8c96bc956a0290344fcc4ea56dc97beaf2380200fe36834c7`
