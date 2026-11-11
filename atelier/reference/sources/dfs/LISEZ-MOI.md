# La déduction forfaitaire spécifique (DFS) — les textes (lot R9, 11/10/2026)

Tout ce qui est ici vient de la **base LEGI de la DILA** (open data, `echanges.dila.gouv.fr/OPENDATA/LEGI/`, l'archive
globale du 13/07/2025 et les 456 livraisons jusqu'au 10/10/2026). Légifrance et le BOSS refusent le conteneur.

**Comment on les a trouvés.** Une sonde (script jetable, pas gardé) a fait défiler l'archive globale puis chaque livraison
en gardant tout article dont le titre parle de « frais professionnels déductibles » ou dont le texte dit « forfaitaire
spécifique », et l'article 5 de l'annexe IV du CGI. Elle a trouvé, **dans les livraisons des 08 et 09/09/2025**, ce que
personne au projet ne savait : **l'arrêté du 20/12/2002 est abrogé depuis le 07/09/2025**, remplacé par l'**arrêté du 4
septembre 2025** (JORFTEXT000052198430), qui inscrit en droit la baisse des taux que seuls Audiens et le BOSS donnaient ;
et l'**arrêté du 4 décembre 2019** (JORFTEXT000039626366) qui borne la réduction générale sous DFS. Les deux arrêtés en
vigueur sont relevés par l'outil (`atelier/outils/relever_les_codes.py`, qui sait depuis ce lot compléter l'entrepôt d'un
texte ajouté, `_completer`) ; ce que l'outil ne garde pas (les annexes sans numéro, une version périmée) est déposé brut.

| dossier | ce que c'est |
|---|---|
| `arrete-2025-frais-professionnels/` | relevé de l'outil (`moteur-atelier releve-codes`) : art. 1, 2, 3, 5, 9 et 10 de l'arrêté du 4 septembre 2025 (art. 3, 5 et 10 relevés le 03/11/2026, `articles-20261103.md`) (`releve.json`, `xml/`, `articles-*.md`) |
| `annexes-2025/` | ses deux annexes, brutes (LEGIARTI000052206429 et …431, en vigueur depuis le 07/09/2025), et leur transcription |
| `arrete-2019-reduction-generale/` | relevé de l'outil : art. 1 et 2 de l'arrêté du 4 décembre 2019 (art. 1er dans la rédaction du 07/09/2025) |
| `cgi-annexe4-art5/` | l'article 5 de l'annexe IV du CGI **dans sa rédaction du 31/03/2000 au 31/03/2002** (LEGIARTI000006300240, état « PERIME ») : celle « en vigueur au 31 décembre 2000 » que l'arrêté cite |
| `historique/` | l'art. 9 de l'arrêté du 20/12/2002 (versions du 27/12/2002 et du 06/08/2005) et l'annexe de l'arrêté du 25/07/2005 : le droit des paies d'avant le 07/09/2025 ; et **l'arrêté du 20/12/2002 entier, dans sa dernière rédaction** (art. 3, 5 et 10 du 02/11/2022 au 07/09/2025), imprimé de Légifrance par Léandre le 03/11/2026 (`legifrance-arrete-20021220-frais-professionnels-20250907.pdf`, SHA-256 `d59a462f…7cd35f3c`, et sa transcription `.txt`, `197bd332…cc3a2e51`) : les règles du repas et du grand déplacement sont celles de 2025, ses montants ceux de 2022, revalorisés chaque 1er janvier par le barème de l'Acoss (art. 10) ; les montants de 2025 : `../legisocial-frais-professionnels-2025/` |
| `autres/` | l'arrêté du 19/12/2024 portant agrément de la convention d'assurance chômage, l'article qui déroge à l'art. 49 du règlement général (journalistes) |

Les codes sont relevés à leur place : CSS R. 242-1, L. 241-13 et D. 241-7 (`../code-de-la-securite-sociale/`, ajoutés à
`TEXTES` dans ce lot), L. 136-1-1, L. 242-1 ; code du travail L. 6331-1, L. 6331-55, L. 6241-1-1, L. 6331-6.

