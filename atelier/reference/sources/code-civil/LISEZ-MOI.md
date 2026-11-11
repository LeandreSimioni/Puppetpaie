# Le code civil

Le code civil n'est pas parmi les codes que `moteur-atelier releve-codes` suit (`atelier/src/releve/codes/textes.rs`) : ses
articles se relèvent sur Légifrance, depuis le poste de Léandre (ticket #307 du privé).

| fichier | ce que c'est | relevé |
|---|---|---|
| `legifrance-code-civil-art1218-force-majeure-20261030.pdf` (et sa transcription `.md`) | article 1218, version en vigueur depuis le 01/10/2016 (ordonnance n° 2016-131), https://www.legifrance.gouv.fr/codes/article_lc/LEGIARTI000032041431 | 30/10/2026, par Léandre |

## Ce qu'on en lit

- **La force majeure a trois conditions, toutes à la fois** : un événement « échappant au contrôle du débiteur », qui « ne
  pouvait être raisonnablement prévu lors de la conclusion du contrat » et dont « les effets ne peuvent être évités par des
  mesures appropriées ». Empêchement temporaire : l'exécution est suspendue ; définitif : le contrat est résolu de plein
  droit.
- C'est la « force majeure » que nomment L. 1243-1 et L. 1243-4 du code du travail (`../code-du-travail/`) pour la rupture
  anticipée d'un CDD, et le motif 083 de S21.G00.62.002 (« Rupture pour force majeure ou fait du prince »). Une
  représentation que la compagnie annule n'en est pas une d'elle-même : c'est la compagnie, débitrice du contrat, qui
  décide, et le salaire se doit (décision de Léandre du 30/10/2026, que ce texte appuie : inférence, la jurisprudence n'est
  pas déposée).

## Où le moteur s'en sert

- `logiciel/donnees/contrats-base.json` : les motifs de rupture 036 (sa phrase « hors faute grave, force majeure ou
  inaptitude », L. 1243-4) et 083 ; la phrase « rupture » du contrat d'usage.
- `routes/src/ouverture.rs` (« LES DATES ET LA TRANSACTION SELON LE MOTIF ») : la date de notification que la norme
  exige pour le motif 083.

## `legifrance-code-civil-art1193-force-obligatoire-20261102.md` — article 1193

Version issue de l'ordonnance n° 2016-131 du 10/02/2016, relevée le 02/11/2026 par Léandre sur Légifrance et recopiée
(transcription, pas de PDF). « Les contrats ne peuvent être modifiés ou révoqués que du consentement mutuel des parties, ou
pour les causes que la loi autorise. »

**Où le moteur s'en sert** : avec L1221-1 du code du travail (le contrat de travail est soumis au droit commun,
`../code-du-travail/LISEZ-MOI.md`), il fonde l'avenant qu'un changement d'un terme du contrat (salaire, durée du travail,
emploi) fait signer au salarié : `engagement::signe_ailleurs`, `documents::avenant`, et les contrats faits dans le logiciel.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `legifrance-code-civil-art1193-force-obligatoire-20261102.md` : `1da82186a2c0ef2821731bd0ce83261eaa5e7df8d2642d8adaf7274ee3828d2e`
- `legifrance-code-civil-art1218-force-majeure-20261030.md` : `81ef25bd2a93fd7db2ce96b63ee69aaeecbd707b5a0b6815af06499fff93bae3`
- `legifrance-code-civil-art1218-force-majeure-20261030.pdf` : `e0581bd45f21ad7676071f4f532b3521e2d21f05baebf1e7e849882507d24a8f`
