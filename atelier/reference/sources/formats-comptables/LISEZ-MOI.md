# Les formats d'import des logiciels comptables

Les notices des éditeurs dont `documents/src/etats_compta.rs` (les exports comptables de la paie) écrit le format.
Documents d'organismes privés : ici leur transcription (`pdftotext -layout` de la page imprimée, les liens de navigation de la base Sage retirés : leurs numéros ont la forme d'un NIR), l'original est dans
`reference-privee/sources/formats-comptables/` du dépôt privé. Fournis par Léandre le 04/11/2026, imprimés depuis
les centres d'aide des éditeurs.

| fichier | ce que c'est | où le moteur s'en sert |
|---|---|---|
| `sage-50-structure-ximport-20261104.txt` | Sage 50 (ex-Ciel Compta), « Comprendre la structure du fichier d'import XImport », base de connaissances Sage, modifiée le 13/06/2026 : enregistrement sans délimiteur de 137 caractères au plus, zones et longueurs | export XIMPORT |
| `sage-50-format-import-parametrable-20261104.txt` | Sage 50, « Créer un format d'import paramétrable » (renvoie à la structure XImport) | — (contexte) |
| `sage-100-description-format-pnm-20261104.txt` | Sage 100 Comptabilité, « Description du format PNM » (2021) | export Sage (PNM) |
| `sage-100-ecritures-format-pnm-20261104.txt` | Sage 100, « Les écritures au format Sage (*.pnm) » : contrôles et exemples | export Sage (PNM) |
| `sage-100-format-import-export-parametrable-20261104.txt` | Sage 100, « Configurer le Format import / export paramétrable », modifiée le 01/12/2025 | — (contexte) |
| `cegid-quadra-fichier-entree-ascii-20261104.txt` | Cegid, « Fichier d'entrée ASCII dans QuadraCOMPTA » (`QC_ASC`) : zones, positions, longueurs | export Quadra |
| `cegid-quadra-aide-export-import-ascii-20261104.txt` | QuadraCOMPTA, aide « Outils, Suivi : Export / Import ASCII » | export Quadra |
| `ebp-format-importation-ecritures-ascii-20261104.txt` | EBP Comptabilité, « Format d'importation des écritures en ASCII » (page de Sofakom, revendeur EBP ; la page porte des publicités) | export EBP |
| `memsoft-oxygene-structure-fichiers-ascii-20261104.txt` | Memsoft Oxygène, « Structure des fichiers ASCII utilisés dans le module d'importation de données » — tout format géré « à l'exception des formats de taille fixe », CSV à point-virgule conseillé | export Oxygène |
| `pennylane-importer-des-ecritures-20261104.txt` | Pennylane, centre d'aide, « Importer des écritures » (Excel ou CSV ; champs obligatoires ; code journal de 5 caractères au plus) | export Pennylane |
| `pennylane-importer-cegid-expert-tra-20261104.txt` | Pennylane, « Importer un fichier de CEGID EXPERT au format TRA » | — (contexte) |
| `droit-org-livre-des-procedures-fiscales-20261030.pdf` | le Livre des procédures fiscales, édition du 30/10/2026 de codes.droit.org (tirée des données de Légifrance), tel que servi le 04/11/2026 par `https://codes.droit.org/PDF/Livre%20des%20proc%C3%A9dures%20fiscales.pdf` (Légifrance refuse un robot ; le LPF n'est pas encore un texte du relevé LEGI, `moteur-atelier releve-codes`) | export FEC |
| `lpf-article-A47-A-1-20261030.txt` | sa transcription (`pdftotext -layout`), l'article A47 A-1 seul (arrêté du 29/07/2013) : I et III, la forme des fichiers ; VI, 1°, c, les zones séparées par une tabulation ou « \| » ; VII, 1° et 4°, les dix-huit champs, dans l'ordre, et leurs noms en première ligne ; XII, le jeu de caractères (ASCII, ISO 8859-15 ou UTF-8), la virgule décimale, les dates AAAAMMJJ | export FEC (`SOURCE_FEC`) |
| `bofip-BOI-CF-IOR-60-40-20-20170607.html` | BOFiP, BOI-CF-IOR-60-40-20, « Format du fichier des écritures comptables », version en vigueur depuis le 07/06/2017, telle que servie le 04/11/2026 par `https://bofip.impots.gouv.fr/bofip/9028-PGP.html/identifiant=BOI-CF-IOR-60-40-20-20170607` (HTTP 200) : § 70, « à blanc si non utilisé », la colonne présente et la case vide | export FEC (`SOURCE_FEC`) |

## Ce que le moteur en lit

`documents/src/etats_compta.rs` écrit chaque format d'après sa notice, et cite le fichier et ses lignes (`SOURCE_*`).
Ce qu'aucune notice ne dit est une **inférence**, écrite comme telle dans la source du format : le jeu de caractères
(Windows, sauf le FEC et Pennylane en UTF-8), la fin de ligne CR LF, le séparateur de Pennylane (point-virgule), le
séparateur décimal d'Oxygène (virgule). Deux points restent ouverts dans les notices elles-mêmes : la structure XImport
dit « 137 caractères » au plus, ses treize zones en font 138 (elles sont écrites telles que sa table les donne) ; le
compte collectif de l'enregistrement C de Quadra est marqué obligatoire, sans valeur dite pour un compte général (il
reste blanc).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `bofip-BOI-CF-IOR-60-40-20-20170607.html` : `81222019e3d4214ae85c4e6626f41eef5f4bb69a28bb4f9ad998be453b824702`
- `cegid-quadra-aide-export-import-ascii-20261104.txt` : `f5f355ba634f6c81c50b2af7e0cbfda1e34e595b367979a9ea890a27da98ec9f`
- `cegid-quadra-fichier-entree-ascii-20261104.txt` : `846a0458e47f23adfaeebfc639b1dd465604bb385f3bee85ba16482f7c3c48ca`
- `droit-org-livre-des-procedures-fiscales-20261030.pdf` : `19bc2d0648c28ac536284d29ceac1e3f7a3f80f1b7fc43b72415fb940889baf6`
- `ebp-format-importation-ecritures-ascii-20261104.txt` : `2f4b03059000ee040612f137311de79fe239ced42065a1982bfd940fb8b1bae6`
- `lpf-article-A47-A-1-20261030.txt` : `2cad4965564e369ef800d56568c2935b124c7619eef281b42e93604df9425f09`
- `memsoft-oxygene-structure-fichiers-ascii-20261104.txt` : `6609b2569c16e6333e1b0e8b3d9dbc5d3fb037e38a203572f64bb8de181476d4`
- `pennylane-importer-cegid-expert-tra-20261104.txt` : `92982ff2411e0c79ee36f50a837cd715248cb532b96c96a08c235b0e67f6b93f`
- `pennylane-importer-des-ecritures-20261104.txt` : `b15a6410e22d167ccd137f7faf385297300171636276e8d4030d06f5f15a78ec`
- `sage-100-description-format-pnm-20261104.txt` : `4e1a6c2227c01886d22b51dcce21a9c1c574586b9a3657069a0ac3b0119cd508`
- `sage-100-ecritures-format-pnm-20261104.txt` : `f9fad4d5d8daa16bda316f498b1f50500c685ed4305a085f41a17b75b3ac6646`
- `sage-100-format-import-export-parametrable-20261104.txt` : `87e1f4f41b0a030a9045f318be914a1d9eb42be22aba81f51e6e4cdeb731332e`
- `sage-50-format-import-parametrable-20261104.txt` : `bbcaa6487f1395c0ae6f9ba62afe0f2ca3fa629af043e8944b431146871bb13d`
- `sage-50-structure-ximport-20261104.txt` : `6bbb371bd8e7416a94b1b63583a7443d065f025f7f654691a724613676913e6d`