## Ce qu'on en lit, et où le moteur s'en sert

**Arrêté du 4 septembre 2025, art. 9** (`calcul::deduction_forfaitaire`, `calcul::abattement_dfs`, barèmes
`deduction_forfaitaire`) :
- I, al. 1 : « les professions, prévues à l'article 5 de l'annexe IV du code général des impôts dans sa rédaction en
  vigueur au 31 décembre 2000 […] peuvent bénéficier d'une déduction forfaitaire spécifique. Cette déduction est, dans la
  limite de **7 600 euros par année civile**, calculée en fonction des taux prévus à l'article 5 […] ». → le plafond
  annuel, suivi par salarié (`dfs_deja_deduite`, `conges::dfs_des_mois_anterieurs`).
- I, al. 2 : « L'employeur peut opter […] lorsqu'une convention ou un accord collectif l'a explicitement prévu ou lorsque
  le comité d'entreprise, le comité social et économique ou les délégués du personnel ont donné leur accord. » → fiche
  entreprise `deduction_forfaitaire_option` : `accord_collectif`, `accord_cse`.
- I, al. 3 : « A défaut, il appartient à chaque salarié d'accepter ou non cette option. Celle-ci peut alors figurer soit
  dans le contrat de travail ou soit dans un avenant […]. A défaut, l'employeur informe et recueille le consentement
  annuel du salarié par tout moyen. […] son silence vaut accord. Le salarié peut toutefois demander à tout moment […] à
  bénéficier ou ne plus bénéficier de la déduction, avec application au 1er janvier de l'année suivante. » → option
  `accord_individuel`, fiche salarié `deduction_forfaitaire_accord` : `contrat`, `consentement_annuel`, `refus` ; sans
  accord dit, la paie refuse.
- I, al. 4 : l'assiette comprend « le cas échéant, les indemnités versées […] à titre de remboursement des frais
  professionnels, à l'exception de celles [de] l'annexe ». → `frais_hors_assiette` des professions ; tout autre
  remboursement de frais reste hors de l'assiette lui aussi, par la tolérance du BOSS, § 2320 (lot DF2, 14/10/2026 ;
  `frais_cumulables` du barème, `calcul::dfs_applicable` ; voir `../boss/LISEZ-MOI.md`).
- I, dernier al. : « sans préjudice du sixième alinéa de l'article R. 242-1 du code de la sécurité sociale » → l'assiette
  minimum (`abattement_dfs`).
- II : le spectacle vivant à 25 % au 31/12/2023 perd 2 points par an depuis 2024, 3 depuis 2026, nul en 2032 (23, 21,
  **18**, 15, 12, 9, 6, 3, 0) ; à 20 % : 1 point depuis 2024, 2 depuis 2026, 3 depuis 2029, nul en 2032 (19, 18, **16**, 14,
  12, 9, 6, 3, 0). → `taux` des barèmes, que la webconférence Audiens 2026 (p. 22) donnait déjà.
- III : les autres professions de l'article 5 perdent, à compter de 2026, 15 % de leur taux de 2025 par an, arrondi à
  l'unité (0,5 compte pour 1). → l'artiste cinématographique (21 % en 2026) ; **inférence** : il ne relève pas du
  spectacle vivant.

**Arrêté du 4 septembre 2025, art. 3, 5 et 10** (`engagement::frais_au_dela`, barèmes `repas_deplacement` et
`logement_grand_deplacement`) — le régime commun des frais de tournée, pour tout salarié qui n'est pas sous DFS, et sous
DFS pour ce que l'annexe 2 ne nomme pas :
- art. 3, 1° : le salarié « en déplacement professionnel et empêché de regagner sa résidence ou lieu habituel de travail »,
  21,10 € par repas ; 3° : hors des locaux, quand « il n'est pas démontré que les circonstances ou les usages de la
  profession l'obligent à prendre ce repas au restaurant », 10,30 € (BOSS § 240-260 : le 1° vaut pour le salarié contraint
  de manger au restaurant). → la case « au restaurant » d'une ligne de repas, qui ne se montre qu'au-dessus de la limite
  basse ; le 2° (la restauration sur le lieu de travail) n'est pas un repas en déplacement, la rubrique `repas` ne le porte pas ;
