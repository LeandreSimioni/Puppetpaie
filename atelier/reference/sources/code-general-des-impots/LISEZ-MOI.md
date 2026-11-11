# Code général des impôts : le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `paie releve-codes`** (`moissonner`, puis
`rapport`) depuis la base LEGI de la DILA (`https://echanges.dila.gouv.fr/OPENDATA/LEGI/`) ; ils ne se retouchent pas à la
main. Les articles suivis sont la liste du texte `impots` de `atelier/sources/paie/src/releve_codes/textes.rs`, où chaque
domaine dit pourquoi il est là ; le dernier relevé est le plus récent des `articles-*.md`. `formulaires/` garde les notices
de la DGFiP de la taxe sur les salaires (2501-SD, 2502-SD), copiées telles que servies, chacune avec sa transcription
`.txt` (`pdftotext -layout`, faite le 04/11/2026 sur le PDF déposé) : 2501-SD 2025, II (taux normal 4,25 % ; 1er taux
majoré 4,25 % au-delà de 9 147 €, 2e taux majoré 9,35 % au-delà de 18 259 €), III (franchise 1 200 €, abattement 24 041 €,
décote des trois quarts jusqu'à 2 040 €) ; 2501-SD 2026, mêmes paragraphes (9 229 €, 18 423 €, 1 200 €, 24 256 €,
2 040 €) ; 2502-SD 2025, II B et C (mêmes valeurs 2025 ; C, 1 : le chiffre d'affaires de 293 B). Où le moteur s'en sert :
`taxe_sur_les_salaires` des barèmes 2025 et 2026. Ce fichier dit, par sujet ajouté
depuis qu'il existe, ce qu'on en lit et où le moteur s'en sert (les sujets plus anciens sont décrits dans les commentaires de
`textes.rs` et dans les LISEZ-MOI des dossiers qui les citent : `bloc-44-assujettissement/`, `suspension-non-remuneree/`…).

## Les plafonds de la franchise en base de TVA (relevé du 21/10/2026, ticket #239)

`articles-20261021.md`, section « La taxe sur les salaires » : `xml/293_B.xml` (`LEGIARTI000052488142`, version en vigueur
depuis le 01/03/2025, modifiée par la loi n° 2025-127 du 14/02/2025, art. 32, et la loi n° 2025-1044 du 03/11/2025, art. 1),
à côté de `xml/231.xml`.

- **CGI art. 231, 1, 2e alinéa** : « Les rémunérations versées par les employeurs dont le chiffre d'affaires réalisé au cours
  de l'année civile précédant le versement de ces rémunérations n'excède pas les plafonds définis aux I et I bis de l'article
  293 B sont exonérées de la taxe sur les salaires. »
- **CGI art. 293 B, I** : la franchise vaut pour qui n'a pas réalisé un chiffre d'affaires « excédant les plafonds suivants »,
  ligne « Année civile précédente » : **85 000 €** de « chiffre d'affaires national total » et **37 500 €** de « chiffre
  d'affaires national afférent aux prestations de services autres que les ventes à consommer sur place et les prestations
  d'hébergement » (ligne « Année en cours » : 93 500 et 41 250 €, qui ne servent pas ici).
- **CGI art. 293 B, I bis** : les avocats, les auteurs d'œuvres de l'esprit et les artistes-interprètes, pour leurs propres
  opérations : 50 000 et 35 000 €.
- **Notice 2502-SD** (`formulaires/impots-2502-sd-2025-notice.pdf`, C, 1) : ce chiffre d'affaires « doit s'apprécier au regard
  de l'ensemble des recettes et autres produits, y compris ceux qui n'entrent pas dans le champ d'application de la TVA
  (notamment la perception de dividendes et subventions non imposables à la TVA) ».

**Inférences**, écrites comme telles :
- la colonne qui compte est « année civile précédente » : 231 compare le chiffre d'affaires de l'année d'avant le versement ;
- une compagnie vend des prestations de services (un spectacle cédé, une billetterie, un atelier) : son plafond est 37 500 €
  pour ces recettes, et 85 000 € pour l'ensemble, subventions comprises (la notice les compte dans le chiffre d'affaires ; elles
  ne sont pas des prestations, elles comptent dans le total) ;
