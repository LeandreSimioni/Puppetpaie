# ISO 32000-1 (PDF 1.7) : ce que la fabrique de PDF lit d'un fichier

La fabrique de PDF (`atelier/sources/fabrique-pdf/`) lit des PDF existants (les pièces d'un assemblage, un contrat à
contresigner ou à marquer, un formulaire à remplir, un bulletin importé) et les déchiffre. Ce qu'elle tient pour une
clé absente, une valeur par défaut ou une forme exigée vient de la norme, dont ce dossier garde les passages cités.

## Les fichiers

| fichier | d'où | relevé |
|---|---|---|
| `adobe-iso32000-1-pdf17-extraits-20261111.md` | **transcription** (extraits, mot pour mot) de https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf (HTTP 200, 22 491 828 octets, sha256 `9de0ca9e8570d6209e8bd48a355be8eb6ec376acfc3fc3ae97cd8730351417ff` ; « PDF 32000-1:2008 », © Adobe Systems Incorporated 2008) ; l'original va dans `reference-privee/sources/iso-32000/` du privé | 11/11/2026 |
| `adobe-iso32000-1-annexe-d-20261111.txt` | **transcription** des tables de l'annexe D (« Character Sets and Encodings ») du même original, pages 653 à 672 : D.2 (nom, codes STD MAC WIN PDF, et ses notes 1 à 6 mot pour mot), D.3 (PDFDocEncoding : code, Unicode, notes), D.4 (MacExpertEncoding), D.5 (Symbol), D.6 (ZapfDingbats) ; une entrée par ligne, dans l'ordre des tables ; la colonne CHAR (le dessin du caractère) n'est pas transcrite. Relevée du texte de l'original (`pdftotext -layout`), les pieds de page écartés ; 229, 256, 165, 189 et 188 entrées, aucun code en double dans une colonne | 11/11/2026 |

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
  95) et les objets PostScript sans effet (8.8.2) ; les polices (tables 111, 112, 114, 117, 121, 122, 126 ; 9.6.2.2, les 14 polices standard par leur nom exact ; 9.7.4.3,
  9.7.6.3) ; les annotations et leur apparence (tables 164, 165, 168) ; les propriétés de `BDC` (14.6.2).
- `fabrique-pdf/src/texte.rs` (`Police::texte`, `predefini`, `differences_de`, `encodage_type1`,
  `encodage_truetype`, `par_cmap`, `programme_cid`) : le texte d'un code dans l'ordre de 9.10.2 : `ToUnicode` ; un
  encodage prédéfini (9.6.6.1, annexe D) ou les noms des `Differences`,
  lus par la liste des glyphes d'Adobe (`adobe-polices/`) ; l'encodage implicite (table 114) : l'encodage propre d'un
  programme Type 1 incorporé (9.6.6.2, table 127 `Length1`), StandardEncoding pour une police non symbolique non
  incorporée, ceux de D.5 et D.6 pour Symbol et ZapfDingbats non incorporées, aucun pour une Type 3 (9.6.6.3) ; une TrueType sans encodage ou symbolique : la « cmap » du programme,
  sous-table (3, 0) puis (1, 0) (9.6.6.4), le dictionnaire d'une non symbolique complété par StandardEncoding ; une
  police CID TrueType : `CIDToGIDMap` (table 117). Le glyphe d'un programme dit son caractère par son nom, sinon par
  la « cmap » Unicode du programme ; ce que rien ne dit est un caractère inconnu du texte (`texte::Inconnu` : la
  police, le code), que seul le lecteur qui en a besoin refuse (9.10.2 : « there is no way to determine what the
  character code represents ») : `import::bulletin_lu`, `aem_lu`, `certificat_lu` (la case), `TexteDePage::lu`.
- `fabrique-pdf/src/encodages.rs` (`codes`, `pdfdoc`) : la transcription de l'annexe D, lue telle quelle
  (`include_str!`) : le nom du glyphe de chaque code de StandardEncoding, MacRomanEncoding, WinAnsiEncoding (D.2),
  MacExpertEncoding (D.4), Symbol (D.5), ZapfDingbats (D.6) ; les notes 3 (WinAnsi : un code inutilisé au-delà de 40
  est le « bullet »), 5 (WinAnsi 255, « soft hyphen ») et 6 (WinAnsi 240, MacRoman 312 : « nonbreaking space ») ; le
  caractère de chaque code de PDFDocEncoding (D.3, colonne Unicode ; « Undefined » se refuse ; un code noté « U » qui
  a pourtant un Unicode dans la table prend celui-ci : inférence, la table donnant les deux).
- `fabrique-pdf/src/encodages.rs` (`chaine_de_texte`, `vers_chaine_de_texte`) : une chaîne de texte (7.9.2.2) est
  en UTF-16BE quand elle commence par les octets 254 255, en PDFDocEncoding (D.3) sinon ; rien d'autre ne se lit (ni
  UTF-16LE, ni UTF-8, que 7.9.2.2 ne connaît pas) ; la séquence d'échappement de langue tombe ; un code « Undefined »,
  de l'UTF-16 mal formé ou une séquence ouverte sans être fermée se refusent. L'écriture (la valeur et le nom d'un
  champ de formulaire) prend PDFDocEncoding quand chaque caractère y a un code que D.3 ne note pas « U », UTF-16BE
  sinon. Lecteurs : `/ActualText` (`texte.rs`), les métadonnées (`inspection.rs`), les champs (`formulaire.rs`).
- `fabrique-pdf/src/inspection.rs` (`chiffrement`) : `Filter`, `R`, `P` exigés, `Length` qui vaut 40, `CFM` qui vaut
  `None` (tables 20, 21, 25).
- `fabrique-pdf/src/formulaire.rs` (les champs) : `Fields` exigé (table 218) ; `Ff` qui vaut 0 (table 220) ; `DA`
  exigé, `Q` qui vaut 0 et n'a que trois valeurs (table 222) ; `MaxLen` (table 229) ; la couleur initiale noire d'un
  `DA` qui n'en dit pas (table 52) ; le corps 0, automatique (12.7.3.3).
