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

## Table 20 (suite) – Entries common to all encryption dictionaries

> Filter name (Required) The name of the preferred security handler for this document.

> Length integer (Optional; PDF 1.4; only if V is 2 or 3) The length of the encryption key, in bits. The value shall
> be a multiple of 8, in the range 40 to 128. Default value: 40.

## Table 21 (suite)

> P integer (Required) A set of flags specifying which operations shall be permitted when the document is opened
> with user access (see Table 22).

## Table 25 – Entries common to all crypt filter dictionaries

> CFM name (Optional) The method used, if any, by the conforming reader to decrypt data. [...] Default value: None.

## 7.3.4.3 Hexadecimal Strings

> If the final digit of a hexadecimal string is missing—that is, if there is an odd number of digits—the final digit
> shall be assumed to be 0.

## Table 5 – Entries common to all stream dictionaries

> Filter name or array (Optional) The name of a filter that shall be applied in processing the stream data found
> between the keywords stream and endstream, or an array of zero, one or several names.

## Table 30 – Entries in a page object

> Resources dictionary (Required; inheritable) A dictionary containing any resources required by the page [...].
> Omitting the entry entirely indicates that the resources shall be inherited from an ancestor node in the page tree.

> MediaBox rectangle (Required; inheritable) A rectangle (see 7.9.5, "Rectangles"), expressed in default user space
> units, that shall define the boundaries of the physical medium on which the page shall be displayed or printed.

> CropBox rectangle (Optional; inheritable) A rectangle, expressed in default user space units, that shall define the
> visible region of default user space. [...] Default value: the value of MediaBox.

> Contents stream or array (Optional) A content stream (see 7.8.2, "Content Streams") that shall describe the
> contents of this page. If this entry is absent, the page shall be empty. The value shall be either a single stream
> or an array of streams.

> Rotate integer (Optional; inheritable) The number of degrees by which the page shall be rotated clockwise when
> displayed or printed. The value shall be a multiple of 90. Default value: 0.

## 7.7.3.4 Inheritance of Page Attributes

> If such an attribute is omitted from a page object, its value shall be inherited from an ancestor node in the
> page tree. If the attribute is a required one, a value shall be supplied in an ancestor node. If the attribute is
> optional and no inherited value is specified, the default value shall be used.

## Table 52 – Device-Independent Graphics State Parameters

> colour [...] There are two separate colour parameters: one for stroking and one for all other painting
> operations. Initial value: black.

## 8.8.2 PostScript XObjects

> A PostScript XObject is an XObject stream whose Subtype entry has the value PS. [...] they shall have no effect
> either when viewing the document on-screen or when printing it to a non-PostScript device.

## Table 95 – Additional Entries Specific to a Type 1 Form Dictionary

> Subtype name (Required) The type of XObject that this dictionary describes; shall be Form for a form XObject.

> BBox rectangle (Required) An array of four numbers in the form coordinate system [...].

> Matrix array (Optional) An array of six numbers specifying the form matrix [...]. Default value: the identity
> matrix [ 1 0 0 1 0 0 ].

> Resources dictionary (Optional but strongly recommended; PDF 1.2) [...] In a PDF whose version is 1.1 and earlier,
> all named resources used in the form XObject shall be included in the resource dictionary of each page object on
> which the form XObject appears [...].

## Table 105 – Text state operators

> font size Tf Set the text font, Tf, to font and the text font size, Tfs, to size. font shall be the name of a font
> resource in the Font subdictionary of the current resource dictionary [...]. There is no initial value for either
> font or size; they shall be specified explicitly by using Tf before any text is shown.

## Table 111 – Entries in a Type 1 font dictionary

> Subtype name (Required) The type of font; shall be Type1 for a Type 1 font.

> BaseFont name (Required) The PostScript name of the font.

> FirstChar integer (Required except for the standard 14 fonts) The first character code defined in the font's
> Widths array.

> Widths array (Required except for the standard 14 fonts; indirect reference preferred) [...] For character codes
> outside the range FirstChar to LastChar, the value of MissingWidth from the FontDescriptor entry for this font
> shall be used.

> For the standard 14 fonts, the entries FirstChar, LastChar, Widths, and FontDescriptor shall either all be present
> or all be absent.

## Table 112 – Entries in a Type 3 font dictionary

> Subtype name (Required) The type of font; shall be Type3 for a Type 3 font.

(La table ne compte pas de `BaseFont`.)

## Table 114 – Entries in an encoding dictionary

> BaseEncoding name (Optional) [...] If this entry is absent, the Differences entry shall describe differences from
> an implicit base encoding. For a font program that is embedded in the PDF file, the implicit base encoding shall be
> the font program's built-in encoding [...]. Otherwise, for a nonsymbolic font, it shall be StandardEncoding, and
> for a symbolic font, it shall be the font's built-in encoding.

