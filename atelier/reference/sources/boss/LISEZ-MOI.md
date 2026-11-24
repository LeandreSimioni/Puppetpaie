# Le Bulletin officiel de la sécurité sociale (BOSS) — les pages déposées

Le BOSS (boss.gouv.fr) ne répond pas depuis le conteneur (connexion coupée) : ses pages sont imprimées en PDF par
Léandre depuis le poste et déposées ici, transcription à côté (`pdftotext -layout`). Nom : `boss-<rubrique>-<date de
la version en vigueur>`. La page « Montant net social » est rangée avec ses textes, dans `../montant-net-social/`.

## `boss-assiette-generale-20260801.pdf` — Règles d'assujettissement › Assiette générale

URL : `http://boss.gouv.fr/portail/sites/siteboss/accueil/regles-dassujettissement/assiette-generale.html`,
**version en vigueur au 01/08/2026**, imprimée le 10/10/2026. Six chapitres : affiliation ; assiette de la CSG et de
la CRDS (§ 290 : abattement de 1,75 % jusqu'à quatre plafonds annuels) ; assiette des cotisations ; forfait social ;
fait générateur ; **chapitre 6, le plafond de la sécurité sociale** (§ 650 à la fin).

Ce que le moteur en lit (chapitre 6) :
- § 680 : les valeurs 2026 (horaire **30 €**, journalier 220 €…) ; « La valeur horaire n'est utilisable que […] pour
  le calcul du plafond pour les artistes du spectacle engagés pour des périodes continues inférieures à cinq jours »
  (renvoi à l'arrêté du 24/01/1975, déposé dans `../arrete-1975-artistes-du-spectacle/`, et à L. 241-3, D. 242-16 à
  D. 242-19).
- § 690 à 750 : l'ajustement du plafond « à proportion des jours couverts par le contrat de travail », jours
  calendaires moins les absences sans maintien de salaire ; entrée ou sortie en cours de mois (R. 242-2).
- § 760 à 790 : temps partiel (proratisation à la durée contractuelle, sur option), forfait jours. **§ 760-770
  appliqués depuis le lot V4b** (l. 1031-1072) : le plafond d'un salarié à temps partiel « peut être calculé par
  l'employeur » à due proportion de sa durée contractuelle, « rapportée à la durée légale du travail (ou
  conventionnelle si elle est inférieure) », et « le salarié à temps partiel et l'employeur ont la possibilité de
  renoncer à cet ajustement » ; le rapport « ne peut pas conduire à un résultat supérieur à la valeur mensuelle du
  plafond ». Le choix est dans la fiche entreprise (`plafond_temps_partiel`), et S21.G00.40.084 le déclare en 2027 :
  `../plafond-temps-partiel/LISEZ-MOI.md`.