- art. 5, I, 1° : en grand déplacement, le repas à la limite du 1° de l'art. 3, sans case (un repas d'un jour dont la nuit, ou celle de la veille, est une nuitée de l'engagement) ; le logement et le petit-déjeuner « 75,60 euros par
  jour [...] à Paris et dans les départements des Hauts-de-Seine, de Seine-Saint-Denis, du Val-de-Marne et 56,10 euros par
  jour [...] dans les autres départements de la France métropolitaine » → la case « Paris ou petite couronne » d'une ligne
  de nuitée ; le grand déplacement se lit d'une nuitée de l'engagement, la nuit même ou la veille : le salarié n'a pas
  regagné son domicile ce soir-là, et « lorsque le travailleur salarié ou assimilé est empêché de regagner son domicile en
  fin de journée pour des circonstances de fait, il est considéré comme étant dans la situation de grand déplacement » ;
  la présomption des 50 km et 1 h 30 n'a pas à être vérifiée en plus ; le II (au-delà de
  trois mois dans un même lieu, 85 % puis 70 %) et les 2° à 4° (outre-mer, étranger) ne sont pas portés ;
- art. 10 : ces montants se revalorisent chaque 1er janvier ; les valeurs de 2026 (21,40, 10,40, 76,60, 56,80 €) sont
  celles de l'Urssaf (`../urssaf-frais-professionnels/`, mise à jour du 30/10/2026) et
  du BOSS (`../boss/boss-frais-professionnels-20261013.txt`, § 210, 240, 1290) ; du 1er janvier au 6 septembre 2025,
  l'arrêté du 20/12/2002 (`historique/`, mêmes règles) et les montants de 2025 (`../legisocial-frais-professionnels-2025/`),
  les mêmes que ceux du 07/09/2025 ;
- au-delà de la limite, la fraction excédentaire entre dans l'assiette, sauf si l'employeur prouve qu'elle a été utilisée
  conformément à son objet (BOSS § 270, § 1280) → les rubriques `repas_au_dela` et `nuitee_au_dela`, et la case
  « justificatif conservé ».

**Annexe 1** (« Principales professions concernées par la DFS ») : « Musiciens, choristes, chefs d'orchestre, régisseurs
de théâtre 20 % » ; « Personnel de création de l'industrie cinématographique 20 % » ; « Artistes dramatiques, lyriques,
cinématographiques ou chorégraphiques 25 % ». Le personnel de création du cinéma n'est pas proposé (pas du spectacle
vivant, pas dans l'article 5 de 2000).

**Annexe 2** (`frais_hors_assiette`) : I, 2° « Les indemnités journalières de "défraiement" versées aux artistes
dramatiques, lyriques, chorégraphiques ainsi qu'aux régisseurs de théâtre, qui participent à des tournées théâtrales » ;
I, 5° « Les allocations et remboursements de frais perçus par les chefs d'orchestre, musiciens et choristes à l'occasion
de leurs déplacements professionnels en France et à l'étranger ». **Inférence** : un défraiement du catalogue
(`defraiement_forfait`, `defraiement_reel`) payé à ces professions est celui d'une tournée ou d'un déplacement ; un
remboursement au réel n'est pas une « indemnité journalière ».

**CGI, annexe IV, art. 5 (rédaction du 31/03/2000)** : « Artistes dramatiques, lyriques, cinématographiques ou
chorégraphiques : 25 %. Artistes musiciens. Choristes. Chefs d'orchestre. Régisseurs de théâtre : 20 %. » → les huit
professions de la fiche salarié et leur catégorie.

**Arrêté du 4 décembre 2019, art. 1er** (rédaction du 07/09/2025 ; `droit::reduction::reduction_generale_sous_dfs`) : « Le montant
de la réduction générale […] calculé pour les rémunérations après application de l'abattement forfaitaire […] ne peut
excéder, pour ces rémunérations, 130 % du montant la réduction calculé sans application du même abattement. » CSS
L. 241-13, III, y renvoie (« que dans des limites et conditions fixées par arrêté »).

