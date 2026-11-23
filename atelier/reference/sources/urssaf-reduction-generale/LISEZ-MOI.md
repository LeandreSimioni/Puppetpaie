# Urssaf — « La réduction générale dégressive unique »

`urssaf-reduction-generale-degressive-unique-20261123.pdf` : la page urssaf.fr « La réduction générale dégressive
unique », « Mis à jour le 13 juillet 2026 », imprimée en PDF par Léandre le 23/11/2026 (urssaf.fr coupe les connexions
du conteneur). URL (celle que cite Service Public, `../service-public-reduction-generale/`) :
`https://www.urssaf.fr/accueil/employeur/beneficier-exonerations/reduction-generale-cotisation.html`. Transcription :
`pdftotext -layout`, `.txt` à côté.

## Ce qu'on en lit

- « Les règles de cumul » (l. 515-545). Cumulables : la déduction patronale des heures supplémentaires, l'aide à domicile
  sous conditions, la déduction forfaitaire spécifique, « les taux réduits de cotisations sociales appliqués aux
  journalistes professionnels, pigistes et assimilés, et les taux réduits de cotisations d'assurance vieillesse
  appliqués aux VRP à cartes multiples et aux membres des professions médicales ». Non cumulable : « avec l'application
  de taux spécifiques, d'assiettes ou de montants forfaitaires de cotisations (exemple : taux spécifiques aux artistes du
  spectacle) ».
- « Bon à savoir » (l. 540-545) : « Vous pouvez renoncer à une exonération de cotisations patronales et appliquer la
  réduction générale des cotisations. » La page range les taux spécifiques des artistes à part des exonérations (deux
  puces distinctes, l. 535 et 538) ; elle ne dit pas qu'on peut renoncer à ces taux.
- La DFS (l. 327-336) : deux calculs, la réduction avec DFS limitée à 130 % de celle sans DFS.
- Les intermittents du spectacle : CTP 671, régularisation 801 (l. 461).

## Où le moteur s'en sert

Nulle part encore. Avec L. 241-13, VI, Service Public et le BOSS (§ 220-280, `../boss/`), elle fonde ce que fait déjà le
barème 2026 (l'artiste n'a pas la réduction générale : `par_statut.technicien.reduction_generale.statuts`), non cité
dans le barème ; et le CTP 671 / 801 du technicien intermittent (`organismes-base.json`, `reduction_generale.dsn`).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `urssaf-reduction-generale-degressive-unique-20261123.pdf` : `ca5723e6381c51eb4fdcd19d55e8fb3af4b348ed5c4529d3cad790279462fd86`
- `urssaf-reduction-generale-degressive-unique-20261123.txt` : `3ac8ab2c51fe19db4d850f9a7b237284026a32ffef07e47ca671c239c57a61d3`
