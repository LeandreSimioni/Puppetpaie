# L'entrepôt KALI : le texte entier de nos conventions collectives

**Ce que ce dossier tient** : tous les articles des conventions collectives que cite `logiciel/donnees/sources.json`,
tels que la base KALI de la DILA les publie (open data, `https://echanges.dila.gouv.fr/OPENDATA/KALI/`) : le texte de
base, **toutes les annexes**, les avenants et les accords (salaires année par année, prévoyance, frais de santé,
formation…), **toutes leurs versions** (en vigueur, étendues ou non, abrogées), avec leurs dates. 7 118 articles, à jour
au 13/11/2026 (`etat.json`, `mis_a_jour_le`).

| IDCC | convention | articles |
|---|---|---|
| 1285 | entreprises artistiques et culturelles (1er janvier 1984) | 1 358 |
| 3090 | entreprises du secteur privé du spectacle vivant (3 février 2012) | 871 |
| 3097 | production cinématographique (19 janvier 2012) | 697 |
| 1790 | espaces de loisirs, d'attractions et culturels (5 janvier 1994) | 683 |
| 2121 | édition (14 janvier 2000) | 616 |
| 3252 | entreprises au service de la création et de l'événement (27 juin 2024) | 546 |
| 3241 | télédiffusion (2 juillet 2021) | 439 |
| 2642 | production audiovisuelle (13 décembre 2006) | 393 |
| 2770 | édition phonographique (30 juin 2008) | 390 |
| 1922 | radiodiffusion (11 avril 1996) | 360 |
| 1734 | artistes-interprètes engagés pour des émissions de télévision (30 décembre 1992) | 305 |
| 2412 | production de films d'animation (6 juillet 2004) | 239 |
| 2411 | chaînes thématiques (19 juin 2017) | 221 |

**Les fichiers** :
- `articles.jsonl.gz` : **un seul fichier compressé**, un article par ligne (JSON). `ls` et `grep` ne voient rien
  dedans : il se lit par les commandes ci-dessous, ou par `zcat`. Clés : `cle` (`[IDCC, identifiant KALIARTI]`), `num`
  (le numéro de l'article), `etat` (`VIGUEUR_ETEN`, `VIGUEUR_NON_ETEN`, `ABROGE`, `REMPLACE`…), `debut`, `fin`,
  `titre_texte` (le texte auquel l'article appartient : la convention, telle annexe, tel accord), `texte` (son
  identifiant KALITEXT), `chemin` (titres et sections au-dessus de l'article), `contenu` (le texte de l'article),
  `conteneur` (la convention, KALICONT).
- `etat.json` : les livraisons de la DILA déjà appliquées et les changements d'état qu'elles ont portés.

**Le lire** (depuis la racine du dépôt ; `<sources>` est `atelier/reference/sources`) :

```
atelier/sources/atelier/bin/linux/moteur-atelier releve-conventions article atelier/reference/sources 1285 X.3
atelier/sources/atelier/bin/linux/moteur-atelier releve-conventions chercher atelier/reference/sources 3090 cachet
zcat atelier/reference/sources/kali-entrepot/articles.jsonl.gz | grep '"cle": \["3090' | grep -o '"titre_texte": "[^"]*' | sort | uniq -c
```

La première rend les articles en vigueur d'un numéro, avec leur état, leurs dates et leur texte ; la deuxième, les
articles en vigueur qui portent un mot ; la troisième, les textes (annexes, accords, avenants) d'une convention et leur
nombre d'articles.

**Le tenir à jour** : `moteur-atelier releve-conventions mettre-a-jour atelier/reference/sources` (les seules
livraisons nouvelles ; `--reconstruire` relit la base entière, 180 Mo, pour une convention nouvelle). Le code :
`atelier/sources/atelier/src/releve/conventions.rs`.

**Où le moteur s'en sert** : nulle part encore directement. Une règle n'entre dans le code que par une transcription
citée (`kali-<IDCC>-…` dans le dossier au-dessus), faite d'après cet entrepôt. Les minima et clauses relevés ne couvrent
que la 1285 et la 3090 (ticket #437 du privé).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles.jsonl.gz` : `c3dab7425d9d050d5076ae4713fe2114ca1c0658d77f42d4f9350574e1bb88a6`
- `etat.json` : `5cd6f921db06eee028bc9ebcaa43da56c704034c5a6908717bfa37ebdb5360ab`
