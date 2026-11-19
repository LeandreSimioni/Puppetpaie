# La compagnie d'essai : un vrai dépôt de test sans `prive/`

Pour un développeur (ou une séance Claude dans le nuage) qui n'a pas les données de sa compagnie sur la machine, mais
qui veut faire passer une DSN, un signalement ou une DPAE **en test** sur la vraie plate-forme — le seul validateur du
projet (`atelier/AGENTS.md`, règle 10). Lot E1, 15/10/2026 ; le code : `atelier/sources/compagnie/src/essai.rs`.

Pour développer et faire passer la suite de tests, **rien de tout cela n'est utile** : les tests tournent sur les
compagnies fictives et de faux serveurs, sans aucune variable.

## Ce qu'il faut poser dans l'environnement

Trois variables, posées dans les réglages de l'environnement (jamais dans la conversation, jamais dans un fichier du
dépôt). Une nouvelle séance les prend ; celle qui tourne ne les voit pas.

| variable | ce qu'elle porte |
|---|---|
| `PUPPETPAIE_COMPAGNIE` | l'identité de **votre** compagnie : un objet JSON sur une ligne (ci-dessous) |
| `NET_ENTREPRISES_MOTDEPASSE` | le mot de passe net-entreprises du déclarant (DSN, signalements, retours DSN) |
| `URSSAF_MOTDEPASSE` | le mot de passe du compte DPAE MtoM d'urssaf.fr (DPAE, retours DPAE) |

