# `insee-pcs-ese-2017-fiches/`

Les fiches de huit rubriques de la PCS-ESE 2017 de l'Insee, le texte de leur page
(`https://www.insee.fr/fr/metadonnees/pcsese2017/rubriqueRegroupee/<code>`, publiée le 19/12/2025), déposées le
15/10/2026 : 353b, 353c, 354b, 354c, 354e, 354f, 465b, 637c. La table entière des codes, sans les définitions, est
`../dsn-nomenclatures/table-PCSESE-20260924.txt`.

## Où le moteur s'en sert

- `droit::fiche::CodePcsEse::statut_de_sa_fiche` : le statut (artiste ou technicien) que la fiche d'un code fonde, et
  lui seul. Artiste : 354b, 354c, 354e (« dans le cadre d'un spectacle vivant ou audiovisuel », l. 35) et 354f
  (« Artistes du cirque et des spectacles divers », l. 34). Technicien : 465b (« une activité d'ordre technique »,
  l. 35) et 637c (« il ne leur est pas reconnu une activité de création », l. 36). 353b et 353c ne fondent ni l'un ni
  l'autre : 353b range un « Producteur réalisateur » (l. 54), 353c le réalisateur et le régisseur général ensemble
  (l. 51-52). Un code sans fiche déposée ne fonde rien.
- **INFERENCE** : une fiche classe une profession ; elle ne dit pas le statut d'artiste du spectacle, ni celui de
  technicien. Ranger tout un code d'un côté se lit dans l'intitulé et la définition de la rubrique, que certains de ses
  métiers ne suivent pas : 354b compte un « Accordeur » (l. 54) ; 354f un « Mannequin », un « Présentateur », un
  « Animateur (radio, télévision » (l. 47, 56, 59) ; 465b des « tâches technico-administratives » (l. 35). Le statut
  ainsi posé se corrige sur la fiche du salarié.
- L'import le lit pour une fiche neuve dont la DSN ne nomme aucun métier (`routes::import`, `fiche_salarie`) : hors de
  ces six codes, le statut reste vide et le calcul le demande.
