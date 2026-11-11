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
