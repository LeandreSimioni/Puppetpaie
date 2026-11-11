# Code du travail : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `atelier/outils/relever_les_codes.py`**
(`moissonner`, puis `rapport`) depuis la base LEGI de la DILA ; ils ne se retouchent pas à la main. Les articles voulus sont la
liste `TEXTES["travail"]["voulus"]` de l'outil ; le dernier relevé est le plus récent des `articles-*.md`. Ce fichier dit, par
sujet, ce qu'on en lit et où le moteur s'en sert.

## La fin d'un CDD : à son terme, ou avant (relevé du 13/10/2026, lot U5)

`articles-20261013.md`, section « La fin d'un CDD : a son terme, ou avant » (XML : `xml/L1243-1.xml`, `xml/L1243-2.xml`,
`xml/L1243-5.xml`), et, dans la section « Le preavis et l'indemnite de licenciement », L1234-1 (`xml/L1234-1.xml`).

- **L1234-1** (`LEGIARTI000006901112`, depuis le 01/05/2008) : « **Lorsque le licenciement n'est pas motivé par une faute
  grave**, le salarié a droit : 1° S'il justifie chez le même employeur d'une ancienneté de services continus inférieure à six
  mois, à un préavis dont la durée est déterminée par la loi, la convention ou l'accord collectif de travail ou, à défaut, par
  les usages […] ; 2° […] entre six mois et moins de deux ans, à un préavis d'un mois ; 3° […] d'au moins deux ans, à un préavis
  de deux mois. » C'est le préavis du **licenciement**, par la première phrase de l'article elle-même ;
  les mises et départs à la retraite y renvoient (L1237-6 et L1237-10, `articles-20261013.md` l. 325 et 345).
- **L1243-5** (`LEGIARTI000029946316`, depuis le 22/12/2014) : « Le contrat de travail à durée déterminée **cesse de plein droit
  à l'échéance du terme**. Lorsqu'il est conclu en application du 6° de l'article L. 1242-2, il prend fin avec la réalisation de
  l'objet pour lequel il a été conclu après un délai de prévenance au moins égal à deux mois. » Le texte ne donne **aucun
  préavis** à un CDD qui arrive à son terme ; le délai de prévenance de deux mois est celui du CDD **à objet défini**
  (L1242-2, 6°), que le moteur ne porte pas.
- **L1243-1** (`LEGIARTI000029946319`) : sauf accord des parties, le CDD ne peut être rompu avant l'échéance du terme qu'en cas
  de faute grave, de force majeure ou d'inaptitude ; **L1243-2** (`LEGIARTI000031087476`) : le salarié qui justifie d'un CDI
  peut le rompre avant le terme, avec un préavis d'« un jour par semaine », deux semaines au plus. Ce préavis-là est celui d'une
  **rupture anticipée à l'initiative du salarié** (motif de rupture 037 de la DSN, `bloc-S21.G00.62.txt` l. 265 ; le 036 est celui de l'employeur) ; le moteur ne le calcule pas.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.motifs_sans_preavis["031"]` (fin de CDD arrivé à
son terme : aucun préavis, cite L1243-5) ; `atelier/sources/paie/src/permanents.rs` (`preavis_mois`, `raison_sans_preavis`) ;
les phrases de `routes_listes.rs` et `routes_ouverture.rs` (`ce_qui_manque`, « La loi ne donne aucun preavis pour ce motif »)
et `solde.rs` (un préavis déclaré non effectué sur un motif sans préavis). Le bloc S21.G00.63 d'un signalement de fin de CDD se
déclare avec le type « 90 - pas de clause de préavis applicable » (norme DSN, `atelier/reference/norme-dsn/texte/bloc-S21.G00.63.txt`).
Test : `tests/routes/routes_listes.rs`, `un_cdd_arrive_a_son_terme_n_a_aucun_preavis`.

**Inférence** : la lecture de « motif de rupture 031 » comme « CDD arrivé à son terme » est celle de la norme DSN
(`bloc-S21.G00.62.txt`, « 031 - fin de contrat à durée déterminée ou fin d'accueil occasionnel »).

## La rupture d'un CDD avant son terme, motif par motif (relevé du 13/10/2026, lot U7)

Ajoutés à `TEXTES["travail"]["voulus"]` (`fin_de_cdd`) et relevés par `moissonner` (0 livraison nouvelle sur 458, « manquants :
aucun »), puis `rapport travail` : **L1243-3**, **L1243-4**, **L1226-4-2**, **L1226-4-3**, **L1226-20** (`articles-20261013.md`,
même section ; `xml/`).

- **L1243-3** (`LEGIARTI000006901214`) : la rupture à l'initiative du salarié « en dehors des cas prévus aux articles L. 1243-1 et
  L. 1243-2 ouvre droit pour l'employeur à des dommages et intérêts ».
- **L1243-4** (`LEGIARTI000024026880`) : la rupture à l'initiative de l'employeur « en dehors des cas de faute grave, de force
  majeure ou d'inaptitude » ouvre au salarié des dommages et intérêts « d'un montant au moins égal aux rémunérations qu'il aurait
  perçues jusqu'au terme du contrat » ; al. 2 : la force majeure d'un sinistre donne « une indemnité compensatrice dont le montant
  est égal aux rémunérations qu'il aurait perçues jusqu'au terme du contrat ».
- **L1226-4-2**, **L1226-4-3**, **L1226-20** : l'inaptitude d'un salarié en CDD ; la rupture « ouvre droit [...] à une indemnité »
  (au moins celle de L1234-9, le double pour une inaptitude d'origine professionnelle), versée comme l'indemnité de précarité.

**Ce qu'ils tranchent, par motif de la nature 02** (`contrats-base.json`, CCH-11) : aucun de ces articles ne donne de préavis,
sauf L1243-2. 036 (employeur), 084 (accord, « Sauf accord des parties », L1243-1), 095 et 096 (faute grave ou lourde, L1243-1),
083 (force majeure, L1243-1 et L1243-4), 033 (inaptitude, L1243-1 et L1226-4-3) : **aucun préavis** ; ce qu'ils ouvrent est une
indemnité ou des dommages et intérêts, que le moteur ne calcule pas. 037 (salarié) : **le préavis de L1243-2** s'il justifie d'un
CDI, un jour par semaine de la durée totale du contrat, deux semaines au plus, sauf accord ; le moteur ne sait ni le CDI ni le
terme d'origine (un CDD rompu porte sa date de rupture en `fin`) : il dit la règle et demande la durée.

**Où le moteur s'en sert** : `permanents-base.json`, `preavis.rupture_anticipee_cdd` (`motifs_sans_preavis`, `motifs_a_dire`),
lue pour un contrat à durée déterminée seulement (083 finit aussi un CDI) ; `permanents.rs` (`preavis_mois`, `regle_du_preavis`,
`raison_sans_preavis`, qui prennent `cdd`) ; `solde::duree_du_preavis` (ni la loi du licenciement ni la convention pour 037) ; les
deux écrans, `routes_listes::ce_qui_manque` et `routes_ouverture::ce_qui_manque_au_signalement`. Tests :
`tests/routes/routes_listes.rs` (`un_cdd_rompu_avant_son_terme_…`, `un_cdd_rompu_par_le_salarie_…`),
`tests/routes/routes_ouverture.rs` (`le_rappel_d_une_fin_de_cdd_…`), `tests/calcul/permanents.rs` (`le_solde_d_un_cdd_rompu_…`).

**Lectures, écrites comme telles** : « aucun préavis » est l'absence de texte qui en donne un, L1234-1 ne visant que le
licenciement ; pour 084, la date de fin est celle que l'accord fixe. **Question ouverte**, close par le lot P1 (section « Le préavis
non effectué, motif par motif ») : le solde de tout compte payait un préavis non effectué au titre de L1234-5 (licenciement) pour
un 037 dont le préavis était saisi à la main ; aucun texte ne la donne côté CDD, il ne la paie plus.

## L1234-5, l'indemnité compensatrice de préavis : ce que le texte place où (lot T1, 13/10/2026)

**L1234-5 était déjà relevé** (`TEXTES["travail"]["voulus"]["rupture"]`, `articles-20261013.md` l. 250-254, `xml/L1234-5.xml`,
`LEGIARTI000006901118`, en vigueur depuis le 01/05/2008) ; l'entrepôt `../legi-entrepot/` est à jour au 13/10/2026 (dernière livraison
appliquée `20261012-212534`, la dernière du serveur le même jour) : `moissonner` n'a rien à ajouter, `rapport` ne ferait que
redoubler `articles-20261013.md`. Ce qui manquait n'était pas l'article, c'était sa place dans le code, que le XML donne
(`<CONTEXTE>`, `<TITRE_TM>`) : **Livre II, Titre III « Rupture du contrat de travail à durée indéterminée », Chapitre IV
« Conséquences du licenciement », Section 1 « Préavis et indemnité de licenciement », sous-section 1 « Préavis et indemnité
compensatrice de préavis ».**

- Le texte : « Lorsque le salarié n'exécute pas le préavis, il a droit, sauf s'il a commis une faute grave, à une indemnité
  compensatrice. L'inexécution du préavis, **notamment en cas de dispense par l'employeur**, n'entraîne aucune diminution des
  salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du préavis, indemnité de
  congés payés comprise. » Il ne nomme ni l'auteur de la rupture ni la nature du contrat ; il se lit dans son chapitre, celui du
  **licenciement d'un CDI**.
- **Un CDD n'y figure pas** : sa rupture avant terme est au Titre IV (L1243-1 à L1243-4, `xml/L1243-*.xml`), qui ne donne qu'un
  préavis (L1243-2, du salarié) et, à l'employeur fautif, des dommages et intérêts (L1243-4), jamais une « indemnité compensatrice
  de préavis ».
- **Pour un 037** (rupture anticipée du CDD à l'initiative du salarié, `bloc-S21.G00.62`), aucun texte déposé n'ouvre à l'employeur
  ni au salarié une indemnité de préavis : L1243-2 donne le préavis du salarié, L1243-3 des dommages et intérêts à l'employeur s'il
  le viole. **Lecture, écrite comme telle** : L1234-5 est un droit du salarié licencié (ou dispensé de préavis par l'employeur) ;
  l'appliquer à un 037 serait lui faire dire ce que son chapitre ne dit pas. Ce que le droit jurisprudentiel dit d'un salarié
  démissionnaire dispensé par l'employeur n'est **pas** un texte du dépôt (non lu, donc non écrit). Le démissionnaire d'un CDI
  relève de L1237-1 (« l'existence et la durée du préavis sont fixées par la loi, ou par convention ou accord collectif »,
  `LEGIARTI000006901174`, dans l'entrepôt, non relevé : aucun point du moteur ne le lit aujourd'hui).
- **Où le moteur s'en sert** : `atelier/sources/paie/src/solde.rs` (l. 322-350, le « préavis non effectué (L1234-5) » : la rubrique
  `indemnite_preavis`, mois × salaire, et l'avertissement « congés payés comprise »). Il s'appliquait à tout motif ; depuis le
  lot P1, motif par motif (section « Le préavis non effectué, motif par motif » ci-dessous) : plus rien pour un 037.

## Le préavis non effectué, motif par motif (relevé LEGI du 13/10/2026, lot P1)

Ajoutés à `TEXTES["travail"]["voulus"]` (`preavis_non_effectue`) et relevés par `moissonner` (0 livraison nouvelle sur 458,
« manquants : aucun », « périmés : aucun »), puis `rapport travail` : **L1237-1**, **L1237-2**, **L1226-4**, **L1226-12**,
**L1226-14**, **L1234-12**, **L1234-13**, **L1221-25**, **L1221-26** (`articles-20261013.md`, section « Le preavis non effectue :
ce que chaque fin de contrat en fait » ; `xml/`). Trouvés en cherchant dans l'entrepôt tous les articles du code du travail en
vigueur qui citent « L. 1234-5 » (L1226-14, L1226-4, L1234-13, et hors sujet L3253-3, L8252-2, L8254-2) ou le mot « préavis ».

Ce que chaque motif connu du moteur (`contrats-base.json`, natures `cdi` et `cdd`) fait d'un préavis non exécuté :

| motif | ce que dit le texte | le moteur |
|---|---|---|
| 020, 014 (licenciement) | **L1234-5** (chapitre « Conséquences du licenciement ») : « il a droit, sauf s'il a commis une faute grave, à une indemnité compensatrice » | paie `indemnite_preavis` (DSN 52 : 023) |
| 087, 088 (faute grave, lourde) | L1234-1 (« Lorsque le licenciement n'est pas motivé par une faute grave ») et L1234-5 (« sauf s'il a commis une faute grave ») | aucun préavis, rien à payer |
| 091 (inaptitude non professionnelle) | **L1226-4**, al. 3 : « Par dérogation à l'article L. 1234-5, l'inexécution du préavis ne donne pas lieu au versement d'une indemnité compensatrice » | **rien** (lot P1 ; payait avant) |
| 092 (inaptitude professionnelle) | **L1226-14**, al. 1 : « une indemnité compensatrice d'un montant égal à celui de l'indemnité compensatrice de préavis prévue à l'article L. 1234-5 » (cas du 2e al. de **L1226-12** ; pas due si refus abusif du reclassement, al. 2) | paie le même montant, sur sa rubrique `indemnite_preavis_inaptitude`, déclarée en « 032 » (lot P2 ; c'était 023) |
| 059 (démission) | **L1237-1** : « l'existence et la durée du préavis sont fixées par la loi, ou par convention ou accord collectif » ; **L1237-2** : dommages et intérêts à l'employeur si la rupture est abusive. Aucune indemnité au salarié | **rien** (lot P1) |
| 039 (départ à la retraite) | L1237-10 : le salarié « respecte un préavis » dont la durée suit L1234-1 ; aucun renvoi à L1234-5 | **rien** (lot P1 ; lecture : la même que pour la démission) |
| 037 (CDD rompu par le salarié) | L1243-2 (son préavis), L1243-3 (dommages et intérêts à l'employeur) ; rien au Titre IV ne donne d'indemnité compensatrice | **rien** (lot P1) |
| 036, 084, 095, 096, 083, 033 (CDD) ; 031 ; 043 | aucun préavis (sections ci-dessus) | rien à payer |
| 034 (fin d'essai, employeur) | **L1221-25** : un délai de prévenance (24 h, 48 h, deux semaines, un mois selon la présence) ; s'il n'est pas respecté, « une indemnité compensatrice [...] égale au montant des salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du délai de prévenance » | la calcule depuis le lot P2 (`preavis.delai_de_prevenance`) : ligne `indemnite_delai_prevenance`, DSN 52 « 017 » ; décompte : une lecture (ci-dessous) |
| 035 (fin d'essai, salarié) | **L1221-26** : un délai de prévenance que le salarié respecte ; aucune indemnité | rien à payer (prouvé par un test, lot P2) |
| 083 (CDI, force majeure) | **L1234-12** : la cessation de l'entreprise pour force majeure « libère l'employeur de l'obligation de respecter le préavis » ; **L1234-13** : un sinistre ouvre une indemnité « d'un montant égal » à L1234-5 + L1234-9 (DSN 52 : 018) | ne sait pas lequel : paie comme avant, et le dit (à trancher) |
| 038 (mise à la retraite) | L1237-6 : le préavis de L1234-1 ; aucun renvoi à L1234-5 | **à trancher** : paie comme avant, et le dit |
| 058 (prise d'acte), 065 (décès de l'employeur) | aucun article du code relevé | **à trancher** : paie comme avant, et le dit |

**Lectures, écrites comme telles** : L1234-5 se lit dans son chapitre (le licenciement d'un CDI) ; un article qui renvoie à L1234-1
pour la **durée** (L1237-6, L1237-10) ne renvoie pas pour autant à L1234-5. Pour la démission, le départ à la retraite et le 037,
c'est le salarié qui doit le préavis : aucun texte ne lui paie celui qu'il n'exécute pas. Ce que la jurisprudence dit d'un
salarié démissionnaire **dispensé par l'employeur** n'est pas un texte du dépôt (non lu, non écrit) ; s'il y a lieu de payer, la
ligne se saisit à la main. Pour 038, 058, 065 et 083, aucun texte déposé ne tranche dans un sens ni dans l'autre : le moteur ne
change pas ce qu'il faisait, et son avertissement le dit.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.indemnite_compensatrice` (`motifs_sans_indemnite`,
`motifs_par_un_autre_article`, `motifs_a_trancher`) et `preavis.motifs_sans_preavis["034"]`, `["035"]` ;
`atelier/sources/paie/src/permanents.rs` (`indemnite_de_preavis`, `IndemniteDePreavis`, et depuis le lot P2 `rubrique_de_l_indemnite_de_preavis` : la rubrique, donc le
code du bloc 52, selon le motif, `preavis.indemnite_compensatrice.rubrique_par_motif`) ; `solde.rs` (`solde_de_tout_compte`,
« le préavis non effectué ») ; `routes_accueil.rs` (`faits_de_rupture` : la case « préavis non effectué, payé en indemnité » n'est
proposée que là où elle paie). Tests : `tests/calcul/permanents.rs` (`un_cdd_rompu_par_le_salarie_ne_paie_aucune_indemnite_de_preavis`,
`une_demission_…`, `un_depart_a_la_retraite_…`, `un_licenciement_pour_inaptitude_non_professionnelle_…`, `un_licenciement_paie_…`,
`un_licenciement_pour_inaptitude_professionnelle_…`, `une_mise_a_la_retraite_…`), `tests/routes/routes_accueil.rs`
(`les_natures_payees_au_mois_descendent_quand_la_compagnie_dit_en_employer`) ; lot P2 : `tests/dsn/preavis.rs`
(`le_preavis_non_effectue_d_une_inaptitude_professionnelle_se_declare_en_032`, le témoin `…_d_un_licenciement_se_declare_en_023`,
`le_code_032_n_accompagne_qu_une_inaptitude`).

