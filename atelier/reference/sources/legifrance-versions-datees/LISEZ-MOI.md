# Légifrance — des versions datées d'articles

Chaque article est déposé **tel que Légifrance l'imprime** (`.pdf`, « Imprimer » de la page de l'article, dans la version
datée qu'il faut), et sa transcription à côté (`.txt`, `pdftotext -layout`). Le nom porte la date de **début** de la
version. Relevés le 04/11/2026 par Léandre sur son poste : Légifrance refuse le conteneur (HTTP 403), et `moteur-atelier
releve-codes` ne garde que la version en vigueur d'un article.

| fichier | version | ce qu'on en lit | où ça sert |
|---|---|---|---|
| `legifrance-css-D242-4-20231231` | CSS D. 242-4, du 31/12/2023 au 01/01/2026 | « A compter du 1er janvier 2024 8,55 % 6,90 % » : l'assurance vieillesse plafonnée, part patronale et salariale ; la déplafonnée à côté | la vieillesse du barème 2025 |
| `legifrance-css-L136-8-20201216` | CSS L. 136-8, du 16/12/2020 au 01/07/2025 | « 1° A 9,2 % pour la contribution sociale » sur les revenus d'activité | la CSG du barème 2025 (premier semestre) |
| `legifrance-css-L136-8-20250701` | CSS L. 136-8, du 01/07/2025 au 31/12/2025 | « 1° A 9,2 % pour la contribution sociale » sur les revenus d'activité | la CSG du barème 2025 (second semestre) |
| `legifrance-css-L136-8-20251231` | CSS L. 136-8, du 31/12/2025 au 27/06/2026 | le même taux de 9,2 % | la CSG du barème 2026, jusqu'au 27/06/2026 |
| `legifrance-cgi-154-quinquies-20230311` | CGI 154 quinquies, du 11/03/2023 au 27/06/2026 | la CSG déductible « à hauteur de 6,8 points » | la part déductible de la CSG en 2025 et 2026 |
| `legifrance-cgi-293-B-20250101` | CGI 293 B, du 01/01/2025 au 01/03/2025 | la franchise en base de TVA (85 000 / 37 500) | la note du barème 2025 qui la cite |
| `legifrance-cgi-204-H-20260221` | CGI 204 H, du 21/02/2026 au 01/07/2026 | les grilles du taux par défaut du prélèvement à la source | `pas_taux_par_defaut` du barème 2026, de mai à juin |
| `legifrance-css-L241-13-20210101` | CSS L. 241-13, du 01/01/2021 au 28/02/2025 (relevé le 06/11/2026) | la réduction dégressive ; IV, 3° : les professions dont les congés sont mutualisés par une caisse | la réduction générale 2025, jusqu'au 27/02/2025 |
| `legifrance-css-L241-13-20250228` | CSS L. 241-13, du 28/02/2025 au 31/12/2025 (modifié par la LFSS 2025, art. 18) | le même IV, 3° | la réduction générale 2025, depuis le 28/02/2025 |
| `legifrance-css-D241-7-20231231` | CSS D. 241-7, du 31/12/2023 au 07/04/2025 (décret 2023-1329) | « Coefficient = (T/0,6) × (1,6 × SMIC calculé pour un an / rémunération annuelle brute-1) », T = 0,3194 (0,3234 au FNAL 0,50 %) | `reduction_generale.t` et `plafond_smic` du barème 2025 |
| `legifrance-css-D241-7-20250407` | CSS D. 241-7, du 07/04/2025 au 01/01/2026 (décret 2025-318) | la même formule, T = 0,3193 (0,3233) | `reduction_generale.t` du barème 2025, depuis le 01/05/2025 |
| `legifrance-ct-L2262-1-20080501` | C. trav. L. 2262-1, depuis le 01/05/2008 | un accord lie les adhérents des organisations signataires | la portée d'un accord de salaires (barème 2026) |
| `legifrance-ct-L1221-3-20080501` | C. trav. L. 1221-3, depuis le 01/05/2008 (relevé le 09/11/2026) | « Le contrat de travail établi par écrit est rédigé en français » ; un emploi au terme étranger s'explique en français ; une traduction dans la langue du salarié étranger, à sa demande, les deux textes faisant également foi | le vérificateur des modèles de contrat le rappelle (`documents::contrat_ecrit::EN_FRANCAIS`, rendu par `modele-contrat` et l’écran des modèles) ; aucune langue ne se détecte |
| `legifrance-css-L911-8-20130617` | CSS L. 911-8, depuis le 17/06/2013 (relevé le 10/11/2026) | la portabilité : la couverture santé et prévoyance collective se maintient gratuitement après une fin de contrat ouvrant droit au chômage, autant que l'indemnisation, dans la limite de la durée du dernier contrat, douze mois au plus ; 6° : l'employeur le signale dans le certificat de travail et en informe l'organisme | la mention du maintien des garanties sur le certificat de travail, sauf faute lourde, avec sa durée maximale (1°) (`engagement::portabilite`, `documents::fin_de_contrat::mention`, marqueur `certificat.portabilite` du modèle, qu'un modèle copié doit citer) ; ce qui reste dû quand une compagnie cesse d'employer des permanents (`engagement::permanents`) |
| `legifrance-decret-2015-87-20150201` | décret n° 2015-87 du 28/01/2015 | le financement mutualisé des organisations syndicales et patronales ; il ne fixe pas lui-même le taux : le 0,016 % est à C. trav. D. 2135-34, déjà dans l'entrepôt LEGI (`legi-entrepot/articles.jsonl.gz`) | aucun calcul ; lu pour la contribution au dialogue social |

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `legifrance-cgi-154-quinquies-20230311.pdf` : `7f4483bc01c3c47e18c4ab0720464dc868977f43f9ff5d4216b5e71b6f9a70db`
- `legifrance-cgi-154-quinquies-20230311.txt` : `a295ec252af922fa64fcb43d882861fa4a8bf101384b9801ab5a9ef8cc9524dd`
- `legifrance-cgi-204-H-20260221.pdf` : `0fc19a4e97c120a7cf5d89dd5f3281df1e0ce9c9728102b9a92e7aa1db43d73a`
- `legifrance-cgi-204-H-20260221.txt` : `20eb55e2637533b683148ab6434426baac0015e2307b63905d568d90304c2281`
- `legifrance-cgi-293-B-20250101.pdf` : `8993eb353367fd96a65d87415f74525e170fa4b3d29f03861e69e680ad2e9acf`
- `legifrance-cgi-293-B-20250101.txt` : `126e8562bd09f63957c6ebe433017f05a3696b56ff5a5e9062ce968f9eecbe37`
- `legifrance-css-D241-7-20231231.pdf` : `c2d81a98cafa61dda3d4c9a0477512c604e66241607b2b88489f618ab762bc24`
- `legifrance-css-D241-7-20231231.txt` : `7546e32e53ab7aca7f19a1075c74634c939ec78345127850ba62ec0b1874b325`
- `legifrance-css-D241-7-20250407.pdf` : `cc7901c47186a439374ae542fdb25b2f1225f72e65589dca4e5b7faf0bf5804e`
- `legifrance-css-D241-7-20250407.txt` : `c889fa2844567849a275d1b1071e356a23531b5b1a3a789f8b63f52d95f0b843`
- `legifrance-css-D242-4-20231231.pdf` : `e25a15912a3b756175f971a314cb081a12913992aa06ead20dd680cb124baa9b`
- `legifrance-css-D242-4-20231231.txt` : `e2f623381198aaa6f7eb701dca0df0c04834ca3365fb41a74bbb2ef2054a1538`
- `legifrance-css-L136-8-20201216.pdf` : `69e5e74634ce603adf6532ccafc01f0842a933113e70969f36b8a9ffb044857a`
- `legifrance-css-L136-8-20201216.txt` : `d43854260d6fb02ed1147e97abb0ae00f2bddf23d66397ed591badf5b00944ec`
- `legifrance-css-L136-8-20250701.pdf` : `ac3be0213f8273aac3066503bb3f7a4ddacd116ed6fecac807528c03b8bc235d`
- `legifrance-css-L136-8-20250701.txt` : `97c02bc78eb90335bfbfd7d03889ec873b24c227f8b040cb45286255e384ad1e`
- `legifrance-css-L136-8-20251231.pdf` : `0cac124520464259017ac0ef20fa47f43fdca8015fff8c62700af830b744bb24`
- `legifrance-css-L136-8-20251231.txt` : `614f2b3afdfcbb3b356b4c3ea453a7ba7245127821dbd378fd561d83e7bf9085`
- `legifrance-css-L241-13-20210101.pdf` : `d8e2e3d3a93d303d53415c52123b58cb7a7feaa573a5165bba16497a04ab247f`
- `legifrance-css-L241-13-20210101.txt` : `a35fc9cb6690a0481feb0a7b7de1ddb12697f6727778a7f9a1312206f35485a4`
- `legifrance-css-L241-13-20250228.pdf` : `a5ee79bf5278e23f102ea69e9583f826894bc87ac1ebafb69e9308da0b9223fe`
- `legifrance-css-L241-13-20250228.txt` : `38aa1f7fe25ded5f18587ffc374e51923751f8f67da1c04097f678e5f633a8cc`
- `legifrance-css-L911-8-20130617.pdf` : `abbd98e62a42317b4fd5f8e6ad4bd6e0c763c392ef8d0dc4666489bea1cb9484`
- `legifrance-css-L911-8-20130617.txt` : `02d7c478f0ff634057fac5ae6755cc0a3b493ef1cc1b9cb2a6a6c6a31fc5a946`
- `legifrance-ct-L1221-3-20080501.pdf` : `9395dfaf34efd64bd68e3e38fc9764bdbebf239945938c04177dcdd44533085a`
- `legifrance-ct-L1221-3-20080501.txt` : `2613926e3e6969ea24f84c0d90fdf08e5524a06c6adfc62f913b52e07cc6450e`
- `legifrance-ct-L2262-1-20080501.pdf` : `be447aa2a6a60748f3c5150f19fad7101bf27dcb796706800449394bedfcdf79`
- `legifrance-ct-L2262-1-20080501.txt` : `2feee5f0be9b19cda14bcc6d81659884b7546126618d2211d112c6ee3ce93c97`
- `legifrance-decret-2015-87-20150201.pdf` : `7508e7deae1457c9720fb40b689ed476cab0cabd7e842fdd1224c94b752ee196`
- `legifrance-decret-2015-87-20150201.txt` : `b8c40a72f4baba50dc40c8eed42ba5646b188e08a3d16d246bc8f792a02c120d`