## Table 117 – Entries in a CIDFont dictionary

> DW integer (Optional) The default width for glyphs in the CIDFont (see 9.7.4.3, "Glyph Metrics in CIDFonts").
> Default value: 1000 (defined in user units).

## 9.7.4.3 Glyph Metrics in CIDFonts

> The W array allows the definition of widths for individual CIDs. The elements of the array shall be organized in
> groups of two or three, where each group shall be in one of these two formats:
> c [ w1 w2 … wn ]
> cfirst clast w

## Table 121 – Entries in a Type 0 font dictionary

> Encoding name or stream (Required) The name of a predefined CMap, or a stream containing a CMap that maps character
> codes to font numbers and CIDs.

> DescendantFonts array (Required) A one-element array specifying the CIDFont dictionary that is the descendant of
> this Type 0 font.

## 9.7.6.3 Handling Undefined Characters

> If the CMap does not contain either a character mapping or a notdef mapping for the code, descendant 0 shall be
> selected and the glyph for CID 0 shall be substituted from the associated CIDFont.

> If the code is invalid—that is, the bytes extracted from the string to be shown do not match any codespace range
> in the CMap—[...] a) If the first byte extracted from the string to be shown does not match the first byte of any
> codespace range, the range having the shortest codes shall be chosen.

## Table 122 – Entries common to all font descriptors

> MissingWidth number (Optional) The width to use for character codes whose widths are not specified in a font
> dictionary's Widths array. [...] Default value: 0.

## Table 126 – Embedded font organization for various font types

> FontFile3 OpenType (PDF 1.6) OpenType® font program, as described in the OpenType Specification v.1.4.

## Table 164 – Entries common to all annotation dictionaries

> Subtype name (Required) The type of annotation that this dictionary describes.

> Rect rectangle (Required) The annotation rectangle, defining the location of the annotation on the page in default
> user space units.

> F integer (Optional; PDF 1.1) A set of flags specifying various characteristics of the annotation (see 12.5.3,
> "Annotation Flags"). Default value: 0.

> AS name (Required if the appearance dictionary AP contains one or more subdictionaries; PDF 1.2) The annotation's
> appearance state, which selects the applicable appearance stream from an appearance subdictionary.

## Table 165 – Annotation flags

> 2 Hidden (PDF 1.2) If set, do not display or print the annotation [...].

> 6 NoView (PDF 1.3) If set, do not display the annotation on the screen [...].

## Table 168 – Entries in an appearance dictionary

> N stream or dictionary (Required) The annotation's normal appearance.

## 14.6.2 Property Lists

> If any of the values are indirect references to objects outside the content stream, the property list dictionary
> shall be defined as a named resource in the Properties subdictionary of the current resource dictionary [...] and
> referenced by name as the properties operand of the DP or BDC operator.

## Table 218 – Entries in the interactive form dictionary

> Fields array (Required) An array of references to the document's root fields.

## Table 220 – Entries common to all field dictionaries

> Ff integer (Optional; inheritable) A set of flags specifying various characteristics of the field (see Table 221).
> Default value: 0.

## Table 222 – Additional entries common to all fields containing variable text

> DA string (Required; inheritable) The default appearance string containing a sequence of valid page-content
> graphics or text state operators that define such properties as the field's text size and colour.

> Q integer (Optional; inheritable) A code specifying the form of quadding (justification) that shall be used in
> displaying the text: 0 Left-justified 1 Centered 2 Right-justified Default value: 0 (left-justified).

## 12.7.3.3 Variable Text

> A zero value for size means that the font shall be auto-sized: its size shall be computed as a function of the
> height of the annotation rectangle.

## Table 229 – Additional entry specific to a text field

> MaxLen integer (Optional; inheritable) The maximum length of the field's text, in characters.

## 9.6.3 TrueType Fonts

> A TrueType font dictionary may contain the same entries as a Type 1 font dictionary (see Table 111), with these
> differences: [...]

## Table 111 (suite) – Entries in a Type 1 font dictionary

> FontDescriptor dictionary (Required except for the standard 14 fonts; shall be an indirect reference) A font
> descriptor describing the font's metrics other than its glyph widths (see 9.8, "Font Descriptors").

## 9.6.6.4 Encodings for TrueType Fonts

