# Le décret n° 2006-781 du 3 juillet 2006 : les frais de déplacement des personnels civils de l'État

`articles-20261103.md`, `xml/`, `releve.json` : relevés par `moteur-atelier releve-codes moissonner` puis `rapport` le
03/11/2026, depuis la base LEGI de la DILA (`JORFTEXT000000242359`, NOR BUDB0620002D ; l'identifiant se lit dans les
liens des articles de l'entrepot qui le citent). Le texte suivi : `atelier/sources/atelier/src/releve/codes/textes.rs`,
clé `missions-2006`. Le décret entier (articles 1 à 14) est dans l'entrepot (`../legi-entrepot/`) ; le relevé garde les
deux qui servent.

**Ce qu'on en lit.**
- Art. 3 : l'indemnité de mission rembourse forfaitairement les frais supplémentaires de repas et d'hébergement ; à
  l'étranger, logé ou nourri gratuitement, elle est réduite d'un pourcentage fixé par arrêté (l'arrêté du même jour,
  art. 2 : `../arrete-2006-indemnites-de-mission/`).
- Art. 7 : à l'étranger, un arrêté fixe les taux des indemnités de mission par pays, ou par ville ou par région.
- **Le décret ne dit ni dans quelle monnaie l'indemnité se paie, ni à quel taux elle se convertit.** Aucun de ses articles
  (1 à 14) ne parle de monnaie, de change ou de taux de chancellerie. La conversion au taux de chancellerie du jour reste
  une INFÉRENCE (`../dgfip-frais-de-mission/LISEZ-MOI.md`).

**Où le moteur s'en sert.** `droit::missions` (l'indemnité du jour à l'étranger, la limite d'exonération que le BOSS
lui adosse, § 1380-1390).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20261103.md` : `2ed8f93ec575e970c9360249ac45dde7ec5d99d6e410094d7c1067708a4667d7`
- `articles-20261110.md` : `767ffb66338a31aab344227e7170c3cf3b8b0bade61b515b5c8d3f8c38fba002`
- `releve.json` : `e85886872e2ca9f68e78cc11ebce0cbe7bf31a3807e422e187ae1f3ab2ce7362`
- `xml/3.xml` : `26959238f65a5661abe2433d62ca260163cff344edd104c9e4bc8e10706ac90c`
- `xml/7.xml` : `9554c5e98618018c59e3b6e3437a38676b5869499bef512f05c70dc2c11ecc8e`
