# ISO 32000-1 (PDF 1.7) : ce que la fabrique de PDF lit d'un fichier

La fabrique de PDF (`atelier/sources/fabrique-pdf/`) lit des PDF existants (les pièces d'un assemblage, un contrat à
contresigner ou à marquer, un formulaire à remplir, un bulletin importé) et les déchiffre. Ce qu'elle tient pour une
clé absente, une valeur par défaut ou une forme exigée vient de la norme, dont ce dossier garde les passages cités.

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `adobe-iso32000-1-pdf17-extraits-20261111.md` | **transcription** (extraits, mot pour mot) de https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf (HTTP 200, 22 491 828 octets, sha256 `9de0ca9e8570d6209e8bd48a355be8eb6ec376acfc3fc3ae97cd8730351417ff` ; « PDF 32000-1:2008 », © Adobe Systems Incorporated 2008) ; l'original va dans `reference-privee/sources/iso-32000/` du privé | 11/11/2026 |

## Où le moteur s'en sert

- `fabrique-pdf/src/lecture.rs` (`designe`, `valeur`, `lue`) : l'objet nul et la référence à un objet absent valent
  une clé absente (7.3.9, 7.3.10) ; une valeur d'un autre type que celui que la norme lui donne se refuse.
- `fabrique-pdf/src/chiffrement.rs` (`dechiffrer`) : `V` absent vaut 0, un algorithme non documenté (table 20) ; `R`
  est exigé (table 21) ; `StmF` et `StrF` absents valent `Identity` (table 20), qui laisse en clair (table 26) ;
  `EncryptMetadata` absent vaut `true` (table 21) ; l'IV en tête et le bourrage PKCS #5 d'une donnée AES (7.6.2).
- `fabrique-pdf/src/lecture.rs` (`contenu`, `herite`, `cadre_de`, `rotation_de`, `ressources_de_page`,
  `contenu_de_page`) : les filtres d'un flux et leurs `DecodeParms` (table 5), ASCIIHex, ASCII85 et RunLength défaits
  (7.4.2, 7.4.3, 7.4.5) ; l'héritage d'un attribut de page (7.7.3.4) ; `Resources` et `MediaBox` exigés là où ils
  servent (assembler, marquer, remplir, inspecter), `CropBox` qui vaut `MediaBox`, `Rotate` multiple de 90 qui vaut
  0, `Contents` absent = page vide (table 30).
- `fabrique-pdf/src/texte.rs` (le texte d'une page, `--texte`, `--inspecter`) : la chaîne hexadécimale d'un CMap
  (7.3.4.3) ; `Tf` avant tout texte (table 105) ; les objets formulaire, leur `Matrix`, leurs ressources, `BBox` (table
  95) et les objets PostScript sans effet (8.8.2) ; les polices (tables 111, 112, 114, 117, 121, 122, 126 ; 9.7.4.3,
  9.7.6.3) ; les annotations et leur apparence (tables 164, 165, 168) ; les propriétés de `BDC` (14.6.2).
- `fabrique-pdf/src/texte.rs` (`Police::texte`, `predefini`, `differences_de`, `encodage_type1`,
  `encodage_truetype`, `par_cmap`, `programme_cid`) : le texte d'un code dans l'ordre de 9.10.2 : `ToUnicode` ; un
  encodage prédéfini (9.6.6.1, annexe D et sa note 5 : le « soft hyphen » de WinAnsi) ou les noms des `Differences`,
  lus par la liste des glyphes d'Adobe (`adobe-polices/`) ; l'encodage implicite (table 114) : l'encodage propre d'un
  programme Type 1 incorporé (9.6.6.2, table 127 `Length1`), StandardEncoding pour une police non symbolique non
  incorporée, aucun pour une Type 3 (9.6.6.3) ; une TrueType sans encodage ou symbolique : la « cmap » du programme,
  sous-table (3, 0) puis (1, 0) (9.6.6.4), le dictionnaire d'une non symbolique complété par StandardEncoding ; une
  police CID TrueType : `CIDToGIDMap` (table 117). Le glyphe d'un programme dit son caractère par son nom, sinon par
  la « cmap » Unicode du programme ; ce que rien ne dit se refuse quand il se montre (9.10.2 : « there is no way to
  determine what the character code represents »).
- `fabrique-pdf/src/inspection.rs` (`chiffrement`) : `Filter`, `R`, `P` exigés, `Length` qui vaut 40, `CFM` qui vaut
  `None` (tables 20, 21, 25).
- `fabrique-pdf/src/formulaire.rs` (les champs) : `Fields` exigé (table 218) ; `Ff` qui vaut 0 (table 220) ; `DA`
  exigé, `Q` qui vaut 0 et n'a que trois valeurs (table 222) ; `MaxLen` (table 229) ; la couleur initiale noire d'un
  `DA` qui n'en dit pas (table 52) ; le corps 0, automatique (12.7.3.3).