> A TrueType font program's built-in encoding maps directly from character codes to glyph descriptions by means of
> an internal data structure called a "cmap" (not to be confused with the CMap described in 9.7.5, "CMaps"). [...]
>
> A "cmap" table may contain one or more subtables that represent multiple encodings intended for use on different
> platforms (such as Mac OS and Windows). Each subtable shall be identified by the two numbers, such as (3, 1), that
> represent a combination of a platform ID and a platform-specific encoding ID, respectively. [...]
>
> The following paragraphs describe the treatment of TrueType font encodings beginning with PDF 1.3.
>
> If the font has a named Encoding entry of either MacRomanEncoding or WinAnsiEncoding, or if the font descriptor's
> Nonsymbolic flag (see Table 123) is set, the conforming reader shall create a table that maps from character codes
> to glyph names:
>
> - If the Encoding entry is one of the names MacRomanEncoding or WinAnsiEncoding, the table shall be initialized
>   with the mappings described in Annex D.
> - If the Encoding entry is a dictionary, the table shall be initialized with the entries from the dictionary's
>   BaseEncoding entry (see Table 114). Any entries in the Differences array shall be used to update the table.
>   Finally, any undefined entries in the table shall be filled using StandardEncoding.
>
> [...]
>
> When the font has no Encoding entry, or the font descriptor's Symbolic flag is set (in which case the Encoding entry
> is ignored), this shall occur:
>
> - If the font contains a (3, 0) subtable, the range of character codes shall be one of these: 0x0000 - 0x00FF,
>   0xF000 - 0xF0FF, 0xF100 - 0xF1FF, or 0xF200 - 0xF2FF. Depending on the range of codes, each byte from the string
>   shall be prepended with the high byte of the range, to form a two-byte character, which shall be used to select
>   the associated glyph description from the subtable.
> - Otherwise, if the font contains a (1, 0) subtable, single bytes from the string shall be used to look up the
>   associated glyph descriptions from the subtable.
>
> If a character cannot be mapped in any of the ways described previously, a conforming reader may supply a mapping
> of its choosing.

## Table 122 (suite) – Entries common to all font descriptors

> Flags integer (Required) A collection of flags defining various characteristics of the font (see 9.8.2, "Font
> Descriptor Flags").

## Table 123 – Font flags

> 3 Symbolic Font contains glyphs outside the Adobe standard Latin character set. This flag and the Nonsymbolic flag
> shall not both be set or both be clear.
>
> 6 Nonsymbolic Font uses the Adobe standard Latin character set or a subset of it.

## 9.10.2 Mapping Character Codes to Unicode Values

> A conforming reader can use these methods, in the priority given, to map a character code to a Unicode value. [...]
>
> - If the font dictionary contains a ToUnicode CMap (see 9.10.3, "ToUnicode CMaps"), use that CMap to convert the
>   character code to Unicode.
> - If the font is a simple font that uses one of the predefined encodings MacRomanEncoding, MacExpertEncoding, or
>   WinAnsiEncoding, or that has an encoding whose Differences array includes only character names taken from the
>   Adobe standard Latin character set and the set of named characters in the Symbol font (see Annex D): a) Map the
>   character code to a character name according to Table D.1 and the font's Differences array. b) Look up the
>   character name in the Adobe Glyph List (see the Bibliography) to obtain the corresponding Unicode value.
>
> [...]
>
> If these methods fail to produce a Unicode value, there is no way to determine what the character code represents
> in which case a conforming reader may choose a character code of their choosing.

## Table 5 (suite) – Entries common to all stream dictionaries

> DecodeParms dictionary or array (Optional) A parameter dictionary or an array of such dictionaries, used by the
> filters specified by Filter. If there is only one filter and that filter has parameters, DecodeParms shall be set to
> the filter's parameter dictionary unless all the filter's parameters have their default values, in which case the
> DecodeParms entry may be omitted. If there are multiple filters and any of the filters has parameters set to
> nondefault values, DecodeParms shall be an array with one entry for each filter: either the parameter dictionary for
> that filter, or the null object if that filter has no parameters (or if all of its parameters have their default
> values). If none of the filters have parameters, or if all their parameters have default values, the DecodeParms
> entry may be omitted.

## 7.4.2 ASCIIHexDecode Filter

> The ASCIIHexDecode filter shall produce one byte of binary data for each pair of ASCII hexadecimal digits (0–9 and
> A–F or a–f). All white-space characters (see 7.2, "Lexical Conventions") shall be ignored. A GREATER-THAN SIGN (3Eh)
> indicates EOD. Any other characters shall cause an error. If the filter encounters the EOD marker after reading an
> odd number of hexadecimal digits, it shall behave as if a 0 (zero) followed the last digit.

## 7.4.3 ASCII85Decode Filter