### Le délai de prévenance d'une fin d'essai (lot P2, 13/10/2026)

**L1221-25** (déjà relevé, `articles-20261013.md`) : le délai (24 h sous huit jours de présence, 48 h de huit jours à un mois,
deux semaines après un mois, un mois après trois mois), « La période d'essai, renouvellement inclus, ne peut être prolongée du
fait de la durée du délai de prévenance », et l'indemnité compensatrice, « sauf s'il a commis une faute grave », « égale au
montant des salaires et avantages que le salarié aurait perçus s'il avait accompli son travail jusqu'à l'expiration du délai de
prévenance, indemnité compensatrice de congés payés comprise ». Pour un CDD : seulement s'il stipule « une période d'essai d'au
moins une semaine ». Le bloc 52 la nomme : « 017 - Dommages et intérêts dus à la non remise du contrat ou dus à un CDD ou à une
rupture de période d'essai (délai de prévenance) » (`../../norme-dsn/texte/bloc-S21.G00.52.txt`, p. 253).

**Lectures, écrites comme telles** (aucun texte déposé ne dit le décompte) : la présence va du premier jour du contrat au jour
de la notification, compris ; « un mois », « trois mois » se mesurent comme l'ancienneté du moteur, « après » voulant dire
au-delà ; le délai court du lendemain de la notification en jours entiers (24 h : un jour ; 48 h : deux ; deux semaines :
quatorze ; un mois : jusqu'au même quantième du mois suivant) ; la part du délai après la fin du contrat se paie au salaire
mensuel, chaque jour au prorata de son mois. Les « avantages » et les congés payés de ces jours ne sont pas ajoutés (le moteur
le dit). À confirmer par l'orchestrateur ou Léandre : une circulaire ou une jurisprudence sur le décompte n'est pas déposée.

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `preavis.delai_de_prevenance` (`tranches`, `_lecture`,
`_cdd`, `_faute_grave`) ; `rubriques-base.json`, `indemnite_delai_prevenance` ; `permanents.rs` (`regle_du_delai_de_prevenance`,
`delai_de_prevenance`) ; `solde.rs` (`delai_de_prevenance`). Tests : `tests/calcul/permanents.rs` (une tranche par test,
`une_fin_d_essai_…`, `le_delai_de_prevenance_ne_se_devine_pas`, `une_fin_d_essai_a_l_initiative_du_salarie_ne_lui_donne_rien`),
`tests/dsn/preavis.rs` (`l_indemnite_du_delai_de_prevenance_se_declare_en_017`).

## L'effectif de la formation professionnelle : L6331-1, L6331-1 A, L6331-3 (relevé LEGI du 13/10/2026, lot F1)

Relevés depuis l'entrepôt LEGI (`relever_les_codes.py`, `assujettissement_fiscal`), dans `articles-20261013.md` :
- **L6331-1** : « L'employeur de moins de onze salariés s'acquitte de la contribution à la formation professionnelle […] par
  le versement de 0,55 % » ; **L6331-3** : « L'employeur d'au moins onze salariés […] 1 % » ;
- **L6331-1 A** (`LEGIARTI000038610054`, depuis le 01/01/2020) : « Pour l'application du présent chapitre, l'effectif salarié
  et le franchissement d'un seuil d'effectif salarié sont déterminés selon les modalités prévues à l'article L. 130-1 du code de
  la sécurité sociale » — cinq années civiles consécutives pour franchir le seuil, et « le franchissement à la baisse d'un seuil
  d'effectif sur une année civile a pour effet de faire à nouveau courir » le délai (CSS L130-1, II, dans
  `../code-de-la-securite-sociale/articles-20261013.md`). Le guide Urssaf des contributions de formation du 12/03/2026
  (`../majoration-caisses-conges/urssaf-guide-contributions-formation-apprentissage-20260312.txt`, l. 568-586) le dit de la CFP.

