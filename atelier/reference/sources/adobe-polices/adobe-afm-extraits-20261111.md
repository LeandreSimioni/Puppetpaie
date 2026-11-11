# Adobe Font Metrics File Format Specification, version 4.1 (Adobe Technical Note #5004) : extraits

Transcription, mot pour mot, de https://adobe-type-tools.github.io/font-tech-notes/pdfs/5004.AFM_Spec.pdf (HTTP 200,
245 931 octets, sha256 `7adf2c10f4ee581e1d5ecf51284b17946c9430410e46336f9b57889a09d2f793` ; « Version 4.1, 7 October
1998 », © Adobe Systems Incorporated), relevée le 11/11/2026 ; la mise en page est aplatie. L'original (« No part of
this publication [...] may be reproduced ») est dans `reference-privee/sources/adobe-polices/` du privé.

## 7 Global Metrics Keywords (Ascender, Descender)

> Ascender number
>
> (Optional.) For roman font programs: usually the y-value of the top of the lowercase d. If this font program
> contains no lowercase d, this keyword might be missing or number might be 0.
>
> Descender number
>
> (Optional.) For roman font programs: typically the y-value of the bottom of the lowercase p. If this font program
> contains no lowercase p, this keyword might be missing or number might be 0.

## 8 Individual Character Metrics

> Each character's metrics are represented as a list of keys and values separated by semicolons, contained on one
> line. The characters are sorted by numerically ascending character code. Characters not encoded follow the encoded
> characters and are identified by character codes of –1. Byte codes for which no character is defined are not
> listed.
>
> Example: A character metric data line might look like this:
>
> C 102 ; WX 333 ; N f ; B 20 0 383 682 ; L i fi ; L l fl ;
>
> This section appears in the AFM file only. It is optional.
>
> StartCharMetrics integer
> EndCharMetrics
>
> (Required if individual character metrics are present.) These keywords delimit the character metrics section of the
> file. The integer value indicates how many entries to expect.
>
> C integer
>
> Decimal value of default character code (−1 if not encoded).
>
> [...]
>
> WX number
>
> W0X number
>
> (Optional.) Character width in x for writing direction 0. y is 0.
>
> [...]
>
> N name
>
> (Optional.) PostScript language character name.
>
> B llx lly urx ury
>
> (Optional.) Character bounding box where llx, lly, urx, and ury are all numbers. [...]