- le I bis ne vise pas une compagnie employeur (elle n'est ni avocat, ni auteur, ni artiste-interprète) : il n'est pas repris ;
- **2025** : cette version de 293 B vaut depuis le 01/03/2025. Les salaires de janvier et février 2025 relevaient de la version
  précédente, qui n'est pas déposée : l'entrepôt LEGI ne garde que la version en vigueur, et le relevé dit seulement qu'elle a
  changé (`LEGIARTI000051215135` → `LEGIARTI000052488142`, livraison du 04/11/2025).

**Où le moteur s'en sert** : `logiciel/donnees/baremes/2025.json` et `2026.json`, `taxe_sur_les_salaires.plafonds_franchise`
(`prestations_de_services`, `total`, `en_vigueur_depuis`) ; `assujettissement::question_franchise`
(`atelier/sources/paie/src/assujettissement.rs`) en écrit l'aide et les deux réponses de la question « Vos recettes de l'an
dernier » (Configuration › Entreprise, « Taxe sur les salaires »), par la route `taxe-sur-les-salaires-regime` (`franchise`) ;
depuis le ticket #245, chaque réponse dit ce que mesure chaque plafond, en mots de compagnie (37 500 € de « spectacles
vendus », 85 000 € de « recettes en tout, subventions comprises » : les deux inférences ci-dessus), et la conclusion sous les
cases (`assujettissement::regime_taxe_sur_les_salaires`) nomme les deux plafonds.
Le test `les_plafonds_de_la_franchise_viennent_du_bareme_qui_cite_le_texte_depose` (`tests/routes/routes_fiches.rs`) relit les
montants dans `xml/293_B.xml`.

## Le taux du prélèvement à la source qui vaut pour un salaire (lu le 30/10/2026)

`xml/204_A.xml` (`LEGIARTI000033812067`) : « 2. Le prélèvement prend la forme : 1° Pour les revenus mentionnés à l'article 204
B, d'une retenue à la source effectuée par le débiteur lors du paiement de ces revenus ». `xml/204_H.xml`, III, 4 : la DGFiP
« transmet le taux au débiteur ».

Ce que dit l'article 1671 (`articles-20261021.md`, l. 132 et 135) : la retenue est « effectuée par le débiteur lors du
paiement », et le débiteur « applique le taux calculé par l'administration fiscale, au plus tard le deuxième mois suivant
sa transmission » ; le BOFiP (`../bofip/BOI-IR-PAS-30-10-20-extrait-taux.md`, § 70 et 80) : le taux est applicable jusqu'à
la fin du deuxième mois qui suit sa mise à disposition, et le débiteur applique celui du compte rendu le plus récent. Un
taux vaut donc pour tout paiement fait à partir du jour de sa mise à disposition, et pour aucun avant.

**Où le moteur s'en sert** : le taux reçu s'écrit dans la fiche du salarié (`taux_pas`, daté de sa mise à disposition :
`compagnie/src/taux_pas.rs`) ; `droit::pas::en_vigueur` (`atelier/sources/droit/src/pas.rs`) y choisit le dernier mis
à disposition au plus tard le jour du paiement ; `engagement::etats::prelevement` le demande au jour du versement de
l'engagement (`Engagement::date_du_versement` : sa date de paiement, sinon la fin de sa période). Le test
`un_taux_vaut_pour_tout_paiement_a_partir_de_sa_mise_a_disposition` (`droit/src/pas.rs`).

## L'exonération d'impôt sur le revenu des heures supplémentaires et complémentaires (relevé du 02/11/2026, lot LONG7)

