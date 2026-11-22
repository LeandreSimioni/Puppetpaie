# IETF : les formats d'Internet que le logiciel lit

## RFC 5322 : le format des messages (en-tête `Date` et fuseau)

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc5322-internet-message-format-20081001.txt` | https://www.rfc-editor.org/rfc/rfc5322.txt, tel que servi (HTTP 200, `text/plain`, 122 322 octets) ; RFC 5322, « Internet Message Format », P. Resnick (éd.), Standards Track, octobre 2008 (obsolète la RFC 2822) | 13/10/2026 |

### Ce qu'on en lit

- **§ 3.3, « Date and Time Specification »** (lignes 828-849 du `.txt`) :
  - « The zone specifies the offset from Coordinated Universal Time (UTC […]) that the date and time-of-day represent. »
  - « The form "+0000" SHOULD be used to indicate a time zone at Universal Time. Though "-0000" also indicates
    Universal Time, it is used to indicate that the time was generated on a system that may be in a local time zone
    other than Universal Time and that the date-time contains no information about the local time zone. »
  - Donc **`-0000` est une heure UT** (comme `+0000`) ; ce qu'il ajoute à `+0000`, c'est que l'expéditeur *ne sait pas
    quel est son fuseau local*. Il ne dit jamais « heure locale du lecteur ».
- **§ 4.3, « Obsolete Date and Time »** (lignes 1838-1900) : `UT` et `GMT` valent `+0000` ; les zones américaines (`EST` =
  `-0500`…) ; « Z » équivaut à `+0000`, mais les zones militaires d'une lettre « SHOULD all be considered equivalent to
  "-0000" unless there is out-of-band information » ; et « Other multi-character (usually between 3 and 5) alphabetic
  time zones have been used in Internet messages. Any such time zone whose meaning is not known SHOULD be considered
  equivalent to "-0000" unless there is out-of-band information confirming their meaning. »
- **Annexe** (ligne 2879) : parmi les changements depuis la RFC 2822, « Specifically allow and give meaning to "-0000"
  time zone ».

### Où le moteur s'en sert

`atelier/sources/paie/src/courriel_lecture.rs` (`date_de`, `parsedate_tz`, `date_lue`) : la date d'un message de réponse
(fiche du salarié, signature), mise en heure de Paris pour le journal. Test : `tests/fichiers/courriel_lecture.rs`,
`les_dates_d_en_tete_en_heure_de_paris`.

### Le § 4.3, suivi depuis le lot U7 (13/10/2026)

- **Un nom de fuseau inconnu** (`CEST` seul, sans parenthèses) et **les zones militaires** d'une lettre : le § 4.3 (lignes
  1886-1900) dit de les tenir pour `-0000` (« SHOULD be considered equivalent to "-0000" unless there is out-of-band
  information confirming their meaning ») ; `-0000` est de l'UT (§ 3.3, plus haut). Le moteur le fait :
  `courriel_lecture::parsedate_tz`, un fuseau fait de lettres que sa table ne connaît pas vaut 0. Il n'a aucune
  information « hors bande » sur `CEST` : lui donner `+0200` serait une table inventée. Les zones américaines (`EST`…) et
  `UT`/`GMT` gardent leur décalage (§ 4.3, lignes 1838-1845 et 1867-1884) ; la table garde aussi `UTC`, `AST`, `ADT`, que le module
  `email` de Python lisait. Effet : un message daté `10:15:00 CEST` en octobre s'écrit 12:15 au journal, et non 10:15
  (l'oracle `courriel_lecture/cas.json` change de cette seule valeur, message 28).
- Tests : `tests/fichiers/courriel_lecture.rs`, `les_dates_d_en_tete_en_heure_de_paris` (`CEST`, `a`, `EST`).

### Ce que le moteur ne suit pas, faute de texte (inférence)

- **Un en-tête sans fuseau** (`Sat, 03 Oct 2026 10:15`) ou dont la seule indication est un commentaire (`(CEST)`, qui est du
  CFWS, pas une zone) : la RFC 5322 exige un fuseau (`date-time = [ day-of-week "," ] date time [CFWS]`, ligne 791 ;
  `zone = (FWS ( "+" / "-" ) 4DIGIT) / obs-zone`, ligne 818) ; ni le § 3.3 ni le § 4.3 ne disent comment lire une date qui
  n'en a pas. Le moteur la lit comme une heure de Paris, comme le faisait le module `email` de Python : un choix du moteur,
  pas un texte. (Le § 3.3 décrit `-0000` comme une date qui « contains no information about the local time zone » ; en
  faire la règle d'une date sans zone serait une analogie, que le moteur ne fait pas.)
- **Relu le 13/10/2026 (lot F1)** : la RFC ne tranche pas. La zone est obligatoire dans la syntaxe actuelle (`time =
  time-of-day zone`, ligne 808) comme dans l'ancienne (`obs-zone`, ligne 1838) : une date sans zone n'est conforme ni à
  l'une ni à l'autre. Ce qu'un lecteur fait d'une forme que le document ne décrit pas, le § 4 le laisse à
  l'implémentation (lignes 1711-1718) : « The fact that a particular form does not appear in any section of this
  document is not justification for computer programs to crash or for malformed data to be irretrievably lost by any
  implementation. It is up to the implementation to deal with messages robustly. » Lire l'heure comme celle de Paris
  reste donc une **inférence du moteur** (le lieu de la compagnie, et ce que faisait le module `email` de Python), nommée
  dans le commentaire de `courriel_lecture::date_de` et de `parsedate_tz` ; elle ne perd pas le message (la règle du
  § 4), et un message sans `Date` lisible prend l'heure de la relève.

## RFC 5321 : le transport du courriel (la casse d'une adresse, les codes de réponse)

`atelier/sources/envois/src/courriel.rs`, `envoi_refuse` : un 550, 551 ou 553 à l'envoi (§ 4.2.2-4.2.3, l. 2870-2930 :
« mailbox unavailable (e.g., mailbox not found, no access, or command rejected for policy reasons) », « User not local »,
« mailbox name not allowed (e.g., mailbox syntax incorrect) ») se dit d'après le § 4.3.2 (l. 3100-3115) : 551 ne
répond qu'à `RCPT`, c'est l'adresse du destinataire ; 553 répond à `MAIL` comme à `RCPT`, et 550 aussi à `DATA` : la
phrase dit les possibilités sans trancher ; un 4xx est passager.

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt` | https://www.rfc-editor.org/rfc/rfc5321.txt, tel que servi (HTTP 200, `text/plain;charset=utf-8`, 225 929 octets) ; RFC 5321, « Simple Mail Transfer Protocol », J. Klensin, Standards Track, octobre 2008 | 02/11/2026 |

