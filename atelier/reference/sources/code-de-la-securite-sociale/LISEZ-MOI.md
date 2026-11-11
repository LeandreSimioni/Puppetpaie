# Code de la sécurité sociale : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `paie releve-codes`** (`moissonner`, puis
`rapport`) depuis la base LEGI de la DILA ; ils ne se retouchent pas à la main. Les articles suivis sont la liste du texte
`securite-sociale` de `atelier/sources/paie/src/releve_codes/textes.rs`, où chaque domaine dit pourquoi il est là ; le dernier
relevé est le plus récent des `articles-*.md`. Ce fichier dit, par sujet ajouté depuis qu'il existe, ce qu'on en lit et où le
moteur s'en sert (les sujets plus anciens sont décrits dans les commentaires de `textes.rs` et dans les LISEZ-MOI des dossiers
qui les citent : `dfs/`, `plafond-intermittents/`, `majoration-caisses-conges/`…).

## Les signalements d'événement de la DSN et leur délai (relevé du 15/10/2026)

`articles-20261015.md`, section « Les signalements d'evenement de la DSN et leur delai » (`xml/R133-13.xml`, `xml/R133-14.xml`,
versions en vigueur depuis le 01/07/2026).

- **R. 133-13, I** : l'employeur « déclare également les événements suivants […] survenus au cours du mois considéré : 1° Le
  début et la fin de l'arrêt de travail […] ; 2° La fin du contrat de travail ».
- **R. 133-14, II** : ces événements se déclarent « au plus tard en même temps que la déclaration sociale nominative relative au
  mois au cours duquel cet événement est survenu » ; « Toutefois, sont déclarés dans un délai fixé par arrêté […], qui ne peut
  excéder cinq jours ouvrés, la fin du contrat de travail ainsi que, **lorsque l'employeur n'est pas subrogé** dans les droits de
  l'assuré aux indemnités journalières, le début et la fin de l'arrêt de travail ». Exceptions : 1° aucune retenue pour l'arrêt
  sur le salaire du mois où il débute ; 2° les fins de CDD d'usage (3° de L. 1242-2 du code du travail), sauf demande du
  salarié. Le délai est celui de l'arrêté du 2 juillet 2013 (`../arrete-2013-delai-evenements-dsn/`) : cinq jours ouvrés.
- **R. 133-14, IV, 1° et 2°** : la DSN tient lieu de l'attestation de salaire (R. 323-10, R. 441-4) et de l'attestation employeur
  (R. 1234-9 du code du travail).

**Où le moteur s'en sert** : `atelier/sources/paie/src/obligations.rs` et `logiciel/donnees/obligations-base.json`
(`signalement_arret`, `signalement_reprise`, `signalement_fin_contrat`).

## La déclaration d'un accident du travail ou de trajet, et celle d'une maladie professionnelle (relevé du 15/10/2026)

`articles-20261015.md`, sections « La declaration d'un accident du travail ou de trajet » et « La declaration d'une maladie
professionnelle (par la victime) ».

- **L. 441-2** : « L'employeur ou l'un de ses préposés doit déclarer tout accident dont il a eu connaissance à la caisse primaire
  d'assurance maladie dont relève la victime selon des modalités et dans un délai déterminés. »
- **R. 441-3** : « dans les quarante-huit heures non compris les dimanches et jours fériés », « par tout moyen conférant date
  certaine à sa réception » ; son second alinéa (le délai ne court que du jour où l'employeur a été informé) vise certaines
  catégories de L. 311-3, accidentées hors des locaux — L. 311-3 n'est pas relevé : le moteur compte de toute façon depuis la
  connaissance de l'accident par la compagnie (L. 441-2), ce qui couvre ce cas.
- **L. 441-1 et R. 441-2** : la victime informe l'employeur « dans la journée où l'accident s'est produit ou au plus tard dans
  les vingt-quatre heures ».
- **R. 441-6** : les réserves motivées de l'employeur, « dans un délai de dix jours francs à compter de la date à laquelle il
  l'a effectuée » ; **R. 441-7** : la caisse statue en trente jours francs, ou enquête (réserves reçues).
- **L. 411-1 et L. 411-2** : l'accident de trajet « est également considéré comme accident du travail » (motif d'arrêt 04).
- **L. 441-4** : le registre des accidents bénins (sans arrêt ni soins) ; **L. 441-5** : la feuille d'accident que l'employeur
  délivre ; **R. 441-1** ; **R. 441-4** : l'attestation de salaire, que la DSN remplit (R. 133-14, IV).
- **L. 461-5 et R. 461-5** : la maladie professionnelle se déclare « par les soins de la victime », dans les quinze jours de la
  cessation du travail. **Le motif d'arrêt 05 n'appelle donc pas de déclaration de l'employeur.**