81 quater (domaine `heures_supplementaires`) : les rémunérations et majorations de L241-17 CSS sont exonérées dans une limite
annuelle de 7 500 €. Leur montant net sort de la rémunération nette fiscale et se déclare en S21.G00.58.003 = 01 (fiche
net-entreprises 2066, `../net-entreprises-base-connaissances/`). Texte dans `articles-20261102.md`. **Où le moteur s'en
sert** : `logiciel/donnees/permanents-base.json` (`heures_en_plus.exoneration_impot`), `atelier/sources/engagement/src/
heures_en_plus.rs` (le net exonéré, la limite de l'année), `atelier/sources/declarations/src/dsn.rs` (`net_social`).

## Les indemnités journalières subrogées et le prélèvement à la source (relevé du 02/11/2026)

`articles-20261102.md`, section « L'impot sur le revenu des indemnites journalieres » (`xml/154_quinquies.xml`,
`xml/80_quinquies.xml`, `xml/81.xml`) : **154 quinquies, I** (la CSG des revenus de L136-8, II, déductible « à hauteur de
3,8 points ») ; **80 quinquies** (les indemnités journalières imposables comme des salaires, sauf la fraction exonérée par
81, 8°, et les affections de longue durée) ; **81, 8°** (celles d'un accident du travail, « à hauteur de 50 % de leur
montant »). Avec le BOFiP BOI-IR-PAS-20-10-10, § 80 à 110 (`../suspension-non-remuneree/`), l'assiette du PAS des
indemnités qu'un employeur subrogé verse (`engagement::maintien`, `etats::prelevement`).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20260919.md` : `5bad43a30b1bb1f1033b6da303f0a4f96cb33e639a36d7f58ef3c9c10c2cadd1`
- `articles-20260921.md` : `69bbd44b93fdfa6609561e040bb8be35845882ff646243b25245e192dc36f916`
- `articles-20260923.md` : `3bbebb2119b6387c2cf4694df9a4fd0d911285fdb6a3a8886c6e50caf6620cac`
- `articles-20260924.md` : `5f349ba11f234960114926032185d11539ee15a5ebd85fad3744a7cf1e7d0bb0`
- `articles-20261010.md` : `34186d1696d27218f2f6b44ea024f126e30a2504af8bfbf3c5082d7268f46a04`
- `articles-20261013.md` : `dcbb6ab6dc66d73f304d5ea229bd152f29d2aa75a355df120814d593b1609ba6`
- `articles-20261015.md` : `f1fbac3211330bc56a59da3daa9fd174f5bc37972197e4899eb14bb92d277092`
- `articles-20261021.md` : `142d9e94d5d7be947933b14d22a4d4d955a823b3cadf88d0f6d42a8fc0a7177e`
- `articles-20261102.md` : `a1f9bacd2d2142559abf9335ed200bf2af0c58d56b2a11cd034d0f2d8e2d2d20`
- `articles-20261103.md` : `5545a70bbafa728d49757f8dc2ee60a31cd4f5dc9e931fe50d8942085f2518af`
- `articles-20261110.md` : `542b7e1e5168813d484ae0bd3351760dffbee4b8541675eeeb81af9764e75d49`
- `formulaires/impots-2501-sd-2025-notice.pdf` : `eb24a1341c4b5b2a6237cbc5a54cfed803d00fef3cf0c6f7e1c2b1faaa9c3ee5`
- `formulaires/impots-2501-sd-2025-notice.txt` : `13c0878828b08a0c9e530badcad3a8f95ca286daf77eff6f40b17c4ba29a3ae1`
- `formulaires/impots-2501-sd-2026-notice.pdf` : `6bac48cbafca098014c98fe3e0337edb5a351c70433af5261de3264d35d39a2e`
- `formulaires/impots-2501-sd-2026-notice.txt` : `7426c910632fa8391d96b0babda6ce43247e30db4da5f037f23b5131d9143cde`
- `formulaires/impots-2502-sd-2025-notice.pdf` : `5594dd62ec563ef38062d4c733d2560fe4df440b686c3c160a7234b776d4cdbd`
- `formulaires/impots-2502-sd-2025-notice.txt` : `a94dfe761e611c731a998cb6b29fb3990b6bd5195db763ae9e88bb18b4444551`
- `releve.json` : `affabd8072b8e7f724180d9f0b24a169ae065b93922162348a4afac6d6a6bc84`
- `xml/154_quinquies.xml` : `4740d7a7e0e5b2f8712267eb5cc50c0066b13ebf198fda854cd743573e193305`
- `xml/1671.xml` : `1b4d346a0dddd9d35947e87598de395f4a17b0e791eedcd4c53da625d97579db`
- `xml/1679.xml` : `5343b9318796e66bbbfad79aec67724287c5a2a1321d96e9d2de9da2660db90d`
- `xml/1679_A.xml` : `11473fd0024220a9f25e0adddbe6b3f4121496f7225a9bd01e202ee716857817`
- `xml/1724.xml` : `bfb28089905081231b1a75272c4cfbb74c0084eedf4738f79aa03b27c90f07b2`
- `xml/204_A.xml` : `525495ee3f985b8eee68b8236394d9c6bb8f3253689b1cb2a393dba16495d460`
- `xml/204_B.xml` : `426ba4076dd5005c2b488306df803cec4afe2a2cee7514d8cf75c2797ff2e672`
- `xml/204_E.xml` : `3d0c4236870131a76b8dd23189c59d2c81d2b1145f2744224d762254d452cff5`
- `xml/204_F.xml` : `dee4c5b7c3c89b7b4fb009d7c60e31972332c551c95a1e4672b9c88c6e163024`
- `xml/204_H.xml` : `6751bbbd48e113da2013fc518660fbaaa11a3df8864630e7f6fbb3b435b6e472`
- `xml/231.xml` : `2fb2460e58f98cbf6c5c23c4a3f46aaea10226752b5ebe77a8fb1853e07cc6f9`
- `xml/235_bis.xml` : `9b6e3101c92916aa606c9435b36fea739d4fc78eb0a45b0d1d8ee5154dde65db`
- `xml/293_B.xml` : `5954095c8bbf8927b911945b984e76c820062c9262a22a6c94d3f36e63cbd5a4`
- `xml/80_duodecies.xml` : `64b775a316d1f1c51c93d7e6042f98979ce217240462280eef54733cc09191c3`
- `xml/80_quinquies.xml` : `71a57ff67649eb19fea9a39c4db1da7b190d947b5823e1eaa78431d89240af12`
- `xml/81.xml` : `c9008c7f01ef0fdcd9ee6537307692b54e32d33ca8590eb3210fdd25123cffe1`
- `xml/81_quater.xml` : `97c24d8f0685ad16a3ad59c6bf3dba1e562adb6e4cc96925c5023b77abe827a7`
- `xml/83.xml` : `fed50d50d119b28c44b1f242639c079c5964d5be8966e1cb0af4a8669337a4b8`
