# Les indemnités journalières de maladie et de maternité des intermittents (professions à caractère saisonnier ou discontinu)

Déposé le 27/11/2026, apporté par Léandre, pour le ticket 728 du privé (estimer l'indemnité journalière de maternité
d'une intermittente, sur la page publique des droits, ticket 724) et le ticket 727 (la maternité dans le calcul des
507 heures). **Le moteur ne s'en sert pas encore.**

## Les fichiers

| Fichier | Ce que c'est | Provenance |
|---|---|---|
| `dss-circulaire-2017-126-prestations-saisonniers-discontinus-20170419` (.pdf, .txt) | Circulaire interministérielle n° DSS/2A/5B/2017/126 du 19 avril 2017 « précisant certaines règles applicables aux prestations en espèces servies au titre de la maladie et de la maternité aux personnes exerçant une profession à caractère saisonnier ou discontinu » (NOR AFSS1711508C, date d'application 1er juin 2017, publiée au BO). Transcription `pdftotext -layout`. | Le PDF apporté par Léandre s'appelle `cir_42175.pdf`, le nom des circulaires de circulaire.legifrance.gouv.fr ; **inférence** : `https://circulaire.legifrance.gouv.fr/pdf/2017/04/cir_42175.pdf` (non relevé depuis le conteneur). |
| `snam-cgt-conge-maternite-paternite-20261127.txt` | « Le congé maternité et paternité », page du SNAM-CGT (syndicat national des artistes musiciens) : les conditions (9 cachets sur un trimestre civil, 36 sur quatre), le calcul de l'indemnité (12 mois civils, plafond mensuel de la Sécurité sociale, abattement de 21 %, diviseur 365 − jours indemnisés par France Travail − jours indemnisés par la Sécurité sociale), un exemple chiffré, et l'incidence sur les droits de l'annexe 10. Transcription `pdftotext -layout` d'une impression de la page faite le 27/11/2026. | Document d'un organisme privé : l'original (PDF) est dans `reference-privee/ij-maternite-intermittents/` du privé. URL de la page : **à relever** (l'impression ne la porte pas). |

| `ameli-conge-paternite-accueil-enfant-ij-20260601` (.pdf, .txt) | « Paternité et accueil de l'enfant : vos indemnités journalières », ameli.fr (Assurance maladie), article daté du 01/06/2026 : conditions (6 mois d'affiliation ; 150 h sur 3 mois civils ou 90 jours, ou 1 015 SMIC horaires sur 6 mois ; en activité saisonnière ou discontinue 600 h ou 2 030 SMIC horaires sur l'année) ; pour l'intermittent, « un cachet équivaut à 16 heures de travail » ; gain journalier de base sur 3 mois (12 en activité discontinue) dans la limite du plafond mensuel (4 005 € au 01/01/2026), abattement forfaitaire de 21 %, maximum 104,02 € par jour au 01/01/2026 ; CSG 6,2 %, CRDS 0,5 %. Transcription `pdftotext -layout` d'une impression faite le 27/11/2026. | Apporté par Léandre le 27/11/2026 ; **inférence** sur l'URL (celle que la page du SNAM-CGT donne pour ce sujet) : `https://www.ameli.fr/assure/remboursements/indemnites-journalieres/conge-paternite-accueil-enfant`. |

## Ce qu'il faut savoir avant de s'en servir

- **Une divergence à trancher** : la circulaire de 2017 et le SNAM-CGT ouvrent le droit d'un artiste au cachet à 9
  cachets au trimestre civil précédent, ou 36 sur quatre (arrêté du 4 mai 2017) ; la page d'Ameli de 2026 (paternité)
  dit seulement que l'intermittent suit les conditions des salariés, un cachet valant 16 heures. Les deux sont-elles
  vraies ensemble (9 × 16 = 144 h, proches des 150 h) ? Le texte en vigueur (CSS, arrêté) tranche.

- La circulaire date de 2017 et cite le Code de la sécurité sociale (L. 313-1, R. 313-1 et suivants, R. 313-3, R. 313-7,
  R. 323-1 et suivants, R. 331-5 et suivants) et des arrêtés (4 mai 2017 pour les cachets ; 28 mars 2013 pour
  l'abattement de 21 %). **Avant tout code, ces articles se relèvent dans leur version en vigueur**
  (`moteur-atelier releve-codes`, l'entrepôt `../legi-entrepot/`) : un seuil ou un taux a pu changer depuis.
- La page du SNAM-CGT parle encore de « Pôle Emploi » : elle n'est pas datée, et ne fonde rien seule ; elle converge
  avec la circulaire (deux émetteurs distincts) sur les 9 et 36 cachets et l'abattement de 21 %.
