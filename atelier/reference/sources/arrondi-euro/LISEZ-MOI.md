# L'arrondi à l'euro des montants déclarés : la fraction de 0,50 est comptée pour 1

Pourquoi le bordereau Urssaf (blocs 22 et 23), les versements (bloc 20) et les lignes de correction de la DSN s'arrondissent à
l'euro **la moitié vers le haut** (`atelier/sources/paie/src/bordereau.rs`, `euro` ; `src/dsn.rs`, `rond`). Déposé le
13/10/2026, lot U5. Jusque-là le bordereau arrondissait la moitié vers le haut et `dsn.rs` au pair (comme `round()` de Python).

## Le texte

**CSS L. 133-10** (`LEGIARTI000029956107`, en vigueur depuis le 01/01/2015), relevé LEGI du 13/10/2026 :
`../code-de-la-securite-sociale/articles-20261013.md` (l. 421-423), XML : `../code-de-la-securite-sociale/xml/L133-10.xml` :

> Le montant des cotisations et contributions sociales et de leurs assiettes déclarées aux organismes de sécurité sociale
> en application du présent code, du code de l'action sociale et des familles ou du code rural et de la pêche maritime est
> arrondi à l'euro le plus proche. La fraction d'euro égale à 0,50 est comptée pour 1.

C'est la règle. **CSS L. 130-1 ne la dit plus** : sa rédaction en vigueur (depuis le 01/01/2025) ne traite que de l'effectif ;
le guide Urssaf de la DSN la cite encore (voir plus bas).

## Les textes qui la répètent ou l'appliquent

- **Guide Urssaf « Comment déclarer et régulariser les cotisations Urssaf en DSN »**, 04/06/2026
  (`../editeur-dsn/guide-urssaf-regularisation.txt`, l. 580-592) : « 1.6. Gestion des arrondis pour les déclarations de
  cotisations. Les arrondis sont effectués selon la règle de l'article L130-1 du Code de la Sécurité sociale, c'est à dire à
  l'euro le plus proche […] Seules les données agrégées se voient appliquer un arrondi […] En revanche les données nominatives
  ne sont pas concernées par la règle d'arrondi […] Tous les montants des blocs "Cotisation agrégée - S21.G00.23" doivent être
  arrondis à l'euro le plus proche, qu'il s'agisse de montants d'assiette ou de montants de cotisations ; Les montants de
  cotisations des blocs "Versement organisme de protection sociale - S21.G00.20" et "Bordereau de cotisation due - S21.G00.22"
  supportent également la même règle d'arrondi. » Le guide renvoie à L. 130-1 par un numéro d'article que la loi a déplacé :
  c'est L. 133-10 qui porte aujourd'hui les mots.
- **Norme DSN 2026.1**, `../../norme-dsn/texte/bloc-S21.G00.20.txt` l. 204 et `bloc-S21.G00.22.txt` l. 263 : « Urssaf :
  montant, arrondi à l'entier le plus proche (les deux décimales étant valorisées à zéro) » ; l. 194 : « DGFiP : montant,
  arrondi à l'entier le plus proche » ; l. 200-201 : « IRCANTEC : montant arrondi à l'euro le plus proche selon la règle
  suivante : jusqu'à 49 centimes, à l'euro inférieur et à partir de 50 centimes à l'euro supérieur ». La norme ne dit le demi
  que pour l'Ircantec.
- **Unédic, règlement général annexé à la convention du 15/11/2024, art. 53**
  (`../chomage-permanents/unedic-reglement-general-convention-20241115.txt`, l. 2225-2226 ; mêmes mots dans les annexes VIII et
  X, `../plafond-intermittents/`) : « Le montant des contributions est arrondi à l'euro le plus proche. La fraction d'euro
  égale à 0,50 est comptée pour 1, conformément aux dispositions de l'article L. 133-10 du code de la sécurité sociale. »
- **CGI art. 1724** (`LEGIARTI000006312863`, `../code-general-des-impots/articles-20261013.md` l. 88-91) : « La liquidation de
  toutes sommes à recevoir, à quelque titre et pour quelque cause que ce soit, est arrondie à l'euro le plus proche. La fraction
  d'euro égale à 0,50 est comptée pour 1. »