### Ce qu'on en lit

- **§ 2.4, « General Syntax Principles and Transaction Model »** (lignes 867-875 du `.txt`) : « The local-part of a mailbox
  MUST BE treated as case sensitive. Therefore, SMTP implementations MUST take care to preserve the case of mailbox
  local-parts. In particular, for some hosts, the user "smith" is different from the user "Smith". […] Mailbox domains
  follow normal DNS rules and are hence not case sensitive. »
- Donc une adresse se garde **telle qu'on l'a reçue** : mettre en minuscules la partie avant l'arobase serait deviner.

### Où le moteur s'en sert

`atelier/sources/import/src/dsn_aem.rs` : le courriel du contact DSN (S20.G00.07.003) et celui d'un salarié
(S21.G00.30.018) se reprennent tels que la DSN de l'ancien logiciel les déclare, capitales comprises.

### L'envoi d'un courriel, écrit sur le flux TLS (`atelier/sources/envois/src/courriel.rs`, `Smtp`, `session_smtp`)

- **§ 4.2** (l. 2588-2598) : la forme d'une réponse, `Reply-code "-" texte` pour une ligne qui en annonce une autre,
  `Reply-code [SP texte]` pour la dernière ; le code seul est une réponse (l. 2570-2573) ; « An SMTP client MUST
  determine its actions only by the reply code, not by the text » (l. 2610-2611) ; un code inconnu se lit par son
  premier chiffre (l. 2623-2625), un premier chiffre hors de 2 à 5 est une faute (l. 2639-2640). **§ 4.2.1**
  (l. 2656-2690) : 2yz fait, 3yz attend la suite, 4yz passager, 5yz définitif (`Smtp::attendre`, `envoi_refuse`).
