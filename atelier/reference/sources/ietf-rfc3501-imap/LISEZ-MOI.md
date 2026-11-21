# La norme IMAP4rev1 (RFC 3501)

## Ce qui est déposé ici

- **`ietf-rfc3501-imap4rev1-20261121.txt`** (relevé le 21/11/2026) : le texte de la RFC 3501, « INTERNET MESSAGE ACCESS
  PROTOCOL - VERSION 4rev1 », tel que servi par l'éditeur des RFC.
  - D'où : `https://www.rfc-editor.org/rfc/rfc3501.txt`.

## Ce qu'on en lit

- **La déconnexion** (§ 6.1.3, « LOGOUT Command », l. 1407-1424) : « The server MUST send a BYE untagged response
  before the (tagged) OK response, and then close the network connection. » Le `* BYE` qui suit `LOGOUT` est donc la
  réponse attendue, pas un refus ; partout ailleurs, un `* BYE` dit que le serveur ferme la connexion.

## Où le moteur s'en sert

- `atelier/sources/envois/src/courriel.rs`, `Dialogue::deconnecter` : la boîte courriel d'une compagnie (relever les
  signatures, « Vérifier » dans Configuration › Mots de passe).