## Ce que le moteur en fait

`bordereau::euro` est le seul endroit qui arrondit à l'euro : la moitié vers le haut (`MidpointAwayFromZero`). Les lignes 23
(`montant_ligne`), les lignes de correction du bordereau, les assiettes du CTP 900/921/381, le montant d'un versement 20 à
taux sur le brut, et le versement à la DGFiP s'en servent. Le nominatif (blocs 78, 79, 81) n'est pas arrondi (guide, § 1.6).

## Le centime (relevé le 07/11/2026)

**Règlement (CE) n° 1103/97 du Conseil du 17 juin 1997, art. 5** (JO L 162 du 19/06/1997, p. 1 ; CELEX 31997R1103).
EUR-Lex ne répondait pas au conteneur (défi anti-robots, HTTP 202 vide) ; l'Office des publications a servi le texte :
`https://publications.europa.eu/resource/celex/31997R1103` avec `Accept: text/html` et `Accept-Language: fr` (HTTP 200,
`text/html`, 15 950 octets). `eurlex-reglement-1103-97-introduction-euro-19970619.html` est tel que servi ; le `.txt` en
transcrit les extraits (considérant 11, art. 4 et 5).

> Les sommes d'argent à payer ou à comptabiliser, lorsqu'il y a lieu de les arrondir après conversion dans l'unité euro […],
> sont arrondies au cent supérieur ou inférieur le plus proche. […] Si l'application du taux de conversion donne un résultat
> qui se situe exactement au milieu, la somme est arrondie au chiffre supérieur.

Ce que le moteur en fait : `socle::num::eur` arrondit à deux décimales, la moitié vers le haut (`MidpointAwayFromZero`), jamais
au pair ; elle porte le calcul de chaque montant du bulletin (`pct`, les rubriques). Test : `socle/src/num.rs`,
`eur_arrondit_le_demi_vers_le_haut`.

**Ce que le texte ne dit pas (inférence, dite telle).** Il fonde le centime le plus proche et le demi vers le haut, mais pour
les sommes arrondies *après conversion* dans l'unité euro (le résultat « de l'application du taux de conversion »), non pour
le produit d'une base par un taux de paie ; le considérant 11 réserve d'ailleurs les pratiques nationales d'arrondi des calculs
intermédiaires. Lire l'art. 5 comme la règle de tout montant du bulletin est une inférence, de même sens que celle de
l'euro (CSS L. 133-10). Il ne dit rien non plus d'un montant négatif : `eur` arrondit -0,005 à -0,01 (comme l'opposé), par
symétrie, comme le fait `bordereau::euro` pour l'euro (voir plus bas).

## Le centime : ce que disent les autres textes (relevé le 14/11/2026)

Question : un montant de paie qui tombe exactement sur un demi-centime (2,125 €) va-t-il à 2,13 ou à 2,12 ? Où le moteur
s'en sert : l'arrondi au centime, `socle::grandeurs` (`Montant::au_centime`) et `socle::num::eur`.

**La Cour de justice limite l'art. 5 du règlement.** CJUE (grande chambre), 14/09/2004, *Verbraucher-Zentrale Hamburg
c/ O2*, C-19/03 (CELEX 62003CJ0019), servi par l'Office des publications
(`https://publications.europa.eu/resource/celex/62003CJ0019`, `Accept: text/html`, `Accept-Language: fr` ; HTTP 200,
`text/html`, 63 852 octets) : `cjue-arret-c-19-03-arrondi-euro-20040914.html`, tel que servi, et sa transcription `.txt`.

> 34 […] ce règlement ne fixe que des règles minimales relatives aux arrondis de certaines sommes et laisse le soin aux
> autorités nationales de maintenir ou d'adopter des règles qui permettraient de mieux contribuer à l'objectif de neutralité
> du passage à la monnaie unique. Le libellé même de ce onzième considérant indique que les modalités d'arrondissage des
> sommes d'argent figurant dans le règlement nº 1103/97 n'ont pas pour objet de régir de façon exhaustive les calculs
> intermédiaires portant sur ces sommes.
>
> 36 L'article 5, première phrase, du règlement nº 1103/97 ne doit donc pas recevoir une interprétation large. Il ne peut
> viser […] que les sommes d'argent citées au point 29 du présent arrêt [les dettes d'argent et les sommes inscrites dans
> des documents comptables ou des relevés de compte], pour lesquelles des raisons pratiques […] imposent un arrondissage au
> cent le plus proche.

