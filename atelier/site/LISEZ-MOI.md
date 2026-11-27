# Le site puppetpaie.fr

Ce dossier **est** puppetpaie.fr : ce qui est ici, sur `main`, est ce que le site sert. Sa racine est la racine du
domaine (`index.html` est la page d'accueil, `utilisation/` et `confidentialite/` ses deux pages, `fiche/` la page où
un salarié prépare sa réponse à « Remplir ma fiche », `demo/` la démo du logiciel dans le navigateur, qui a sa propre
politique de sécurité : `demo/LISEZ-MOI.md` ; `droits/` la page où un intermittent voit ses droits depuis ses
bulletins, par le moteur de la démo, qui a aussi la sienne : `droits/LISEZ-MOI.md`).

## Comment il part en ligne

Le serveur est celui de la compagnie UEUEUE ; c'est le déploiement de son dépôt (`ueueue-site`,
`.github/workflows/deploy.yml`) qui prend ce dossier sur `main` de PuppetPaie et le pose tel quel sous
`dist/puppetpaie/`, que nginx sert pour puppetpaie.fr. Il passe chaque jour à 6 h (heure de Paris), ou quand Léandre
lance « Run workflow ». Rien ne se pose à la main sur le serveur : le déploiement suivant l'effacerait. Si la prise
échoue, le site déjà en ligne est gardé.

**Le compteur des postes.** La ligne `<!--COMPTEUR-->` d'`index.html` est la place du nombre de postes qui se sont
servis du logiciel dans les trente derniers jours : le déploiement le demande au serveur du filet (`GET /compte`) et
y écrit « PuppetPaie est utilisé par N employeurs. » (Léandre, 26/11/2026 : sur trente jours, ce sont les
utilisateurs actifs). Le filet injoignable, ou aucun poste, la ligne s'efface : on n'écrit pas un
chiffre qu'on n'a pas lu. La politique de sécurité interdit à la page d'aller le chercher elle-même.

Ce `LISEZ-MOI.md` n'est pas mis en ligne.

## Ce que le site admet

La politique de sécurité de puppetpaie.fr (`ueueue-site`, `infra/nginx.conf`) : `default-src 'none'`,
`script-src 'self'`, `style-src 'unsafe-inline'`, `font-src 'self'`, `img-src 'self'`, `form-action 'none'`. Donc :

- le style est dans la page (`<style>`), jamais dans une feuille à part ;
- un script est un fichier du site, jamais en ligne ;
- aucune requête (`fetch`), aucune ressource d'un autre domaine, aucun formulaire envoyé ;
- les polices et les favicons communs se prennent sous `/assets/`, que le serveur sert depuis le site de la compagnie.

Rien n'est gardé en cache (`expires -1`) : un fichier changé ici est servi dès le déploiement suivant.