**Où le moteur s'en sert** : `logiciel/donnees/baremes/2025.json` et `2026.json`, `seuils_effectif.valeurs[formation]`
(`source`, `sous_le_seuil`) ; `contexte::controler_effectif` refuse une surcouche qui porte la formation à une autre valeur que
0,55 % tant que le seuil n'est pas franchi (`Effectif::atteint`). Test : `tests/calcul/seuils_effectif.rs`.

## Le maintien de salaire d'un arrêt, et le congé payé suspendu : L1226-1, D1226-1 à D1226-5, L3142-2 (imprévus, 15/10/2026)

Rien de neuf relevé : les articles sont ceux de `articles-20261015.md` (L1226-1, D1226-1 à D1226-8) et de
`articles-20261010.md` (L3142-2). Ce qui change, c'est où le moteur s'en sert depuis qu'un arrêt de travail ou une suspension
est un **imprévu** saisi une fois sur le salarié (`paie/src/imprevus.rs`) :
- **L1226-1** (« incapacité résultant de maladie ou d'accident », pas pour les « salariés intermittents ») et **D1226-3** (pas de
  carence pour un accident du travail ou une maladie professionnelle, « à l'exclusion des accidents de trajet ») : le champ
  `origine` des motifs 01 (`maladie`), 04 (`trajet`), 05 (`maladie_professionnelle`) et 06 (`accident_du_travail`) de
  `logiciel/donnees/arrets-base.json` (une inférence, écrite comme telle), lu par `droit::maintien::Origine::du_motif` ; seul un
  motif qui la porte ouvre le maintien (`engagement::imprevus::lignes_du_mois`) — plus de case à remplir ;
- **D1226-4** (les jours déjà indemnisés dans les douze mois) : le champ `jours_deja_indemnises` de l'arrêt ;
- **L3142-2** (les congés de L3142-1 « n'entraînent pas de réduction de la rémunération ») : une suspension au motif 637 ne
  retire rien du salaire (`imprevus::lignes_du_mois`).
Tests : `tests/calcul/imprevus.rs`, `tests/routes/routes_imprevus.rs`.

## Le congé de deuil d'un enfant : L3142-1-1, D3142-1-1 (relevé LEGI du 15/10/2026, lot « le deuil »)

`articles-20261015.md`, section « Le conge de deuil d'un enfant » (`xml/L3142-1-1.xml`, `LEGIARTI000041976515`, depuis le
01/07/2020 ; `xml/D3142-1-1.xml`, `LEGIARTI000042411489`, depuis le 10/10/2020) ; domaine `conge_de_deuil` de
`paie/src/releve_codes/textes.rs`.

- **L3142-1-1** : « en cas de décès de son enfant âgé de moins de vingt-cinq ans ou d'une personne âgée de moins de vingt-cinq
  ans à sa charge effective et permanente, le salarié a droit, sur justification, à un congé de deuil de huit jours qui peuvent
  être fractionnés […]. **Le salarié informe l'employeur vingt-quatre heures au moins avant le début de chaque période
  d'absence.** Le congé de deuil peut être pris dans un délai d'un an à compter du décès de l'enfant. » C'est le motif d'arrêt
  « 19 - Deuil d'enfant » de la DSN (S21.G00.60.001) ; ses indemnités journalières sont celles de CSS L. 331-9
  (`../code-de-la-securite-sociale/`).
- **D3142-1-1** : deux périodes au plus, d'au moins une journée chacune.

**Où le moteur s'en sert** : `logiciel/donnees/obligations-base.json`, obligation `attestation_salaire` (ses `textes`, la
`connaissance_presumee` qui rappelle que le salarié prévient vingt-quatre heures avant, `a_avoir`) ; le calcul est dans
`atelier/sources/paie/src/obligations.rs` (`de_l_arret`). Tests : `tests/routes/routes_obligations.rs`.

## La déclaration préalable à l'embauche (21/10/2026)

Relevés le 21/10/2026 depuis l'entrepôt LEGI (`paie releve-codes moissonner`, puis `rapport` : `articles-20261021.md`,
section « La declaration prealable a l'embauche ») ; domaine `dpae` de `paie/src/releve_codes/textes.rs`.

- **L1221-10** (`xml/L1221-10.xml`, `LEGIARTI000006900849`) : « L'embauche d'un salarié ne peut intervenir qu'après
  déclaration nominative accomplie par l'employeur auprès des organismes de protection sociale désignés à cet effet. »
- **R1221-1** (`xml/R1221-1.xml`, `LEGIARTI000035492419`) : la DPAE comporte, 3°, la « Date et heure d'embauche ».
- **R1221-3** (`xml/R1221-3.xml`, `LEGIARTI000039809055`, relevé le 30/10/2026) : elle « est adressée par l'employeur [...]
  à l'organisme de recouvrement des cotisations du régime général de sécurité sociale dans le ressort territorial duquel
  est situé l'établissement devant employer le salarié ». **Où le moteur s'en sert** : `declarations/src/dpae.rs`
  (`urssaf_d_un_etablissement`) : la DPAE d'un salarié d'un autre établissement que le siège porte en C_URSSAF l'Urssaf du
  département de cet établissement (`logiciel/donnees/urssaf-dpae-base.json`, `par_departement`) ; celle du siège, le code
  de la fiche entreprise.
- **R1221-4** (`xml/R1221-4.xml`, `LEGIARTI000024214323`) : elle « est adressée au plus
  tôt dans les huit jours précédant la date prévisible de l'embauche ».

Ce qu'on en tire quand la date de début d'un contrat déjà déclaré change (inférence, écrite comme telle dans le code) :
avancée, l'embauche a eu lieu avant la date que la DPAE déclarait, la déclaration ne l'a pas précédée (L1221-10) — une DPAE à
la date vraie est à déposer ; reculée, la DPAE déposée a précédé l'embauche et portait sa date prévisible (R1221-4) — rien à
refaire. Le guide de l'API DPAE de l'Urssaf (`../../norme-dpae/5491-API-DPAE-Fonctionnement.pdf`, p. 3 et 9) : pas
d'annule-et-remplace en DPAE ; un doublon est rejeté (code 98).

**Où le moteur s'en sert** : `atelier/sources/paie/src/routes_mois.rs` (`etat_par_paie`, `embauches_a_declarer`, l'étape
DPAE de `bilan_du_mois`) et `routes_regularisation::debut_declare_avant`. Tests : `tests/routes/modifier_apres_echeance.rs`.

## L'entrepreneur de spectacles vivants et sa licence (relevé du 23/10/2026, #236)

Ajoutés à `TEXTES` (`licence_spectacles`, `paie/src/releve_codes/textes.rs`) et relevés par `paie releve-codes moissonner`
(2 livraisons nouvelles sur 472, « manquants : aucun ») puis `rapport travail` : **L7122-1** à **L7122-6**, **L7122-22**,
**L7122-23** (`articles-20261023.md`, section « L'entrepreneur de spectacles vivants et sa declaration » ; `xml/`).

- **L7122-2** : « Est entrepreneur de spectacles vivants toute personne qui exerce une activité d'exploitation de lieux de
  spectacles, de production ou de diffusion de spectacles [...] quel que soit le mode de gestion, public ou privé, à but
  lucratif ou non ».
- **L7122-3** : il exerce « sous réserve de [...] déclarer son activité » ; la déclaration « donne lieu à la délivrance d'un
  récépissé de déclaration d'entrepreneur de spectacles vivants, valant licence ».
- **L7122-22** et **L7122-23** : celui qui n'a pas pour activité principale l'exploitation de lieux, la production ou la
  diffusion de spectacles déclare ses artistes et techniciens auprès d'un organisme habilité (le guichet unique), pas en DSN.

La norme DSN (S21.G00.40.049, `../../norme-dsn/texte-2027/bloc-S21.G00.40.txt` l. 1418-1426) y renvoie (« L.7122-3 et
suivants ») et demande, à défaut de licence, la référence du récépissé.

**Où le moteur s'en sert** : `demarrage::licence_exigee` (qui doit la licence : la convention dont c'est le champ, 3090 et
1285, `exige_si` du champ `licence_spectacle` de `logiciel/donnees/champs-base.json` ; en 1790 et 3252, dès qu'un spectacle
est configuré, ou, dans la DSN, pour un contrat de spectacle), lu par « Pour démarrer » et par `dsn::licence_du_contrat`.
Tests : `tests/routes/demarrage.rs` (`aucun_manque_ne_pointe_vers_un_champ_masque_pour_aucune_convention`) et
`tests/dsn/licence.rs`.

**Inférence** : qu'un spectacle configuré (ou un contrat de spectacle, CDDU portant un objet spectacle) fasse de la
compagnie un entrepreneur de spectacles qui déclare en DSN : par L7122-1 (« s'assurent la présence physique d'au moins un
artiste du spectacle percevant une rémunération ») et L7122-22/23 (l'organisateur occasionnel ne déclare pas en DSN).

## Les échéances d'un engagement : la remise du contrat (L1242-13), le bulletin (L3243-2), le paiement (L3242-1) (28/10/2026, #257)

L1242-13 ajouté au domaine `precarite` de `paie/src/releve_codes/textes.rs` et relevé le 28/10/2026 (`paie releve-codes
moissonner`, puis `rapport travail` : `articles-20261028.md`, l. 216-218, `xml/L1242-13.xml`). L3243-2 (domaine
`bulletin`) et L3242-1 (domaine `mensualisation`) étaient déjà suivis ; le même relevé les porte (`articles-20261028.md`,
sections « Le bulletin de paie » et « La mensualisation » ; `xml/L3243-2.xml`, `xml/L3242-1.xml`).

- **L1242-13** (`LEGIARTI000006901207`) : « Le contrat de travail est transmis au salarié, au plus tard, dans les deux jours
  ouvrables suivant l'embauche. » Une convention plus exigeante l'avance : la 1285, titre XV (artistes musiciens), « au
  plus tard 15 jours ouvrables précédant l'engagement » (`../chomage-intermittents/kali-1285-organisation-travail-repetitions-representations-20261020.md`,
  l. 763), écrite dans la couche de la convention (`logiciel/donnees/clauses-conventionnelles-base.json`,
  `remise_du_contrat`, avec son inférence sur les métiers).