Le point 29 range parmi les « sommes d'argent à payer ou à comptabiliser » « toutes les dettes d'argent » : un net à payer,
une cotisation due en sont. Mais l'arrêt ne lit que la première phrase de l'art. 5 (le cent le plus proche), tranchée dans
le cadre de la conversion ; la troisième phrase, celle du demi, parle du résultat de « l'application du taux de
conversion ». **L'art. 5 ne fonde donc pas en droit le demi-centime d'un montant de paie calculé en euros** : c'est une
inférence, que la Cour ne contredit pas (le règlement « ne s'oppose pas » à un arrondi au cent le plus proche, dispositif,
point 2).

**La DGFiP dit le demi-centime vers le haut, pour le prélèvement à la source.** BOFiP, `BOI-IR-PAS-20-20-30-10-20230626`,
§ 180 (`https://bofip.impots.gouv.fr/bofip/11252-PGP.html`, redirigé vers `…/identifiant=BOI-IR-PAS-20-20-30-10-20230626`,
HTTP 200, 115 212 octets) : `bofip-boi-ir-pas-20-20-30-10-20230626.html`, tel que servi, et l'extrait transcrit `.txt` :

> Les montants obtenus sont arrondis au centime le plus proche, la fraction de centime égale à 0,50 étant comptée pour 1.

Il vise les montants ramenés à la grille mensuelle du taux par défaut (revenu divisé ou limites multipliées par un
coefficient de périodicité), pas une cotisation.

**La norme DSN** (cahier technique 2026.1, déjà déposé dans `../../norme-dsn/`) :
- `texte/chapitre-4-controles.txt` l. 244 (§ 4.4.7.1) : « Tous les montants sont exprimés avec deux décimales obligatoires
  et un séparateur de décimales qui est le point « . ». L'opposabilité de cette approche pour toutes les procédures fondées
  sur la DSN sera précisée dans les décrets à paraître. » Elle dit le centime, pas la façon d'y arriver.
- `texte/bloc-S21.G00.50.txt` l. 183 (`S21.G00.50.009`, montant du PAS) : « Il prend en compte la règle de l'arrondi sur
  les deux décimales après la virgule, en application du f) du 3° de l'article 39 C de l'annexe III au Code général des
  impôts. Exemple : Si le montant de PAS calculé s'élève à 120,5678 €, la somme à renseigner dans cette rubrique sera
  120,57 €. » L'exemple ne tombe pas sur un demi ; l'article cité n'est pas déposé (voir plus bas).
