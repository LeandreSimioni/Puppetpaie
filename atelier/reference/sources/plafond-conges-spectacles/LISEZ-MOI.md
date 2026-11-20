# Le plafond de la base Congés Spectacles (ticket #363, lot K2, 19/11/2026)

Ce qui fonde, et ce qui borne, la base de la cotisation à la caisse des Congés Spectacles plafonnée par la convention
(`droit::clauses::plafond_conges_spectacles`, la section `plafond_conges_spectacles` de la 1285 dans
`logiciel/donnees/clauses-conventionnelles-base.json`, la case `plafond_conges_spectacles` de la fiche Entreprise).

## Les fichiers de ce dossier

- `kali-1285-accord-plafonds-conges-spectacles-20170504-20261119.md` : IDCC 1285, accord du 4 mai 2017 relatif aux
  plafonds des congés spectacles (KALITEXT000035657631), son préambule et ses articles 1er à 3, **VIGUEUR_NON_ETEN**.
- `kali-3252-art5-et-3097-art16-art44-conges-spectacles-20261119.md` : IDCC 3252, art. 5 (VIGUEUR_ETEN) ; IDCC 3097,
  art. 16 (passage « Congés spectacles ») et art. 44 (VIGUEUR_ETEN).

Transcriptions littérales du champ `contenu`, relevées le 19/11/2026 dans `../kali-entrepot/articles.jsonl.gz` (base
KALI de la DILA, open data `https://echanges.dila.gouv.fr/OPENDATA/KALI/`).

## Les textes déposés ailleurs

- C. trav. **D. 7121-37** (`../conseils/legi-travail-D7121-28-a-41-et-L3141-3-a-5-20261021.md`) : l'indemnité journalière
  est « la rémunération journalière moyenne » ; elle « ne peut excéder le chiffre maximum fixé dans les conventions
  collectives » ; « En cas d'absence de convention collective », le triple du salaire minimum de la catégorie.
- Audiens, fiche pratique « Plafonds conventionnels » (juillet 2025, `../audiens-dsn/audiens-fiche-pratique-plafonds-conges-spectacles-202507.txt`) :
  « ce montant s'entend de la base de calcul de la cotisation » ; « L'application de tels plafonds par un employeur
  relève de sa responsabilité » ; le tableau dit un « plafond de cotisation applicable par jour » ; « À défaut, il
  convient d'appliquer la règle générale ».
- Audiens, « Congés Spectacles employeur — Mode d'emploi » (avril 2023, `../audiens-dsn/audiens-conges-spectacles-mode-emploi-employeur-20261030.txt`,
  l. 203-206) : « l'entreprise peut plafonner la base de calcul au triple du montant du salaire minimum de la catégorie
  professionnelle concernée » ; l. 415 : « 2,5 jours pour 24 jours de travail ou cachets » (le droit au congé, pas le
  plafond).

## Les trois questions, et ce que les textes répondent

1. **Un choix de l'employeur ?** Oui pour la 1285 : l'accord du 4 mai 2017 n'est pas étendu, il ne lie que les adhérents
   des organisations signataires, ce que seule la compagnie sait ; Audiens : « relève de sa responsabilité », « peut
   plafonner ». D'où la case de la fiche Entreprise, que seule la 1285 ouvre (`champs-base.json`, `si`). Non cochée, la
   base est le brut, la règle générale.
2. **Quel minimum ?** Celui de la grille qui s'applique à l'engagement (art. 2 de l'accord : « trois fois les minima des
   cachets existants »). Le moteur ne le sait pas toujours : le palier se compte sur le mois ou sur trois mois, la grille
   dépend de la jauge, et ses minima prennent exprès le plus bas (`minima-conventionnels.json`, `note`), ce qui
   baisserait à tort la base. **Non codé** : pour ces emplois, la base reste le brut, ce que l'aide de la case dit. Sont
   codés les seuls montants que l'accord écrit (art. 1er) : 375 € (metteur en scène MET040, chorégraphe CHO010, maître de
   ballet MAI010) et 860 € (chef d'orchestre CHE190 et CHE510, concertiste soliste MUS070 ; *inférence* pour CHE510 et
   MUS070, dite dans la donnée).
3. **Un cachet vaut-il un jour ?** Aucun texte déposé ne l'écrit pour le plafond. D. 7121-37 parle d'« indemnité
   journalière » et de « rémunération journalière moyenne », Audiens d'un plafond « par jour » : le moteur multiplie le
   plafond par les **jours travaillés** de la paie (ceux que la paie déclare, bloc 53 unité 12, `Engagement::jours`), et
   la base plafonnée est le brut des rubriques de l'assiette dans cette limite (`engagement::calcul`, `Assiette::BrutCs`).

## Où le moteur s'en sert

- `droit/src/clauses.rs` : `PlafondCongesSpectacles`, `PlafondDesEmplois`, `plafond_conges_spectacles`.
- `engagement/src/calcul.rs` : l'assiette `brut_cs` de chaque rubrique, ramenée au prorata dans la limite du plafond ;
  la ligne de la caisse, le 066 et la base 27 de la DSN, et le certificat (`documents/src/conges_spectacles.rs`,
  `MoisPaye::base_conge`) lisent cette base.

## Ce qui n'est pas porté

- **3252**, art. 5 (étendu) : « 2 fois le salaire journalier minimum conventionnel de la catégorie d'emploi [...] dans la
  limite d'un plafond journalier de 272 € » : le moteur n'a pas de minimum journalier de la 3252 par catégorie.
- **3097**, art. 16 et 44 (étendus ; cinéma) : le triple des salaires journaliers minimaux sur 39 heures (titre II) ; le
  titre III n'écrit pas de montant que l'entrepôt donne sous le mot « plafond ».
- **3090** : aucun accord de plafond en vigueur dans l'entrepôt ; D. 7121-37 ne donne le triple du minimum qu'« en cas
  d'absence de convention collective ». Le « triple du minimum » des « autres branches » n'est que d'Audiens : un seul
  émetteur, qui ne fonde rien seul.
- L'art. 2 de l'accord 1285 (trois fois le minimum du cachet) : voir la question 2.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `kali-1285-accord-plafonds-conges-spectacles-20170504-20261119.md` : `3bb2bfd00ec9853ddc184374783f12af75b8d26d1d39692c81a0610a96d5395f`
- `kali-3252-art5-et-3097-art16-art44-conges-spectacles-20261119.md` : `571ce672c51ef5d315df75b444b768e87581d8c1202bcac36158fe3320c7bda2`
