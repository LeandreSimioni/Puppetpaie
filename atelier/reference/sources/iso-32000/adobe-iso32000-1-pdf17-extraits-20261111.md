# PDF 32000-1:2008 (ISO 32000-1, PDF 1.7) : extraits

Transcription mot pour mot des passages que la fabrique de PDF cite. Source :
https://opensource.adobe.com/dc-acrobat-sdk-docs/pdfstandards/PDF32000_2008.pdf, relevé le 11/11/2026 (voir
`LISEZ-MOI.md`). « © Adobe Systems Incorporated 2008 – All rights reserved ».

## 7.3.9 Null Object

> The null object has a type and value that are unequal to those of any other object. There shall be only one
> object of type null, denoted by the keyword null. An indirect object reference (see 7.3.10, "Indirect Objects") to
> a nonexistent object shall be treated the same as a null object. Specifying the null object as the value of a
> dictionary entry (7.3.7, "Dictionary Objects") shall be equivalent to omitting the entry entirely.

## 7.3.10 Indirect Objects

> An indirect reference to an undefined object shall not be considered an error by a conforming reader; it shall be
> treated as a reference to the null object.

## 7.6.2 General Encryption Algorithm

> If using the AES algorithm, the Cipher Block Chaining (CBC) mode, which requires an initialization vector,
> is used. The block size parameter is set to 16 bytes, and the initialization vector is a 16-byte random
> number that is stored as the first 16 bytes of the encrypted stream or string.

> Strings and streams encrypted with AES shall use a padding scheme that is described in Internet RFC
> 2898, PKCS #5: Password-Based Cryptography Specification Version 2.0; see the Bibliography. For an
> original message length of M, the pad shall consist of 16 - (M mod 16) bytes whose value shall also be 16
> - (M mod 16).

## Table 20 – Entries common to all encryption dictionaries

> V number (Optional) A code specifying the algorithm to be used in encrypting and decrypting the document:
> 0 An algorithm that is undocumented. This value shall not be used. [...]
> The default value if this entry is omitted shall be 0, but when present should be a value of 1 or greater.

> CF dictionary (Optional; meaningful only when the value of V is 4; PDF 1.5) A dictionary whose keys shall be crypt
> filter names and whose values shall be the corresponding crypt filter dictionaries (see Table 25). Every crypt
> filter used in the document shall have an entry in this dictionary, except for the standard crypt filter names
> (see Table 26).

> StmF name (Optional; meaningful only when the value of V is 4; PDF 1.5) The name of the crypt filter that shall be
> used by default when decrypting streams. The name shall be a key in the CF dictionary or a standard crypt filter
> name specified in Table 26. [...] Default value: Identity.

> StrF name (Optional; meaningful only when the value of V is 4; PDF 1.5) The name of the crypt filter that shall be
> used when decrypting all strings in the document. The name shall be a key in the CF dictionary or a standard crypt
> filter name specified in Table 26. Default value: Identity.

## Table 21 – Additional encryption dictionary entries for the standard security handler

> R number (Required) A number specifying which revision of the standard security handler shall be used to
> interpret this dictionary [...]

> EncryptMetadata boolean (Optional; meaningful only when the value of V is 4; PDF 1.5) Indicates whether the
> document-level metadata stream (see 14.3.2, "Metadata Streams") shall be encrypted. Conforming products should
> respect this value. Default value: true.

## Table 26 – Standard crypt filter names

> Identity Input data shall be passed through without any processing.
