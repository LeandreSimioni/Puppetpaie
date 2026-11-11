# Circulaire DGT n° 2009-4 du 17 mars 2009 (rupture conventionnelle d'un CDI)

`travail-circulaire-dgt-2009-04-20090317.pdf` : la circulaire telle que publiée au Bulletin officiel du ministère du
travail (NOR : MTST0980929C, « texte non paru au Journal officiel »), envoyée par Léandre le 05/11/2026.
`travail-circulaire-dgt-2009-04-20090317.txt` : son texte, extrait par `pdftotext -layout`, sans retouche.

Ce qu'on en lit : l'indemnité spécifique de rupture conventionnelle d'un salarié de moins d'un an d'ancienneté « lui est
due au prorata du nombre de mois de présence » (`.txt` l. 212) ; le salaire de référence lisse au prorata les primes
(l. 192).

**Où le moteur s'en sert** : `logiciel/donnees/permanents-base.json`, `rupture_conventionnelle.indemnite` (sa `source`, une convergence avec R1234-1 et R1234-2) : sous huit mois d'ancienneté, le plancher au prorata des mois complets, au taux de R1234-2 en vigueur (un quart) et non à celui de l'exemple de la circulaire (un cinquième, le taux de 2009) ; `droit::permanents::indemnite_rupture_conventionnelle`.

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `travail-circulaire-dgt-2009-04-20090317.pdf` : `4eba314cc2e6eeb442878b71698f4200800ab683caaec940ba4a737a82c894c6`
- `travail-circulaire-dgt-2009-04-20090317.txt` : `8a2f97b53437855eb2fd4bb59a4391c006a3f0d2b99cbf5054f7de49f8f60bf9`