**CSS R. 242-1, al. 6** et BOSS « Assiette générale » § 410 (`../boss/boss-assiette-generale-20260801.txt`, l. 483-506) :
la base ne descend pas sous le SMIC majoré des seuls éléments légaux. → `abattement_dfs` : le SMIC du contrat que la DSN
retient (SMIC horaire × heures). **Inférence** : pour un cachet, les heures sont les 7 h que le moteur lui donne
(`heures_par_cachet`) ; aucun texte déposé ne dit les heures travaillées d'un cachet.

**Ce qui ne s'abat pas** :
- le chômage des intermittents : annexes VIII et X du 15/11/2024, art. 49 (`../plafond-intermittents/`, l. 1198-1204 et
  1176-1180) : « avant application de l'abattement pour les professions admises au bénéfice de la déduction pour frais
  professionnels de 20 % ou de 25 % » ;
- la CSG/CRDS : son propre abattement (CSS L. 136-1-1 ; BOSS assiette générale § 240) ;
- les Congés Spectacles : norme DSN 2026.1, bloc 78, p. 292 (« avant toute déduction forfaitaire spécifique ») ;
- les contributions conventionnelles : FNAS 1285 « sur la totalité des salaires bruts avant abattement »
  (`../kali-1285-titres-ii-iii-x-xi-xii-20260923.md`, l. 403) ; la prévoyance des intermittents, dont les textes ne
  renvoient pas à l'assiette de la sécurité sociale.

**Ce qui s'abat** : les cotisations de sécurité sociale (R. 242-1, al. 3 ; arrêté de 2025, art. 1er) et ce qui s'assoit
sur leur assiette (formation, L. 6331-1, L. 6331-55 ; apprentissage, L. 6241-1-1 ; FNAL, CSA, VM, dialogue social), majorées
après l'abattement (Acoss 2013-019, R6, `../majoration-caisses-conges/`) ; l'Agirc-Arrco (cahier Agirc-Arrco CT2026,
`../agirc-arrco/…ct2026…txt` l. 3490-3498 : « La solution ainsi retenue pour le régime général […] s'applique en principe
au régime AGIRC-ARRCO »).

**La DSN** (`dsn::cle_abattue`, `droit::base_assujettie::BaseAssujettie::abattue_par_la_dfs`, `dsn::brut_du_51`, `dsn::remunerations`,
`dsn::autre_revenu`, `dsn::cotisations`, `dsn::assiettes_ctp`) :
- 40.023 = le taux, « systématiquement le taux de l'abattement appliqué ou applicable » (fiche net-entreprises 1336,
  `../net-entreprises-base-connaissances/net-entreprises-fiche1336-dfs-taux-40-023-20250322.txt`) ; il change au
  1er janvier (`changements_contrat`, modalité annuelle) ;
- bloc 54 de type 10 = la déduction retenue, par contrat (V2d ; norme 2026.1, bloc 54 p. 257, CCH-16 du bloc 50) ;
- **le 51 de type 001 est le brut abattu** : le guide Urssaf du 04/06/2026, p. 36 (`../editeur-dsn/guide-urssaf-regularisation.txt`,
  l. 1440-1459), recompose les revenus bruts du montant net social par « 51 de type 001 + 54 de type 10 − 54 de type 35 »,
  et le BOSS (montant net social, Q 26, `../montant-net-social/boss-montant-net-social-20261001.txt`, l. 387-393) veut
  « l'ensemble de la rémunération brute du salarié […] indépendamment des abattements » : un 51 de type 001 au brut entier
  compterait la déduction deux fois ;
