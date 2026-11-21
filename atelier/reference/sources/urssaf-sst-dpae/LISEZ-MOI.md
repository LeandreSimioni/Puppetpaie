# Le référentiel des services de prévention et de santé au travail (SPST)

## Ce qui est déposé ici

- **`urssaf-sst-dpae-20261121.csv`** (relevé le 21/11/2026) : le référentiel « sst_dpae » de l'Urssaf, tel que servi
  (UTF-8 avec BOM, séparateur `;`, fins de ligne CRLF, 411 lignes plus l'en-tête). Colonnes :
  `mt_code_ur;mt_lib_court;mt_lib;mt_adr_part_1;mt_adr_part_2;mt_code_post;mt_bur_dist`.
  - D'où : `https://open.urssaf.fr/api/explore/v2.1/catalog/datasets/sst_dpae/exports/csv?delimiter=%3B`.
  - **`urssaf-sst-dpae-meta-20261121.json`** : les métadonnées du jeu, servies le même jour
    (`https://open.urssaf.fr/api/explore/v2.1/catalog/datasets/sst_dpae`) : « modified » : « 2026-11-12T15:58:01+00:00 »,
    « records_count » : 411, licence ODbL, éditeur « Urssaf Caisse nationale ». C'est la version du référentiel.
  - C'est celui que le moteur livre (`logiciel/donnees/spst-base.json`, ci-dessous).
- **`urssaf-sst-dpae-20261023.csv`** (relevé le 23/10/2026, 409 lignes) : la version d'avant, gardée. D'une version à
  l'autre : MT162 (SANO, Paris) et MT311 sous deux Urssaf de plus arrivent, MT128 (SPSTI GAS BTP, Reims) disparaît,
  et plusieurs services changent de nom ou d'adresse (MT047, MT267, MT311, MT346, MT378, MT524).
- **`datagouv-spst-liste-20261023.html`** : la page data.gouv du jeu, « Services de prévention et de santé au travail
  (SPST) », Urssaf Caisse nationale. Elle dit que c'est la liste « qui doit être utilisée dans le cadre de la DPAE » (la
  description du jeu dans les métadonnées le dit aussi : « la liste des Services de Prévention et de Santé au Travail
  (SPST) qui doit être utilisée dans le cadre de la Déclaration Préalable à l'Embauche (DPAE) »).

## Ce qu'on en lit

- **Le code d'un SPST est dans `mt_lib_court`** (`MT161`, `MT176`, …), répété sous chaque Urssaf qui connaît le service
  (`mt_code_ur`) : 321 services distincts dans la version du 21/11/2026. Un même code y a partout le même nom (`mt_lib`,
  « 176 - THALIE SANTE ») et la même commune (`mt_bur_dist`).
- **La médecine interne** est `MT01`, « MEDECINE INTERNE ETABLISSEMENT », commune « SANS ». Elle figure une fois par
  Urssaf.
- **Thalie Santé** (ex-CMB, Centre médical de la Bourse) est `MT176`, « 7 RUE BERGERE, 75009 PARIS », présent sous 25
  Urssaf.

## La fiche consigne 3370 : le référentiel vaut pour la DSN

La fiche n° 3370 de net-entreprises, « Service de prévention et de santé au travail (SPST) en DSN mensuelle », est
déposée dans `../net-entreprises-base-connaissances/` (`net-entreprises-fiche3370-spst-dsn-mensuelle-20260826.*`) :
« La déclaration du SPST en DSN s’appuie sur le référentiel « sst_dpae » disponible sur le site internet de l’Urssaf »
(l. 10) ; « Les rubriques permettant de renseigner le SPST en DSN doivent être renseignées à partir des valeurs
disponibles en colonne « mt_lib_court » du référentiel (par exemple : MT189) » (l. 11). Le LISEZ-MOI de ce dossier-là
en dit le reste.

## Inférence, écrite comme telle

Le code de la DPAE (C_MT_DCL, « Code centre de médecine du travail », celui de `medecine_travail` dans la fiche) et la
valeur de S21.G00.11.025 viennent du même référentiel : la page du jeu le dit pour la DPAE, la fiche 3370 pour la DSN.
Le moteur en fait le même code `MTxxx`. **Aucun texte lu ne dit en ces mots** que l'identifiant de la DSN est le code de
la DPAE.

## Une discordance, laissée telle

**La médecine interne ne s'écrit pas pareil.** La norme DPAE v5.3 déposée (`atelier/reference/norme-dpae/
v5.3_FormatDPAE_EMAIL.txt`, l. 492-497) dit : « Les codes médecine du travail sont dans le document contenant les Codes
Service santé. Il est attendu ici un code du type MT+ 3 positions numériques. […] Si vous disposez d’un centre de
médecine interne à l’établissement, renseignez MT999 ». Le référentiel et la fiche 3370 (l. 19) disent `MT01`, et **`MT999` n'est pas dans le référentiel**.
Le moteur n'a rien changé à la DPAE : elle écrit `MT999` (la case de la fiche garde le code de la DPAE,
`droit::fiche::CodeMedecine::INTERNE`) ; la DSN 2027 écrit `MT01` (`CodeMedecine::identifiant_spst`), et la liste de la
case offre la médecine interne sous `MT999` (`CodeMedecine::du_referentiel`). Que `MT01` et `MT999` nomment la même
médecine interne est une inférence. À trancher le jour où une DPAE de médecine interne revient de l'Urssaf.

## Thalie Santé et les intermittents : ce que disent les textes déposés

Question posée le 21/11/2026 : les intermittents du spectacle relèvent-ils **obligatoirement** de Thalie Santé ?
Ce que disent les textes de nos conventions (`../kali-entrepot/`, `moteur-atelier releve-conventions`) :

- **3090, art. 11.8** (`KALIARTI000038665855`, en vigueur étendu depuis le 26/02/2019) : « Conformément au code du
  travail et à l'article 3.2 de l'accord collectif national interbranche relatif à la santé au travail des salariés
  intermittents du spectacle qui désigne le centre médical de la bourse, tout salarié doit bénéficier, selon une
  périodicité n'excédant pas 5 ans, d'une visite individuelle d'information et de prévention périodique. »
- **3090, art. 8.18** (`KALIARTI000028157405`) : la seconde visite annuelle d'un travailleur de nuit en CDD d'usage se
  demande « auprès du centre médical de la Bourse (CMB) ».
- **3097, art. 26** (`KALIARTI000028053482`, production cinématographique) : « pour les salariés des titres II et III et
  en vertu de l'accord interbranches du 29 juin 2009 précité, l'employeur doit obligatoirement cotiser au centre médical
  de la Bourse, pour assurer notamment le suivi médical de ses salariés embauchés en CDD d'usage ».
- **1285, art. XVIII.1** (`KALIARTI000046992879`) : l'affichage doit donner « les coordonnées du service de prévention
  et de santé au travail Thalie Santé (ex-CMB) » (prévention des violences sexuelles) ; **1790**, accord du 10 mai 1996
  (`KALIARTI000005804078`) : « l'attestation d'aptitude du Centre médical de la Bourse (CMB) pour les salariés
  intermittents du spectacle ».

**Ce qui manque** : l'accord interbranches du 29 juin 2009 lui-même et son arrêté d'extension ne sont pas déposés.
Legifrance refuse le conteneur (403), et la seule copie trouvée (un site d'organisation professionnelle) ne sert plus
le PDF. Deux sources secondaires, non déposées, disent l'accord étendu par arrêté du 17 mai 2010 **à l'exception de son
article 3.2** : celui-là même que la 3090 cite comme désignant le CMB. Rien de déposé ne dit donc nettement qu'une
compagnie de spectacle vivant (3090, 1285) doit adhérer à Thalie Santé pour ses intermittents ; la 3097 le dit, pour la
production cinématographique seulement.

**Ce que le moteur en fait** : il ne propose pas Thalie Santé ; il la range en tête de la liste
(`declarations::spst::THALIE_SANTE`). L'accord du 29 juin 2009 et l'arrêté du 17 mai 2010 sont à relever (Léandre).

## Où le moteur s'en sert

- **`logiciel/donnees/spst-base.json`**, tiré du CSV du 21/11/2026 par
  `moteur-atelier extraire spst atelier/reference/sources/urssaf-sst-dpae/urssaf-sst-dpae-20261121.csv
  logiciel/donnees/spst-base.json --remplacer` (`atelier/sources/atelier/src/extraire/spst.rs`) : un service par
  identifiant, son nom et sa commune. `atelier/tests/atelier/referentiels.rs` vérifie que le fichier livré est ce que le
  CSV déposé donne ; `logiciel/donnees/sources.json`, clé `urssaf-sst-dpae`, dit sa version et son condensat.
- **La case de la fiche Entreprise** se choisit dans cette liste, par le nom du service : `declarations::spst`
  (`BaseSpst::choix`), posé dans la description de la case par `declarations::champs` (lecture de la table).
- **La DSN 2027** écrit l'identifiant en S21.G00.11.025 (`declarations::dsn::spst_ecrit`), et une DSN 2027 reprise le
  rend à la case (`declarations::dsn_lue`).
- **Un code absent du référentiel** (tapé en ligne de commande, ou d'une version d'avant, comme MT128) se lit quand même
  s'il a la forme `MT` et trois chiffres : ni la norme DPAE ni la fiche 3370 ne donnent de contrôle qui le refuse. La
  page le montre tel quel, hors de la liste.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `datagouv-spst-liste-20261023.html` : `799e34c7923c658a71490b7052c61e0941625f55b88489a4f156429bdfb3eccb`
- `urssaf-sst-dpae-20261023.csv` : `04dc77d8bb22cf99e64901c85ac6c520fcc2f3ae0806554503f5345e0d563ecb`
- `urssaf-sst-dpae-20261121.csv` : `c2f717e90b3f42728dcbac667c8a456b9f93b227ab24ebdf71c9290ae54baf33`
- `urssaf-sst-dpae-meta-20261121.json` : `066b8353effabe77062b00ec18c8474385f949223db15005f8b0a5c98416b759`