- **L3243-2** (`LEGIARTI000033024092`) : « Lors du paiement du salaire, l'employeur remet [...] une pièce justificative dite
  bulletin de paie. » Le bulletin accompagne le paiement ; le moteur n'en tire aucune échéance : la case Documents du
  mois n'en a pas, chaque compagnie envoie ses documents quand elle veut (souvent après la DSN, qui fait remonter l'AER).
- **L3242-1** (`LEGIARTI000006902858`) : « Le paiement de la rémunération est effectué une fois par mois » ; « ces
  dispositions ne s'appliquent pas [...] aux salariés intermittents ». Il ne donne donc aucun jour pour un cachet.

**Inférence** (écrite dans `droit/src/echeances.rs`) : un jour ouvrable va du lundi au samedi, moins les fériés chômés de la
compagnie (`conges::jours_ouvrables`, L3141-3) ; « dans les deux jours ouvrables suivant l'embauche » se compte depuis le
lendemain de l'embauche.

**Où le moteur s'en sert** : `atelier/sources/droit/src/echeances.rs` (`remise_du_contrat`), lu par
`routes/src/mois.rs` (`etat_par_engagement`, `echeances` de chaque engagement ; l'échéance de la case Contrats
de `bilan_du_mois`). Test : `droit/tests/droit/clauses.rs`
(`la_remise_du_contrat_suit_l1242_13_et_la_1285_pour_ses_musiciens`).

**R3243-1, R3243-2 et R3243-5 : ce que tout bulletin imprime.** R3243-1 dit les mentions du bulletin (1° le nom et
l'adresse de l'employeur ; 2° son code APE et son Siret ; 3° la convention ; 4° le nom et l'emploi du salarié ; 5° la
période ; 7° le brut ; 8° les cotisations, avec le taux de la part salariale ; 9° l'impôt prélevé ; 10° la somme reçue ;
11° la date du paiement ; 14° le total versé par l'employeur) ; R3243-2 renvoie leurs libellés et leur ordre au modèle de
l'arrêté du 25 février 2016 (`../montant-net-social/legi-arrete-20160225-bulletin/`). **Où le moteur s'en sert** :
`atelier/sources/import/src/bulletin_lu.rs`, le lecteur des bulletins importés : il trouve ces mentions par leurs
libellés (`logiciel/donnees/bulletins-lecture-base.json`, `mentions`), refuse une page où le salarié, la période, le brut
ou la somme reçue ne se trouvent pas, et lit un taux de colonne qui ne dit pas sa part comme celui du salarié, le seul que
le 8°, a, exige (**inférence**). Tests : `moteur/tests/fichiers/bulletin_lu.rs`. Le 1° dit aussi « le cas échéant, la
désignation de l'établissement dont dépend le salarié », et le 2° le code APE « de l'établissement d'emploi » :
`atelier/sources/documents/src/bulletin.rs` (`blocs_un`) imprime, pour un contrat d'un autre établissement que le siège,
« Établissement : » et le SIRET de cet établissement dans le cadre EMPLOYEUR (`droit::bareme::entreprise_du_salarie`).

## Le contrat écrit : L1242-12 (CDD), L1221-5-1 et R1221-34 à R1221-40 (tout salarié), L3123-6 (temps partiel) (relevé du 29/10/2026, #285)

Le domaine `information_ecrite` (L1221-5-1, R1221-34, R1221-35, R1221-38, R1221-39, R1221-40) et L3123-6 (domaine
`temps_partiel`) ajoutés à `atelier/src/releve/codes/textes.rs`, relevés le 29/10/2026 (`moteur-atelier releve-codes
moissonner`, une livraison nouvelle, puis `rapport`) : `articles-20261029.md`, l. 203-218 (L1242-12, L1242-13), l. 697
(L3123-6), l. 813-858 (L1221-5-1 à R1221-40) ; XML : `xml/L1242-12.xml`, `xml/L1221-5-1.xml`, `xml/R1221-34.xml`,
`xml/R1221-35.xml`, `xml/R1221-38.xml`, `xml/R1221-39.xml`, `xml/R1221-40.xml`, `xml/L3123-6.xml`. Ce sont les articles qui
transposent la directive (UE) 2019/1152 (loi n° 2023-171 du 9 mars 2023, décret n° 2023-1004 du 30 octobre 2023, que les
notes des articles citent).

- **L1242-12** (`LEGIARTI000006901206`) : le CDD « est établi par écrit et comporte la définition précise de son motif » ;
  1° la personne remplacée (nom et qualification) pour un remplacement (1°, 4° et 5° de L1242-2) ; 2° le terme ; 4° le
  poste ou l'emploi ; 5° la convention collective ; 6° la période d'essai éventuelle ; 7° la rémunération et ses
  composantes ; 8° le nom et l'adresse de la caisse de retraite complémentaire et, le cas échéant, de l'organisme de
  prévoyance.
- **L1221-5-1** (`LEGIARTI000047285930`) : l'employeur remet au salarié des documents écrits contenant les informations
  principales relatives à la relation de travail ; **R1221-34** (`LEGIARTI000048288642`) en donne la liste (1° les parties,
  2° le lieu de travail et l'adresse de l'employeur, 3° l'intitulé du poste et la catégorie, 4° la date d'embauche, 5° le
  terme d'un CDD, 7° la période d'essai, 8° le droit à la formation, 9° les congés payés, 10° la procédure de cessation, 11°
  la rémunération, sa périodicité et ses modalités de paiement, 12° la durée du travail, 13° les conventions, 14° les
  régimes obligatoires et la protection sociale complémentaire) ; **R1221-35** permet de dire les 7° à 12° et 14° par un
  renvoi aux textes ; **R1221-39** la forme (papier ou électronique, date certaine) ; **R1221-40** : ce qui change se dit
  dans un document au plus tard à sa prise d'effet (l'avenant) ; **R1221-38** : un arrêté fixe des modèles (non relevé :
  aucun texte de LEGI suivi ne le porte).
- **L3123-6** (`LEGIARTI000033020080`) : le contrat à temps partiel est écrit et mentionne la qualification, la
  rémunération, la durée hebdomadaire ou mensuelle et sa répartition entre les jours de la semaine ou les semaines du mois.

**Inférence** : le contrat d'usage du spectacle est un CDD (L1242-2, 3°) ; L1242-12 s'y applique comme à tout CDD, et
R1221-34 à tout salarié. Les phrases de renvoi (formation, congés, rupture) citent L6321-1, L3141-3, L1243-1, L1243-2,
L1243-5, L1234-1, L1237-1 et L1237-11. **L6321-1** (`LEGIARTI000052437104`, l. 859 ; `xml/L6321-1.xml`, relevé le 29/10/2026
au domaine `information_ecrite`) : « L'employeur assure l'adaptation des salariés à leur poste de travail » et veille
au maintien de leur capacité à occuper un emploi ; c'est ce que la phrase de renvoi sur la formation cite (R1221-34, 8°).

**Où le moteur s'en sert** : `logiciel/donnees/contrats-base.json`, `contrat_ecrit` de chaque nature (le modèle livré,
celui de l'avenant, les **mentions obligatoires**, chacune par son marqueur et le texte qui l'exige, les phrases de
renvoi) ; `atelier/sources/documents/src/contrat_ecrit.rs` (`marqueurs_au_mois`, `poser_les_mentions`,
`refuser_les_mentions_absentes`, `champs_des_modeles`) ; `documents/src/contrat.rs` (le contrat et l'avenant refusés si
leur modèle tait une mention) ; les modèles `logiciel/donnees/modeles/contrat-cdd.modele`, `contrat-cdi.modele`,
`avenant-permanent.modele`, `contrat-cddu.modele` ; `routes/src/modeles_contrat.rs` (le modèle de la compagnie
vérifié avant d'être posé) ; `routes/src/mois.rs` (`champs_du_contrat_ecrit` : la personne remplacée,
L1242-12 1°, et la répartition d'un temps partiel, L3123-6, que l'engagement appelle). Tests :
`documents/tests/documents/contrat_ecrit.rs`.

## La modification du contrat : L1221-1, L1222-6, et l'avenant d'un contrat signé hors du logiciel (relevé du 02/11/2026, #338)

Le domaine `modification_du_contrat` (L1221-1, L1222-6) ajouté à `atelier/src/releve/codes/textes.rs`, relevé le
02/11/2026 (`moteur-atelier releve-codes moissonner`, aucune livraison nouvelle : les articles étaient dans l'entrepôt ; puis
`rapport`) : `articles-20261102.md`, l. 972-980 ; XML : `xml/L1221-1.xml`, `xml/L1222-6.xml`.

- **L1221-1** (`LEGIARTI000006900839`) : « Le contrat de travail est soumis aux règles du droit commun. » Le droit commun
  (code civil, article 1193 : « Les contrats ne peuvent être modifiés ou révoqués que du consentement mutuel des parties », déposé
  dans `../code-civil/legifrance-code-civil-art1193-force-obligatoire-20261102.md`, relevé par Léandre le 02/11/2026) : un
  terme du contrat (salaire, durée du travail, emploi) qui change se fait signer
  par le salarié, d'où l'avenant envoyé à la signature.
- **L1222-6** (`LEGIARTI000028724382`) : la modification d'un élément essentiel pour motif économique se propose par lettre
  recommandée, et le salarié a un mois pour refuser. **Le moteur ne le porte pas** : il ne connaît pas le motif d'un
  changement.
- **R1221-40** (plus haut) fonde l'écrit : ce qui change se dit « dans un document indiquant ces modifications », au plus
  tard à sa prise d'effet. Il ne demande pas de citer le texte d'origine : pour un contrat signé hors du logiciel, dont le
  logiciel ne connaît pas le texte, l'avenant dit en clair chaque terme qui change, l'ancienne valeur, la nouvelle et à
  partir de quand.

**Où le moteur s'en sert** : `engagement::signe_ailleurs` (`engagement/src/lib.rs`) ; `engagement::etats::a_refaire`
(l'avenant à faire d'un contrat signé ailleurs) ; `engagement::versions::termes_changes` ;
`documents::avenant::changements_en_clair` ; `documents::contrat::ecrire_avenant` (`avenant.reference`,
`modification.phrase`) ; les modèles `avenant-permanent.modele` et `avenant-cddu.modele` (le paragraphe
`{{modification.phrase}}`) ; `routes/src/contrats.rs` (`avenant-apercu`, `signature-apercu`) ;
`routes/src/gestes/signature.rs` (le dossier du contrat signé, joint à l'envoi de l'avenant).