- **les bases 02, 03 (sécurité sociale), 23 et 43 (Agirc-Arrco) sont abattues, et leurs 81.003 avec elles** ; la
  réduction générale (81 de code 018/106 sous la base 03) aussi. Ce qu'on lit : le cahier Agirc-Arrco CT2026
  (`../agirc-arrco/agirc-arrco-aide-codification-dsn-ct2026-v1.0-20251201.txt`, l. 6230-6236) déclare la « Base VRP
  multicartes après abattement pour frais professionnels » en « 03 - Assiette brute déplafonnée » et « 02 - Assiette
  brute plafonnée » ; la norme 2026.1, chapitre 2 (`../../norme-dsn/texte/chapitre-2-principes-de-constitution.txt`,
  l. 2116-2160), part, pour le net fiscal, du « montant brut […] servant de base au calcul des cotisations de sécurité
  sociale » et y AJOUTE « le montant de l'abattement forfaitaire d'assiette pour frais professionnels » ; le bloc 81
  (p. 301) : « L'assiette est déterminée par application de plafonnements et/ou d'abattements sur la base assujettie ».
  **INFÉRENCE**, écrite comme telle : aucun texte déposé ne dit en toutes lettres qu'un artiste sous DFS déclare sa base 03
  abattue ; la fiche 1336 ne dit rien des bases (D7 reste à confirmer, voir plus bas) ;
- bases 04 (CSG), 07 (chômage, et le 51 de type 002), 27 (Congés Spectacles, « avant toute déduction forfaitaire
  spécifique », bloc 78 p. 292) et 31 (prévoyance) sur le brut ;
- le bordereau suit les bases : les CTP de la sécurité sociale (027, 312, 334, 432, 636, 983, 992, 100, 332…) sur
  l'abattu, le 260 (CSG) sur sa propre assiette ;
- le montant net social n'est pas diminué de l'abattement (BOSS, montant net social, Q 26).

## Ce qui manque, pour Léandre

