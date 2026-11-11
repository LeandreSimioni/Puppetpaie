# La convention collective d'un employeur : un champ d'application, pas une activité

Relevé le 2026-11-09 dans les entrepôts du dépôt, tenus depuis les données ouvertes de la DILA (KALI et LEGI ; Légifrance
refuse les robots, 403) :

| texte | où il est |
|---|---|
| CCN 3090, article 1er (KALIARTI000028157291, en vigueur étendu depuis le 01/07/2013) | `kali-3090-1285-champ-d-application-transcription-20261109.md` ; `../kali-entrepot/articles.jsonl.gz`, clé `["3090", "KALIARTI000028157291"]` |
| CCNEAC (1285), article I.1 (KALIARTI000037814017, en vigueur étendu depuis le 28/03/2018) | la même transcription ; `../kali-entrepot/articles.jsonl.gz`, clé `["1285", "KALIARTI000037814017"]` |
| Code du travail, L2261-15 (LEGIARTI000042654539, depuis le 01/06/2021) | `../legi-entrepot/articles.jsonl.gz`, clé `["travail", "L2261-15"]` |

**Pas déposés** : les arrêtés d'extension du 4 janvier 1994 (1285) et du 29 mai 2013 (3090). Ils sont au JORF, que ni
l'un ni l'autre entrepôt ne porte, et Légifrance refuse les robots. KALI dit seulement, dans le titre du texte de la 1285,
« Etendue par arrêté du 4 janvier 1994 JORF 26 janvier 1994 » ; pour la 3090, les articles relevés sont « VIGUEUR_ETEN »
(en vigueur, étendus), sans la date de l'arrêté.

## Ce qu'on en lit

- **Le champ de chaque convention se définit par des faits que les pièces de paie ne portent pas.** La 3090 vise « les
  personnes physiques et morales du secteur privé […] dont l'activité principale est le spectacle vivant », « indépendantes
  des pouvoirs publics » ; la 1285 vise « les entreprises du secteur public du spectacle vivant » : direction nommée par la
  puissance publique, un représentant de la puissance publique dans un organe de décision, un label de l'État (« compagnies
  dramatiques conventionnées […] »), ou des subventions dans le cadre de conventions pluriannuelles ou d'aides aux projets.
  Deux compagnies de même activité relèvent donc de deux conventions selon leur financement et leur gouvernance : **la
  convention ne se déduit pas de l'activité** (règle 1), elle se **lit** là où l'employeur l'a dite (la DSN, le bulletin,
  l'AEM) ou se demande.
- **L2261-15** : une convention étendue est « rendue obligatoire pour tous les salariés et employeurs compris dans [son]
  champ d'application ». Un salarié payé sous une autre convention que celle de l'employeur, ou un employeur qui change de
  convention d'une DSN à l'autre, est donc un écart à dire, pas à trancher en silence (inférence de lecture : le texte ne
  parle pas de déclarations).

## Où le moteur s'en sert

- `atelier/sources/routes/src/importation/bulletins.rs` (`trier_et_lire`, « LA CONVENTION COLLECTIVE DE L'IMPORT ») : la convention
  de l'import est celle de la fiche Entreprise, sinon celle que déclarent les DSN (S21.G00.11.022, S21.G00.40.017), sinon
  celle que les bulletins impriment, sinon celle des AEM ; aucune, la fiche Entreprise la demande (`idcc`). Les bulletins
  d'une autre convention que celle de l'import se disent en avertissement.
- `atelier/sources/import/src/dsn_aem.rs` (`LectureDsn::conventions_divergentes`) : des contrats d'une même DSN sous
  d'autres conventions que l'établissement, une convention qui change d'une DSN à l'autre, se disent.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `kali-3090-1285-champ-d-application-transcription-20261109.md` : `7c4b0ee3b97efa80ca83003c429ff937ee9863526ba863eabec9f0a55b8791f6`
