# DGFiP : les comptes rendus métier (CRM) du prélèvement à la source

- `dgfip-comptes-rendus-metier-pas-20261111.pdf` : « Les Comptes rendus métier (CRM) pour le PAS », note de la DGFiP,
  relevée et fournie par Léandre le 11/11/2026 (le nuage ne l'atteint pas ; l'adresse d'origine n'est pas notée).

## Ce qu'on en lit, et où le moteur s'en sert

- « Lorsqu'aucun taux n'est transmis par la DGFiP dans le CRM nominatif, ou en cas de nouveau bénéficiaire pour le
  mois, le collecteur doit appliquer un taux issu de la grille taux par défaut. »
- Parmi les erreurs que la DGFiP relève dans l'application du taux : « Utilisation d'un taux par défaut alors même que le
  collecteur dispose d'un taux transmis par la DGFiP qui est encore valide ».
- Le moteur : un compte rendu sans taux transmis ne fait pas oublier un taux transmis encore valide ; la grille ne
  s'applique que s'il n'en reste aucun de valide (`droit::pas::en_vigueur`, ticket #429).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `dgfip-comptes-rendus-metier-pas-20261111.pdf` : `ce2b60a63ad889b3c92a8a6de6a14a6105e92ee48c33e0732505869d23d2bf9f`
