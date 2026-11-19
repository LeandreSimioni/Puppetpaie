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

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `353b.txt` : `9738ee51eddc01a5cc343c9aae291f1288b2280115840918b25a848f8b9b0556`
- `353c.txt` : `53ae5c99aa175d94f5e2e05b9a925b897a8127ce3ec81aae4be6dd1b6943a8da`
- `354b.txt` : `f499486370e0e3b2ee56b3deb869c17fa6fe19e3dab3c3971179b38f136f8d51`
- `354c.txt` : `12de43e53595e4cd0f3fe725840f47acb3646703738145e3e9216c720b3b16d3`
- `354e.txt` : `4a57d0ddf29052a8bbd761ec4436670580abb43c4565fea94102a73a1d07af78`
- `354f.txt` : `a63dc3469a7187094b85e9b5dc14eec6c725c9f809c7b1f605321a70525e8704`
- `465b.txt` : `8bb787f4e7cfbd6b39e53d7e6a91ed4c3efe9b3c33d7f3747a3c8aafce2b898e`
- `637c.txt` : `d2bb5ab088cbb11c273fe2c43ecbccd434bd6c9793c6c9f01d079650aad37864`
