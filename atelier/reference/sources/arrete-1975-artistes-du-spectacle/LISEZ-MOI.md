# Arrêté du 24 janvier 1975 (artistes du spectacle)

Relevé le 10/10/2026 dans la base LEGI de la DILA (https://echanges.dila.gouv.fr/OPENDATA/LEGI/, archive globale du
13/07/2025 et 455 livraisons jusqu'au 09/10/2026), par `atelier/outils/relever_les_codes.py` (texte `artistes-1975`,
JORFTEXT000000310494 : la base range un texte non codifié sous son JORFTEXT). `releve.json` dit les versions, `xml/`
les pièces, `articles-20261010.md` le texte.

- art. 1 et 2 : les taux des artistes (cotisations, FNAL, versement transport) à 70 % du régime général. Le moteur les
  porte « cuits » dans les taux du barème (`logiciel/donnees/baremes/2026.json`, `_statuts`) et le facteur AT/MP 0,70.
- art. 3, al. 2 : le plafond d'un artiste engagé moins de cinq jours continus, douze fois le plafond horaire par
  journée de travail. Le moteur : `paie/src/plafond.rs` (`plafond_urssaf`, `pour_paie`, `plafond_journees`), à la
  lettre depuis la vague 1l (10/10/2026, `prive/journal/archives/2026-10-08-lots/norme/corrections/PASSATION-V1l.md`) : la « période
  d'engagement continu » est le contrat, du début à la fin, en jours calendaires ; une journée de travail est une date
  distincte des lignes de paie (répétitions comprises, quel que soit le nombre de cachets du jour) ; le plafond « est
  égal » à 12 × le plafond horaire par journée, sans maximum avec le prorata ; une journée dite et non datée refuse.
  L'arrêté ne vise que les artistes (art. 1, 15° de L. 311-3 CSS) : un technicien a le prorata de R. 242-2 (depuis la
  vague 1o : le prorata des jours rémunérés de la paie, BOSS § 1090, comme un artiste engagé cinq jours ou plus). Avant la
  vague 1l, `plafond.rs` comptait des cachets, jugeait « isolé » un cachet sur les jours travaillés d'affilée et prenait
  le maximum avec le prorata. Depuis la vague 1i, ce plafond porte la vieillesse plafonnée, le FNAL et le chômage dans
  `calcul::calculer` (voir `../plafond-intermittents/LISEZ-MOI.md`).
- Lectures concordantes : BOSS, assiette générale, § 680 et § 1100 (`../boss/`, version du 01/08/2026 ; le § 1100 dit
  « par jour d'engagement » là où l'arrêté dit « journée de travail accomplie » : le moteur suit l'arrêté) ; modèle de
  calcul de l'Urssaf, `… artiste . plafond proratisé` (`../urssaf-mon-entreprise/`, « nombre jours travaillés »).
- art. 3 bis : les utilisations secondaires ; le moteur ne les traite pas.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20261010.md` : `58e7b6abbf6d4fcd8ae48ab6f7345fc740f7e8670e0f85981bef3a5a42efc663`
- `articles-20261015.md` : `154912d457ba35708d07e52c1cbc6e8d1f35313b64c4f243721da03878d18802`
- `articles-20261103.md` : `3928abeac1729fbd91a2bf4cd8b120c986af538b62c6af946f32a843dc6145e8`
- `articles-20261110.md` : `656b00a73a79d8eb14f60197bdb90593766fff57c4f02a1a16ea49d1d96ea0ca`
- `releve.json` : `b6157b53e2fab4e1f5369200aff8cf9082dd35b21cc60425df0e9946df66b1bb`
- `xml/1.xml` : `3a72dd9ec4fa1a36d25bf9b93d70cbc83046060ba0bf402cddcb153e599affd5`
- `xml/2.xml` : `14ed482ba4ed2f8536e88a22cc0272141b67b9d5db363d3d0b960a418d3ac2c3`
- `xml/3.xml` : `3013ded1569b95027c9085d5592f5719cc86076fbe601de78f40f8809a00d9d7`
- `xml/3_bis.xml` : `2fae2ce0e9715f98379a6c5bf3a7b45e8f0d19184e0b192919592c88e1d04f3c`
