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

## Où le moteur s'en sert

- `fabrique-pdf/src/glyphes.rs` (`caracteres`) : la liste et la liste Zapf Dingbats, lues telles quelles
  (`include_str!`) ; un nom de glyphe se lit selon « 2. The mapping » de la spécification : le suffixe après le premier
  point tombe, les composants séparés par `_`, chacun par la liste Zapf Dingbats (police ZapfDingbats), la liste, puis
  les formes `uniXXXX` et `uXXXX[XX]` ; un composant que rien ne dit ne donne rien.
- `fabrique-pdf/src/texte.rs` (`encodage_type1`) : l'encodage propre d'un programme Type 1 incorporé (`FontFile`, sa
  partie en clair) : `/Encoding StandardEncoding def`, ou la suite des `dup index charactername put` jusqu'à `def` ou
  `readonly` (10.3).