- **§ 3.1** et l'accueil `220` ; **§ 4.1.1.1** (l. 1785-1801, 1832-1872) : `EHLO`, un littéral d'adresse quand le client
  n'a pas de nom de domaine (§ 4.1.3, l. 2386-2392 : `EHLO [127.0.0.1]`), les extensions une par ligne, sans égard à
  la casse (`Extensions`).
- **§ 3.3** (l. 1017-1027) et **§ 4.1.2** (l. 2264, 2314) : `MAIL FROM:<…>`, `RCPT TO:<…>`, une adresse
  `Local-part "@" Domain` entre chevrons (`adresse_d_enveloppe`) ; **§ 4.1.1.4** (l. 2003-2006) : la fin des données,
  `<CRLF>.<CRLF>` ; **§ 4.5.2** (l. 3431-3433) : le point doublé en tête de ligne (`transparent`) ; **§ 4.1.1.10**
  (l. 2207-2218) : `QUIT`, qui ne défait pas une transaction finie ; **§ 4.5.3.1.5** (l. 3506-3507) : une ligne de
  réponse fait 512 octets au plus (la borne d'une réponse lue, `REPONSE_SMTP_MAXIMUM`).

## RFC 3207 : STARTTLS (le port 587)

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc3207-smtp-service-extension-for-secure-smtp-over-tls-20020201.txt` | https://www.rfc-editor.org/rfc/rfc3207.txt, tel que servi (HTTP 200, `text/plain;charset=utf-8`, 18 679 octets) ; RFC 3207, « SMTP Service Extension for Secure SMTP over Transport Layer Security », P. Hoffman, Standards Track, février 2002 | 22/11/2026 |

- **§ 4** (l. 96-110) : `STARTTLS`, sans paramètre ; `220 Ready to start TLS`. **§ 4.2** (l. 208-220) : « The client
  MUST discard any knowledge obtained from the server, such as the list of SMTP service extensions […]. The client
  SHOULD send an EHLO command as the first command after a successful TLS negotiation. »
- Où le moteur s'en sert : `envois/src/courriel.rs`, `session_smtp` (le port 587 commence en clair, passe en TLS, puis
  reprend à `EHLO`).

## RFC 4616 : le mécanisme SASL PLAIN

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc4616-plain-sasl-mechanism-20060801.txt` | https://www.rfc-editor.org/rfc/rfc4616.txt, tel que servi (HTTP 200, `text/plain;charset=utf-8`, 20 270 octets) ; RFC 4616, « The PLAIN Simple Authentication and Security Layer (SASL) Mechanism », K. Zeilenga (éd.), Standards Track, août 2006 | 22/11/2026 |

- **§ 2** (l. 122-126) : `message = [authzid] UTF8NUL authcid UTF8NUL passwd`.
- Où le moteur s'en sert : `envois/src/courriel.rs`, `Smtp::identifier` (`AUTH PLAIN`, avec la réponse initiale de la
  RFC 4954, § 4, l. 139-150, en base64, l. 192-194).

## Le mécanisme SASL LOGIN (brouillon de l'IETF)

| fichier | d'où | relevé |
|---|---|---|
| `ietf-draft-murchison-sasl-login-00-login-sasl-mechanism-20030828.txt` | https://www.ietf.org/archive/id/draft-murchison-sasl-login-00.txt, tel que servi (HTTP 200, `text/plain; charset=utf-8`, 11 939 octets) ; « The LOGIN SASL Mechanism », K. Murchison et M. Crispin, Internet-Draft (Informational), 28 août 2003 | 22/11/2026 |

- Un brouillon, jamais devenu RFC ; c'est le seul texte du mécanisme, que des serveurs n'offrent qu'à lui.
- **§ 2.1** : le client répond au premier défi par l'identifiant, au second par le mot de passe ; « The contents of both
  challenges SHOULD be ignored. »
- Où le moteur s'en sert : `envois/src/courriel.rs`, `Smtp::identifier`, quand le serveur n'offre pas PLAIN.

