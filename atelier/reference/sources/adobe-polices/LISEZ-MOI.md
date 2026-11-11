# Les polices d'Adobe : les noms de glyphes et l'encodage d'un programme Type 1

Ce qu'un PDF reçu dit de son texte par les noms de ses glyphes, et l'encodage propre d'une police Type 1 incorporée.
ISO 32000-1 y renvoie : 9.10.2 (« Look up the character name in the Adobe Glyph List »), table 114 et 9.6.6.2 (« A
Type 1 font's built-in encoding shall be defined by an Encoding array that is part of the font program »).

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `adobe-glyphlist-20261111.txt` | tel que servi : https://raw.githubusercontent.com/adobe-type-tools/agl-aglfn/master/glyphlist.txt (HTTP 200, 78 060 octets, sha256 `a3b2f61ced9f3644cc0d4ecde5c59df34ca286c689d9484a43a710a81c466789`) ; l'Adobe Glyph List 2.0, licence BSD d'Adobe | 11/11/2026 |
| `adobe-zapfdingbats-20261111.txt` | tel que servi : https://raw.githubusercontent.com/adobe-type-tools/agl-aglfn/master/zapfdingbats.txt (HTTP 200, 3 879 octets) ; l'ITC Zapf Dingbats Glyph List | 11/11/2026 |
| `adobe-agl-specification-20261111.md` | tel que servi : https://raw.githubusercontent.com/adobe-type-tools/agl-specification/master/README.md (HTTP 200, sha256 `eb14692b0ff134fd44dd2dae014b7a0ea81fccb893982f950e6bf163eb6827a3`) ; « 2. The mapping » : d'un nom de glyphe à des caractères | 11/11/2026 |
| `adobe-type1-font-format-extraits-20261111.md` | **transcription** (extraits) de https://adobe-type-tools.github.io/font-tech-notes/pdfs/T1_SPEC.pdf ; l'original va dans `reference-privee/sources/adobe-polices/` du privé | 11/11/2026 |
| `adobe-cff-extraits-20261111.md` | **transcription** (sections 2 à 13, mise en page aplatie) de https://adobe-type-tools.github.io/font-tech-notes/pdfs/5176.CFF.pdf (HTTP 200, 1 062 494 octets, sha256 `60e0696c4fac326de597722af37bcdbbd688e0337a548709a287405ff3b3996a` ; Adobe Technical Note #5176, « The Compact Font Format Specification », 4 Dec 03) ; l'original va dans `reference-privee/sources/adobe-polices/adobe-cff-5176-20261111.pdf` du privé | 11/11/2026 |
| `adobe-core14-afm-20261111/` | tels que servis, l'archive https://download.macromedia.com/pub/developer/opentype/tech-notes/Core14_AFMs.zip (HTTP 200, 127 150 octets, sha256 `8c892c3c49553cfd2d2a27c4495b4bb12e2875115be7fd127ed3876df19d8654`) dépliée : les 14 fichiers de métriques (AFM 4.1) des polices standard et `MustRead.html`, leur licence (« may be used, copied, and distributed [...] provided [...] that the AFM files are not distributed without this file »), sans modification | 11/11/2026 |
| `adobe-afm-extraits-20261111.md` | **transcription** (sections 7, `Ascender` et `Descender`, et 8, « Individual Character Metrics ») de https://adobe-type-tools.github.io/font-tech-notes/pdfs/5004.AFM_Spec.pdf (HTTP 200, 245 931 octets, sha256 `7adf2c10f4ee581e1d5ecf51284b17946c9430410e46336f9b57889a09d2f793` ; Adobe Technical Note #5004, version 4.1, 7 Oct 98) ; l'original va dans `reference-privee/sources/adobe-polices/adobe-afm-5004-20261111.pdf` du privé | 11/11/2026 |
| `adobe-cff-tables-20261111.txt` | **transcription** des annexes A (391 chaînes standard), B (encodages Standard et Expert, 256 codes chacun) et C (jeux ISOAdobe, Expert, ExpertSubset : 228, 165 et 86 glyphes, dans l'ordre des GID) du même original ; relevée du texte (`pdftotext -layout`, colonnes relues dans l'ordre de la page), chaque SID et son nom vérifiés contre l'annexe A | 11/11/2026 |

## Où le moteur s'en sert

- `fabrique-pdf/src/glyphes.rs` (`caracteres`) : la liste et la liste Zapf Dingbats, lues telles quelles
  (`include_str!`) ; un nom de glyphe se lit selon « 2. The mapping » de la spécification : le suffixe après le premier
  point tombe, les composants séparés par `_`, chacun par la liste Zapf Dingbats (police ZapfDingbats), la liste, puis
  les formes `uniXXXX` et `uXXXX[XX]` ; un composant que rien ne dit ne donne rien.
- `fabrique-pdf/src/texte.rs` (`encodage_type1`) : l'encodage propre d'un programme Type 1 incorporé (`FontFile`, sa
  partie en clair) : `/Encoding StandardEncoding def`, ou la suite des `dup index charactername put` jusqu'à `def` ou
  `readonly` (10.3).
- `fabrique-pdf/src/cff.rs` (`noms_des_codes`, appelé par `texte.rs`, `encodage_cff`) : l'encodage propre d'un
  programme compact incorporé (`FontFile3` de sous-type `Type1C`), lu selon la spécification CFF : l'en-tête (section
  6), les INDEX (5), le Top DICT (4, tables 3 et 9 : `charset` 15, `Encoding` 16, `CharStrings` 17 ; `ROS` 12 30 d'une
  CIDFont, sans encodage), les chaînes (10, annexe A), le jeu de caractères (13, formats 0, 1, 2 ; ISOAdobe, Expert,
  ExpertSubset de l'annexe C) et l'encodage (12, formats 0 et 1 et leurs suppléments, table 14 ; Standard et Expert de
  l'annexe B, du code au SID, note 4), les tableaux commençant au GID 1 (11). Un code que l'encodage ne donne pas n'a
  pas de glyphe : rien n'est complété par StandardEncoding.
- `fabrique-pdf/src/afm.rs` (`ascendant_descendant`, appelé par `texte.rs` pour les boîtes de `--inspecter`) :
  `Ascender` et `Descender` (section 7 de la spécification AFM, « Optional ») du fichier de métriques d'une police
  standard non incorporée que le `/BaseFont` nomme exactement (Courier, Helvetica, Times et leurs variantes ; ceux de
  Symbol et ZapfDingbats n'en donnent pas : le descripteur, s'il y en a un, les dit).
- `fabrique-pdf/src/afm.rs` (`largeurs`, appelé par `texte.rs`, `charger_police`) : la largeur d'un glyphe d'une
  police standard sans `/Widths` (ISO 32000-1, table 111 et 9.6.2.2, qui renvoie aux fichiers de métriques et à la
  note technique 5004), par le nom exact de la police parmi les 14 : la section 8 de son fichier (`StartCharMetrics`
  … `EndCharMetrics`, chaque ligne son `N` et son `WX`) ; le code mène au nom du glyphe par l'encodage de la police
  (prédéfini, `Differences`, propre au programme, D.5 et D.6 ; les doubles des notes 5 et 6 de D.2 au glyphe dont ils
  sont « typographically the same »). Un code sans nom, ou dont le nom n'est pas dans le fichier, et toute police d'un
  autre nom prennent `MissingWidth` (table 111). Le marquage ne s'en sert pas : il lit nos contrats, qui portent
  leurs largeurs (`W`).
