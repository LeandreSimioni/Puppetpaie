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
