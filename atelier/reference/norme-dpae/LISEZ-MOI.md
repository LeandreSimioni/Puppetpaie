# Norme DPAE — les textes de l'Urssaf qui font foi pour la déclaration préalable à l'embauche

Documents publics de l'Urssaf et de net-entreprises, déposés tels que servis (entrés dans le dépôt le 15/10/2026). Les
URL de téléchargement n'ont pas été notées au dépôt : chacun se retrouve sous son titre sur net-entreprises.fr (fiches
5491 à 5493) et urssaf.fr. L'empreinte SHA-256 de chaque fichier (début) permet de voir qu'une copie n'a pas bougé.

| Fichier | Contenu | SHA-256 (début) |
|---|---|---|
| `v5.3_FormatDPAE_EMAIL.pdf` | Le « Format 120 » v5.3 : l'enregistrement de 120 zones d'une DPAE, positions, longueurs, valeurs. | `e7999761c05cc60f` |
| `v5.3_FormatDPAE_EMAIL.txt` | Sa transcription en texte, une cellule par ligne : c'est elle que le code cite, par ses lignes. | `43faaccb646142bd` |
| `5491-API-DPAE-Fonctionnement.pdf` | Guide de l'API DPAE, principes de fonctionnement (codes retour, dont 99). | `41ba99cb60fa59c7` |
| `5492-DPAE-Guide-ImplementationAPI-Oct2021.pdf` | Guide d'implémentation de l'API (octobre 2021). | `61f89c908c4b3124` |
| `API-DPAE-Guide-Implementation-NAT5492.pdf` | Le même guide, version servie sous le n° 5492. | `c2867ccb8495a2bc` |
| `5493-API-DPAE-FAQ.pdf` | Foire aux questions de l'API (retour 41). | `ae28b7b1b50cad42` |
| `Protocole d'échanges DPAE V4.pdf` | Protocole d'échanges, version 4. | `2bc889d1bbaeae21` |

## Où le moteur s'en sert

- `atelier/sources/declarations/src/dpae.rs` : l'enregistrement de la DPAE, zone par zone, selon le Format 120
  (`v5.3_FormatDPAE_EMAIL.txt`). La zone N_JJ_ESSAI (positions 885-887, l. 474-478 : « Durée de la période d'essai (en
  jours) ») porte, pour un CDDU, la période d'essai que sa carte stipule (code du travail L1242-10,
  `../sources/code-du-travail/LISEZ-MOI.md`, « La période d'essai ») ; `000` sans essai ; trois blancs pour un CDI ou un
  CDD, dont le logiciel ne connaît pas l'essai (code retour 99, `5491-API-DPAE-Fonctionnement.pdf`, p. 8-9).
- `atelier/sources/declarations/src/urssaf.rs` : les codes des organismes (annexe II.1 du Format 120).
- `atelier/sources/droit/src/fiche.rs` : la longueur et les caractères des zones que la fiche d'un salarié ou de
  l'entreprise remplit (raison sociale, adresse, lieu de naissance, téléphone, service de santé au travail…).
- `atelier/sources/droit/src/natures.rs` : la nature du contrat que la DPAE déclare.
- `atelier/sources/envois/src/retours.rs` et `releve_retours.rs` : les retours du canal DPAE (FAQ, guide
  d'implémentation).
