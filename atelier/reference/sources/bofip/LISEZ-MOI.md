# BOFiP — ce que la paie lit de la doctrine fiscale

Chaque page est déposée **brute** (`.html`, telle que servie, nom = identifiant BOFiP + date de version) ; quand
seuls quelques paragraphes servent, un `-extrait-*.md` à côté les transcrit, dit la date du relevé et où le moteur
s'en sert. Le BOFiP se laisse lire par un robot (contrairement à Légifrance) : une page se redépose à chaque version.

| fichier | ce que c'est | où ça sert |
|---|---|---|
| `BOI-BAREME-000037-*.html` | prélèvement à la source : les grilles de taux par défaut et l'abattement des contrats courts, quatre versions (20240228 : grilles « en 2024 » ; 20250410 : grilles « à compter du 1er mai 2025 », § 30 : SMIC net imposable 1 477,63 €, abattement 739 € ; 20260407 : § 30 : SMIC net imposable « au 1er janvier 2026 » 1 495,04 €, abattement « 748 euros (1 495,04 / 2) » ; 20260706 : § 30 : SMIC net imposable « au 1er juin 2026 » 1 531,12 €, abattement « 766 euros (1 531,12 / 2) »). Les deux versions 2026, relevées le 04/11/2026 sur `https://bofip.impots.gouv.fr/bofip/11255-PGP.html/identifiant=BOI-BAREME-000037-20260407` et `…-20260706` (HTTP 200) | `pas_taux_par_defaut` des barèmes 2025 et 2026 (`droit::pas`) |
| `BOI-RSA-BASE-30-10-20-20260810.html`, `-extrait-prevoyance.md` | charges déductibles du salaire : prévoyance complémentaire (§ 200-240) | net imposable : part patronale santé réintégrée, prévoyance non ; Fonds santé des intermittents |
| `BOI-IR-PAS-30-10-20-20181203.html`, `BOI-IR-PAS-30-10-20-extrait-taux.md` | prélèvement à la source : la mise à disposition du taux par le compte rendu (§ 20-30), sa validité jusqu'à la fin du deuxième mois qui suit (§ 70), un taux encore valide au jour où il s'applique (§ 80) | `droit::pas::VALIDITE_MOIS` ; la datation d'un taux lu dans une DSN reprise (`moteur/src/commandes/pas.rs`, une inférence) ; la réponse simulée à une demande de taux (`envois/src/organismes_simules.rs`) |
| `BOI-IR-PAS-30-10-30-20-20190329.html`, `BOI-IR-PAS-30-10-30-20-extrait-reversement.md` | prélèvement à la source : la date du reversement (§ 50-60 : le 15 ou le 5 du mois suivant ; option trimestrielle sous onze salariés) | boîte de dépôt de la DSN, date du versement à la DGFiP (`paie/src/ordres_paiement.rs`, ticket #223) |
| `BOI-IR-PAS-30-10-50-20190227.html`, `BOI-IR-PAS-30-10-50-extrait-regularisation.md` | prélèvement à la source : un rappel de paie se prélève au mois de son versement (§ 1) ; un trop-versé non compensé se régularise au taux initialement utilisé (§ 90) ; seule une erreur du débiteur, au regard de ce qu'il savait au versement, se régularise, jamais un premier taux personnalisé reçu après la grille (§ 20, remarque) | le prélèvement du versement d'une correction (`engagement/src/etats.rs`, `prelevement_de_la_correction`), lu par la DSN et le bulletin rectificatif ; une paie partie garde sa grille quand le taux arrive ensuite (`engagement/src/etats.rs`, `rafraichir` ; la phrase de `routes/src/gestes/depart.rs`, `sans_taux_recu`) |
| `BOI-TPS-TS-30-20200624.html`, `BOI-TPS-PEEC-10-20141218.html`, `BOI-TPS-PEEC-20-20141218.html`, `BOI-TPS-TS-30-et-PEEC-extrait-bloc-44.md` | taxe sur les salaires : liquidation, taux majorés et part assujettie (§ 40, 110, 240) ; PEEC : qui la doit (§ 40-50), sa base (§ 1-20) | DSN de décembre, bloc S21.G00.44 (`paie/src/assujettissement.rs`, lot V3b) |

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `BOI-BAREME-000037-20240228.html` : `e2845c424ea192464ef6d4ffbb9e24e154cb5d6e4c879a1e58b87f5d15bde032`
- `BOI-BAREME-000037-20250410.html` : `7d6a960a73dd69b513537fe6e80f4e583fffdf238c896d1d777a72d3c41dd24b`
- `BOI-BAREME-000037-20260407.html` : `dd441f9cc22db6277af072246bc76bba9de2ef590f319d820fb2ea53641f7c64`
- `BOI-BAREME-000037-20260706.html` : `0602b09f6b0c67541c7a0733e30660689137105991ca9d97d3b53f2a2d4041b7`
- `BOI-IR-PAS-30-10-20-20181203.html` : `6eb7759df6d36af8d81e1768e9a45886a84f3220fd39be35de2a957cb52b34b0`
- `BOI-IR-PAS-30-10-20-extrait-taux.md` : `b2f4b4731692939207a903bd4628287b338d74f1c8e503dec680c5e5c68f6d0f`
- `BOI-IR-PAS-30-10-30-20-20190329.html` : `9011297c47214426ca585f51ba731949fd3ef9d8e0687ea4bf14042f7d659698`
- `BOI-IR-PAS-30-10-30-20-extrait-reversement.md` : `a5b014627f282c4bae37ceeb69f096b37bb04992d091723c26f44774c95bea9d`
- `BOI-IR-PAS-30-10-50-20190227.html` : `8ac1b6c68f7278121b40ffcfc9e9bbc43ee96d0b14b5ff8443dd82b629beb1a2`
- `BOI-IR-PAS-30-10-50-extrait-regularisation.md` : `6f4f4838f8ec8c95c5018aae4bec716ec5d6aea4a1649e9338acab92502fe3bf`
- `BOI-RSA-BASE-30-10-20-20260810-extrait-prevoyance.md` : `da70282da59e2a4691996f86e03a779c6a6754f8ec559fce27d49247f3bb2e05`
- `BOI-RSA-BASE-30-10-20-20260810.html` : `34c2e65183fd26c619971f2fab71dea076d4ec578e29bfdb08b818047d857762`
- `BOI-TPS-PEEC-10-20141218.html` : `c9b30274abea0722fb66c7ac0bfac237d9f21dfce10a1d8d0392b1da244e131d`
- `BOI-TPS-PEEC-20-20141218.html` : `6d22ce5acd33d2423be4fb6240bc027b36df59d5b0f9509ec99493ed16cf9555`
- `BOI-TPS-TS-30-20200624.html` : `aa3268782953b7137cda1dc8cf4831153b93eb68b2f250c7e62ec9b7339cc222`
- `BOI-TPS-TS-30-et-PEEC-extrait-bloc-44.md` : `1ec6249443553ee58235769673935b0d87e796b26ee6bc72ce2d8b7d20e14b81`
