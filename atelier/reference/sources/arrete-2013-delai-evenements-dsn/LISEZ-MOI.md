# Arrêté du 2 juillet 2013 fixant le délai de transmission des événements dans le cadre de la DSN — le relevé LEGI

Les `articles-AAAAMMJJ.md`, `releve.json` et `xml/` de ce dossier sont **écrits par `paie releve-codes`** (`moissonner`, puis
`rapport`) depuis la base LEGI de la DILA (open data, `echanges.dila.gouv.fr/OPENDATA/LEGI/`) ; ils ne se retouchent pas à la
main. Le texte est suivi sous la clé `evenements-dsn-2013` de `atelier/sources/paie/src/releve_codes/textes.rs`
(JORFTEXT000027683422, rangé sous son JORFTEXT comme l'arrêté de 1975). Ajouté et relevé le 15/10/2026 : l'outil a
complété l'entrepôt pour ce seul texte (archive globale du 13/07/2025 puis les 461 livraisons jusqu'au 15/10/2026).

**Trouvé comment.** R. 133-14, II, du code de la sécurité sociale (`../code-de-la-securite-sociale/xml/R133-14.xml`) renvoie le
délai à « un arrêté conjoint du ministre chargé de la sécurité sociale et du ministre chargé de l'emploi, qui ne peut excéder
cinq jours ouvrés » sans le nommer ; une recherche sur Légifrance (référence JORFTEXT000027683422, « Arrêté du 2 juillet 2013
fixant le délai de transmission des événements dans le cadre de la déclaration sociale nominative ») l'a nommé, et le relevé
LEGI l'a confirmé (`xml/1.xml`).

## Ce qu'on en lit

- **Article 1er** (`LEGIARTI000027684779`, en vigueur depuis le 11/07/2013, `xml/1.xml`) : « Le délai prévu par le II de
  l'article R. 133-14 du code de la sécurité sociale est fixé à **cinq jours ouvrés**. »
- Ce délai vaut, par R. 133-14, II, pour la **fin du contrat de travail** et, **quand l'employeur n'est pas subrogé**, pour le
  **début et la fin de l'arrêt de travail** ; les autres événements partent avec la DSN mensuelle du mois.
- « Jours ouvrés » n'y est pas défini : le moteur prend du lundi au vendredi, hors fêtes légales chômées par la compagnie, le
  jour de l'événement exclu. **C'est une inférence** (`logiciel/donnees/obligations-base.json`, `jours_ouvres`).

## Où le moteur s'en sert

`atelier/sources/paie/src/obligations.rs` (`de_l_arret`, `de_la_fin`, `Calendrier::jours_ouvres_apres`) et
`logiciel/donnees/obligations-base.json` (`signalement_arret`, `signalement_reprise`, `signalement_fin_contrat`) : la date
butoir des signalements DSN d'arrêt, de reprise et de fin de contrat, rendue par les routes `arrets` et `fins-de-contrat` et par
le rappel de l'accueil (`routes_ouverture.rs`, `rappels_fin_de_contrat`).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `articles-20261015.md` : `bf8d5c323f4eb0861550b9bc69f37a157b258bae2c38866c03f522d9005412ff`
- `articles-20261103.md` : `2cbf3ac99b66e3b72ae80a6f55a09754885a2c34a1b9fd69a650b8a9ff046191`
- `articles-20261110.md` : `6a43bc7c094b0fe22ac9cf58839b44943cd3db0f07824d0f8081a8960e12c2c6`
- `releve.json` : `4720b05f2acd4ebf45e5f4be92af004ddb4d81a44c3bf61e0dcc988e2a6bcb8d`
- `xml/1.xml` : `741d6c1f800a7c5d539e22d40cb54782b1b38998ae447299fa5ffbed788370bc`
