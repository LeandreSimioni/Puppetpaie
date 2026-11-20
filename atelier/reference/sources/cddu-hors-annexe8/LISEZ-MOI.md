# Le CDD d'usage d'un technicien que la liste de l'annexe 8 ne nomme pas (ticket #344, lot K2, 19/11/2026)

Pourquoi le moteur paie le chômage de ce contrat au règlement général, à l'Urssaf, et garde le reste du CDDU
(`droit::caisse_conges::cddu_hors_des_annexes`, l'étage `droit::bareme::bareme_chomage_hors_des_annexes`, la section
`chomage_hors_des_annexes` des barèmes 2025 et 2026).

## Les fichiers de ce dossier

- `legi-travail-D1242-1-L5424-20-L5427-1-20261119.md` : code du travail, D. 1242-1 (LEGIARTI000051768762),
  L. 5424-20 (LEGIARTI000037388345) et L. 5427-1 (LEGIARTI000048600544), versions en vigueur. Transcription littérale du
  champ BLOC_TEXTUEL, relevée le 19/11/2026 dans l'entrepôt local `../legi-entrepot/articles.jsonl.gz` (base LEGI de la
  DILA, open data `https://echanges.dila.gouv.fr/OPENDATA/LEGI/`).
- `kali-1285-politique-contractuelle-20080624-art3-3-annexe-c-20261119.md` : accord du 24 juin 2008 relatif à la
  politique contractuelle dans le spectacle vivant public et privé (IDCC 1285, KALITEXT000019747515, étendu), art. 3.3
  (KALIARTI000019747547) et annexe C (KALIARTI000019747569). Transcription littérale du champ `contenu`, relevée le
  19/11/2026 dans `../kali-entrepot/articles.jsonl.gz` (base KALI de la DILA).

## Ce qu'on en lit

- **Le CDD d'usage est permis** à ces fonctions : D. 1242-1, 6° (« Les spectacles, l'action culturelle… ») ; pour la
  1285, l'accord du 24/06/2008, art. 3.3.1 (« Les fonctions artistiques et techniques liées à la création, la
  production, l'exploitation et la diffusion d'un spectacle vivant sont des fonctions pour lesquelles le CDD dit d'usage
  est autorisé ») et son annexe C, qui nomme le poursuiteur, le prompteur, le plumassier, le menuisier de décors…, que la
  liste 6 de l'annexe 8 ne nomme pas (`logiciel/donnees/emplois-base.json`, `conventions_hors_liste`).
- **L'annexe 8 ne le range pas** : « Les bénéficiaires de la présente annexe sont les ouvriers et techniciens engagés
  dans le cadre d'un contrat à durée déterminée, pour assurer l'une des fonctions énumérées dans la liste figurant au
  titre XIV » (art. 1er, § 2, `../plafond-intermittents/unedic-annexe8-convention-20241115.txt`). Son chômage est celui
  du règlement général (`../chomage-permanents/unedic-reglement-general-convention-20241115.txt`) : art. 50-1, § 1er,
  4 % à la charge de l'employeur ; le § 2 (4,50 % pour un CDD d'usage de trois mois au plus) ne vise que les dockers
  occasionnels ; art. 50 : les contributions salariales ne sont que celles des annexes VIII, IX et X. La circulaire
  Unédic 2025-05, § 2.1.1 (`../plafond-intermittents/unedic-circulaire-2025-05-contributions-20250501.txt`), ne porte la
  majoration de 0,5 % que pour les dockers occasionnels et « des salariés intermittents du spectacle visés à l'article
  L. 5424-20 », ceux des annexes.
- **L'Urssaf le recouvre** : L. 5427-1, al. 3 (« Le recouvrement des contributions [...] est assuré [...] par les
  unions pour le recouvrement des cotisations de sécurité sociale et d'allocations familiales ») ; France Travail ne
  recouvre, pour le spectacle, que « lorsque l'activité exercée bénéficie de l'aménagement des conditions
  d'indemnisation mentionné à l'article L. 5424-20 » (e). Au nominatif : 040 et 048 à l'Urssaf ; au bordereau : CTP 772
  et 937 ; la réduction générale « étendue » (CTP 668, régularisation 669), comme pour un permanent.
- **Le reste du CDDU ne change pas** : la caisse des Congés Spectacles (D. 7121-40, D. 7121-41 : « le personnel
  artistique et technique », sans liste ; `../caisse-conges-affiliation/LISEZ-MOI.md`) ; la formation des intermittents
  à 2 % (L. 6331-55 : les intermittents des secteurs « pour lesquels il est d'usage constant de ne pas recourir au
  contrat à durée indéterminée ») et la majoration de 10 % de leur taxe d'apprentissage (CSS D. 243-0-2, 2°) ; la
  prévoyance de l'accord interbranche (`../accord-interbranche-prevoyance-intermittents/accord-interbranche-prevoyance-20061220.md`,
  art. 2.2 : la fonction « reprise, soit dans la liste des emplois d'une des conventions collectives du spectacle pour
  lesquels le recours au contrat à durée déterminée d'usage est autorisé, soit dans la liste des emplois des Annexes 8 et
  10 ») ; la retraite complémentaire des « personnels techniques intermittents du spectacle »
  (`../audiens-fp-taux-repartition-retraite-2026.txt`, tableau n° 1 : aucune liste).

## Où le moteur s'en sert

- `droit/src/caisse_conges.rs` : `hors_de_l_annexe8`, `cddu_hors_des_annexes`, `chomage_a_l_urssaf`,
  `personnel_du_spectacle` (la caisse ne suit pas la liste).
- `droit/src/bareme.rs` : `bareme_chomage_hors_des_annexes`, dans `bareme_applique` (le bulletin, la DSN, le bordereau
  lisent cette pile).
- `logiciel/donnees/baremes/2025.json` et `2026.json`, `chomage_hors_des_annexes` : chaque taux porte sa `source`, que le
  test des sources vérifie dans les textes de ce dossier et des dossiers cités.
- `declarations/src/dsn.rs` : `assiettes_ctp` et `assiette_declaree` (CTP 772, 937 et 668).
- `engagement/src/lib.rs` : l'avis de la saisie (« son chômage est celui du droit commun »).

## Ce qui n'est pas porté

- Un métier hors du catalogue (`emplois-base.json`) ne dit pas sur quelle liste il est : son CDDU reste payé aux annexes.
- L'attestation employeur : France Travail édite l'AER Cinéma Spectacle d'après le code emploi de la nomenclature du
  spectacle (S21.G00.40.005) ; aucun texte déposé ne dit ce qu'elle édite pour un CDDU hors des annexes.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `kali-1285-politique-contractuelle-20080624-art3-3-annexe-c-20261119.md` : `5b4c219c952654c909b50589b08b363f03ed5074d001654565d2393894eeb41a`
- `legi-travail-D1242-1-L5424-20-L5427-1-20261119.md` : `b28d3e89c4089a0925ff107c754c169ba00ae1068386481b1d3569818865abf7`