## La période d'essai et sa durée maximale : L1242-10 (CDD), L1221-19 à L1221-24 (CDI) (relevé du 29/10/2026)

Le domaine `periode_essai` ajouté à `atelier/src/releve/codes/textes.rs`, relevé le 29/10/2026 (`moteur-atelier
releve-codes moissonner`, aucune livraison nouvelle : les articles étaient dans l'entrepôt ; puis `rapport`) :
`articles-20261029.md`, l. 869-912 ; XML : `xml/L1242-10.xml`, `xml/L1221-19.xml` à `xml/L1221-24.xml`.

- **L1242-10** (`LEGIARTI000006901204`) : le CDD « peut comporter une période d'essai » ; sauf usages ou stipulations
  conventionnelles prévoyant des durées moindres, elle « ne peut excéder une durée calculée à raison d'un jour par
  semaine, dans la limite de deux semaines lorsque la durée initialement prévue au contrat est au plus égale à six mois
  et d'un mois dans les autres cas » ; sans terme précis, elle se calcule sur la durée minimale du contrat.
- **L1221-19** (`LEGIARTI000019071113`) : la période d'essai d'un CDI a pour durée maximale deux mois (ouvriers et
  employés), trois mois (agents de maîtrise et techniciens), quatre mois (cadres). **L1221-21** : un renouvellement, si un
  accord de branche étendu le prévoit, jusqu'à quatre, six et huit mois. **L1221-22** : ces durées sont impératives, sauf
  des durées plus courtes fixées par un accord collectif postérieur à la loi du 25 juin 2008, ou par le contrat.
  **L1221-23** : la période d'essai et son renouvellement ne se présument pas, ils sont stipulés au contrat. L1221-20 (son
  objet) et L1221-24 (le stage déduit) sont relevés avec eux.
- La convention 3090 (`../kali-3090-titres-vii-x-xi-annexes-20260924.md`, art. 7.4 et 7.5, du 01/03/2020) : en CDI,
  « artistes interprètes : 2 mois de travail effectif », les autres catégories aux durées de la loi ; en CDD, les durées
  de L1242-10 ; « Toute période d'essai exprimée en jours se décompte en jours calendaires ».

**Inférences**, écrites comme telles dans le code : une semaine de contrat commencée compte pour un jour (le plafond le
plus large que la phrase permet) ; « six mois », « un mois », « n mois » se comptent du premier jour du contrat au même
quantième ; la catégorie de L1221-19 se lit dans le classement conventionnel (S21.G00.40.002 : 03 et 04 cadres, 05
profession intermédiaire, 06 employés, 07 ouvriers ; 01 et 02 ne sont dans aucune des trois, rien ne se juge). Le
renouvellement (L1221-21) n'est pas jugé : l'engagement ne porte que la période stipulée au contrat.

**Où le moteur s'en sert** : `atelier/sources/droit/src/essai.rs` (`plafond`, `refuser_au_dela`), appelé par
`engagement/src/calcul.rs` (`essai_dans_son_plafond`) : un engagement dont la période d'essai dépasse son plafond ne se
calcule pas ; il s'enregistre à compléter, et sa validation refuse en citant le texte. Test :
`droit/tests/droit/essai.rs`.

## La durée maximale du travail et le repos quotidien (relevé du 30/10/2026, séance 8, lot P2)

`articles-20261030.md`, section « La duree maximale du travail et le repos quotidien » (XML : `xml/L3121-18.xml`,
`xml/L3121-19.xml`, `xml/L3121-20.xml`, `xml/L3131-1.xml`, `xml/L3131-2.xml`), relevés par `moteur-atelier releve-codes moissonner`
depuis l'entrepôt LEGI (domaine `duree_maximale` de `atelier/src/releve/codes/textes.rs`).

- **L3121-18** (`LEGIARTI000033020428`, depuis le 10/08/2016) : « La durée quotidienne de travail effectif par salarié ne
  peut excéder dix heures, sauf : 1° En cas de dérogation accordée par l'inspecteur du travail […] ; 2° En cas d'urgence […] ;
  3° Dans les cas prévus à l'article L. 3121-19. »
- **L3121-19** (`LEGIARTI000033020422`) : une convention ou un accord (d'entreprise, d'établissement, à défaut de branche) peut
  prévoir le dépassement de la durée maximale quotidienne « à condition que ce dépassement n'ait pas pour effet de porter
  cette durée à plus de douze heures ».
- **L3121-20** (`LEGIARTI000033020414`) : 48 heures au plus dans une même semaine. **L3131-1** et **L3131-2** : onze heures
  consécutives de repos quotidien, sauf dérogation par convention ou accord.

**La convention 3090** (spectacle vivant privé), lue dans l'entrepôt KALI (`../kali-entrepot/articles.jsonl.gz`, par
`moteur-atelier releve-conventions chercher atelier/reference/sources 3090 "durée quotidienne"`) : la durée quotidienne dépend de
l'annexe et de la filière. Par exemple : 10 heures, portées à 12 « dans les cas suivants » pour le personnel technique
(`KALIARTI000028157685`, titre III, III.3, art. 2) ; 10 heures, portées à 12 « selon les nécessités et spécificités des
tournées », dans une amplitude de 15 heures (`KALIARTI000028157839`, titre III, 3.2.6) ; 10 heures pour l'artiste-interprète
(`KALIARTI000028157888`, 3.3.3) ; 8 heures, portées à 12 pour les salariés directement liés au spectacle
(`KALIARTI000028157882`, 2.2.3) ; 12 heures pour la filière salle et restauration (`KALIARTI000028157805`, 7.1.2) ; 8 heures
pour un travailleur de nuit (`KALIARTI000028157401`, titre VIII, 8.14). Aucune ne dépasse les 12 heures de L3121-19.

**Où le moteur s'en sert** : `atelier/sources/engagement/src/duree.rs`. Une journée saisie au-delà de 10 heures se dit sur la
carte de l'engagement (`au_dela_du_maximum`, un avertissement : la phrase cite L3121-18 et L3121-19, sans refuser — c'est
l'employeur qui en répond, et une annexe de sa convention peut la porter à 12 heures) ; au-delà de 24 heures, la saisie se
refuse (`plus_d_une_journee` : une journée n'en a que 24, aucun texte n'est nécessaire). **Inférence** : une ligne qui paie
des heures sur plusieurs jours les répartit également ; la journée la plus chargée en compte au moins cette moyenne.
Test : `moteur/tests/routes/duree_du_travail.rs`. L3121-20 (la semaine) et L3131-1 (le repos) ne sont pas encore contrôlés :
la saisie ne dit pas les heures de début et de fin d'une journée.

## Les jours fériés chômés d'un permanent : L3133-3, L3133-5 (relevé LEGI du 29/10/2026, séance 8, lot Q2)

Relevés par `moteur-atelier releve-codes` : `articles-20261029.md`, l. 111-124 ; XML : `xml/L3133-3.xml`, `xml/L3133-5.xml`.

- **L3133-3** (`LEGIARTI000033020893`) : « Le chômage des jours fériés ne peut entraîner aucune perte de salaire pour les
  salariés totalisant au moins trois mois d'ancienneté dans l'entreprise ou l'établissement » ; les saisonniers cumulent
  leurs contrats ; ni les travailleurs à domicile, ni les intermittents, ni les temporaires.
- **L3133-5** (`LEGIARTI000033020881`) : « Le chômage du 1er mai ne peut être une cause de réduction de salaire. »

**Lecture** : a contrario, un férié que l'entreprise chôme n'est pas dû à un permanent qui n'a pas trois mois
d'ancienneté le jour du férié, sauf le 1er mai. Une convention peut faire mieux : celles de la 3090 et de la 1285 sur les
fériés ne sont pas relevées ici (la 3090, titre VII, dit seulement quand ils sont chômés).

**Où le moteur s'en sert** : `droit::conges::feries_non_payes` (les fériés chômés de l'entreprise, `feries::chomes_entre`,
qui tombent un jour travaillé du salarié, avant ses trois mois comptés depuis « Dans la compagnie depuis ») ;
`droit::periodes::part_du_mois` les retire des heures payées du mois, et l'aperçu le dit. Test :
`droit/tests/droit/mois_incomplet.rs`, `un_ferie_chome_ne_se_paie_qu_apres_trois_mois_d_anciennete`.

## Les heures supplémentaires et complémentaires d'un salarié au mois (relevé du 02/11/2026, séance 8, lot LONG7)