**Où le moteur s'en sert** : `atelier/sources/paie/src/obligations.rs` (`de_l_arret`) et `logiciel/donnees/obligations-base.json`
(`declaration_accident`, `declaration_maladie_professionnelle`). Ce que disent le service et le formulaire :
`../accident-du-travail/`.

## Le congé de deuil : ses indemnités journalières et l'attestation de salaire (relevé du 15/10/2026, lot « le deuil »)

`articles-20261015.md`, section « Le conge de deuil d'un enfant : ses indemnites journalieres et l'attestation de salaire »
(`xml/L331-9.xml`, `xml/D331-7.xml`, `xml/R323-10.xml`) ; domaine `conge_de_deuil` de `paie/src/releve_codes/textes.rs`.

- **L. 331-9** (version du 31/12/2025, loi n° 2025-1403) : « Lorsqu'il exerce son droit au congé prévu à l'article L. 3142-1-1
  du code du travail, l'assuré perçoit, pendant la durée du congé et selon les mêmes conditions de liquidation et de service,
  l'indemnité journalière mentionnée à l'article L. 331-3 » ; « L'employeur qui a maintenu le salaire de l'assuré en application
  de l'article L. 3142-2 du code du travail est **subrogé de plein droit** dans les droits de son salarié à l'indemnité
  journalière. »
- **D. 331-7** : l'indemnisation se fractionne comme le congé (D. 3142-1-1 du code du travail).
- **R. 323-10** : l'employeur établit « une attestation se rapportant aux payes effectuées pendant les périodes de référence »,
  adressée à la caisse « sous forme électronique, par l'employeur » ; elle porte, pour chaque paye, la période, les journées et
  heures, le montant et la date, la retenue d'assurances sociales, le numéro de cotisant et l'organisme. R. 133-14, IV dit que la
  DSN en tient lieu — **pas pour le deuil**, que la DSN « ne couvre pas à ce jour » (la page de net-entreprises,
  `../accident-du-travail/`). Que R. 323-10, écrit pour la maladie, serve le deuil est ce que dit cette page (« Deuil
  d'enfant ») : une **inférence** du côté du code, dite dans `obligations-base.json`.

**Où le moteur s'en sert** : `logiciel/donnees/obligations-base.json`, obligation `attestation_salaire` (`textes`, `a_avoir`,
`inferences`) ; `atelier/sources/paie/src/obligations.rs` (`de_l_arret`, route `attestation-declare`).

## La majoration de la réduction générale chez un employeur affilié à une caisse de congés (relevé du 29/10/2026)

`articles-20261029.md`, section « La reduction generale et les deductions forfaitaires pour frais professionnels »
(`xml/D241-10.xml`, version en vigueur depuis le 15/06/2026, issue du décret n° 2026-509 du 12/06/2026) ; domaine
`reduction_generale` de `atelier/src/releve/codes/textes.rs`.

- **D. 241-10, I** : pour les salariés du IV de L. 241-13, « Coefficient = (T min + (T delta × [(1/2) × (3 × a × salaire minimum
  calculé pour un an / rémunération annuelle brute - 1)] P )) × b », les autres valeurs étant celles de D. 241-7.
- **D. 241-10, IV** : pour les salariés du 3° du IV de L. 241-13 (le paiement des congés mutualisé par une caisse de congés),
  « les valeurs a et b sont respectivement fixées à 1 et à 100/90 ».
- Avec **D. 241-7, II** (`xml/D241-7.xml`), qui arrondit « le résultat obtenu par application de cette formule » à quatre
  décimales : le coefficient majoré s'arrondit une fois, après la multiplication par b.

**Où le moteur s'en sert** : `atelier/sources/droit/src/reduction.rs` (`reduction_generale`, argument `caisse_conges`) et la
note `_majoration_caisse_conges` de `logiciel/donnees/baremes/2026.json` ; le test de la règle est
`droit/tests/droit/reduction.rs`.

## L'effectif au sens de la sécurité sociale, et son décompte (relevé du 29/10/2026)

`articles-20261029.md`, section « L'effectif au sens de la securite sociale » (`xml/L130-1.xml`, `xml/R130-1.xml`, R. 130-1
en vigueur depuis le 02/01/2020).

- **L. 130-1, I** : l'effectif annuel est « la moyenne du nombre de personnes employées au cours de chacun des mois de
  l'année civile précédente » ; **II** : un seuil n'est franchi qu'après cinq années civiles consécutives.
- **R. 130-1, I** : les mois sans salarié ne comptent pas dans la moyenne ; l'effectif se tronque au centième.
- **R. 130-1, II** : comptent les titulaires d'un contrat de travail, sauf les CDD qui remplacent un salarié absent
  (deuxième phrase du 2° de L. 1111-2 du code du travail) ; à temps plein, entièrement ; à temps partiel, au rapport des
  horaires du contrat à la durée légale ou conventionnelle (dernier alinéa de L. 1111-2) ; chacun « à due proportion du
  nombre de jours du mois pendant lequel [il a] été employé ».