- § 660 : le plafond se détermine à chaque paie et se régularise (progressivement ou en fin d'année).
- **§ 1090** (l. 1472-1483) : les artistes du spectacle « ne sont pas éligibles à la réduction du plafond » ; non
  mensualisés, le plafond « dépend uniquement de la périodicité de la paie » ; « Toutefois, il est admis que : pour les
  salariés intermittents, le plafond soit proratisé pour ne tenir compte que des jours ayant donné lieu à
  rémunération ». **La tolérance n'est pas appliquée** (Léandre, 24/11/2026 : « pas de tolérance, c'est la loi ») :
  tout intermittent hors de l'arrêté de 1975 (artiste engagé cinq jours ou plus, technicien) a, pour chaque paie, le
  PSS mensuel × jours calendaires du contrat dans la période de paie / jours du mois (CSS R. 242-2, I, al. 3 ; la
  formule du § 700 et le § 740), quels que soient les jours travaillés. La première phrase du § 1090 (l'artiste
  « n'est pas éligible à la réduction du plafond ») fait qu'aucun jour d'absence ne sort de son prorata.
- § 1100 : « 12 fois le plafond horaire par jour d'engagement » pour un artiste engagé moins de cinq jours ; le moteur
  suit l'arrêté (« par journée de travail accomplie »).

Où le moteur s'en sert : `atelier/sources/droit/src/plafond.rs` (`pour_paie`, `plafond_de_la_periode`),
`engagement::calcul::plafond_de_l_engagement`.

## `boss-frais-professionnels-20261013.pdf` — Autres éléments de rémunération › Frais professionnels

URL : `https://boss.gouv.fr/portail/accueil/autres-elements-de-remuneration/frais-professionnels.html`, **version en vigueur
au 01/01/2026**, imprimée par Léandre le 13/10/2026 à 21 h 30 (47 pages), transcription `pdftotext -layout` à côté. Dix
chapitres ; le moteur lit le **chapitre 9, « Déduction forfaitaire spécifique pour frais professionnels »** (§ 2120 à 2330,
l. 2614-2956 de la transcription). Les règles de la DFS elles-mêmes (taux, plafond, option, annexe 2) sont dans l'arrêté
du 4 septembre 2025, déposé dans `../dfs/` ; le BOSS y ajoute la doctrine et les tolérances ci-dessous.

Ce qu'on en lit (chapitre 9) :
- **§ 2120** : professions de l'article 5 de l'annexe IV du CGI (rédaction au 31/12/2000) ; plafond **7 600 €** par salarié
  et par année civile ; les secteurs à taux personnalisés, dont **le spectacle vivant**, suivent la section 3.
- **§ 2130-2140** : la DFS suppose des frais effectivement supportés, l'employeur garde les justificatifs. **§ 2310** : pour
  les huit secteurs, dont le spectacle vivant, **par tolérance depuis le 01/01/2023, le bénéfice est admis même sans frais
  réellement supportés** (l'exemple cité : la caisse de congés payés qui applique la DFS aux indemnités de congés). Les
  autres conditions restent dues.
- **§ 2150, absence** : la DFS ne s'applique « que sur la rémunération correspondant à un travail effectif » ; un mois
  complet d'absence (maladie, congés) n'ouvre pas la DFS pour ce mois. **À confronter au moteur** : un intermittent au
  cachet n'a pas d'absence ; un salarié au mois sous DFS est refusé aujourd'hui (`calcul::dfs_applicable`), la règle
  servira le jour où il sera porté.
- **§ 2160, plusieurs employeurs** : la limite de 7 600 € « peut être appréciée séparément, au niveau de chaque
  employeur » (tolérance, sauf abus manifeste). C'est ce que le moteur fait (`calcul::dfs_deja_deduite`, par salarié et par
  compagnie) : le point 5 du `LISEZ-MOI` de `../dfs/` (le cumul d'avant le logiciel, chez un autre employeur) n'est donc pas
  un manque.
- **§ 2180-2210, l'option** : convention ou accord collectif, ou accord du CSE ; à défaut, le consentement de chaque
  salarié, informé des conséquences sur ses droits, recueilli chaque année ; le silence vaut accord ; la décision du salarié
  prend effet l'année civile suivante. **§ 2330, huit secteurs** : un consentement à durée indéterminée couvre toute la
  période de transition ; pour un salarié embauché depuis le 01/01/2023 sans accord collectif, le consentement est requis,
  et l'employeur peut proposer qu'il vaille pour les contrats ultérieurs chez lui. Le moteur : `dfs_salarie` de la fiche
  (`contrat` / `consentement` / `refus`, `calcul.rs` l. 553-660).
- **§ 2220-2230, assiette minimale** : la DFS ne ramène pas l'assiette d'une paie sous le SMIC et ses accessoires légaux ;
  **les artistes hors du champ du SMIC (pas de durée du travail) n'y sont pas soumis**. Le moteur : l'assiette minimum ne
  vaut que pour les contrats à l'heure ou au mois (refusés sous DFS aujourd'hui).
- **§ 2240-2290, non-cumul** : les remboursements de frais (réels ou forfaitaires) et prises en charge directes entrent dans
  l'assiette avant l'abattement, sauf l'annexe 2 de l'arrêté (défraiements de tournée des artistes dramatiques, lyriques,
  chorégraphiques et régisseurs de théâtre ; allocations de saison ; frais des chefs d'orchestre, musiciens et choristes en
  déplacement…). Les outils de travail (véhicule, ordinateur) ne sont pas une prise en charge de frais. Le moteur suit la
  règle (l. 2794-2796 et 2808) : `calcul::dfs_applicable` rend les remboursements de frais (rubrique de nature `frais`,
  catalogue ou compagnie, dans leurs limites d'exonération) que l'annexe 2 ne laisse pas dehors pour la profession, et
  `calcul::abattement_dfs` abat le brut et ces frais ensemble ; le chômage d'un intermittent les compte avant
  l'abattement ; la DSN les déclare en bloc 54 de type 35 (`dsn::autre_revenu`) ; test :
  `moteur/tests/routes/frais_de_deplacement.rs`, `sous_dfs_le_remboursement_de_frais_entre_dans_l_assiette_avant_l_abattement`.
  Le § 2320 (l. 2917-2921) admet « par tolérance », pour les huit secteurs dont le spectacle vivant, le cumul de tous les
  remboursements de frais avec la déduction : le moteur ne l'applique pas (Léandre, 24/11/2026 : « Pas de tolérance,
  c'est la loi »). Le rescrit BOSS-RES-000010 (§ 2325, l. 2923-2934, dans la page déposée) : une indemnité de transport
  conventionnelle n'est pas un remboursement de frais au sens de l'arrêté, elle entre dans l'assiette dès le premier
  euro ; une ligne hors assiette qui n'est pas un remboursement de frais refuse (`calcul::dfs_applicable`).
- **§ 2300, le calendrier du spectacle vivant** : à 20 % → −1 point en 2024 et 2025, −2 points de 2026 à 2028, −3 points de
  2029 à 2031, zéro au 01/01/2032 ; à 25 % → −2 points en 2024 et 2025, −3 points de 2026 à 2031, zéro au 01/01/2032. Donc
  **2026 : 16 % et 18 %** ; 2027 : 14 % et 15 %. Même calendrier que l'arrêté du 4 septembre 2025 (annexe 1), que les barèmes
  2025 et 2026 suivent (`_professions`, `_plafond` de `logiciel/donnees/baremes/<année>.json`).
- Hors chapitre 9, pour mémoire : § 760 (chapitre 4) et § 990, le non-cumul de la DFS avec la prise en charge des
  transports et du carburant, et sa dérogation pour les 50 % obligatoires des titres de transport.

## `boss-heures-supplementaires-20260601.pdf` — Exonérations › Exonérations heures supplémentaires et complémentaires

URL : `http://boss.gouv.fr/portail/sites/siteboss/accueil/exonerations/exonerations-heures-supplementai.html`,
**version en vigueur au 01/06/2026**, imprimée par Léandre le 02/11/2026. Chapitre 1 : la réduction de cotisations
salariales d'assurance vieillesse sur la rémunération des heures supplémentaires et complémentaires (L. 241-17 CSS).
Ce que le moteur en lit : le champ (§ 40-150 : heures supplémentaires d'un temps plein, complémentaires d'un temps
partiel, pas de « complément d'heures »), le taux horaire majoré (§ 280-310 : le moteur prend le salaire mensuel sur les heures
par mois, une inférence), le taux de la réduction (§ 350-370 : le taux moyen de la vieillesse salariale, plafonné à 11,31 %),
son imputation sur la vieillesse de base et sa limite (§ 450-480), sa ligne au bulletin (§ 400), la déduction patronale
(chapitre 2 : 1,50 € sous vingt salariés, 0,50 € à partir de vingt, heures supplémentaires seules, § 570), et la CSG des heures
exonérées (§ 440). Où : `droit/src/heures_en_plus.rs`, `engagement/src/heures_en_plus.rs`, `logiciel/donnees/permanents-base.json`
(`heures_en_plus`). Non porté : les heures structurelles (§ 70, 230-250), le forfait en heures ou en jours (§ 100, 160), le
rachat de RTT (chapitre 3).

## `boss-protection-sociale-complementaire-20260101.pdf` — Autres éléments de rémunération › Protection sociale complémentaire

URL : `http://boss.gouv.fr/portail/sites/siteboss/accueil/autres-elements-de-remuneration/protection-sociale-complementair.html`,
**version en vigueur au 01/01/2026**, imprimée le 04/11/2026. Chapitre 1 : les contributions des employeurs au
financement de la prévoyance complémentaire exclues de l'assiette des cotisations (principe, conditions du caractère
collectif et obligatoire, limites). C'est l'assiette **sociale** ; le net **imposable** relève du BOFiP
(`../bofip/BOI-RSA-BASE-30-10-20-*`). Où : la prévoyance et le Fonds collectif santé des intermittents au barème
(leur `source`), et la part patronale de la mutuelle des permanents.

## `boss-allegements-generaux-20261123.pdf` — Exonérations › Allègements généraux

URL (en tête de la transcription) : `http://boss.gouv.fr/portail/sites/siteboss/accueil/exonerations/allegements-generaux.html`,
**version en vigueur au 01/08/2026** (l. 11), « Mis à jour le 27/07/2026 » (dernière ligne), imprimée par Léandre le
23/11/2026. C'est la réduction générale dégressive unique (RGDU) de 2026 : la page ne parle plus des taux réduits de
maladie et d'allocations familiales de 2025.

Ce qu'on en lit :
- **§ 190** (l. 241-252) : sous une déduction forfaitaire spécifique, « la rémunération intègre les sommes versées au
  titre du remboursement de frais professionnels, puisque ces sommes ne sont plus exclues de l'assiette des cotisations
  lorsqu'une déduction forfaitaire spécifique est appliquée » ; « La rémunération à prendre en compte pour la réduction
  est égale à 1 900 × (1 – 10%) soit 1 710 € ». C'est la lecture, par l'administration, de « la rémunération [...]
  définie à l'article L. 242-1 » (L. 241-13, III) : l'assiette abattue de la déduction.
- **§ 220** (l. 331-332) : la réduction « ne peut pas être cumulée avec une autre exonération partielle ou totale de
  cotisations patronales, ainsi qu'avec l'application de taux spécifiques, d'assiettes ou de montants forfaitaires ».
- **§ 230** (l. 334-339) : « Lorsque les employeurs choisissent de ne pas appliquer une exonération totale ou partielle
  [...], ils peuvent alors appliquer la réduction générale. Cette décision n'est pas irrévocable » ; changement au premier
  jour du mois civil suivant. Le paragraphe dit « exonération », pas « taux spécifiques ».
- **§ 250-280** (l. 371-405) : les taux réduits cumulables avec la réduction : journalistes rémunérés à la pige, VRP à
  cartes multiples, professions médicales à temps partiel. Les artistes du spectacle n'y sont pas.
- **§ 820** (l. 1281-1292) : pour un salarié intermittent, le SMIC du coefficient « est basé sur la durée de travail
  inscrite au contrat de travail du salarié » ; rien sur la durée d'un cachet.

Où le moteur s'en sert : § 190, dans la règle du seuil des taux réduits de 2025 (`droit::reduction::au_dela_du_seuil`,
les revenus déduction forfaitaire spécifique déduite). § 220 à 280 : ce que fait déjà le barème 2026 (l'artiste n'a pas
la réduction générale, `par_statut.technicien.reduction_generale.statuts`), non cité dans le barème. § 220 (l. 331) :
`logiciel/donnees/ecarts-connus-base.json`, la ligne `reduction_generale` de Coolpaie, l'écart connu d'une réduction
générale appliquée à la paie d'un artiste (`engagement::ecarts_connus::Constat::AbsenteDuCalcul`), avec l'arrêté de 1975
et la page Urssaf.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `boss-allegements-generaux-20261123.pdf` : `96939404eefe87b53bf974d4245018a61a9a9e26ef204bb999286ff8008b099e`
- `boss-allegements-generaux-20261123.txt` : `a811e5596fd06fc72689dbd3562913e62961686576aa64862eebc875ee7f0e85`
- `boss-assiette-generale-20260801.pdf` : `24e0891513382a6e6fe3304604b182e851dd14945d733d176713c53c641a759e`
- `boss-assiette-generale-20260801.txt` : `601c928726d512d018a43b6a272ea578245bad6898165920b57436e30708033d`
- `boss-frais-professionnels-20261013.pdf` : `6b37fa2f738615c30e2af2dc1f4b694e04695732fcd49fffc59a397a5c249b48`
- `boss-frais-professionnels-20261013.txt` : `1c96b200a616bd52802adde62bc5b54f461846219814f0fde33b5eead21cd88a`
- `boss-heures-supplementaires-20260601.pdf` : `77d69b0ecb007584a3b408da76fbad57fc762cf3ec6cc0672fe5d99310fdb8ba`
- `boss-heures-supplementaires-20260601.txt` : `9f76cf518c953422a353c24c8cedbe2bcdaed77bf4f1bffae1a48dac8c82aefd`
- `boss-protection-sociale-complementaire-20260101.pdf` : `6368aef4057122cde0852e06699447c2b3356a77422e0f633ffeb8a5fb6a5411`
- `boss-protection-sociale-complementaire-20260101.txt` : `b505d5c421edc56fcd898a4d2df35dd2dae4d74034abbb65a2f0cfe01282e3ac`
