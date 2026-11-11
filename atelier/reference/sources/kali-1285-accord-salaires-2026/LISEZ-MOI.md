# CCNEAC (IDCC 1285) : l'accord de salaires du 1er juillet 2026 et son extension

Relevé le 2026-11-02 dans les données ouvertes de la DILA, chaque XML tel que l'archive le livre (octet pour octet) :

| fichiers | archive (données ouvertes DILA) |
|---|---|
| `jorf-1285-arrete-extension-*-20261102.xml` : l'arrêté du 7 septembre 2026 (JORFTEXT000054860820, version) et ses articles 1 à 3 (JORFARTI000054860824, …826, …828) | https://echanges.dila.gouv.fr/OPENDATA/JORF/JORF_20260919-002738.tar.gz (`jorf/global/texte/version/…`, `jorf/global/article/…`) |
| `kali-1285-accord-salaires-2026-*-20261102.xml` : l'accord (KALITEXT000054981887, version et structure) et ses articles (préambule, 1er, 1.1, 1.1.1 à 1.1.4, 1.2, 2, 3, 4, 5) | https://echanges.dila.gouv.fr/OPENDATA/KALI/KALI_20261015-214025.tar.gz (`kali/global/texte/…`, `kali/global/article/KALI/ARTI/00/00/54/98/…`) |

Transcription : `kali-1285-accord-salaires-2026-transcription-20261102.md` (articles 2 à 5 de l'accord, les trois articles
de l'arrêté, et ce qu'on en lit). Les grilles des articles 1.x sont transcrites depuis le texte signé dans
`../ccneac-nao-2026-grilles-transcription-20260929.md`.

## Ce qu'on en lit

- L'accord est **étendu** : arrêté du 7 septembre 2026, NOR TRST2622487A, JORF n° 0219 du 19 septembre 2026 ; son
  article 1 rend obligatoires toutes ses stipulations pour tous les employeurs de la convention (les articles 1.1.1 à 1.1.3
  sous réserve du principe « à travail égal, salaire égal ») ; son article 2 fait prendre effet à l'extension **à la date
  de publication**, le 19/09/2026.
- L'accord lui-même s'applique au 01/08/2026 pour les minima et au **01/09/2026 pour les indemnités et primes** (art. 5) ;
  avant l'extension, il ne lie que les adhérents de ses six organisations d'employeurs signataires (C. trav. L2262-1,
  `../legi-entrepot/articles.jsonl.gz`, clé `["travail", "L2262-1"]`).
- L'indemnité de déplacement (art. 2) : à compter du 01/09/2026, **21,40 €** chaque repas principal, **76,60 €** chambre et
  petit déjeuner, 7,50 € le petit déjeuner seul (119,40 € au total) ; jusqu'au 31/08/2026, 20,70 €, 74,30 € et 7,30 €.

## Où le moteur s'en sert