- **R. 130-1, III** renvoie à L. 1111-3 du code du travail (apprentis, contrats aidés…), qui n'est pas relevé : ces
  exclusions ne sont pas appliquées.

**Où le moteur s'en sert** : `atelier/sources/envois/src/organismes_simules.rs` (`effectif_annuel`), le compte rendu de
nature 118 que le simulateur rend à une compagnie fictive ; `droit/src/contexte.rs` (`Effectif::atteint`) pour la règle
des cinq ans, et `Effectif::a_dire` pour les années qui manquent ; `routes/src/fiches.rs` (route `effectif`) pour
l'effectif qu'une compagnie venue d'un autre logiciel dit d'une année sans compte rendu 118, tronqué au centième (R. 130-1, I),
qu'un 118 de la même année remplace (`envois/src/releve_retours.rs`, `poser_effectif`).

## Le mode de tarification des AT/MP : D. 242-6-2 et D. 242-6-14 (relevé du 29/10/2026)

Domaine `atmp_tarification` de `atelier/src/releve/codes/textes.rs`, relevé le 29/10/2026 (`moteur-atelier releve-codes
moissonner`, puis `rapport`) : `articles-20261029.md`, section « Le mode de tarification des accidents du travail et
maladies professionnelles » ; `xml/D242-6-2.xml`, `xml/D242-6-14.xml`.

- **D. 242-6-2** (`LEGIARTI000034668407`, en vigueur depuis le 01/01/2018) : « Le mode de tarification est déterminé en
  fonction de l'effectif global de l'entreprise, tel que défini à l'article R. 130-1, que celle-ci comporte un ou plusieurs
  établissements : 1° La tarification collective est applicable aux entreprises dont l'effectif est inférieur à 20
  salariés ; 2° La tarification individuelle est applicable aux entreprises dont l'effectif est au moins égal à 150
  salariés ; 3° La tarification mixte est applicable aux entreprises dont l'effectif est au moins égal à 20 et inférieur à
  150. »
- **D. 242-6-14** (`LEGIARTI000006735892`, en vigueur depuis le 08/07/2010) : « I. - Les établissements exerçant une
  activité dont la liste est fixée par arrêté du ministre chargé de la sécurité sociale conservent un taux de cotisation
  collectif quel que soit leur effectif de salariés ou celui de l'entreprise dont ils relèvent. II. - Les travailleurs à
  domicile des entreprises constituent des établissements distincts auxquels sont applicables les taux collectifs. »

Ce qu'on en lit : l'arrêté annuel de tarification marque « TC » les activités du I (son article 5) ; pour elles, le taux
est le taux collectif de l'arrêté, quel que soit l'effectif. Hors « TC », sous 20 salariés (effectif de R. 130-1), le taux
est aussi collectif.

**Où le moteur s'en sert** : nulle part encore (déposés pour que la règle du taux AT/MP s'écrive dessus).

## Le taux AT/MP d'un établissement (relevé du 30/10/2026)

`articles-20261030.md`, section « Le mode de tarification des accidents du travail et maladies professionnelles »
(`xml/D242-6-1.xml`, `xml/D242-6-17.xml`, versions issues du décret n° 2026-951, en vigueur au 15/10/2026 et applicables aux
cotisations dues à compter de 2027 : `../tarification-atmp/`).

- **D. 242-6-1** : « Le taux de la cotisation due au titre des accidents du travail et des maladies professionnelles est
  déterminé par établissement » ; « Le classement d'un établissement dans une catégorie de risque est effectué en fonction de
  l'activité exercée » (la nomenclature et les modalités : arrêté du 17/10/1995, `../tarification-atmp/`).