- `texte/bloc-S89.G00.89.txt` l. 67 (et `S89.G00.87` l. 61, `S89.G00.88` l. 67 ; mêmes mots en 2027,
  `texte-2027/`) : « La valeur indiquée comprend deux chiffres après la virgule. A titre d'exemple, une valeur unitaire d'un
  montant de 0,385 euro sera indiquée à "0.38", une valeur unitaire d'un montant de 0,386 euro sera indiquée à "0.39" ».
  **C'est le seul texte déposé qui tranche un demi-centime, et il le porte vers le bas** (ou au pair : 8 est pair) — pour
  le prix unitaire d'un titre (BSPCE, blocs S89), pas pour un montant de paie. Qu'il vienne d'un calcul en virgule
  flottante (0,385 s'y écrit 0,38499…) est une hypothèse, non un texte.
- Les blocs 20, 22, 23 disent l'euro (plus haut) ; aucun bloc S21 ne dit le demi-centime d'une cotisation nominative
  (blocs 78, 79, 81).

**Le Code de la sécurité sociale et le Code du travail n'ont pas de règle générale du centime.** Recherche dans
`../legi-entrepot/` (articles en vigueur, « arrondi » près de « centime ») : seulement des règles propres à un montant —
CSS L. 613-6-1 (« arrondis au centime d'euro le plus proche », micro-entrepreneurs, par dérogation à L. 133-10), CGI 81
(titres-restaurant, « au centime d'euro le plus proche »), C. trav. R. 6323-1 (CPF, « arrondi à la deuxième décimale, au
centime d'euro supérieur »), CSS D. 241-7-1 (un taux, « au centième le plus proche ») ; aucune ne dit le demi.

**Ce qu'on en tire.** Aucun texte déposé ne dit l'arrondi au centime d'une cotisation, d'une ligne de bulletin ou d'un
montant nominatif de la DSN, ni son demi. Quand un texte dit le demi, il le porte vers le haut : la loi pour l'euro des
cotisations (CSS L. 133-10), la DGFiP pour le centime du PAS (BOFiP ci-dessus), le Conseil pour le centime de conversion
(règlement 1103/97, art. 5). Appliquer le demi vers le haut aux montants de paie est **une inférence par analogie** de ces
trois textes, non un fondement au sens de `FONDER-UN-TAUX` : aucun ne vise ces montants. La norme DSN donne un exemple
contraire, pour un autre objet (les blocs S89).

## Ce qui est une inférence

- **Le demi pour la DGFiP** (`dsn.rs`, versement 20 du prélèvement à la source) : L. 133-10 vise « les organismes de sécurité
  sociale » ; la norme dit « arrondi à l'entier le plus proche » sans le demi. CGI 1724 le dit pour « toutes sommes à recevoir »
  du fisc : lire le versement du PAS comme une telle somme est une inférence.
- **Un montant négatif** (une régularisation qui diminue) : « comptée pour 1 » ne dit pas le signe ; le moteur arrondit comme
  l'opposé (-2,50 donne -3). Aucun texte déposé ne le dit (relu au lot U7, 13/10/2026 : L. 133-10, CGI 1724, le guide Urssaf
  § 1.6, la norme aux blocs 20, 22 et 23 — dont la règle de l'Ircantec, « jusqu'à 49 centimes, à l'euro inférieur et à partir
  de 50 centimes à l'euro supérieur », `bloc-S21.G00.20.txt` l. 200-201 — ne parlent que de la fraction). Raison de garder la
  symétrie : une régularisation qui retire un montant déclaré (-2,50 après +2,50) retire l'euro que l'arrondi avait ajouté
  (-3 après +3) ; arrondir -2,50 à -2 laisserait un euro. Prouvé, comme inférence, par
  `tests/calcul/bordereau.rs`, `un_montant_negatif_s_arrondit_comme_son_oppose_par_inference`.
- **Les 74 bordereaux réels retrouvés au centime** (`atelier/sources/paie/tests/donnees/bordereaux-reels.json`, 75 déposés par
  CoolPaie) sont un **constat**, pas un texte : ils concordent avec L. 133-10, ils ne le fondent pas.
- **Le bloc 54 des types 92 et 93** (parts patronales de santé et de prévoyance déclarées au nominatif, `dsn.rs`) s'arrondit à
  l'euro : ni L. 133-10 ni la norme (`bloc-S21.G00.54.txt` ne parle pas d'arrondi) ne le disent, le guide excepte le
  nominatif. La fraction de 0,50 y suit la même fonction que le reste de la DSN, faute de texte contraire.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `bofip-boi-ir-pas-20-20-30-10-20230626.html` : `9abd874cf6e64e1f8109601d694d1eb11c7528e2050c47ea0af3b59289c2f547`
- `bofip-boi-ir-pas-20-20-30-10-20230626.txt` : `ab42b93b56d27d3827395ece8ed1abeb5878e6b8692fd20fcfba0e777d55b776`
- `cjue-arret-c-19-03-arrondi-euro-20040914.html` : `14be9364b3c89911aad0b2a1c4bc5aa7a739c0449fe565c3fccb9d6cb5f9f162`
- `cjue-arret-c-19-03-arrondi-euro-20040914.txt` : `46393b08a9c27d721777b42a455e2dbd0ce64cb2d18b5c9e4d544f8598b58eca`
- `eurlex-reglement-1103-97-introduction-euro-19970619.html` : `1f11f46eaf67fa2eca78af2f25f5badf5ae0118562d5d64ed3f41b816736256a`
- `eurlex-reglement-1103-97-introduction-euro-19970619.txt` : `64432f17a25555cf485cccfeb883730d045d3a160d31a58840d618d4446c9b81`
