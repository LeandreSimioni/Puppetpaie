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

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `adobe-afm-extraits-20261111.md` : `d6188a06d7555615a108d8eb6548c25813fac4eeafdf3dd8e9f43bf6ab4c0a9c`
- `adobe-agl-specification-20261111.md` : `eb14692b0ff134fd44dd2dae014b7a0ea81fccb893982f950e6bf163eb6827a3`
- `adobe-cff-extraits-20261111.md` : `098584c94ea180f40cb079972a415a88436a0456674b5b5f5377d39369dc9a91`
- `adobe-cff-tables-20261111.txt` : `7b4f6a7681e9661ba616fabeb76436d4fa7cfca135e6c6055c827269b0b5e9f1`
- `adobe-core14-afm-20261111/Courier-Bold.afm` : `ad0150d4bedcc8877742bf94251fcec13e348dd599d4603f679d92027d1e6e99`
- `adobe-core14-afm-20261111/Courier-BoldOblique.afm` : `cb82e69ef5f6d421e8f404fe00bb0d993425aae79725331d0ebf847e94e97e92`
- `adobe-core14-afm-20261111/Courier-Oblique.afm` : `b27103b2a2ef6030c110626597e2ab47bb8279075a039ab9170facb0aa1f70e1`
- `adobe-core14-afm-20261111/Courier.afm` : `521e0d7c7521efd4be78a5a9c5398e4c67d0771e396115b0346bc4ef74ada53d`
- `adobe-core14-afm-20261111/Helvetica-Bold.afm` : `b880d96baf56d0cc059f258f60b4d764ef49b555ab9db294b959c0016dee41f2`
- `adobe-core14-afm-20261111/Helvetica-BoldOblique.afm` : `69984a35ca26973a39f261cf83e0d367ea2e6517c590b0b030d4d4a219d9c269`
- `adobe-core14-afm-20261111/Helvetica-Oblique.afm` : `b4609b71b660a392ac09df35060271a876c2ce66617dd83bf826f742bb9d9721`
- `adobe-core14-afm-20261111/Helvetica.afm` : `da33f1870474c8e68bfe3e2353ff107ab6c6eea1f9836ce2aaf1e1a07b17982f`
- `adobe-core14-afm-20261111/MustRead.html` : `be71e055bb2551a6fc8b60dcf094bf40357858980abde86f76cc9a3c144d5403`
- `adobe-core14-afm-20261111/Symbol.afm` : `3d2128a820375a10de9bc8bf6cfb15ded482c01ca0f95cc0b3277f37ec8bde66`
- `adobe-core14-afm-20261111/Times-Bold.afm` : `b4a000ed85cb22c6cdd985aa0fd3f6f78ed5079b7c6860dc4f0234e0d0e3c522`
- `adobe-core14-afm-20261111/Times-BoldItalic.afm` : `93c4744ba955215de02c4aae0b777133442ada2f7ab5a2af30a040b792b3c55d`
- `adobe-core14-afm-20261111/Times-Italic.afm` : `ed37fa2e6a67b5b17dfd47f36fc7e90df32891a4408860dbc8d4cbbe9959e242`
- `adobe-core14-afm-20261111/Times-Roman.afm` : `768e1cabea085d489a63da3e80b96bc5abf0ec98d3073c9b4d6ba76e7bccba64`
- `adobe-core14-afm-20261111/ZapfDingbats.afm` : `a32565c90afd1b57a7008fc567b78d95cf1c22adff5e086094d666d88b039859`
- `adobe-glyphlist-20261111.txt` : `a3b2f61ced9f3644cc0d4ecde5c59df34ca286c689d9484a43a710a81c466789`
- `adobe-type1-font-format-extraits-20261111.md` : `ea6db85bdc4c97e2d3f2b566919f7ec87d48190e431a9d5fcb8f88b5dbe7ef7e`
- `adobe-zapfdingbats-20261111.txt` : `f6394e3cb8a447e84a1dad75d4baaf2aa7f45dc104faf369f4720e1a774ef2dc`