## RFC 6152 : 8BITMIME

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc6152-smtp-service-extension-for-8bit-mime-transport-20110301.txt` | https://www.rfc-editor.org/rfc/rfc6152.txt, tel que servi (HTTP 200, `text/plain;charset=utf-8`, 13 034 octets) ; RFC 6152, « SMTP Service Extension for 8-bit MIME Transport », J. Klensin, N. Freed, M. Rose, D. Crocker (éd.), Standards Track, mars 2011 | 22/11/2026 |

- **§ 2** et **§ 3** : le mot-clé `8BITMIME` à `EHLO`, et `BODY=8BITMIME` à `MAIL` pour un message qui n'est pas en ASCII.
- Où le moteur s'en sert : `envois/src/courriel.rs`, `Smtp::transmettre`.

## RFC 6531 : SMTPUTF8

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc6531-smtp-extension-for-internationalized-email-20120201.txt` | https://www.rfc-editor.org/rfc/rfc6531.txt, tel que servi (HTTP 200, `text/plain;charset=utf-8`, 40 977 octets) ; RFC 6531, « SMTP Extension for Internationalized Email », J. Yao et W. Mao, Standards Track, février 2012 | 22/11/2026 |

- **§ 3.4** : « If the envelope or message being sent requires the capabilities of the SMTPUTF8 extension, the
  SMTPUTF8-aware SMTP client MUST supply the SMTPUTF8 parameter with the MAIL command. »
- Où le moteur s'en sert : `envois/src/courriel.rs`, `Smtp::transmettre` (une adresse qui n'est pas en ASCII).

## RFC 4954 : l'authentification SMTP (le mot de passe refusé)

| fichier | d'où | relevé |
|---|---|---|
| `ietf-rfc4954-smtp-service-extension-for-authentication-20070701.txt` | https://www.rfc-editor.org/rfc/rfc4954.txt, tel que servi (HTTP 200, 43 493 octets) ; RFC 4954, « SMTP Service Extension for Authentication », R. Siemborski et A. Melnikov, Standards Track, juillet 2007 | 22/11/2026 |

### Ce qu'on en lit

- **§ 4, « The AUTH Command »** (lignes 274-276 du `.txt`) : « If the server is unable to authenticate the client, it
  SHOULD reject the AUTH command with a 535 reply unless a more specific error code is appropriate. »
- **§ 6, « Status Codes »** (lignes 600-605) : « 535 5.7.8 Authentication credentials invalid. This response to the AUTH
  command indicates that the authentication failed due to invalid or insufficient authentication credentials. In this
  case, the client SHOULD ask the user to supply new credentials (such as by presenting a password dialog box). »

### Où le moteur s'en sert

`atelier/sources/envois/src/courriel.rs`, `envoi_refuse` : un 535 à l'envoi dit que le mot de passe de la boîte est
refusé, et renvoie la compagnie le poser de nouveau.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `ietf-draft-murchison-sasl-login-00-login-sasl-mechanism-20030828.txt` : `1d1748fcca3141cdf733bb87db47f036e289a6eed80e5502a538a5110c8fc39b`
- `ietf-rfc3207-smtp-service-extension-for-secure-smtp-over-tls-20020201.txt` : `b004a203791a1ca664d39a49d73369f88f5f3870486a6639e3f833df2118e4ac`
- `ietf-rfc4616-plain-sasl-mechanism-20060801.txt` : `33aa677ab7b8efe52e86147c63b38dd8dd5de96438c63e10ba8a04504bbdbca4`
- `ietf-rfc4954-smtp-service-extension-for-authentication-20070701.txt` : `6bf118f556749d08d1bfae92cefbdebf6084324ceba262a87af1efbe0e16d50a`
- `ietf-rfc5321-simple-mail-transfer-protocol-20081001.txt` : `7d560eddff1b213c17d4e1e5f95005728b2f05bc2b3947f0fcc91717c6707634`
- `ietf-rfc5322-internet-message-format-20081001.txt` : `cb459acca15a2aa895863eaeff6f1d5b9c4ca15578ae3e3706f757787a385969`
- `ietf-rfc6152-smtp-service-extension-for-8bit-mime-transport-20110301.txt` : `f3bb53c64929a09984fc2c34daaf3ea4d4e8e90e771decf3c2453ed6c9035ad7`
- `ietf-rfc6531-smtp-extension-for-internationalized-email-20120201.txt` : `4f48303f7b7659d0b3149730b51a03d59799a95b4d2ca7d913533f97b9db4b40`
