# Service Public Entreprendre : la réduction générale dégressive unique (RGDU)

Relevé le 23/11/2026 dans le nuage, tel que servi : `https://entreprendre.service-public.fr/vosdroits/F24542` (adresse
canonique de la page : `https://entreprendre.service-public.gouv.fr/vosdroits/F24542`), « Réduction générale
dégressive unique (RGDU) de cotisations patronales », « Vérifié le 15 juin 2026 - Entreprendre Service Public /
Direction de l'information légale et administrative (Premier ministre) ». La page dit d'elle-même : « Cette page est en
cours de mise à jour. »

| fichier | ce que c'est |
|---|---|
| `service-public-rgdu-reduction-generale-20260615.html` | la page, telle que servie |
| `service-public-rgdu-reduction-generale-20260615.txt` | sa transcription (`pandoc -t plain --wrap=none`) |

Les pages que celle-ci cite et que le nuage n'atteint pas (connexion coupée, le 23/11/2026) : la page Urssaf
`https://www.urssaf.fr/accueil/employeur/beneficier-exonerations/reduction-generale-cotisation.html` et le BOSS,
`https://boss.gouv.fr/portail/accueil/allegements-et-exonerations/allegements-generaux.html`.

## Ce qu'on en lit (lignes de la transcription)

- Les taux réduits de 2025 n'existent plus en 2026 (l. 164-168) : « Les taux réduits des cotisations d'assurance
  maladie et d'allocations familiales, qui existaient avant la réforme des allègements généraux de cotisations
  patronales, sont supprimés. » Ils ne sont maintenus que pour les exonérations spécifiques dégressives (Lodeom, TO-DE,
  AADPA, ZRD, ZFRR), pas pour les artistes du spectacle.
- Le non-cumul (l. 428-436) : « Enfin elle n'est pas cumulable avec l'application de taux spécifiques, d'assiettes ou
  de montants forfaitaires de cotisations. » « Exemple : C'est le cas des taux spécifiques aux artistes du spectacle
  qui ne sont pas cumulables avec la réduction. » La page cite L. 241-13 du code de la sécurité sociale, dont le VI dit
  la même règle (« ne peut être cumulé [...] avec [...] l'application de taux spécifiques, d'assiettes ou de montants
  forfaitaires de cotisations », `../legi-entrepot/`, version en vigueur depuis le 01/01/2026).
- Ce qui se cumule (l. 418-426) : la déduction forfaitaire des heures supplémentaires, les « taux réduits de
  cotisations sociales appliqués aux journalistes professionnels, pigistes et assimilés », la déduction forfaitaire
  spécifique pour frais professionnels. Les artistes du spectacle n'y sont pas.

## Où le moteur s'en sert

Nulle part encore. Ce texte et L. 241-13, VI fondent ce que le barème 2026 fait déjà, sans source citée :
`logiciel/donnees/baremes/2026.json`, `par_statut.technicien.reduction_generale.statuts = ["technicien"]` (l'artiste
n'a pas la réduction). Le barème n'est pas changé : Léandre décide s'il cite ces deux textes.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `service-public-rgdu-reduction-generale-20260615.html` : `974bd41cb61947b1ac1d73198eddbc6f94cbe4c28de5d5ddf54c7cac9a3b817e`
- `service-public-rgdu-reduction-generale-20260615.txt` : `36117eaa53f3c9a0fa52402735486d4e49fb9a444ec2f6597c78f382cb708c82`
