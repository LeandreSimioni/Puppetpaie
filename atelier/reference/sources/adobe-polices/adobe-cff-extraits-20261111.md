# The Compact Font Format Specification : extraits

Transcription (extraits, mot pour mot, mise en page aplatie) de https://adobe-type-tools.github.io/font-tech-notes/pdfs/5176.CFF.pdf
(Adobe Technical Note #5176, « The Compact Font Format Specification », version 1.0, 4 Dec 03), sections 2 à 13. Les
tables des annexes A, B et C sont dans `adobe-cff-tables-20261111.txt`.

2 Data Layout
 Conceptually the binary data is organized as a number of
 separate data structures. The overall layout within the binary
 data is shown Table 1. Since some of these data structures are
 reached via offsets the ordering could be changed, although the
 first five occupy fixed locations.
 7
 Table 1 CFF Data Layout
 Entry Comments
 Header –
 Name INDEX –
 Top DICT INDEX –
 String INDEX –
 Global Subr INDEX –
 Encodings –
 Charsets –
 FDSelect CIDFonts only
 CharStrings INDEX per-font
 Font DICT INDEX per-font, CIDFonts only
 Private DICT per-font
 Local Subr INDEX per-font or per-Private DICT for CIDFonts
 Copyright and –
 Trademark Notices
 Appendix D shows an annotated example of a CFF font.
 3 Data Types
 This section describes data representation and types used by the
 format.
 All multi-byte numeric data and offset fields are stored in big-
 endian byte order (high byte low offset) and do not honor any
 alignment restrictions. This leads to a format that is free from
 padding bytes.
 Data objects are often specified by byte offsets that are relative
 to some reference point within the CFF data. These offsets are 1
 to 4 bytes in length. This document uses the convention of
 enclosing the reference point in parentheses and uses a
 reference point of (0) to indicate an offset relative to the start
 of the CFF data and (self) to indicate an offset relative to the
 data structure containing the offset.
 The CFF format data types are shown in Table 2.
 Table 2 CFF Data Types
 Name Range Description
 Card8 0 – 255 1-byte unsigned number
 Card16 0 – 65535 2-byte unsigned number
 Offset varies 1, 2, 3, or 4 byte offset (specified by
 OffSize field)
 OffSize 1– 4 1-byte unsigned number specifies the
 size of an Offset field or fields
 SID 0 – 64999 2-byte string identifier
 This document describes data structures by listing field types,
 names, and descriptions. Data structures may be given a type
 name and subsequently described. Arrays of objects are
 indicated by the usual square bracket convention enclosing the
 array length.
 The majority of CFF data is contained by either of two data
 structures called DICT and INDEX which are described in
 subsequent sections.
4 DICT Data
 Font dictionary data comprising key-value pairs is represented
 in a compact tokenized format that is similar to that used to
 represent Type 1 charstrings. Dictionary keys are encoded as 1-
 or 2-byte operators and dictionary values are encoded as
 variable-size numeric operands that represent either integer or
 real values. An operator is preceded by the operand(s) that
 specify its value. A DICT is simply a sequence of
 operand(s)/operator bytes concatenated together.
 A number of integer operand types of varying sizes are defined
 and are encoded as shown in Table 3 (first byte of operand is b0,
 4 DICT Data 9
 second is b1, and so on).
 Table 3 Operand Encoding
 Size b0 range Value range Value calculation
 1 32 – 246 –107 – +107 b0–139
 2 247 – 250 +108 – +1131 (b0–247)*256+b1+108
 2 251 – 254 –1131 – –108 –(b0–251)*256–b1–108
 3 28 –32768 – +32767 b1<<8|b2
 5 29 –(2^31) – +(2^31–1) b1<<24|b2<<16|b3<<8|b4
 Note 1 The 1- , 2-, and 3- byte integer formats are identical to those used by
 Type 2 charstrings.
 Examples of the integer formats are shown in Table 4.
 Table 4 Integer Format Examples
 Value Encoding
 0 8b
 100 ef
 –100 27
 1000 fa 7c
 –1000 fe 7c
 10000 1c 27 10
 –10000 1c d8 f0
 100000 1d 00 01 86 a0
 –100000 1d ff fe 79 60
 A real number operand is provided in addition to integer
 operands. This operand begins with a byte value of 30 followed
 by a variable-length sequence of bytes. Each byte is composed
 of two 4-bit nibbles as defined in Table 5. The first nibble of a
 pair is stored in the most significant 4 bits of a byte and the
 second nibble of a pair is stored in the least significant 4 bits of
a byte.
Table 5 Nibble Definitions
 Nibble Represents
 0–9 0–9
 a . (decimal point)
 b E
 c E–
 d <reserved>
 e – (minus)
 f end of number
A real number is terminated by one (or two) 0xf nibbles so that
it is always padded to a full byte. Thus, the value –2.25 is
encoded by the byte sequence (1e e2 a2 5f) and the value
0.140541E–3 by the sequence (1e 0a 14 05 41 c3 ff).
Operators and operands may be distinguished by inspection of
their first byte: 0–21 specify operators and 28, 29, 30, and
32–254 specify operands (numbers). Byte values 22–27, 31, and
255 are reserved. An operator may be preceded by up to a
maximum of 48 operands.
An operator may have one or more operands of the types
shown in Table 6.
Table 6 Operand Types
 Type Description
 number Integer or real number
 boolean Integer type with the values 0 (false) and 1 (true)
 SID String id (see section 10)
 array One or more numbers
 delta A number or a delta-encoded array of numbers
 (see below)
The length of array or delta types is determined by counting the
operands preceding the operator. The second and subsequent
numbers in a delta are encoded as the difference between
 4 DICT Data 11
 successive values. For example, an array a0, a1, ..., an would be
 encoded as: a0 (a1–a0) (a2–a1) ..., (an–a(n–1)).
 Two-byte operators have an initial escape byte of 12.
 Further compaction of dictionary data is achieved by
 establishing default values for various DICT keys. For those keys
 that have a default value the absence of the corresponding
 operator in a DICT implies a key should take its default value.
 (A list of DICT operators for the Top and Private DICTs may be
 found in sections 9 and 15, respectively.)
 5 INDEX Data
 An INDEX is an array of variable-sized objects. It comprises a
 header, an offset array, and object data. The offset array
 specifies offsets within the object data. An object is retrieved by
 indexing the offset array and fetching the object at the
 specified offset. The object’s length can be determined by
 subtracting its offset from the next offset in the offset array. An
 additional offset is added at the end of the offset array so the
 length of the last object may be determined. The INDEX format
 is shown in Table 7.
 Table 7 INDEX Format
 Type Name Description
 Card16 count Number of objects stored in INDEX
 OffSize offSize Offset array element size
 Offset offset Offset array (from byte preceding
 [count+1] object data)
 Card8 data Object data
 [<varies>]
 Offsets in the offset array are relative to the byte that precedes
 the object data. Therefore the first element of the offset array
 is always 1. (This ensures that every object has a corresponding
 offset which is always nonzero and permits the efficient
 implementation of dynamic object loading.)
 An empty INDEX is represented by a count field with a 0 value
 and no additional fields. Thus, the total size of an empty INDEX
 is 2 bytes.
Note 2 An INDEX may be skipped by jumping to the offset specified by the last
 element of the offset array.
 6 Header
 The binary data begins with a header having the format shown
 in Table 8.
 Table 8 Header Format
 Type Name Description
 Card8 major Format major version (starting at 1)
 Card8 minor Format minor version (starting at 0)
 Card8 hdrSize Header size (bytes)
 OffSize offSize Absolute offset (0) size
 Implementations reading font set files must include code to
 check version numbers so that if and when the format and
 therefore the version number changes, older implementations
 will reject newer versions gracefully. If the major version
 number is understood by an implementation it can safely
 proceed with reading the font. The minor version number
 indicates extensions to the format that are undetectable by
 implementations that do not support them although they will
 be unable to take advantage of these extensions.
 The hdrSize field must be used when locating the Name INDEX.
 It is provided so that future versions of the format may
 introduce additional data between the offSize field and the
 Name INDEX in a manner that is compatible with older
 implementations.
 The offSize field specifies the size of all offsets (0) relative to the
 start of CFF data.
 7 Name INDEX
 This contains the PostScript language names (FontName or
 CIDFontName) of all the fonts in the FontSet stored in an INDEX
 structure. The font names are sorted, thereby permitting a
 binary search to be performed when locating a specific font
 within a FontSet. The sort order is based on character codes
 treated as 8-bit unsigned integers. A given font name precedes
 another font name having the first name as its prefix. There
 must be at least one entry in this INDEX, i.e. the FontSet must
 contain at least one font.
 For compatibility with client software, such as PostScript
 interpreters and Acrobat®, font names should be no longer
 than 127 characters and should not contain any of the following
 ASCII characters: [, ], (, ), {, }, <, >, /, %, null (NUL), space, tab,
 carriage return, line feed, form feed. It is recommended that
 font names be restricted to the printable ASCII subset, codes 33
 through 126. Adobe Type Manager® (ATM®) software imposes
 a further restriction on the font name length of 63 characters.
 Note 3 For compatibility with earlier PostScript interpreters, see Technical Note
 #5088, “Font Naming Issues.”
 A font may be deleted from a FontSet without removing its data
 by setting the first byte of its name in the Name INDEX to 0
 (NUL). This kind of deletion offers a simple way to handle font
 upgrades without rebuilding entire fontsets. Binary search
 software must detect deletions and restart the search at the
 previous or next name in the INDEX to ensure that all
 appropriate names are matched.
 8 Top DICT INDEX
 This contains the top-level DICTs of all the fonts in the FontSet
 stored in an INDEX structure. Objects contained within this
 INDEX correspond to those in the Name INDEX in both order
 and number. Each object is a DICT structure that corresponds to
 the top-level dictionary of a PostScript font.
 A font is identified by an entry in the Name INDEX and its data
 is accessed via the corresponding Top DICT.
 9 Top DICT Data
 The names of the Top DICT operators shown in Table 9 are,
 where possible, the same as the corresponding Type 1 dict key.
 Operators that have no corresponding Type 1 dict key are noted
 in the table below along with a default value, if any. (Several
operators have been derived from FontInfo dict keys but have
been grouped together with the Top DICT operators for
simplicity. The keys from the FontInfo dict are indicated in the
Default,notes column of Table 9.)
Table 9 Top DICT Operator Entries
 Name Value Operand(s) Default, notes
 version 0 SID –, FontInfo
 Notice 1 SID –, FontInfo
 Copyright 12 0 SID –, FontInfo
 FullName 2 SID –, FontInfo
 FamilyName 3 SID –, FontInfo
 Weight 4 SID –, FontInfo
 isFixedPitch 12 1 boolean 0 (false), FontInfo
 ItalicAngle 12 2 number 0, FontInfo
 UnderlinePosition 12 3 number –100, FontInfo
 UnderlineThickness 12 4 number 50, FontInfo
 PaintType 12 5 number 0
 CharstringType 12 6 number 2
 FontMatrix 12 7 array 0.001 0 0 0.001 0 0
 UniqueID 13 number –
 FontBBox 5 array 0000
 StrokeWidth 12 8 number 0
 XUID 14 array –
 charset 15 number 0, charset offset (0)
 Encoding 16 number 0, encoding offset (0)
 CharStrings 17 number –, CharStrings offset
 (0)
 Private 18 number –, Private DICT size
 number and offset (0)
 SyntheticBase 12 20 number –, synthetic base font
 index
 PostScript 12 21 SID –, embedded
 PostScript language
 code
 Table 9 Top DICT Operator Entries (continued)
 Name Value Operand(s) Default, notes
 BaseFontName 12 22 SID –, (added as needed
 by Adobe-based
 technology)
 BaseFontBlend 12 23 delta –, (added as needed
 by Adobe-based
 technology)
 The embedded PostScript operator provides an escape
 mechanism that may be used to address extensibility or
 compatibility issues in a printer font (see Appendix F).
 The separation of dictionary data into top-level and Private
 dictionaries reflects Type 1 usage where Top DICT data is parsed
 at findfont time and used to construct a valid Type 1 font
 dictionary. The Private operator value specifies a size and an
 offset that is followed at font rendering time in order to
 construct the data structures associated with Private DICT data.
 The operators BaseFontName and BaseFontBlend are added to
 multiple master font instances embedded in PDF documents.
 BaseFontName is the FontName of the original multiple master
 font and BaseFontBlend is the User Design Vector used to
 compute the instance.
 CIDFonts require the additional Top DICT operators shown in
 Table 10.
 Table 10 CIDFont Operator Extensions
 Name Value Operand(s) Default, notes
 ROS 12 30 SID SID –, Registry Ordering
 number Supplement
 CIDFontVersion 12 31 number 0
 CIDFontRevision 12 32 number 0
 CIDFontType 12 33 number 0
 CIDCount 12 34 number 8720
 UIDBase 12 35 number –
 FDArray 12 36 number –, Font DICT (FD)
 INDEX offset (0)
 Table 10 CIDFont Operator Extensions
 Name Value Operand(s) Default, notes
 FDSelect 12 37 number –, FDSelect offset (0)
 FontName 12 38 SID –, FD FontName
 The ROS operator combines the Registry, Ordering, and
 Supplement keys together. CIDFonts are described fully in
 section 18.
 The Top DICT begins with the SyntheticBase and ROS operators
 for synthetic and CIDFonts, respectively. Regular Type 1 fonts
 begin with some other operator. (This permits the
 determination of the kind of font without parsing the entire
 Top DICT.)
10 String INDEX
 All the strings, with the exception of the FontName and
 CIDFontName strings which appear in the Name INDEX, used by
 different fonts within the FontSet are collected together into an
 INDEX structure and are referenced by a 2-byte unsigned
 number called a string identifier or SID. Only unique strings are
 stored in the table thereby removing duplication across fonts.
 Further space saving is obtained by allocating commonly
 occurring strings to predefined SIDs. These strings, known as the
 standard strings, describe all the names used in the ISOAdobe
 and Expert character sets along with a few other strings
 common to Type 1 fonts. A complete list of standard strings is
 given in Appendix A.
 The client program will contain an array of standard strings with
 nStdStrings elements. Thus, the standard strings take SIDs in the
 range 0 to (nStdStrings –1). The first string in the String INDEX
 corresponds to the SID whose value is equal to nStdStrings, the
 first non-standard string, and so on. When the client needs to
 determine the string that corresponds to a particular SID it
 performs the following: test if SID is in standard range then
 fetch from internal table, otherwise, fetch string from the String
 INDEX using a value of (SID – nStdStrings) as the index.
 An SID is defined as a 2-byte unsigned number but only takes
 values in the range 0–64999, inclusive. SID values 65000 and
 above are available for implementation use.
 A FontSet with zero non-standard strings is represented by an
 empty INDEX.
 11 Glyph Organization
 The glyphs within a font constitute a charset and are accessed
 via an encoding. An encoding is an array of codes associated
 with some or all glyphs in a font and a charset is an array of
 “names” for all glyphs in the font. (In CFF these names are
 actually SIDs or CIDs, which must be unique.)
 In order to understand how charsets, encodings, and glyphs are
 related in CFF it is useful to think of them as 3 “parallel” arrays
 that are indexed in unison. Thus, it is possible to name and
 encode the glyph at the given glyph index (GID) by using the
 GID to index the charset and encoding arrays, respectively. By
 definition the first glyph (GID 0) is “.notdef” and must be
 present in all fonts. Since this is always the case, it is not
 necessary to represent either the encoding (unencoded) or
 name (.notdef) for GID 0. Consequently, taking advantage of
 this optimization, the encoding and charset arrays always begin
 with GID 1.
 12 Encodings
 Encoding data is located via the offset operand to the Encoding
 operator in the Top DICT. Only one Encoding operator can be

 Encoding data is located via the offset operand to the Encoding
 operator in the Top DICT. Only one Encoding operator can be
 specified per font except for CIDFonts which specify no
 encoding. A glyph’s encoding is specified by a 1-byte code that
 permits values in the range 0 – 255.
 Each encoding is described by a format-type identifier byte
 followed by format-specific data. Two formats are currently
 defined as specified in Tables 11 and 12.
 Table 11 Format 0
 Type Name Description
 Card8 format =0
 Card8 nCodes Number of encoded glyphs
Table 11 Format 0
 Type Name Description
 Card8 code Code array
 [nCodes]
Each element of the code array represents the encoding for the
corresponding glyph. This format should be used when the
codes are in a fairly random order.
Table 12 Format 1
 Type Name Description
 Card8 format =1
 Card8 nRanges Number of code ranges
 struct Range1 Range1 array (see Table 13)
 [nRanges]
The format of a Range1 is described in Table 13.
Table 13 Range1 Format (Encoding)
 Type Name Description
 Card8 first First code in range
 Card8 nLeft Codes left in range (excluding first)
Each Range1 describes a group of sequential codes. For
example, the codes 51 52 53 54 55 could be represented by the
Range1: 51 4, and a perfectly ordered encoding of 256 codes can
be described with the Range1: 0 255.
This format is particularly suited to encodings that are well
ordered.
A few fonts have multiply-encoded glyphs which are not
supported directly by any of the above formats. This situation is
indicated by setting the high-order bit in the format byte and
supplementing the encoding, regardless of format type, as
 shown in Table 14.
 Table 14 Supplemental Encoding Data
 Type Name Description
 Card8 nSups Number of supplementary mappings
 struct Supplement Supplementary encoding array (see
 [nSups] Table 15 below)
 The format of a Supplement is specified in Table 15.
 Table 15 Supplement Format
 Type Name Description
 Card8 code Encoding
 SID glyph Name
 Each Supplement describes a single code-to-glyph mapping
 which provides another encoding for a glyph that has already
 been mentioned in the main encoding table.
 Sorting glyphs by encoding and then SID for unencoded glyphs
 (remembering that .notdef must be first) typically yields very
 small font encodings. Still more optimization is possible by
 observing that many fonts adopt one of two common
 encodings. In these cases the operand to the Encoding operator
 in the Top DICT specifies a predefined encoding id, in place of
 an offset, as defined in Table 16.
 Table 16 Encoding ID
 Id Name
 0 Standard Encoding
 1 Expert Encoding
 If the font uses Standard Encoding the Encoding operator can
 be omitted from the Top DICT since its default value is 0. Details
 of predefined encodings can be found in Appendix B.
 It is not necessary for a font to contain all the glyphs specified
 by a predefined encoding in order to be able to use it. The only
 requirement is that every glyph in the font has an identical
 encoding to those in the predefined encoding (including
 unencoded glyphs).
 Two or more fonts may share the same encoding by setting the
 offset operand of the Encoding operator to the same value in
 each font.
 By definition, glyphs that are not specified by custom or
 predefined encodings are unencoded.
Note 4 Predefined encodings may be applied to a variety of fonts regardless of
 charset whereas custom encodings may only be applied to fonts with
 specific charsets. Consequently, predefined encodings are specified as
 code to SID mappings and custom encodings are specified as code to
 GID mappings.
 Charset data is located via the offset operand to the charset
 operator in the Top DICT. Each charset is described by a format-
 type identifier byte followed by format-specific data. Three
 formats are currently defined as shown in Tables 17, 18, and 20.
 Table 17 Format 0
 Type Name Description
 Card8 format =0
 SID glyph Glyph name array
 [nGlyphs–1]
 Each element of the glyph array represents the name of the
 corresponding glyph. This format should be used when the SIDs
 are in a fairly random order. The number of glyphs (nGlyphs) is
 the value of the count field in the CharStrings INDEX. (There is
 one less element in the glyph name array than nGlyphs because
 the .notdef glyph name is omitted.)
 Table 18 Format 1
 Type Name Description
 Card8 format =1
 struct Range1 Range1 array (see Table 19)
 [<varies>]
 The Range1 format is shown in Table 19.
 Table 19 Range1 Format (Charset)
 Type Name Description
 SID first First glyph in range
 Card8 nLeft Glyphs left in range (excluding first)
 Each Range1 describes a group of sequential SIDs. The number
 of ranges is not explicitly specified in the font. Instead, software
 utilizing this data simply processes ranges until all glyphs in the
 font are covered. This format is particularly suited to charsets
 that are well ordered.
 Table 20 Format 2
 Type Name Description
 Card8 format =2
 struct Range2 Range2 array (see Table 21)
 [<varies>]
 The Range2 format is shown in Table 21.
 Table 21 Range2 Format
 Type Name Description
 SID first First glyph in range
 Card16 nLeft Glyphs left in range (excluding first)
 Format 2 differs from format 1 only in the size of the nLeft field
 in each range. This format is most suitable for fonts with a large
 well-ordered charset — for example, for Asian CIDFonts.
 Careful attention to the allocation order of SIDs typically yields
 very small font charsets. Still more optimization is possible by
 observing that many fonts adopt one of 3 common charsets. In
 these cases the operand to the charset operator in the Top DICT
 specifies a predefined charset id, in place of an offset, as shown
 in Table 22.
 Table 22 Charset ID
 Id Name
 0 ISOAdobe
 1 Expert
 2 ExpertSubset
 If the font has an ISOAdobe charset, the charset operator can be
 omitted from the Top DICT since its default value is 0. Details of
 predefined charsets can be found in Appendix C. A font may use
 a predefined charset if it exactly matches in the first nGlyphs.
 CID fonts must not use predefined charsets.
 Two or more fonts may share the same charset by setting the
 offset operand of the charset operator to the same value in
 each font.
14 CharStrings INDEX
 This contains the charstrings of all the glyphs in a font stored in
 an INDEX structure. Charstring objects contained within this
 INDEX are accessed by GID. The first charstring (GID 0) must be
 the .notdef glyph. The number of glyphs available in a font may
 be determined from the count field in the INDEX.
 The format of the charstring data, and therefore the method of
 interpretation, is specified by the CharstringType operator in
 the Top DICT. The CharstringType operator has a default value
