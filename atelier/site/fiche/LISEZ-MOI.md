# La page « Remplir ma fiche »

Le site puppetpaie.fr l'héberge sous `https://puppetpaie.fr/fiche/` : son déploiement prend ce dossier sur `main`
(sans ce fichier). Le courriel d'une demande de fiche (`fiche-demander`) y mène ; le salarié y remplit ce qui manque à
sa fiche, et la page prépare le courriel de sa réponse, qu'il envoie de sa messagerie. Elle ne garde rien et n'envoie
rien au serveur : tout ce qu'elle sait est dans le fragment du lien.

- `index.html` (le style y est : la politique du site n'admet pas de feuille à part) et `fiche.js` : la page. Elle ne
  connaît aucune question ; elle dessine le formulaire que le moteur déclare
  (`atelier/sources/documents/src/fiche_questionnaire.rs`).
- `formulaires/<empreinte>.js` : le formulaire, un fichier par forme, jamais retiré (un lien déjà envoyé nomme le
  sien). `moteur-atelier formulaire-fiche` l'écrit ; un test de structure tient que le fichier commité est celui que le
  moteur déclare.