- `logiciel/donnees/baremes/2026.json`, `par_convention."1285".bareme.tarifs_conventionnels` : `repas` et `nuitee`, listes
  datées (20,70 / 74,30 jusqu'au 18/09/2026 ; 21,40 / 76,60 depuis le 19/09/2026, date de l'extension) ; la note `_source`
  dit pourquoi le 19/09 et pas le 01/09. Lues par `engagement::tarif_conventionnel` (le taux d'une ligne « Repas » ou
  « Nuitée » saisie sans taux, `engagement::lignes_au_tarif`) et par `routes::mois::dire_les_tarifs` (la phrase
  qui dit d'où vient le montant).
- Les minima de l'article 1 (au 01/08/2026) : `logiciel/donnees/minima-conventionnels.json` n'en porte qu'une partie, sous
  `1285.non_etendu` (cachets dramatiques et répétitions, pour les adhérents d'une organisation signataire).
- `logiciel/donnees/sources.json`, source `kali-1285-accord-salaires-20260701`.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `jorf-1285-arrete-extension-JORFTEXT000054860820-20261102.xml` : `0d8bf14f32ecf9e31c86cbd2059ff16ff3d009518aa5fb30884b57326df3a330`
- `jorf-1285-arrete-extension-art1-JORFARTI000054860824-20261102.xml` : `23ad85864cf35e6c9cf985918f1ccba88a9dcc39c25759afa68b89533fa074fe`
- `jorf-1285-arrete-extension-art2-JORFARTI000054860826-20261102.xml` : `209bee3bd07f59448b0370f53c63324bd18234bbb433f4641499d0c4587a7580`
- `jorf-1285-arrete-extension-art3-JORFARTI000054860828-20261102.xml` : `bb65b8faef1fb90ef85bfecdf2b2c9f6a8e38307f1e78dd56e7178efc8d37c17`
- `kali-1285-accord-salaires-2026-art1-KALIARTI000054981894-20261102.xml` : `03c06ef387915e1a594bf3cd9b3d2d4123b82a4f6832d25f323076250342e476`
- `kali-1285-accord-salaires-2026-art1.1-KALIARTI000054981895-20261102.xml` : `55ae294ee8ad69b950a2b10122cc62f8ccaad633a48abd801b35ec6bb1a48ea9`
- `kali-1285-accord-salaires-2026-art1.1.1-KALIARTI000054981896-20261102.xml` : `1ba94242f0cbcb4a00480b7d48ae32b1be0e9f5d1066f90ab2693aa31c4c17b4`
- `kali-1285-accord-salaires-2026-art1.1.2-KALIARTI000054981897-20261102.xml` : `8446869a643747a348bdce0771ca7e3fc220d6aa427c6aa842343e5f0fbaa801`
- `kali-1285-accord-salaires-2026-art1.1.3-KALIARTI000054981898-20261102.xml` : `e60e4a48532e587cecc98a2f789408755151c1d8ee5172d1532ba31e8569f38b`
- `kali-1285-accord-salaires-2026-art1.1.4-KALIARTI000054981899-20261102.xml` : `adb5a42185b190825adeb98a1ad04f2d630f36f9dad574b6fb54fc823c7ca4aa`
- `kali-1285-accord-salaires-2026-art1.2-KALIARTI000054981900-20261102.xml` : `44b76af3d68c90c6b5da1b8240be86e2a1fd353cbdef588fe5279c91c9345b11`
- `kali-1285-accord-salaires-2026-art2-KALIARTI000054981901-20261102.xml` : `5d8f467e1ffc9ed30b3d2dcae3306bce4699a05e7bf1bfd4e7aa9e9af9b7f674`
- `kali-1285-accord-salaires-2026-art3-KALIARTI000054981902-20261102.xml` : `5c7af12b5b059093d55f34fdf73c2c2e76c688c531083d468dc802d6408b8ea4`
- `kali-1285-accord-salaires-2026-art4-KALIARTI000054981903-20261102.xml` : `3e17647679d2a9e835157b1017c507da001dbe5871908d772c1ee99ca858ef2b`
- `kali-1285-accord-salaires-2026-art5-KALIARTI000054981906-20261102.xml` : `d4545b937d83fc06ef5d29a168026742ea0ee43b992796da0ef1c174ba72693f`
- `kali-1285-accord-salaires-2026-preambule-KALIARTI000054981890-20261102.xml` : `58fb01e55442f348d5673cb396374da44be927bc92c71e8e1f5e0ff303759080`
- `kali-1285-accord-salaires-2026-texte-KALITEXT000054981887-struct-20261102.xml` : `9a34537765057cb1439fb42e6680fe1885f8c5d855f024f20c4456090c6026af`
- `kali-1285-accord-salaires-2026-texte-KALITEXT000054981887-version-20261102.xml` : `9b8c6e524076bbff6353cbcccb399db75421ae2568cc871cefae51282419cd87`
- `kali-1285-accord-salaires-2026-transcription-20261102.md` : `05b0c88b4490c2af8886330d8ece43631da27f917d1370f04f4c7f11cdb8c58c`
