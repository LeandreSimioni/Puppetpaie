# Adobe Type 1 Font Format (1990) : extraits

Transcription mot pour mot de passages de https://adobe-type-tools.github.io/font-tech-notes/pdfs/T1_SPEC.pdf (HTTP 200,
1 812 144 octets, sha256 `1ca29a1f6d49ae129463c03aa6a51bc247ef94b36c7ee9a3c04d8694c67dae32`, « Adobe Type 1 Font
Format », © Adobe Systems Incorporated). L'original est dans `reference-privee/sources/adobe-polices/` du privé.

## 2.3 Explanation of a Typical Font Program (l'exemple, p. 12)

> /Encoding 256 array
> 0 1 255 {1 index exch /.notdef put } for
> dup 32 /space put
> %...
> % . . . repetitive assignments to Encoding array omitted
> %...
> dup 254 /bracerightbt put
> readonly def

> Next in the example program is the Encoding array. The Encoding array determines which character codes are
> associated with which character names in the font program. This character encoding can be changed without altering
> anything else in the font program.

## 10.3 Keywords (p. 84)

> Values for certain keywords must conform to their own rules as described as follows:
>
> • The tokens following /Encoding may be StandardEncoding def, in which case the Adobe Standard Encoding will be
> assigned to this font program. For special encodings, assignments must be performed as shown in the example in
> section 2.3, "Explanation of a Typical Font Program," using the repetitive sequence:
> dup index charactername put
>
> where index is an integer corresponding to an entry in the Encoding vector, and charactername refers to a PostScript
> language name token, such as /Alpha or /A, giving the character name assigned to a particular character code. The
> Adobe Type Manager parser skips to the first dup token after /Encoding to find the first character encoding
> assignment. This sequence of assignments must be followed by an instance of the token def or readonly; such a token
> may not occur within the sequence of assignments.