- **D. 242-6-17** : un établissement nouvellement créé d'une entreprise en tarification collective, ou dans une catégorie de
  risque que l'entreprise n'avait pas, a les taux nets collectifs l'année de sa création et les deux années civiles
  suivantes (version du décret, cotisations de 2027 et après : le moteur ne s'en sert pas). **La version applicable aux
  cotisations jusqu'à 2026** (`xml-anterieur/D242-6-17.xml`, `LEGIARTI000022446511`, en vigueur du 01/01/2012 au
  15/10/2026, relevée le 30/10/2026 par `moteur-atelier releve-codes comparer` dans l'archive globale LEGI) : « Les taux nets
  collectifs sont applicables aux établissements nouvellement créés durant l'année de leur création et les deux années
  civiles suivantes, quel que soit leur effectif ou celui de l'entreprise dont ils relèvent », sauf le taux unique d'une
  entreprise qui l'a choisi, et sauf un établissement issu d'un précédent (même activité, mêmes moyens, la moitié du
  personnel repris).

- **D. 242-6-15** (`xml/D242-6-15.xml`, version du même décret, cotisations de 2027 et après ; la version applicable
  jusqu'à 2026 : `xml-anterieur/D242-6-15.xml`, `LEGIARTI000028429426`, relevée le 30/10/2026) : « le taux net notifié ne
  peut varier d'une année sur l'autre ». Il ne dit pas que le taux notifié vaut pour une année : il le suppose. Que le taux
  d'un risque sans « TC » ne vaille que pour l'année de sa date est une **inférence** (l'arrêté est « pour l'année », la
  CARSAT notifie chaque année).

**Où le moteur s'en sert** : `droit/src/bareme.rs` (`taux_atmp_de_l_annee`, la même règle pour le siège et chaque
établissement, `surcharger`) : une ligne du taux vaut à partir de sa date ; celle d'un risque sans « TC » ne vaut pas
au-delà du 31 décembre de son année (l'inférence ci-dessus), et la paie demande la suivante. `droit/src/bareme.rs`
(`etablissement_neuf`, D. 242-6-17 de 2012) et `routes/src/atmp.rs` (`inscrire_l_arrete`) : un établissement
ouvert dans l'année ou les deux précédentes reçoit, jusqu'en 2026, le taux collectif de l'arrêté pour son risque (TC ou
non), inscrit à l'ouverture ; ce que l'article excepte se dit par le taux notifié, qui s'inscrit.
`routes/src/etablissements.rs` (`ecrire`) : la ligne d'un établissement dont le taux change se date à la date de
ce taux.

## La réduction de cotisations salariales des heures supplémentaires et complémentaires, et la déduction patronale (relevé du 02/11/2026, lot LONG7)

Domaine `heures_supplementaires` de `atelier/src/releve/codes/textes.rs` : L241-17 (la réduction, son taux par
décret, son imputation sur la vieillesse de L241-3 et sa limite), L241-18 et L241-18-1 (la déduction forfaitaire patronale
par heure supplémentaire, sous vingt salariés et à partir de vingt), D241-21 (le taux : la somme des taux de vieillesse à la
charge du salarié, au plus 11,31 %), D241-22, D241-24 (1,50 € et 0,50 €), D241-25 ; D241-23 et D241-26 n'ont plus de
version en vigueur (le relevé les dit périmés : retirés de la liste). D241-7, IV (déjà relevé) majore le SMIC de la
réduction générale des heures supplémentaires et complémentaires. Texte dans `articles-20261102.md`, avec la page du BOSS
(`../boss/boss-heures-supplementaires-20260601.txt`). **Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`
(`heures_en_plus`), `atelier/sources/droit/src/heures_en_plus.rs`, `atelier/sources/engagement/src/heures_en_plus.rs`
(`apres_les_cotisations`) ; la DSN (codes 114 et 021, CTP 003 et 004) dans `atelier/sources/declarations/src/dsn.rs`.

## Les indemnités journalières, la subrogation et leur CSG (relevé du 02/11/2026)

`articles-20261102.md`, sections « Les indemnites journalieres et la subrogation de l'employeur » et « La CSG des revenus
de remplacement » (`xml/L321-1.xml` … `xml/R433-3.xml`, `xml/L136-1-2.xml`, `xml/L136-8.xml`).

- **L323-1, R323-1, 1°** : l'indemnité journalière est « due pour chaque jour ouvrable ou non », à partir du « quatrième
  jour de l'incapacité de travail ». **L433-1** : celle d'un accident du travail, dès « le premier jour qui suit l'arrêt du
  travail consécutif à l'accident » (la journée de l'accident est « intégralement à la charge de l'employeur ») ;
  **R433-1** et **R433-3** : 60 % du salaire journalier, puis 80 % « à partir du vingt-neuvième jour ».
- **L323-4, R323-4, R323-5, R323-9** (R323-9 relevé le 03/11/2026, `articles-20261103.md`) : son montant, la moitié de
  1/91,25 des trois dernières paies d'un salaire réglé mensuellement, chacune dans la limite de 1,4 SMIC du mois, et au
  plus 1/730 du plafond annuel. Le moteur le **propose** sur l'arrêt d'une maladie avant le relevé de la caisse
  (`maintien::ij_de_principe`) ; la personne le garde ou le corrige au relevé.
- **L433-2, R433-1 à R433-4** (L433-2, R433-2, R433-4 relevés le 03/11/2026) : l'accident du travail, 60 % puis 80 % de la
  paie du mois précédent / 30,42, plafonnée à 0,226 % du plafond annuel (R433-2, version du 02/10/2026 ; Ameli dit le
  plafond valoir pour les accidents survenus à compter du 1er novembre 2026 et donne les maxima, 65,17 et 86,90 €,
  `../accident-du-travail/ameli-reglementation-atmp-1er-novembre-2026-20261030.md`), et l'indemnité au plus le gain
  journalier net du taux forfaitaire de R331-5 (relevé le 03/11/2026), fixé à 21 % (`../arrete-2013-taux-forfaitaire-r331-5/`). Le moteur le
  **propose** pour un accident du travail ou de trajet, ou une maladie professionnelle, survenu depuis le 1er novembre 2026
  (L461-1, relevé le 03/11/2026 : la date de la première constatation médicale vaut date de l'accident ; `maintien::ij_de_principe`) ;
  pas pour un accident d'avant (la version d'avant de R433-2 n'est pas déposée).
- **R323-11** : l'employeur qui maintient le salaire en totalité est « subrogé de plein droit » ; en partie, il l'est à
  condition que le salaire maintenu soit au moins égal aux indemnités.
- **L136-8, II, 1°, b** : la CSG des indemnités journalières est de 6,2 %, dans la version en vigueur (`xml/L136-8.xml`,
  du 27/06/2026 ; la même, imprimée de Légifrance par Léandre le 03/11/2026 : `legifrance-css-L136-8-20260627.pdf`/`.txt`)
  comme dans les deux versions de 2025 (`xml-anterieur/L136-8-20201216.xml`, en vigueur du 16/12/2020 au 01/07/2025, et
  `xml-anterieur/L136-8-20250701.xml`, du 01/07/2025 au 31/12/2025 ; LEGIARTI000042683546 et …051287395, relevées dans la
  livraison LEGI du 02/01/2026, qui les republie avec leur date de fin ; la seconde aussi imprimée de Légifrance par Léandre,
  `legifrance-css-L136-8-20250701.pdf`/`.txt`). Entre les deux, seul le I, 3° change (6,2 % puis 7,2 % pour la contribution
  de L136-7-1, art. 32 de la loi n° 2025-199, en vigueur le 01/07/2025) : le II, 1°, b ne bouge pas. La CRDS de 0,5 % :
  `../ordonnance-96-50-crds/`.

**Où le moteur s'en sert** : `atelier/sources/engagement/src/maintien.rs` (les jours que la caisse indemnise, la subrogation),
`logiciel/donnees/permanents-base.json` (section `indemnites_journalieres`).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20260919.md` : `b5e15225c72f130ae44b26bafc47756756a827793545c1290f266ea350acee01`
- `articles-20260921.md` : `a62c1dfd90be51b251f2126afbdbe4596d652851c361d200cf5a566e21631059`
- `articles-20260923.md` : `a580ef7b491b6bdb8cef68417edcff847b414662c3054b605245b559a1bb64eb`
- `articles-20260924.md` : `f932002f82b3a766c35f7394e786b4f9baf39378768f490cf644b845b3392d33`
- `articles-20261010.md` : `e460aa722f7c392d3a9913304509be5be4f794633c84c675f61d556bdef38b36`
- `articles-20261011.md` : `5b8b8995e1102d28fba34709803add331c9b2d05e027ee40acfba4511fb39210`
- `articles-20261013.md` : `aef1ff5ed8a5ccfa6a793d7d2030ad81d3bd428d469dbb0d86b10f8ea19e7b9c`
- `articles-20261015.md` : `decf70df79a023937f249e3a649b1aea907f2a05495c54a7f6e83581960d914f`
- `articles-20261029.md` : `9c06fddb37bb511de2c022201ca27d033f88d8c0f19a56ddc9e22589d35d9e4e`
- `articles-20261030.md` : `1370db390e43d004c0197cef39d94d638e978feb9c63150fe5738a0ec9034c90`
- `articles-20261102.md` : `dcf9b7b7164a77dc8d4203799ed77f425d80279216edd3f17fefdaf1df438027`
- `articles-20261103.md` : `7618ce7c13dd17df9585ded4a8147405f2670d69858d4dcaec304d737fc4b57b`
- `articles-20261110.md` : `ec305c16ca578a0ce714273d5558a151dde47467a66461c8d164182d308cf206`
- `legifrance-css-L136-8-20250701.pdf` : `c00cfb281af77488f3f6976b41cc032a891e69ba0d317535f7714e77a4e01b71`
- `legifrance-css-L136-8-20250701.txt` : `97c02bc78eb90335bfbfd7d03889ec873b24c227f8b040cb45286255e384ad1e`
- `legifrance-css-L136-8-20260627.pdf` : `1f8d636b432bab0a9f96659d3c986d21e33bcb9abe9a64d469d5902b7722c627`
- `legifrance-css-L136-8-20260627.txt` : `75dd8f1c2405b0f6c156dcd185a73a0ebbc1f3f20da16e4792e7e4b54da5fb83`
- `releve.json` : `154673bca5d0e185f40a18f3d2c257418907dc1fc5704a456b39c9e2101ed3c3`
- `xml/D241-10.xml` : `5dfccfbcb9f7f5e0b513aef3ab317e8e3a1d11c3676329a01ea2e53969e8c593`
- `xml/D241-21.xml` : `e909234c7195bb9192381322628a32901e95864ffed79a3037473c9dab4a3c41`
- `xml/D241-22.xml` : `120bf2e0cf7385962c153094cffeff08cffe1976207efc44e54808c0281a358d`
- `xml/D241-24.xml` : `e118ec1d73138729fde4e4e506caa4d058a2226562bd137c776d161f9ed81e73`
- `xml/D241-25.xml` : `22acaf6d2733420de30e8c1744ccb5bc932b1d8f86b218fdcfc63d685799c025`
- `xml/D241-7.xml` : `b5ad91c8acb7b6f2e283f30eeb1dccce86565d2520c81815e020163420521a84`
- `xml/D242-17.xml` : `1e13d588ff1c2efe120601540aaa2d5777c5997a8807c9a50268ca40ea80f134`
- `xml/D242-18.xml` : `3cce1e93f9920a22ab715fa4836a5b6c5f989593928dcd44e5060e24f6ad3bc4`
- `xml/D242-19.xml` : `b4fe7db5c412a24e2c099eede68a6ec95893066a5b36b1fdedafa6eaf76c6551`
- `xml/D242-4.xml` : `169637e3ada36ab646a0e9031d7fb34a7326a08430aeeeb37be11ad64b298fda`
- `xml/D242-6-1.xml` : `50fc220ee4f2223e29dad81ef7fd30a3db0a227e6d860cc0c393b904c23cf853`
- `xml/D242-6-14.xml` : `5af322b4e2c67294a10c75ab72e94a8a45d8ece24692864a093e0d938393a7ed`
- `xml/D242-6-15.xml` : `732dc87f93aa792292d40d9cac308f2292c6b76c8dc26b5f7a3eb408bf6afac9`
- `xml/D242-6-17.xml` : `e829356f6e1ef145df489d3f7618b175824fe2495c941260b0184f411d03da03`
- `xml/D242-6-2.xml` : `c5c217186544ce99b71c6ee758586df9ca0ae42eb221b0c468f5e21c80bc24c9`
- `xml/D243-0-2.xml` : `7c27eff1591dba2898efe0889099b63c383ab018788998a1a65e7557e71d0fad`
- `xml/D243-0-3.xml` : `95f17d387045b76ab04a601c5058ebde49b4b9465ed17efb02d0896d872ad3a1`
- `xml/D331-7.xml` : `df043506bd946bc3c1e3bda9aa3e2199f1d1cdcd2751b4a8e866aa4b60952ad3`
- `xml/D911-1.xml` : `45101535dde6fb43cbf6bd12d2485fd7b8f1b64ef1b832d69016dcbfc6f6079c`
- `xml/D911-2.xml` : `db17d1df851921b14de3a414faaa5e10f43a9177e54760c748f590ffeabcf76b`
- `xml/D911-3.xml` : `8e62dba657d76e38a79c0ad152cde1ee00cd37a300617913a86a308a707a0a2b`
- `xml/D911-4.xml` : `6ebbcc525be0461845c8aa024876bde7d885d1d8c23b9184c3b28eeb8dc696f8`
- `xml/D911-5.xml` : `a0d4ee186143cc7a31fab2fdddc216e03a47947970b728f55f6e46f892c10aec`
- `xml/D911-6.xml` : `05bd99cf6aff1fbe09e955d977af46ffcafe6e881a425dfc9be45f12ba4192f5`
- `xml/D911-7.xml` : `13c0c80d58f7f4ece554c0a12735c9225dfaed988fd31bcb8a2cf924961cf017`
- `xml/D911-8.xml` : `6634cd532301255724e0a1e78ac3db38a7c5eb9d36b90ef87519dd102af2ccae`
- `xml/L130-1.xml` : `819f8e979b20bfe4ee7b10e32c5a348e91253f48745cb755fdf1b061ecb557da`
- `xml/L133-10.xml` : `d4479e9b9e3d99862125c21f567eb7b3861400ed10e4cf7c982561780a9ee05d`
- `xml/L136-1-1.xml` : `b2e746dd1fd7f4b5333bc90cd078294a2e123c1141bdcd07d5f395d90bc12fe6`
- `xml/L136-1-2.xml` : `8ad74c26c15ad0ede69793eac07c6b3f40587460d5647fe4a9a6ecf1a763d433`
- `xml/L136-2.xml` : `6e84db1b8828a450b08bd02cc7c09f5dbf87fa444e873d270fbad39bac5f0c05`
- `xml/L136-8.xml` : `91799cceb9b9ccf0a0d5f04d66e507103c566d5c378a0a8470a147e79a28696f`
- `xml/L137-12.xml` : `3e58ad5ce2392f60e1b6211bb75fba0dd5902fbd6dadd5058f351ab99a91845b`
- `xml/L161-17-2.xml` : `aec2d7d984e9ad3891365e91f1a9ba1c4c459ce44959487db29464fdf2950bd3`
- `xml/L241-13.xml` : `c19fc46783932b165cfb275e15e748c62bc3333bfda416aac3c7d1ebe61458be`
- `xml/L241-17.xml` : `c3d09f8b27d7daa7dd56e449018bb7df2bd161b3aba6a7b71b687f442f483b05`
- `xml/L241-18-1.xml` : `e132f8df8279a758dacad4ebcf0c858d7c5df5c4730c0d1f4572e0c0044c3556`
- `xml/L241-18.xml` : `4c284d5de59c142da81544521eb5511ddbfef5341301c1b432b987c1b09e2bc0`
- `xml/L241-3.xml` : `f24a47a718341114ed58d5d347d73421a343ddda2f51c3de23e245d594c2ee28`
- `xml/L242-1.xml` : `04b88310983895d8a9cb264179eaace3aee891c76fa557dd477c393915cf5fe1`
- `xml/L242-10.xml` : `60a9f4bf7fb2ade57e677026e22157e7d09120edb136dedd646ce10c4ea459f3`
- `xml/L242-8.xml` : `47b5e90dce23f78e5ee17222617d7d5047653bd41baef57e597323d109f7456b`
- `xml/L243-1-3.xml` : `8a1777ff85bb9f3b52763131b277eb60a08a2b30a2ba2d3b1688bcb3908be454`
- `xml/L321-1.xml` : `5c6650eadf1d58b9b115d520efbee0658df51f00d4408ed189b22f2c997acd40`
- `xml/L323-1.xml` : `f339717713c1ad57358165606352d6047f5455596537997b543d7e7ab0a06bef`
- `xml/L323-4.xml` : `0f4199ed507e2ad9702a9e49fec0e6b72b16842c01609c317360a5ceabef123e`
- `xml/L331-9.xml` : `3c18c4e4fd05ea59c0dc64d2c7cb90e3517081ebabb1368337d25e55e15a4204`
- `xml/L351-1.xml` : `83180abaf071b2009c7410028f5be6cc9b4b7bd2dcdc5e9c4f3f5e5d3935f19b`
- `xml/L351-8.xml` : `54073e0d57ef8baa6303171eb8a5d955a4cc93f4dcb86b5739fc388e08f164e0`
- `xml/L411-1.xml` : `167f6d7b209c3068f13bfec8d2004a3f8c7f8234b87d6d63bb6f5acb3ad163d8`
- `xml/L411-2.xml` : `4ecf3a47648b85a780d97ed7ebcd2af89d16615282a07c15d9a8d08cb99cd4f7`
- `xml/L433-1.xml` : `55c0a02027db6505b5bf11d7a6fe3b772ef439e3de5e70d4468082b3c8c12d98`
- `xml/L433-2.xml` : `8aa4f9df8197d845f70c9ba5a7ec6e692865735aa1344e8d5fa53465569559db`
- `xml/L441-1.xml` : `a0de00d5b58d7be97dcadb78b91ea9847de4bd4d29896b9fbd32fcae079da10d`
- `xml/L441-2.xml` : `e6e5f60feea5de06856784499849d48d5aa2c5fd142c30184bf9a2dec3361ded`
- `xml/L441-4.xml` : `a34a09941b8c55006f2d21e6992058af147cd5cf3afdcd9112817566225dde4f`
- `xml/L441-5.xml` : `4017c53e693acca690a87a0f520b56e73f6943ef681c68623600eb6c4d709427`
- `xml/L461-1.xml` : `5baaf1ed4c914544ca29cf57e389547b6549206ff25d5ef97a67a99804dc10d2`
- `xml/L461-5.xml` : `ac6343dcee445034fe96e72c893ca52f7d8683e7d7157a13d9a4af72d3a9f123`
- `xml/L911-1.xml` : `ddfa71fc0b2268721d124bf6334bda50f44b3ae3e22235f539351986ddd32faa`
- `xml/L911-2.xml` : `8a9f53608511e2a885aaed1aee4c02ba88d52779dae7fb9c2d8009dd04a15b25`
- `xml/L911-7.xml` : `72b9ca069d468cdae3c7a0f1bebd3cff0c4c46344cd3e16cf8ed7a81ed795486`
- `xml/L911-8.xml` : `001065a50d6c78e091e20b2cfa2aa47adeb5d732a87b89dbed45ff6b06975c09`
- `xml/R130-1.xml` : `cae41072bc0c2a6cb957fc3b1b606551b241c2b198e506fe4b222a0227f07e06`
- `xml/R133-13.xml` : `24a3108ff435086cc3bf2b0286bcad34719b63f35037ce8e1976ea2cac2692c3`
- `xml/R133-14.xml` : `c684487e204559d0255fa59396b7be9614d16fcb74240349c8832e7a5381c985`
- `xml/R242-1-1.xml` : `47148e546a61b398bfbefd9c9e71952c41a9b9030f70933a198ff98b8a3a43b3`
- `xml/R242-1-2.xml` : `3b17d327e70b150cdea0cd83e584ce1151264150ccf31486a93fda700ead9156`
- `xml/R242-1-3.xml` : `3e55cc09005693df8ec03300491d1e05ea956c88290ecabfc702a05739be59df`
- `xml/R242-1-4.xml` : `0ba7c0aa27cfda49c8bb1554c4982cf24bc16b9e2051173ed9bbcdf12264ade1`
- `xml/R242-1-5.xml` : `8925e3e0f25c2b46cd975e4f568fbacf2f79c0739a6799c77806fc08e1c794c3`
- `xml/R242-1-6.xml` : `7084a4559cf8ff5742591b879f19312c127ff906eb78fbae391d71b9844f9d7e`
- `xml/R242-1.xml` : `9d6dbcec05776cf545e679cbc3b156d288221bf282b6334fb90cb51f7285f216`
- `xml/R242-2.xml` : `39986878cb3f02e1aa76b18086053e226cf5e2d4e13ad9efeb30e7a549b620c8`
- `xml/R323-1.xml` : `c76d572baeeb6d3dd93903fb884201303e7dc8213771fb929ee2adcc951c7785`
- `xml/R323-10.xml` : `35ab4a608c6de8400da0f75b0544a84f0f719cccf556e92e15dc4cac79bcdac3`
- `xml/R323-11.xml` : `ab80c24ba8bc08124ed875fd84d60676662734b38e434a243e10f5e0ce3147a2`
- `xml/R323-4.xml` : `d3965f539a99d483443244af1ec6833f418bac919c95aaf8ad7f0791488a44ae`
- `xml/R323-5.xml` : `b88d1ad11686d4ddc46161838713a5eda9ba5410b1e495c011bcff1518407a69`
- `xml/R323-9.xml` : `983724ef279db7f06c4455150b6d4f25c29d69822376685010a03cb033de1f95`
- `xml/R331-5.xml` : `7673f43fc711ef4d32341e22734c02e0d3382f4772e6c8df334b432c75d97789`
- `xml/R433-1.xml` : `423eed05818feaf8ec5ad6791a8a53b71248bab5d6d74ad7a95ce01208a85f30`
- `xml/R433-2.xml` : `88e9180016938e55882a0e1a2ad074b81c4a0bbdc338618432e8db1596448ba8`
- `xml/R433-3.xml` : `da924e24eef597cc0bd5286def7b88c0abf2b59308ba66d2ad1705b0fdd206e6`
- `xml/R433-4.xml` : `d5dbb23f7905fca67202b8167fd1998e3d8f55a1deeae6e855f7dd0cf049b9cd`
- `xml/R441-1.xml` : `0ba4bfdceb2928fdd08fda243e6d3e8c2ffac5b45e768b2ba9b03f88fca9ec6b`
- `xml/R441-2.xml` : `90c976ed16e64763722b955635ef2f6c59793a13b2f59b86128ca01ef1b2b5db`
- `xml/R441-3.xml` : `0962a8e429c27d00f5a9e8c70ce0fab4f02ad6a84415c50b1877d83f95a45941`
- `xml/R441-4.xml` : `22e81e25be80c9f950d4b7bc07883eb476949a5ff105e50fc660586a90d33f80`
- `xml/R441-6.xml` : `cff284f17fb98127a4c5b5d663a7880b3782781b8256d63a2d470b57a329ee7c`
- `xml/R441-7.xml` : `df5613aa51afc9e888bdbdef0443103166509c4e568c415082f77f9d5b46b2eb`
- `xml/R461-5.xml` : `10a2a30cc087ee59c17be072b9d58c04bf7c1d83f33723d50637ccef3f168f10`
- `xml/R844-1.xml` : `1930dfb7c5439caedab36f3b9b62df12d72fa51ef3936ad7c7de376ead075039`
- `xml-anterieur/D242-6-15.xml` : `1c7d2a3ce4c0f6862c258b1bb420ac7af4e7c2d8dc1512670153e5c4cf4552fd`
- `xml-anterieur/D242-6-17.xml` : `db854fe741d9cddf09220cd1a8dcf3420c16ef9e17378bf9e57c8b6c490569e3`
- `xml-anterieur/L136-1-1.xml` : `bcd33f519488f8de0e20ce20271cae3d8c2438beef71c97c982e4cac676131c3`
- `xml-anterieur/L136-8-20201216.xml` : `0248ca93171c920827fb0dd98d18412a6da9d89fe57f8128c7779aa998d37a1d`
- `xml-anterieur/L136-8-20250701.xml` : `582acbe8dff1a6fe3abbc6853d30ae79b34b3451cb8b00aa458012b96f3de541`
- `xml-anterieur/L137-12.xml` : `ca1848b99a00afc5455d141a676e7d9fcd9803e9854f49b74798ac765c7bd5bd`
- `xml-anterieur/L161-17-2.xml` : `111fd29f2814dae3fae89abcc9bd070d5305e39dd4adad9bc829873296c66dff`
- `xml-anterieur/L242-1.xml` : `01cf84ddf8d1e127229fbe83a467936d36045a23c6ae19a5f1a92ef3edc474a4`