> The ASCII base-85 encoding shall use the ASCII characters ! through u ((21h) - (75h)) and the character z (7Ah),
> with the 2-character sequence ~> (7Eh)(3Eh) as its EOD marker. The ASCII85Decode filter shall ignore all white-space
> characters (see 7.2, "Lexical Conventions"). Any other characters, and any character sequences that represent
> impossible combinations in the ASCII base-85 encoding shall cause an error.
>
> Specifically, ASCII base-85 encoding shall produce 5 ASCII characters for every 4 bytes of binary data. Each group
> of 4 binary input bytes, (b1 b2 b3 b4 ), shall be converted to a group of 5 output bytes, (c1 c2 c3 c4 c5 ), using
> the relation (b1 × 256³) + (b2 × 256²) + (b3 × 256¹) + b4 = (c1 × 85⁴) + (c2 × 85³) + (c3 × 85²) + (c4 × 85¹) + c5
>
> In other words, 4 bytes of binary data shall be interpreted as a base-256 number and then shall be converted to a
> base-85 number. The five bytes of the base-85 number shall then be converted to ASCII characters by adding 33 (the
> ASCII code for the character ! ) to each. [...] As a special case, if all five bytes are 0, they shall be
> represented by the character with code 122 (z) instead of by five exclamation points ( ! ! ! ! ! ).
>
> If the length of the data to be encoded is not a multiple of 4 bytes, the last, partial group of 4 shall be used to
> produce a last, partial group of 5 output characters. Given n (1, 2, or 3) bytes of binary data, the encoder shall
> first append 4 - n zero bytes to make a complete group of 4. It shall encode this group in the usual way, but shall
> not apply the special z case. Finally, it shall write only the first n + 1 characters of the resulting group of 5.
> These characters shall be immediately followed by the ~> EOD marker.

## 7.4.5 RunLengthDecode Filter

> The encoded data shall be a sequence of runs, where each run shall consist of a length byte followed by 1 to 128
> bytes of data. If the length byte is in the range 0 to 127, the following length + 1 (1 to 128) bytes shall be
> copied literally during decompression. If length is in the range 129 to 255, the following single byte shall be
> copied 257 - length (2 to 128) times during decompression. A length value of 128 shall denote EOD.

## 9.6.6.1 General (Character Encoding)

> Except for Type 3 fonts, every font program shall have a built-in encoding. [...] Symbolic fonts contain other
> character sets, to which the encodings mentioned previously ordinarily do not apply. Such font programs have
> built-in encodings that are usually unique to each font. The standard 14 fonts include two symbolic fonts, Symbol
> and ZapfDingbats, whose encodings and character sets are documented in Annex D.
>
> [...] The value of the Encoding entry shall be either a named encoding (the name of one of the predefined encodings
> MacRomanEncoding, MacExpertEncoding, or WinAnsiEncoding) or an encoding dictionary.

## Table 114 (suite) – Entries in an encoding dictionary

> BaseEncoding name (Optional) The base encoding—that is, the encoding from which the Differences entry (if present)
> describes differences— shall be the name of one of the predefined encodings MacRomanEncoding, MacExpertEncoding, or
> WinAnsiEncoding (see Annex D).

## 9.6.6.2 Encodings for Type 1 Fonts

> A Type 1 font's built-in encoding shall be defined by an Encoding array that is part of the font program, not to be
> confused with the Encoding entry in the PDF font dictionary. [...] If an encoding maps to a character name that does
> not exist in the Type 1 font program, the .notdef glyph shall be substituted.

## 9.6.6.3 Encodings for Type 3 Fonts

> A Type 3 font's mapping from character codes to glyph names shall be entirely defined by its Encoding entry, which is
> required in this case.

## Table 117 (suite) – Entries in a CIDFont dictionary

> CIDToGIDMap stream or name (Optional; Type 2 CIDFonts only) A specification of the mapping from CIDs to glyph
> indices. If the value is a stream, the bytes in the stream shall contain the mapping from CIDs to glyph indices: the
> glyph index for a particular CID value c shall be a 2-byte value stored in bytes 2 × c and 2 × c + 1, where the first
> byte shall be the high-order byte. If the value of CIDToGIDMap is a name, it shall be Identity, indicating that the
> mapping between CIDs and glyph indices is the identity mapping. Default value: Identity.

## Table 127 – Additional entries in an embedded font stream dictionary

> Length1 integer (Required for Type 1 and TrueType fonts) The length in bytes of the clear-text portion of the Type 1
> font program, or the entire TrueType font program, after it has been decoded using the filters specified by the
> stream's Filter entry, if any.

## 9.10.2 (suite) Mapping Character Codes to Unicode Values

> • If the font is a composite font that uses one of the predefined CMaps listed in Table 118 (except Identity–H and
> Identity–V) or whose descendant CIDFont uses the Adobe-GB1, Adobe-CNS1, Adobe-Japan1, or Adobe-Korea1 character
> collection: [...] d) Obtain the CMap with the name constructed in step (c) (available from the ASN Web site; see
> the Bibliography). e) Map the CID obtained in step (a) according to the CMap obtained in step (d), producing a
> Unicode value.

## Annex D.2, Table D.1, notes

> 5. The hyphen character is also encoded as 255 in WinAnsiEncoding. The meaning of this duplicate code shall be
> "soft hyphen," but it shall be typographically the same as hyphen.