Domaine `heures_supplementaires` de `atelier/src/releve/codes/textes.rs` : L3121-29 (le décompte par semaine),
L3121-30 (le contingent annuel), L3121-33 (l'accord fixe le taux, au moins 10 %, et le contingent), L3121-34, L3121-35,
L3121-36 (à défaut d'accord : 25 % pour les huit premières heures de la semaine, 50 % au-delà), L3123-22 (le complément
d'heures par avenant, qui n'est pas une heure complémentaire), L3123-29 (à défaut d'accord : 10 % dans le dixième des heures
du contrat, 25 % jusqu'au tiers) ; avec, déjà relevés, L3121-28, L3123-20, L3123-21 et L3123-28. Texte dans
`articles-20261102.md`. **Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json` (`heures_en_plus`), lu par
`atelier/sources/droit/src/heures_en_plus.rs` ; la convention l'emporte quand elle fixe un taux
(`clauses-conventionnelles-base.json` : 1285, VI.9, VI.12 et VI.13 bis ; 3090, 8.6), `kali-1285-titre-vi-duree-du-travail-
20261102.md` et `kali-3090-titre-viii-duree-du-travail-20261102.md`, relevés le même jour par `moteur-atelier releve-conventions`.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20260919.md` : `27e08f8e4dc71e9627e2223a0b89bbad3d95386d700516398b6c1f1168b3ba70`
- `articles-20260921.md` : `b012dbe94513122c5b001780193ff7868026cc714b29abd034ca6d6e7b37a5d7`
- `articles-20260923.md` : `3e2323c0a813690cb8b42d9e6e00c2b6de69cc965151717704b4e65351a6ca88`
- `articles-20260924.md` : `29afae77617bedd2397ad2e13853db0e7b6ec0e70e96293dfa165a9998f48d64`
- `articles-20261010.md` : `2543854fc3ff783e83e52f5ed045c2333ee8e4324b0cd0091308e405160e2ce7`
- `articles-20261013.md` : `95e4fc5a8798fcfd0485e9c1ea71f6e1542f1459627f0892a1bebc090aa560dc`
- `articles-20261015.md` : `435dd934687913f7b3df90b703eccd94692fd6f1cd3dd353edb61cdf8cbcafdf`
- `articles-20261021.md` : `c9bb44eea5524fe2282fff4444b80e8cc6c5c7cd69b563cd6f9ede96e346da6d`
- `articles-20261023.md` : `a47756476c0823b14c2f8b17e29d2e778249da78c7749d579cb073df912ba088`
- `articles-20261028.md` : `2dec7a1396defd3879876c9d68089f6debb0a0ffca581b40c849d8da6e481cf1`
- `articles-20261029.md` : `5907bec5055aecfe8028163431862ae9db64b1da99490e38dd5543a1b0f476d9`
- `articles-20261030.md` : `0200c8ae01fcf673c220d99a1e49064b1c65d45e9050a3a1067bf04e1aa05828`
- `articles-20261102.md` : `f6c01912b3f4ddb46598cf72eeba29b46779e7901fd8714e95a93caa1369d656`
- `articles-20261103.md` : `c8b8f90709205c049ad1e8185506d087d770172d9ac9639719e86b63c62be066`
- `articles-20261110.md` : `da26ce0e65042c9cccfac66b206ea04a07db366907647054f6aac1f43bc819c9`
- `oeth-articles-20260925.md` : `b7c3fbab2052b29d6bd6f100635d754528af097d9dc3a323b38f89eb29e1b829`
- `permanents-articles-20260918.md` : `9fc9b957ab2332934faa6ce422d93f3d563b874a156378d4a4ef73f72cee60a6`
- `releve.json` : `9681a53bd35246612dff819cfd13262b914829f234f87760f5d5d94f26075fd8`
- `xml/D1221-23.xml` : `5ed56c43915ba969ebe234b15ca834a0b71b106b70f16c36fba3e9a5d53658dc`
- `xml/D1221-24.xml` : `405da960d426fe170e51f4d7dc72e025181f749d465fe96c9a0954e387d8d833`
- `xml/D1226-1.xml` : `e38480b66c2550800da291e63d28194d7e601846a1993f8d1623cba068d08633`
- `xml/D1226-2.xml` : `310aef82ad5532afd784cc85253e55bcb3c0a6cb1be55382551bf1fd54dcb95e`
- `xml/D1226-3.xml` : `9dcf983703f1581057e025360d17a2e11d18d3fc38854311cd2f57da00194e50`
- `xml/D1226-4.xml` : `35c6881582247696cfb996978873622c6c9d703e1734be41558771cc1ffab2db`
- `xml/D1226-5.xml` : `e64a41e1744e37e30c0c3f64fcda2088f0e229eed6b1df7505e463db13d7cc51`
- `xml/D1226-6.xml` : `282333ae20ebfd1d24990165221d1fcd87d6e9c517719dc5e681f98607af95f2`
- `xml/D1226-7.xml` : `b8f54be2995f1ba4f9ce74e7da7b3973599031da6766039bcc4ac303b2312b35`
- `xml/D1226-8.xml` : `fd64b8f950fedacb5f2069d8cc242660f67ad0d69657dc234088de30e0d73b7f`
- `xml/D1234-6.xml` : `534b33cb1bde77972c1166d83f572d23c29b6542f0270f4561e4d99f62b2b4f8`
- `xml/D1234-7.xml` : `3ecf2a09e36d12ee06de0c9a4d7eaeac8ce94b93a953778b67323d2d4218e700`
- `xml/D1234-8.xml` : `09a03749cda7fdedea8d639b6900139f7244346ce9009f950f0bde1280f6eee2`
- `xml/D1237-1.xml` : `178a49c316e1f639709c2d001b7ca770b41c5ddf2031013652e109fc46d9148a`
- `xml/D1237-2-1.xml` : `c940e7ccc13f28388837e7bafb6e78cf04d34dd9b498529f1f37b97dbb5889e1`
- `xml/D1237-2.xml` : `83c6e92d6c017aae34cb9d09848675c182542bd025dbc283143a7bec3f85a0e5`
- `xml/D3142-1-1.xml` : `4b844fe8a6c2ba8a361b974f7e602cc8a524d17aff33f1b2573e6e5dee055e2e`
- `xml/D6331-72.xml` : `f8ddebe1e506509232b2a4558dbfdb3a4fa51856bec8f357c8d0b17b93f474ca`
- `xml/D7121-28.xml` : `d5c7303f26f88474e00e9da8d32c5dd4be1af6d6a8a4327b4e30e2114e31f646`
- `xml/D7121-38.xml` : `565890c431b55e0553e551786da151f6097b026bb64efb28bba77bd094d86667`
- `xml/D7121-40.xml` : `0b31ceca1b269dab26f98e21ed4a762dff9106810ef99f9071e3eff7ad4554db`
- `xml/D7121-41.xml` : `8e60a383433b5ed708696677f3d877a474b39e398c77b1875d6da38e9e799f1e`
- `xml/L1111-2.xml` : `33bab1e20d64f51a3c213156af9e52472562df9a159d223b198e9fe8d4d8b2c7`
- `xml/L1221-1.xml` : `dce6d47e87b2e615c820eb33413028e543e641bbcc2e4b741f0d1262c03beeca`
- `xml/L1221-10.xml` : `783007c0b4928a29e9b952948d4eb244788b8a5e2c03dc4d190b9cc66d895bfc`
- `xml/L1221-13.xml` : `22e751cdc8879e4eb1ceed024b8725b9e903d690f000e9d53b90e86213d3f966`
- `xml/L1221-19.xml` : `f5fa9c106d6c42dcac00da489e69d441d909551c0f643a45613376fa38a3cf99`
- `xml/L1221-20.xml` : `37480db70f2df748719d7d1f895691e0162d1553c9ec64639162fd8ffb71a620`
- `xml/L1221-21.xml` : `df88b654f9e1ad1b03cf87aeaf706ac8ea88694b23bdad9ff95d252ff9d608af`
- `xml/L1221-22.xml` : `a74dea699377fbdcf9be2ad0a7b8f0ab52126d3d8ac94e1afa3e8865225e1ff5`
- `xml/L1221-23.xml` : `e6160ae12d08e659ea81e4b8a6b155f3382a0e9b859c9bdeaa0a3acf4eb9a385`
- `xml/L1221-24.xml` : `b9012c89b78b5edda144c0493108885f368a9e2072123820618ee36bf3a818b7`
- `xml/L1221-25.xml` : `e3fd725ad6a51fc310aff6a0d83f6b29da07ec60e1e0a602b1f5f4ec70e57fef`
- `xml/L1221-26.xml` : `2431c36cf1363d6004daec0ce6b13a3b0fbf951875be891c89a62cc32a3f18ba`
- `xml/L1221-5-1.xml` : `f36faa0aecfe4f31e1cdd051d588e468fc8fc21c9d24803624cb36bd406c2fa7`
- `xml/L1222-6.xml` : `2f9006f495c686ccc9800ce704770031b2bdd31109c2988c0654318bb4ece4b5`
- `xml/L1225-47.xml` : `940d8025f34bc957e290ddcc51f1fc7707869463077f91bd57106d2e1b6bb2d1`
- `xml/L1225-48.xml` : `18763d70a2416f40e2beb79eb26801a6d19d7b1ac181631afe52f94855c1b6cc`
- `xml/L1225-49.xml` : `68132ab105b2ac6d2e93a2de221afea8a7f55d0a092038425b5a024b1f057ed6`
- `xml/L1225-50.xml` : `539071c295e061f329f0af28c65ff05ef88846cb9a64a7af3a4303e161a9ef6d`
- `xml/L1225-51.xml` : `8ed581f0aba5ffe59cfb10a41996c03ba3e159c069f6418c88305310f27226df`
- `xml/L1225-52.xml` : `b87cc631c03822d20150c0fbab0592f8843c681fc72966f67bfd496eeacdbddc`
- `xml/L1225-53.xml` : `9e9358091f26349a305bee75575b9b9ea1ac3f5002ee656f441e4c6fe70430e5`
- `xml/L1225-54.xml` : `481bf85771fa452d28ec1fb37a9a88796f49eeb2b4f027ae42bd0312f1f5d8fc`
- `xml/L1225-55.xml` : `e2768239e362fdd52d16d2ded27f53d6ebf7dbc8093e0ca4f89042c3da1bbd47`
- `xml/L1225-56.xml` : `2f26626108099c38c185cc54a753fd07a813345733dabe3b8e93286425b97d78`
- `xml/L1225-57.xml` : `f5e8b4c5f23ad49323f6c547e2839abf0304508435ff2a91d0ab42d9ca3f4d42`
- `xml/L1225-58.xml` : `3c4d4de2fc262a3dde5cff8ceaac294f3a465550536283cf0d302a29d9fe7c69`
- `xml/L1225-59.xml` : `04e716f9f0ad9bef4bb4c3ad0bfc5cf0eb927b7d41e4f49c9d11a4eef5afd94f`
- `xml/L1226-1.xml` : `f315a6b21506dea47b88f4a4a3af0d9e45528e5f67ed2a3c60cac71f12975a3b`
- `xml/L1226-12.xml` : `06e4cf92aedbc59530fa47b999023795aa494e18eae8622cc00cea95515364bc`
- `xml/L1226-14.xml` : `b1687fb028284fce8838ad4c780c4b4dd05b048c50d05ae9814fce523b9a908f`
- `xml/L1226-20.xml` : `4c177c60993e20cf9b308c0d7efa589398843f00ca74fe96fe99f5f1f239b395`
- `xml/L1226-4-2.xml` : `ac041c248b4ac173499cf76208f8414566c4b5ae3a8d6c3f003adb160e71ba67`
- `xml/L1226-4-3.xml` : `5696e52a04ff9ffa8a91b6e16e414fbaa5c294838462c71bb20c70bd285d4835`
- `xml/L1226-4.xml` : `6bff848a1dd23f9186b5baf0829154496252dc54e9924df4e07f08466d0d3c82`
- `xml/L1234-1.xml` : `263c966f0220c97c9f469106826520c1ced2885992e1c7b97cad26a7a094252f`
- `xml/L1234-12.xml` : `9c75bed5a366dd83decd50eabb60b9343b440767eeedab79fa30247da0f16eda`
- `xml/L1234-13.xml` : `e6c54674b5c7a938a1b1877029033855348d3c63865f9ec9934c8525a851e482`
- `xml/L1234-19.xml` : `6244213372fd37d55ee8af83f02ebd6c2ce746cfca74a8591b6dfd2c2a985e30`
- `xml/L1234-20.xml` : `121563cc94cdb152250b8b9b0d709f1d3b6d208528fe8db8ed6c1c399b2ab66e`
- `xml/L1234-4.xml` : `33b58249b4f93d0a5926d352b32c3af96776385d4e586bfa178bc21346b0f07c`
- `xml/L1234-5.xml` : `60cc0ae42be35ccbf3fcbb6d76c726c7fb367baf6ced87066a7954dff0fdae09`
- `xml/L1234-9.xml` : `f75eb95ecd8b08c44a1f6eb9caacdfe256f2cbc85d3266514e9a75304f1784a2`
- `xml/L1237-1.xml` : `23cf5f35120d37049b19ace2e9ee016d16ab339547206011c83d7474298df6b2`
- `xml/L1237-10.xml` : `57f0b9d38cbef6b04c1063504cbccae29f1b957b246ab411aab148a8a55cc50d`
- `xml/L1237-11.xml` : `45fef0aa7e725da51957daeeca42e6348e6a4e89c19cd0e790a04fa884b3743b`
- `xml/L1237-12.xml` : `e3e1766c42b94231b61e905548bb4ef0aac87feb0abd3785bd3aaf5b5ccd6a8a`
- `xml/L1237-13.xml` : `1df6a067babfdb222a73ccc02bdac533d93e4bf899a02fa5efe67d5ff5c8d188`
- `xml/L1237-14.xml` : `32a9c617559e89a6c7ad908917b3c30d52a2a8c7e57887e183844290ede5cd4e`
- `xml/L1237-15.xml` : `6ae367046cb5495bc6af2f84062e6a67c35fa8e5cfed37c7eeacaaaa4fa22964`
- `xml/L1237-16.xml` : `113b92c7539eb498b682fe863612d42b400de2bb69e3262a34833c86e5098dc1`
- `xml/L1237-2.xml` : `67a0f680c7e6d587cb85856bd381ae1f328e8c9ffd200b5328327068ac22a32c`
- `xml/L1237-5.xml` : `948ec70f2d82b59f8a5812ab5f4ef07eb9c97a31fc190d8d7f33426b92bb2d25`
- `xml/L1237-6.xml` : `5741192a4e87e7a258d84433461145a4b10497ebd2f97fcaa95df03383db55cf`
- `xml/L1237-7.xml` : `2431dca161bc133d20b383e32d584c1691a596eb38fb29001eb37fb6122a50d0`
- `xml/L1237-8.xml` : `4bf36b8953d325687369a3419b743e9e110b89a0f34b44a22a48be7a30403fbc`
- `xml/L1237-9.xml` : `0f08864fe88a378eaf40da923f2d138a9e72454a97e1f9f8567cb20fd40fe4cb`
- `xml/L1242-10.xml` : `07fc1fb1d25314958a76e90a3ff4b774080958a3c867431e16dbf1e64901d26e`
- `xml/L1242-12.xml` : `23c9a050292d6319d0687d07ab383e04565a4d19dce38cd1c9d86e4250367dc0`
- `xml/L1242-13.xml` : `c330d58743898dbb2176695aca67fc68ba88e75e252e01930ca330469f52f948`
- `xml/L1242-16.xml` : `87b2331f694e0883b261033b271a034a5426710fd2406067a25c1c82bdd5d068`
- `xml/L1242-2.xml` : `f22c6fcb5b1283243c19e999f4c19f802ea5faf8a24288535d85e9d0af509767`
- `xml/L1243-1.xml` : `85470e7c80175833696974f3efb6ed5d8d5cee3a8a0a7efc6b130e2475f88742`
- `xml/L1243-10.xml` : `07ab94e99702f8da12cb45ebf969ff0824f1aa995c4db314ec92e8d34be004dc`
- `xml/L1243-2.xml` : `6e11e20e12907576807b288b8a520b615b467da8fe405da0a59e8b3aba6afe13`
- `xml/L1243-3.xml` : `6032ae88607210650a0bb99abc5f28f420193115e91b2bff770c672448ae4588`
- `xml/L1243-4.xml` : `38ff60584322327d10668c1308cfe4904633a30e72c115152c49646bd7ebdf3b`
- `xml/L1243-5.xml` : `b4589aa15a3d6eccadfc52ff761a068e02503bdd43824cf689898d680fd4ec86`
- `xml/L1243-8.xml` : `f94c1b40ac8060f2998fdf491288bc835492b1a5f221b86f55bcee958538cc8b`
- `xml/L1243-9.xml` : `e490b7dfacbba27a95719a1ab0b0423e2e9f2a755a8bdc41c01a84a8b1cbf2c4`
- `xml/L2311-2.xml` : `53b0781f96a7922292520ef9ffe34eb4b40c6c14ced328eb072fe79e0b2b1c36`
- `xml/L2313-10.xml` : `d7802aa832b23f785163275400c195159594bc7b65fa797f1bc9633550222844`
- `xml/L2314-33.xml` : `e23585c83a65d22344f2907d28b9803312704bea5a99d6d30f7be653fe83ffc4`
- `xml/L2314-34.xml` : `64ad3aca1b03efcb595fa7dc0bf0de64d57a9d2547b29b1f361726a4abfc9cf5`
- `xml/L2315-61.xml` : `3a810bcbe9da33b3efbca6ea332ff3a0b0f0fd4617f8872adf10566e5c28d45c`
- `xml/L3121-18.xml` : `b6d510efc24c1b6173799e93099848f0aaa03675b34d16888d7b74ab930fb42f`
- `xml/L3121-19.xml` : `f09be13001185f9318b022e64cc4582e7a3425cd666dd772405008003771ac45`
- `xml/L3121-20.xml` : `d9ab6c0f17e227280915b6ff2c3b01fb451e553a9169438f3379544fadae177b`
- `xml/L3121-27.xml` : `593ba3a18127eec9b27e0b7216b5f957a3b5376625a57cffc6180e4ff82d53ab`
- `xml/L3121-28.xml` : `7e97ad9b8ec05fabeea5e1cff10cd14c8cbcfe81b1f8e473b75ac0d6720b9876`
- `xml/L3121-29.xml` : `4ba7be4f2d6ebdccc77f5625e1e178229c8e6179cbccc8adb3b071bde007d2d1`
- `xml/L3121-30.xml` : `19a26498737f17b44f9b7eb04223d40d54532c279bc75028991b9c70db83368d`
- `xml/L3121-33.xml` : `2c7ffb943921024a46a82433d16322bb4ea8cac58700612ee4b2e4ce52e7094b`
- `xml/L3121-34.xml` : `a9a3e693888ee1f57d42501c7c588f38d22039d533eff1a424aea00cb9c58dfa`
- `xml/L3121-35.xml` : `dbc2c046f1a6dce6e31658e73f899343d9314f400d4a8709cf380bd1d3459538`
- `xml/L3121-36.xml` : `b4fd29027e7af4182d0910e66b36fb6806f23fffc859574e44cb5e5690e4f9c5`
- `xml/L3123-1.xml` : `3e7c65d4ae5657dd3ec03bd50d4def297784cf4d212a0d023e4714ceeb69252e`
- `xml/L3123-20.xml` : `a31b680f2c9c2164a743e8df9c3bbc5bc13f9c9e251d33afca16ab899b993a8b`
- `xml/L3123-21.xml` : `17c23ae095c9f14fa07fe145fae7e13704e2fc22a67e9daf3fb9c2084cb88d19`
- `xml/L3123-22.xml` : `963235ecd79c9b0663fee6ad1dfa33472caeca1a17edb3aa7c24b9eb4b75645a`
- `xml/L3123-28.xml` : `034eab38a15fd04c7cc1b38ad9a3e4854eabe2aed211dc37fcca5787e39a9e33`
- `xml/L3123-29.xml` : `183d532165a8f09146651249c97b6f875c923878dcb2ed900d83cc784e024fc9`
- `xml/L3123-6.xml` : `fc40895dd277376a2a283993eacefc096718a7680af3801e757d461b878b3855`
- `xml/L3123-8.xml` : `a13755abb1631a1967df506b298d10645e7b091c99ddbb4ffde9388681117e2d`
- `xml/L3131-1.xml` : `79c46df27fd4e34641a1a749c5f576e2c46dea128e19173afa3f9464aec97a8e`
- `xml/L3131-2.xml` : `223e508ec1dbba2d3af791ab04b43003558e8b6185c1db3b04b8e7703621081e`
- `xml/L3133-1.xml` : `555fe0d94fd490771370ad3a345f57937f1bf577b6f39ebc8397fc0f99a6b5cd`
- `xml/L3133-2.xml` : `7f2e37460df209863284a61dc25f210cb0f3b441e8ed1b50e78dd69d93e1f5fc`
- `xml/L3133-3.xml` : `79689c51b67335942d1c8564cfee600a9a1ad776d1a3a572791667dfc695b667`
- `xml/L3133-4.xml` : `1b811d1b48d461df3a0565063c6f1f58eae019b36f4c9f50c38036804e55784d`
- `xml/L3133-5.xml` : `5a30d5c372614a8f78298d10a01374a7c481aa87d6ea5fcecba67f07cf89e0c3`
- `xml/L3133-6.xml` : `5b100e13193c8adc8875692d99d79cf4b6da02097a43f7bcc19a97af9f24e4ce`
- `xml/L3141-10.xml` : `bd1c653713d2a822a98ee90787b45a3b457d5749e5c2bfa2a89864450da464a0`
- `xml/L3141-24.xml` : `c9f63fe007813935be30e1fbc213b8de40370cdc2b8f742ba9e27b869cde9164`
- `xml/L3141-28.xml` : `cec6ec9122294009f082c8bf543ce627138f108abff00dbb822f214074f4de3e`
- `xml/L3141-3.xml` : `c77849ea1852f87e775f5c53ebdee78f6b37aac2a42bf680a889a94377a01d84`
- `xml/L3141-30.xml` : `c8cf0e4fcce37fb67fd4c8202023b1148bd169fa76e2164460d74f5bf51c286b`
- `xml/L3141-32.xml` : `8c92808039eff2d9606c12ca41295f7399c1a4c852b490ae4ccf31e654569461`
- `xml/L3141-4.xml` : `7aff750709e8251e4d2a45c28113fe9674c20cb547209f3e6e32061b37825193`
- `xml/L3141-5-1.xml` : `271e664fddfa9997507f55f60fd4a3a69e0fc4ec0d34c35bf170e928726bcd88`
- `xml/L3141-5.xml` : `7b694342fa2203150807fce2eb4ff3a0aaac5f9c308536e2b29aa3f142249710`
- `xml/L3141-6.xml` : `5841f15139066312989ea3acd0e1c6a34098676d77b54448d6c136bc397471ae`
- `xml/L3141-9.xml` : `89db9406c61b75408e9d3ef8d3545da273dff82bb84f3ce1549c7afce3aebcec`
- `xml/L3142-1-1.xml` : `f7f19b469fcbdf8dd6e527aa3f5706b2895c8f4087de13d32d27c1b6b21f2cf9`
- `xml/L3142-1.xml` : `463bb6810f128d6b3502f5b92192993c975eb44dd3a10547ac25d11bff9bf60a`
- `xml/L3142-105.xml` : `229a1444de27b731aa3cb5f9052c382d65f0447d4b20a6b764feda10283223aa`
- `xml/L3142-106.xml` : `82f1c61ce0c2bb9d6aada482272953acfa2f0abf98f9b0f10386852255c78933`
- `xml/L3142-107.xml` : `7a5488988b300b33393c15fb1ee34ad728fc1000768aee97c0f6f2ada2a87b55`
- `xml/L3142-108.xml` : `e83dc00a95ccfb5589c1cf5225f88df296314c0d600595a414b2f58ae4372581`
- `xml/L3142-109.xml` : `66761f7f3eeca545ef629871af0a7baf3d5b9a8a61494bbd8c996aa51ed73bdd`
- `xml/L3142-2.xml` : `8ee7de9196fafe2213f8cf8583a26911b28e2da752d19adfc2d6361c305ebcca`
- `xml/L3142-28.xml` : `e9a1d39f6528e429d1f34b737debfc06dc238961bfef48efb01d6e1fbf713974`
- `xml/L3142-29.xml` : `5bb816d10dd44949bc45ee47cfc11b1599cd1d1d8aeeef67a449fa246eb1a4b4`
- `xml/L3142-30.xml` : `cb5abc546b782eb11d8e5416ad83dbcca5abefa64c621933403495bab2d19b39`
- `xml/L3142-31.xml` : `31d0d8bb6376984b86e216a9b647579c82c04bd7e32202898e485052c99bd266`
- `xml/L3142-32.xml` : `9d1347b073dff8492e9e004a71d9233eb980afa3c62f58c4d5576ea58f835903`
- `xml/L3142-33.xml` : `91c518f524a8f3001fe45293a2cbac63583270928bb4514628698fb5fa94e4db`
- `xml/L3142-34.xml` : `ceba7c5590b7664d02459534b9a12044d0e9b607bb416eac749bfb75bde74fcf`
- `xml/L3242-1.xml` : `f8718a26ba4a7f1119ff92bfb149a194682c84515f7194d97686401f1b9d8c58`
- `xml/L3242-2.xml` : `38b82932c348b9e38aa3a241fdfcda7e584507d6ecd5f1d71aaa9f898c045e5c`
- `xml/L3242-3.xml` : `06e7d06c49fa8b612dddd3744a10968754caffcceb52c357f243d54f711409cf`
- `xml/L3243-1.xml` : `d40d9b4c42ed76e7465ec7c67385b4cb0af7ebb6ea63fbb18581e12333eb7bd3`
- `xml/L3243-2.xml` : `55bf12bc42e9b2b6a6a78e16bc925cb13f34201b90e3ed6d1abb831d428dee65`
- `xml/L3243-3.xml` : `900847d398bebed5fc980338d446844a30d54bf72468e0880b5380b2779cc97a`
- `xml/L3243-4.xml` : `f50c9c6054835b0a31a11c2dd98455442d5b125c07e7e06fef31311ddbb03318`
- `xml/L3251-1.xml` : `19ba549c40def848950565af6cc66f4501a12e6e74a569c2e9a65c53c1c7d533`
- `xml/L3253-18.xml` : `609ed8aa262333d6cc636407a80fbd93684f61782b6d3beac2d5803d84a9685f`
- `xml/L5422-14.xml` : `edfb4a0c63b1eedd5106c95eccc63b38aaa2288d3c6616708ed8534b22d20600`
- `xml/L5422-20.xml` : `6b627e69c2f72b2fa0a557bc2e63ba3a48891447fd13d22b0f7329740ac8de25`
- `xml/L5422-9.xml` : `40dae3a608365de1fe1ae6e45f9c64b9303da453174b2d5fed3a87a2207a9ae7`
- `xml/L6131-1.xml` : `ead711e99cb48ab7de0b9d19f7c0e09095976af7077b529aa1df278145f4b28f`
- `xml/L6131-2.xml` : `536a98477648c4b07a9019e7d7cc0038aab0afd7ca712bf1d7252f64a88a539d`
- `xml/L6241-1-1.xml` : `a793ba0c38b1f025da07949c67782944c3d6483b0f195977e2ae240300d20d3d`
- `xml/L6241-1.xml` : `15bc2485174f0aa27981f576f6d831622d27fa36a867e841a13198030fffc51b`
- `xml/L6242-1.xml` : `caf04ef96c6ca745ff9b1ca1131f354b196c3ca3470a4917fa813b8e828b6672`
- `xml/L6321-1.xml` : `3881b2cca0740f047a8955b10e25a8a57a2288d8a7506a4943015e1420f855f6`
- `xml/L6321-9.xml` : `194917ba6f6f0b632a3f0777b131bccd149f4b20777d0462aa99d7cc8884d7b6`
- `xml/L6331-1.xml` : `99d8a77b03ecc2f3a36ef54a1f5cceb2f94d8157944037a844dc96735a385163`
- `xml/L6331-1_A.xml` : `cce02e0dcb4b70c7abed1cf4b0a69a70cd2213669c94ba12722a4e0eb58a06b8`
- `xml/L6331-3.xml` : `e0224645080c06d1f0965ea61e9e1f677890ce49fd748d9734775fb307e00eaa`
- `xml/L6331-55.xml` : `9a5767408831e7aea3d7b81bed52dea7a82e9351a65d477fdc803306557c7e1b`
- `xml/L6331-6.xml` : `72ace87e6ebc4b3ba14afa7e3ffcb4a6e7e4e3a32449882c1e0a2ebf28a1eeba`
- `xml/L7122-1.xml` : `92e2a9881765c185e3767a5a0757cfb2c6f2e584147ff7c847bdea2639d7c9c7`
- `xml/L7122-2.xml` : `9beb91d780513f26222f830cb6e1abccccbecf48aa3af761a020c9615508dba8`
- `xml/L7122-22.xml` : `22892179fbdc75e3cda66897720fc9a6e92959a6a465519cab8dc47c29ba0421`
- `xml/L7122-23.xml` : `c42a531665ea838460a0935a1709f9c78a7d45bdbfadf8ed2a53a0b0bf189eda`
- `xml/L7122-3.xml` : `b223baa424694d372eede24382289c51844542fb924f72e5222df5c8b62e4e8c`
- `xml/L7122-4.xml` : `6d5c354d72ee21971bb7b0cc15e88d867e982fefa847ceac37e89da86c177a82`
- `xml/L7122-5.xml` : `26c361cef7aacf70d1cfdffa35ed00d678866ac5dc39d26af1f28e25a66de046`
- `xml/L7122-6.xml` : `09d0fa45e754c277cd786f4618847871bc0c9527bebb1bec9db90f5c49960aa8`
- `xml/R1221-1.xml` : `5cd2183259a6a4e2452f16be2679bb8e3ae33834597186d789874be4c0075b6a`
- `xml/R1221-3.xml` : `2dd33bc2eb5136b86f040d0502d8fbcbf7e91d23179d2cb802b34481b18bb2db`
- `xml/R1221-34.xml` : `29f093a4c2481ea028f62335eb963255b7b94452ff627af08e57d87a5df01934`
- `xml/R1221-35.xml` : `243e95fdfdbaf24c176671eb6c12e1d0d3222bd9e7d78ef6239ab3ba1d64354e`
- `xml/R1221-38.xml` : `9184b530e7bbb7053f73008efae77363a255306d9a3eab0caf8216d0a89cc670`
- `xml/R1221-39.xml` : `cd139aeddcf756eb96b981a265aa40481f92e4de791af9f5db78bfeaf0ff3d1e`
- `xml/R1221-4.xml` : `54e30454f53685baf39d6b053f35017348b61ace3263d601b37763f6a4e18c3e`
- `xml/R1221-40.xml` : `e565d087b936e5fa64c98024d5e3975105e46a384a0533e39dc5820b7c9d0ef8`
- `xml/R1234-1.xml` : `5f6ec8ee58567e51e4541862b0ce1ec8526ce26b0217bc726dab423914bc0de1`
- `xml/R1234-2.xml` : `359d6afe6dad158feb91f7fbde85f0d87ee80a3a29203d9b2fc626ef3fc559a1`
- `xml/R1234-4.xml` : `23e4613af99f2827dfb89f4d3ee1d5dfb4a7445d1b62165c4ae1988e78045f79`
- `xml/R3141-4.xml` : `6f107f50e9f523fb662e08d957f95ddb0b9539cc4d1f1fa9e819e5008c99cf66`
- `xml/R3243-1.xml` : `d885ac5f9d30d3457ecf4828b0cfa67d2776b7b3d4003f2f3e03192a95f783cc`
- `xml/R3243-2.xml` : `22bb7943b3df0bf57df319725539696e51d019496c7c4d8a27ba1173abcef98b`
- `xml/R3243-4.xml` : `73a8132040c386d8b129f9d86b1bbcc8e31ac93e4ad45bb4328c4d05d131d3a8`
- `xml/R3243-5.xml` : `7f09b2c92709f3c274ac928c67263ebbaf99a65bc91c999d568c7fd6cf433d34`
- `xml/R3243-6.xml` : `3c404dc02d36decb3d39fbb77dcd6d32076f82d5361f7d24bd1cfd22f352de6c`
- `xml-anterieur/L1226-1.xml` : `c0e959addc9a726cb1d258ec5bc19c56c96a4b8310e20faa36b429a320152b4d`
- `xml-anterieur/L1237-5.xml` : `efb64290f25a78f5faa9f4ee403d599647073c1183acbedf7b65e6ba00f86752`
- `xml-anterieur/L1237-9.xml` : `b3f7236feb33793bed3c0197499ab02e794970cd2bd0419003a8178a45e40422`
- `xml-anterieur/L3141-5.xml` : `ff05a46c5697342baf78fe37e25b321027865ef8852cd40660fd7bc035566744`
- `xml-anterieur/L6241-1-anterieures-20261011.md` : `d6cf17f16d51d9e237ce0e6c7118a634673b938f5fc8c2dc0b4cc59fa9e49db4`
- `xml-anterieur/L6241-1-v20220101.xml` : `21d521337c1182e6e162d44a9ab896e795e38fde415efbce2507c914afc2d263`
- `xml-anterieur/L6241-1-v20250216.xml` : `e4f747026ba718694f93b5f876cfb302b6590e067c20e4f7e954aac8aa738513`