`PUPPETPAIE_COMPAGNIE` a **exactement** ces clés, toutes des textes (une clé qui manque ou en trop est refusée, en
la nommant ; chaque valeur est confrontée, au montage, au format des rubriques de la DSN qu'elle remplit) :

```
siren, nic                      le SIREN (9 chiffres) et le NIC de l'établissement (5) ; le SIRET s'en déduit
raison_sociale, adresse,        l'en-tête de l'entreprise (S10.G00.01, S21.G00.06, S21.G00.11 ; le bloc employeur
code_postal, commune, naf       de la DPAE)
medecine_travail                le code du centre de médecine du travail (C_MT_DCL de la DPAE, 10 caractères au plus)
declarant      {nom, prenom}    la personne inscrite au service DSN de net-entreprises, telle qu'inscrite
declarant_dsn  {civilite,       la personne que les organismes contactent : civilité 01 (Monsieur) ou 02 (Madame),
                nom_prenom,     nom et prénom, téléphone, courriel (S10.G00.02, S20.G00.07) ; qualite est le CODE du
                tel, email,     type de contact (S20.G00.07.004, deux caractères : 04 pour les cotisations de Sécurité
                qualite}        sociale ; cahier technique, bloc S20.G00.07), pas un intitulé
api_dpae       {nom, prenom,    le compte DPAE d'urssaf.fr, sans accents ; siret et service peuvent être vides
                siret, service} (siret vide : celui de la compagnie)
```

Ces valeurs ne sont pas des secrets, mais elles désignent une compagnie et des personnes : **elles ne s'affichent
jamais** (le moteur n'en cite aucune, ni en succès ni en refus ; un agent dit « posée » ou « absente »). Elles sont
les **vôtres** : un déclarant n'est habilité que pour ses SIRET, et c'est lui qui répond de ce qui part.

Pour savoir si elles sont posées, sans lire leur valeur :

```bash
for v in PUPPETPAIE_COMPAGNIE NET_ENTREPRISES_MOTDEPASSE URSSAF_MOTDEPASSE; do
  if [ -n "${!v}" ]; then echo "$v posee"; else echo "$v absente"; fi
done
```

## Monter la compagnie d'essai

```bash
moteur essai monter [DOSSIER] [--depuis FICTIVE]
```

- copie une compagnie **fictive** livrée (défaut : `fictive-fil-de-fer`, livrée d'abord par `moteur-atelier scenario
  livrer atelier/fictives/fictive-fil-de-fer fictives/fictive-fil-de-fer` ; `--depuis` prend un autre nom de `fictives/` ou un chemin) dans `DOSSIER` (défaut :
  `<dossier temporaire>/puppetpaie-essai`), **hors de tout dépôt Git** — le montage refuse sinon ;
- donne à sa fiche (`config/entreprise.json`) l'identité de `PUPPETPAIE_COMPAGNIE`, et en retire celle de la fictive ;
  tout le reste (salariés inventés, contrats, paies, barème, organismes) reste celui de la fictive ;
- ne garde ni ses retours, ni ses dépôts de DSN (joués sous le SIREN de la fictive) ; garde ses DPAE et ses contrats
  revenus signés, qui disent qu'un mois peut se fermer ;
- pose le marqueur d'une **compagnie d'essai** : ses dépôts de **test** vont au réseau (une fictive les joue), et
  **tout dépôt réel y est refusé** (par la dernière porte avant le réseau, et faute de coffre) ;
- relancé sur une compagnie d'essai existante, ne réécrit que l'identité (journal, DSN et retours gardés).

Un refus ne laisse rien sur le disque.

## La suite d'une séance

Toutes les commandes suivantes nomment la compagnie par `PUPPETPAIE_DONNEES`. Sans `prive/` voisin, c'est
indispensable : le moteur prendrait sinon le logiciel lui-même pour compagnie. Dans le nuage, le moteur Linux livré
est `atelier/sources/moteur/bin/linux/moteur` (il peut retarder sur les sources : sinon, `cargo build --release`).

```bash
moteur essai monter
export PUPPETPAIE_DONNEES=/tmp/puppetpaie-essai     # le dossier que la commande a dit
```

**Une DSN mensuelle de test** (les routes, par `moteur route --texte`) :

```bash
echo '{"route":"deposer","charge":{"quoi":"dsn","mois":"2026-09"}}' | moteur route --texte            # la DSN du mois, sous votre identité, en test
echo '{"route":"ouverture","charge":{"sauvegarder":false,"signatures":false}}' | moteur route --texte                     # quelques minutes plus tard : le bilan (CCO/BAN) fait foi
```

La base d'essai de net-entreprises garde les essais **par SIRET**, y compris ceux faits depuis un autre poste ; le
journal de la compagnie d'essai, neuf, ne les connaît pas (et les bilans ne disent pas le mois d'une déclaration : le
moteur ne peut pas le lire sur la plate-forme). Chaque nouvel essai du même mois part à un ordre plus grand que le
précédent (le journal compte ses essais) : si le bilan refuse l'ordre (B1-208-11, « dernier ordre accepté AAAAMMnn »),
redéposer jusqu'à le passer.

Une DSN copiée de la fictive (écrite avant le montage, sous son SIREN) est refusée : déposer la DSN par son mois, qui
la fabrique sous votre identité.

**Un signalement** (fin de contrat, arrêt) : sa route l'écrit (`fin-contrat-signaler`, `arret-signaler`), puis
`deposer` avec `fichier` (le fichier que la route a rendu). Son ordre se lit sur la plate-forme à l'écriture.

**Une DPAE de test** :

```bash
echo '{"route":"deposer","charge":{"quoi":"dpae","mois":"2026-11"}}' | moteur route --texte           # une seule embauche : avec engagements
echo '{"route":"ouverture","charge":{"sauvegarder":false,"signatures":false}}' | moteur route --texte
```

## Ce qu'il faut pour qu'un dépôt de test revienne « OK » (appris le 19/11/2026)

Trois choses font refuser un dépôt de test par l'enveloppe, avant tout contrôle du contenu ; aucune ne tient au
logiciel :

1. **La banque de la fictive doit exister.** La plate-forme contrôle le code banque **et** le code guichet des
   coordonnées de prélèvement (S21.G00.20.003 et .004) : « B1-205-01 Le code banque 12345 est inconnu », puis
   « B1-205-02 Le code guichet 00001 est inconnu ». Les fictives ont une banque inventée (« Banque du Marais », code
   12345). Il n'y a pas besoin du vrai RIB de la compagnie : un **faux compte** sous un **vrai couple banque et
   guichet** suffit. Dans la compagnie d'essai montée (jamais dans le dépôt), `config/entreprise.json`,
   `banque[0].valeur` : le `bic` de la banque choisie, et un `iban` composé du code banque, du code guichet, d'un
   numéro de compte de onze zéros et des deux clés calculées (la clé RIB, puis la clé de l'IBAN). Le couple employé le
   19/11/2026 : banque 30001, guichet 00064 (BIC `BDFEFRPPCCT`), clé RIB 40, clé de l'IBAN 76. Ce compte n'existe
   pas ; l'IBAN assemblé ne s'écrit pas dans le dépôt (le garde avant commit refuse tout IBAN). Un dépôt de test ne
   conserve rien et ne prélève rien.
2. **Le mois ne doit pas être déjà déclaré sous ce SIRET.** La base d'essai connaît les DSN réelles de la compagnie :
   une DSN de test d'un mois déjà déposé en réel est prise pour un « annule et remplace » hors délai (« B1-207-05 »,
   « B1-208-12 »). Déposer sur le mois en cours, pas encore déclaré. Pour cela, rejouer la fictive jusqu'au mois
   d'avant (`moteur-atelier scenario rejouer <fictive> <installation> --jusqu-au AAAA-MM-JJ`), ajouter le cas par les
   routes, puis monter la compagnie d'essai depuis cette compagnie rejouée (`moteur essai monter DOSSIER --depuis
   <installation>/fictives/ajoutee-…`).
3. **L'ordre d'un signalement.** Un signalement refusé « B1-208-10 Il existe déjà une déclaration normale » : l'ordre
   est pris par un essai antérieur. Refaire le signalement par sa route (`fin-contrat-signaler`) puis le redéposer : il
   prend l'ordre suivant.

Le bilan qui tranche est le retour de nature 11 (`retours/AAAAMMJJ-HHMMSS_nature11_….xml`, `etat_conformite`), relevé
par `ouverture` une à deux minutes après le dépôt. La relève rapporte aussi les retours réels du déclarant (ses
attestations, ses bilans) dans le dossier de la compagnie d'essai : ce dossier reste hors de tout dépôt et ne se cite
pas.

Prouvé ainsi le 19/11/2026 : le signalement de fin d'un contrat d'usage à cheval sur deux mois (nature 07, « OK »,
aucune anomalie) et la DSN mensuelle de novembre 2026 qui porte sa fin hors du circuit dérogatoire (62.017 = 02,
« OK », conforme).

## Ce qui reste à confirmer au premier dépôt

Inférences, qu'aucun texte déposé ne dit (le premier bilan tranche) : que la plate-forme d'essai accepte un en-tête
réel sur un contenu fictif (NIR inventés à clé juste, références d'adhésion et organismes de la fictive) ; que la
civilité, la qualité et l'adresse ne sont pas confrontées au répertoire SIRENE.

Les retours relevés s'archivent dans la compagnie d'essai et disparaissent avec elle : ils ne remplacent pas le relevé
du poste de la compagnie.

## Le nuage parle au filet comme le poste de l'éditeur

Indépendant de la compagnie d'essai, et à poser une fois. Une fictive rejouée dans le nuage est une installation neuve :
sans identité de poste, elle en demande une au serveur de l'éditeur (`POST /poste`), qui n'en donne que 50 par jour
pour tout le monde, 3 par adresse ; une séance qui rejoue beaucoup les épuise, et un retour écrit sur un vrai poste
reste « pas encore parti — Trop de demandes ». Le serveur ne plafonne pas le poste de l'éditeur
(`FILET_POSTE_EDITEUR`, côté serveur) : le nuage porte donc **l'identité du poste de l'éditeur**.

| variable | ce qu'elle porte |
|---|---|
| `PUPPETPAIE_ATELIER_POSTE` | le numéro du poste de l'éditeur : 32 chiffres hexadécimaux minuscules |
| `PUPPETPAIE_ATELIER_CLE_POSTE` | la clé de ce poste, un secret : 64 chiffres hexadécimaux minuscules |

Seul le binaire de l'atelier (`moteur-atelier`, jamais livré) les lit (`compagnie/src/incidents.rs`,
`identite_de_l_atelier`) ; le moteur que les compagnies reçoivent n'en contient pas même les noms
(`moteur/tests/atelier.rs`). Posées toutes les deux, il parle avec elles au **seul** serveur de l'éditeur
(`https://filet.puppetpaie.fr`, `editeur::SERVEUR_PAR_DEFAUT`), ne demande jamais d'identité et ne les écrit dans aucun
fichier : `config/diagnostic.json` de la fictive n'en reçoit rien. Un autre serveur (le faux serveur local d'un
essai) ne les reçoit jamais : il garde l'identité qu'il a donnée. Une seule des deux posée, ou l'une mal formée : le
moteur refuse en nommant la variable, jamais sa valeur, et ne demande pas d'identité à la place. Aucune posée : rien ne
change. Le banc d'essai (`outils/banc/banc.js`) les retire de sa fenêtre, qui ne sort pas de la machine.

**Les poser** : sur le poste de l'éditeur, le numéro et la clé sont les clés `poste` et `cle` de
`prive\config\diagnostic.json` (dans le dossier de PuppetPaie) ; les copier de là, sans les coller dans une
conversation, dans les réglages de l'environnement du nuage (variables d'environnement). Elles y sont en clair, lisibles
par toute séance de cet environnement. Les copier sans espace ni retour à la ligne : une valeur mal formée
est refusée. Vérifier sur le serveur que `FILET_POSTE_EDITEUR` vaut ce même numéro :
`sudo grep '^FILET_POSTE_EDITEUR=' /etc/puppetpaie-filet.env` (cette ligne seule : le reste du fichier porte des
secrets). Une nouvelle séance les prend.

**Deux machines, un numéro** : les incidents du nuage et ceux du poste de l'éditeur partent sous le même `poste`. Ils
se distinguent par leur machine : `systeme` et `os` (Linux dans le nuage, Windows sur le poste) et `chemin` (un dossier
du conteneur, `/tmp/…` pour une fictive rejouée, contre `C:\…`) ; `compagnie` dit « fictive » pour toute fictive, y
compris une fictive ouverte sur le poste, et `installation` ne dit « atelier » que d'un moteur lancé dans un dépôt
(une installation de rejeu n'en est pas un) : seuls, ils ne tranchent pas. `moteur` (l'empreinte des sources) distingue en plus un moteur du jour d'un moteur publié.