1. **Le BOSS « Frais professionnels », chapitre 9 (DFS)** : **déposé le 13/10/2026** (imprimé en PDF par Léandre depuis le
   poste : `../boss/boss-frais-professionnels-20261013.pdf`, version en vigueur au 01/01/2026 ; ce qu'on en lit, § par §, dans
   `../boss/LISEZ-MOI.md`). Il confirme le calendrier des taux (16 et 18 % en 2026), le plafond de 7 600 € apprécié par
   employeur (§ 2160 : le point 5 ci-dessous n'est plus un manque), l'option et le consentement (§ 2180-2210, § 2330), et
   dit deux choses de plus : **§ 2320, pour le spectacle vivant, les remboursements de frais au sens de l'arrêté se cumulent
   avec la DFS par tolérance, sans réintégration** (le refus du point 4 ci-dessous est à lever : un lot de code), et § 2150,
   pas de DFS sur un mois complet d'absence (servira au point 2). **Le refus est levé (lot DF2, 14/10/2026)** : le barème
   porte la tolérance (`deduction_forfaitaire.frais_cumulables`, 2025 et 2026), `calcul::dfs_applicable` la lit.
2. **Un salarié au mois sous DFS** (permanent, CDD de droit commun) : refusé. Son chômage relève de l'art. 49 du règlement
   général (« entrant dans l'assiette des cotisations de sécurité sociale ») sans l'« avant application de l'abattement »
   des annexes VIII et X ; l'arrêté d'agrément du 19/12/2024 (`autres/`) ne le dit « avant abattement » que pour les
   journalistes : *a contrario*, le chômage d'un permanent sous DFS serait abattu (inférence, non portée). Sa prévoyance :
   3090 « servant de base aux cotisations de la sécurité sociale » (titre XII, 12.6.1), 1285 rien. Ses tranches Agirc-Arrco
   mensuelles et son assiette minimum (151,67 h). Un lot à part.
3. **Un cadre sous DFS** : refusé (l'assiette de l'Apec sous DFS n'est dans aucun texte déposé ; ses tranches journalières).
4. ~~**Un remboursement de frais réintégré** (hors annexe 2) : refusé.~~ **Levé (lot DF2, 14/10/2026)** : le BOSS, § 2320,
   laisse tout remboursement de frais au sens de l'arrêté hors de l'assiette pour les huit secteurs ; rien n'est réintégré,
   le bloc 54 de type 35 ne s'écrit toujours pas. Reste refusée (`calcul::dfs_applicable`, lot DF2) une ligne hors
   assiette qui n'est pas un remboursement de frais (une rubrique de nature `indemnite`) : le rescrit BOSS-RES-000010
   (`../boss/boss-frais-professionnels-20261013.txt`, § 2325) veut une indemnité de transport conventionnelle dans
   l'assiette « dès le 1er euro », et aucun texte déposé ne dit ce qu'une autre indemnité hors assiette devient sous DFS.
   Le catalogue livré n'a aucune indemnité conventionnelle de ce genre pour un contrat d'usage. Réintégrer toucherait l'assiette du chômage (les annexes disent
   « avant abattement », pas « sans les frais ») et la DSN (bloc 54 de type 35, `bloc-S21.G00.54.txt` l. 39-43).
5. **Le cumul d'avant le logiciel** : une compagnie qui arrive en cours d'année avec une DFS déjà déduite ailleurs n'a
   pas de case pour la reprendre ; le plafond de 7 600 € compterait de zéro.
6. **Les paies 2025 d'avant le 07/09/2025** : l'arrêté de 2002 en vigueur (art. 9, version 2005, `historique/`) ne portait
   aucun calendrier de baisse ; le barème 2025 suivait la doctrine (18 / 21 %). Rien n'est refait (règle 4).
7. **D7, à moitié levé (lot T1, 13/10/2026)** : voir « Lot T1 » plus bas. Restent sans texte : les bases **23 et 43** (Agirc-Arrco)
   et les **81.003** d'un salarié sous DFS ; aucune fiche dsn-info ou Urssaf ne les nomme (dsn-info.fr et urssaf.fr refusent le
   conteneur). Le moteur les abat par inférence (ci-dessus, « La DSN ») ; un dépôt de test portant une DFS (bilan CCO/BAN) le
   confirmerait.
8. **L'annexe 2, I, 4°** (les allocations de « saison » des théâtres municipaux, subventionnés et des casinos) : le
   catalogue n'a pas de rubrique pour elles ; depuis le lot DF2, une rubrique de frais de la compagnie qui les paierait
   reste hors de l'assiette (point 4).
9. **Les paies d'avant le 07/09/2025** : l'arrêté de 2002 (`historique/`) voulait, à défaut d'accord collectif ou du CE,
   une lettre recommandée avec coupon-réponse ; le moteur applique à toute l'année 2025 les modalités d'accord de
   l'arrêté de 2025 (contrat, avenant ou consentement par tout moyen). Une paie 2025 ne se refait que pour une
   régularisation.

## Lot T1 (13/10/2026) : ce que les fiches net-entreprises disent de la DFS en DSN

La base de connaissances net-entreprises répond le 13/10/2026 (la recherche, non ; le « Plan du site » non plus) : on en a parcouru les
catégories (`list-search/p/<n>/page/<m>`), obtenu le catalogue de **869 fiches**, puis relevé celles dont le texte intégral parle
de DFS (866 lues sur 869 ; 3 refusées par le serveur à chaque essai, 1643, 2598 et 3273, `Connection reset by peer`). Les fiches utiles sont
déposées dans `../net-entreprises-base-connaissances/` (nom, URL, date et ce qu'on en lit : son `LISEZ-MOI.md`).

| fiche | ce qu'elle dit de la DFS | accord avec le moteur |
|---|---|---|
| **484** (modifiée le 01/07/2025) | le 51 de type **001** « doit prendre en compte l'abattement » ; le 002 aussi dans le cas général, sauf journalistes et **intermittents du spectacle** (« avant application de l'abattement […] de 20 % ou de 25 % ») ; les primes et indemnités de fin de contrat (bloc 52) en brut non abattu | **oui** : `dsn::remunerations` (001 = `brut_du_51`, 002 = `assiette_chomage`) ; c'est le texte qui manquait à D7 pour le 51 de type 001 |
| **1970** (27/06/2025) | l'exemple d'un VRP multicarte sous DFS à 30 % : brut 20 000, **54/10 = 6 000**, **base 03 = 14 000** (abattue), base 02 = 10 284 (3 × plafond) | **oui** pour 54/10 et la base 03 ; **inférence** de l'étendre à un artiste |
| **1336** (22/03/2025, déjà déposée) | 40.023 = le taux appliqué ; « la base de calcul des cotisations est constituée par le montant global des rémunérations […] à laquelle on applique la déduction » | oui |
| **827** (22/03/2025) | l'avantage en nature du bloc 54 « ne tient pas compte de cette déduction » | oui (`dsn::autre_revenu`) |
| **2681** (09/06/2026) | les types 028/029 (SMIC de la réduction générale en cas d'absence) sont « avant DFS » | **à voir** : le moteur n'écrit pas ces types |
| 862 (déjà déposée) | le salaire de référence des Congés Spectacles est « avant toute déduction forfaitaire spécifique » | oui (base 27) |
| 2336 | la DFS du BTP (10 %), `S21.G00.40.023` et `41.017` pour un changement | sans objet (BTP) |

**Ce que ces fiches ne disent pas** : qu'un artiste sous DFS déclare ses bases **23 et 43** et ses **81.003** abattus (inférence
du cahier Agirc-Arrco, inchangée) ; aucune fiche lue (866 sur 869) ne contredit le moteur. **Ce qui contredit le moteur** : rien.

**Le BOSS, chapitre « Frais professionnels »** : `https://boss.gouv.fr/portail/accueil/frais-professionnels.html` (et la page DFS du
chapitre 9 qu'on y trouve). `curl` : `Recv failure: Connection reset by peer` à l'ouverture TLS (HTTP en clair : redirection 302 vers
HTTPS). Même résultat pour `boss.gouv.fr/` et `www.boss.gouv.fr/`. À imprimer depuis le poste.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `annexes-2025/legi-arrete-20250904-frais-professionnels-annexe1-professions-20250907.txt` : `686f9532e3e0c0a99b8941e8ba1f3e521a4854eafbee31db51783192a1ee3468`
- `annexes-2025/legi-arrete-20250904-frais-professionnels-annexe1-professions-20250907.xml` : `9542246976b2d4ffd7bac587a2f67761972f45290dcd5da1ca8b6f32ed7ca066`
- `annexes-2025/legi-arrete-20250904-frais-professionnels-annexe2-frais-hors-assiette-20250907.txt` : `99b1f38c0981b8b90c1e284020c43f3c0b4247121b4b74110ad65a7541eada32`
- `annexes-2025/legi-arrete-20250904-frais-professionnels-annexe2-frais-hors-assiette-20250907.xml` : `c590a7197e558c679947568b34812f34f91974033c86b840a3f872925de55cda`
- `arrete-2019-reduction-generale/articles-20261011.md` : `e29fbc6afa3e3338e8ba6948f6e2083a260be9cb97ba2dbbac9ff71cf0f345cf`
- `arrete-2019-reduction-generale/articles-20261015.md` : `3bb0883569013109f40fbf615a23cec422cc7ea76bf52d29a03ecd1240786ade`
- `arrete-2019-reduction-generale/articles-20261103.md` : `f4b364f9b17bffe639692b140bca7bd7ca20679dc280adc8d2a1c95222c08021`
- `arrete-2019-reduction-generale/articles-20261110.md` : `23081ab095968c590073f43bfb1fea75323dd63e2d78fc39f9cacacb33c8e0a6`
- `arrete-2019-reduction-generale/releve.json` : `599ed2aca448ab0e33c0c93c8fb8648b467de6a534f70abeb7398bd140bae0e5`
- `arrete-2019-reduction-generale/xml/1.xml` : `91b8ea4c353f6d123c49c6509a5a73a7a7ee7f33ca73fefff6194193efd2aeca`
- `arrete-2019-reduction-generale/xml/2.xml` : `723738266d111f18a4bccadc2f80c8dde7e9deef7a7c951bd7f74549274f240b`
- `arrete-2025-frais-professionnels/articles-20261011.md` : `cb91ee888f3394284b6cabc9dd3e4ef9c57e39fef8ff4b146d0a07ff296aa6c5`
- `arrete-2025-frais-professionnels/articles-20261015.md` : `57df1ca7b27ca3cb72f5249b7c5237ecc52edbd7e4dbc6ee572f93c37113ffec`
- `arrete-2025-frais-professionnels/articles-20261103.md` : `f53d6c5a772b8329d685ecf38cfa8fab2f761a5d201eeaf26450e7a27a69ed65`
- `arrete-2025-frais-professionnels/articles-20261110.md` : `9cb557bc8624342798dbf58a6256d2bd0a4b66695700b98056403fa5e5b57b29`
- `arrete-2025-frais-professionnels/releve.json` : `e63afb8d25402184e86bfa8fb01c50b5d436f2962bcf5d71f82fed1eebc35950`
- `arrete-2025-frais-professionnels/xml/1.xml` : `a2bfabe8e3dda6bebb4fbae9ad7e3556dab529ed022062e8580fa79db62f4a75`
- `arrete-2025-frais-professionnels/xml/10.xml` : `e326fd7bbe34531f9abef66101949bb065b40c871d75d9d85b640ddfdb1987c4`
- `arrete-2025-frais-professionnels/xml/2.xml` : `b6305eae2039634b05ee16ef3ca3f53191c3c589dc920890904eae3e2acee250`
- `arrete-2025-frais-professionnels/xml/3.xml` : `206ec30866ace071c8a7cfa4da963d969768bf39015bbfc0be57a62c707b35ef`
- `arrete-2025-frais-professionnels/xml/5.xml` : `150a95725bf392b9e663ec3369cb8a284505807c19a64f4c8e9a182048537940`
- `arrete-2025-frais-professionnels/xml/9.xml` : `60230694b3558a17634c0897b51f4b778d0581f31ab656d31768d72a9bfa5441`
- `autres/legi-arrete-20241219-agrement-convention-chomage-annexes-20241219.txt` : `eff1a43441529ea199b2462120603e87899a3727d5f6ab4b07e3e5960318decb`
- `autres/legi-arrete-20241219-agrement-convention-chomage-annexes-20241219.xml` : `e22f80fef3f834d9af14325325b91c7b5d034c90c49658ae2a51c670e5a3c0ce`
- `cgi-annexe4-art5/legi-cgi-annexe4-art5-redaction-20000331.txt` : `20e9e9b4d7a36c7ae210eb33779dfe2a8f88b86f052cd67e1d854e173aa39228`
- `cgi-annexe4-art5/legi-cgi-annexe4-art5-redaction-20000331.xml` : `9d2fafc86e62ef4580c7ef294a4789dc93c0147e8665f8b04e644ee0a111cc84`
- `historique/legi-arrete-20021220-art9-redaction-20021227.txt` : `5d7a44bd1bc18d6320fbfe89ef34b14260b921fc4fa52e96bc42bc37aabab78e`
- `historique/legi-arrete-20021220-art9-redaction-20021227.xml` : `9aa62bc95866f380596af5a72827e90fd0f6ceed6655bc918f84360587b3c494`
- `historique/legi-arrete-20021220-art9-redaction-20050806.txt` : `afce31ddcba7988073709546f2d6c343cc6874b57534ab64ff88a6bcd2b0f314`
- `historique/legi-arrete-20021220-art9-redaction-20050806.xml` : `84ccd913fbb1ed2b667c48095a68bd20afda2a162ee16632efbc4d4527130254`
- `historique/legi-arrete-20050725-annexe-frais-hors-assiette-20050806.txt` : `8aed3613255c9a4d6c25bdebc9f665c8548d7f507e4084404fc554f66b9c0cee`
- `historique/legi-arrete-20050725-annexe-frais-hors-assiette-20050806.xml` : `fdb67f6eee8ae646e7c78e35a69c024db0c3f36aad7ec177d2c001d76a9627b4`
- `historique/legifrance-arrete-20021220-frais-professionnels-20250907.pdf` : `d59a462f902e6ba03989c796577dc46ef7fb34acb251610d724731dc7cd35f3c`
- `historique/legifrance-arrete-20021220-frais-professionnels-20250907.txt` : `197bd3325f1c719305c85e1d4bc4428f275254f4e13e9ddaa3a0c044cc3a2e51`
