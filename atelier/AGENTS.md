# PuppetPaie — ce qu'un agent doit savoir avant de toucher à quoi que ce soit

L'architecture est dans `docs/ARCHITECTURE.md` ; le journal, les méthodes de chaque rôle et les tickets sont dans le
privé. **Les numéros des règles de fond (0 à 11) ne bougent pas : le code les cite.**

## Es-tu l'orchestrateur, un exécutant ou le trieur des retours ?

Trois rôles, trois méthodes (`journal/methodes/` du privé).

- **L'orchestrateur** est la séance que Léandre a ouverte : il parle à Léandre, **tout le temps**, et lui dit les choses
  simplement, avec une image ou un exemple plutôt qu'un résumé ; il trie ce qui remonte pour que Léandre ne reçoive
  que des décisions. Il découpe, lance les sous-agents, fusionne, lance **la** suite complète (en arrière-plan : il reste
  disponible pour Léandre), pousse, ouvre la PR, commente et ferme les tickets, écrit le journal. « À la fin de la
  séance », « on ferme le ticket » : c'est lui (`methodes/ORCHESTRATEUR.md`).
- **Le trieur des retours** (un Opus, bientôt lancé seul de temps en temps) ne croit aucun retour sur parole : il le
  rejoue, le vérifie contre les textes, le range, et prépare pour chaque vrai problème un traitement justifié que
  Léandre n'a plus qu'à accepter ou refuser ; il ne change pas le logiciel (`methodes/TRIER-LES-RETOURS.md`).
- **Un exécutant** (un sous-agent, dans un worktree) fait son lot, commite sur sa branche, rend une passation ; **jamais**
  la suite complète, un push, une PR, une fusion, un ticket, le journal, un mot à Léandre. Au doute, sa consigne tranche.
- **Une seule vérification, à l'arrivée** : l'exécutant code et ne vérifie que les types (`cargo check`, `--tests`
  compris) ; il ne compile pas le programme, ne lance aucun test, ne livre aucune fictive. L'orchestrateur, à l'arrivée
  du lot, compile une fois, lance les tests, compare les fictives et **corrige lui-même** les petits écarts, sans
  renvoyer le lot ; une grosse casse devient un lot. **Une consigne ne parle ni de compilation ni de preuve** (modèle :
  `journal/methodes/CONSIGNE-EXECUTANT.md` du privé) ; si elle demande de compiler, de tester ou de livrer, c'est une faute de
  l'orchestrateur : l'exécutant suit cette règle, pas la consigne.

## Par quoi on commence

1. **`journal/ETAT.md` du privé** : une page, le point de reprise ; il nomme ce qui se lit ensuite, rien d'autre d'office.
2. **Les tickets** du privé que Léandre nomme (les siens, `retour`, `incident`). **Toutes leurs captures se regardent,
   avant le reste.**

## Deux règles de travail — elles passent avant le reste

1. **On finit ce qu'on commence**, dans la séance, jusqu'à ce que ça marche pour de vrai : un trou trouvé, un « laissé
   de côté » se corrige dans la même PR, avant de dire de fusionner. Ne remonte à Léandre qu'une **décision d'éditeur**
   (argent, engagement envers un organisme, publication, ce qui change le produit pour toutes les compagnies) ou **un
   geste sur son poste**, en une ligne.
2. **Pas de précautions de transition tant que Léandre est le seul utilisateur** : ni compatibilité, ni circuit
   provisoire, quitte à tout casser.

## L'architecture s'impose, elle ne se raconte pas

**`docs/ARCHITECTURE.md` fait foi** : le corps tout en Rust et ses portes, les garde-fous dans la structure, compiler
peu. **Les os** (l'engagement, un seul chemin), **puis les muscles** (les routes, une par geste, bien faites), **puis la
graisse** (taux, imports, fonctions avancées) : un lot de graisse attend que les os et les muscles qu'il traverse
soient droits.

- **Un garde-fou se met dans la structure, jamais dans une consigne ni dans le code** : le type d'abord (un champ
  obligatoire n'est pas `Option`), puis le verrou des dépendances et le test de structure. Un verrou n'a aucune
  exception : une passation qui garde une lecture par nom (`.get("…")`, `json!`, `Value`) ou la dit « légitime » n'est
  pas finie, l'orchestrateur la renvoie comme travail. Un repli de métier se range : la valeur vient d'un texte (dans la
  donnée, avec sa `source`) ; seule la compagnie la connaît (une case de sa fiche, et le calcul refuse en la nommant) ;
  technique (l'erreur remonte). Clippy n'est pas validé.

- **Le code dit ce qui est** : un commentaire donne le *pourquoi* (le texte déposé), jamais l'historique ; celui
  qu'on croise s'efface.
- **Les mots** : l'**engagement** est au centre et porte ses calculs ; le **contrat** est ce qu'il fait signer ; la
  **paie** part avec le bulletin et ne nomme jamais l'engagement. Tout document se fabrique depuis l'engagement validé,
  sans recalcul.
- **Un seul chemin pour chaque chose** ; un second se ferme dans le commit qui ouvre le premier. **Une carte écrit
  l'engagement de son salarié, et lui seul** ; le mois n'est jamais une unité d'écriture ; un mois importé ne reçoit pas
  d'engagement neuf. L'engagement ne lit pas le journal ; le moteur le lit une fois par geste (`envois::journal::lu`) ;
  le numéro du contrat est la clé de ce qui est parti.
- **Chaque forme est déclarée une fois, en Rust** : une donnée lue ou rendue (fichier, barème, charge et réponse d'une
  route) est une structure (`serde`), lue par une seule fonction stricte ; aucune lecture par clé texte, aucune clé
  inconnue, aucune exception. Les notes d'un fichier sont `source`, `note` et `quoi`.
- **Un champ facultatif ne s'écrit que s'il est rempli** : absent ou `null`, il est vide (`None`), et vide n'est pas
  zéro ; on n'invente rien pour lui. Une case qui n'a pas de sens pour ce qu'on saisit n'existe pas. Un repli, c'est une
  absence qui devient une valeur (0, CDDU, « 12 mois ») : c'est cela qui est interdit.
- **Un geste, une route, qui obéit** : ni état, ni bouton, ni refus de plus pour une règle (le droit vit dans le calcul) ;
  un refus dit quoi corriger et n'écrit rien ; la route rend ce que la page lit, **phrases comprises** (gardé par
  `la_page_n_ecrit_aucune_phrase`).
- **Reconstruire, c'est renommer et rebrancher** : vérifier une fois à la main sur une fictive, puis supprimer l'ancien
  et ses tests dans le même geste ; un lot finit avec moins de lignes et de tests (sauf une forme qu'on déclare). Rien
  sans l'aval de Léandre ; une pause demandée est une pause.

## Un seul logiciel, qui que soit la main

**Le logiciel, c'est le moteur et ses routes.** Cliquer dans la page ou appeler `moteur route` en ligne de commande,
c'est le même geste : la même route, la même réponse, les mêmes phrases, les mêmes refus. **Il n'y a ni « côté écran »,
ni « côté LLM »**, et on ne les distingue jamais, que ce soit dans un ticket, une consigne, un plan ou un commit : une
chose est faite quand la route la fait. La page ne parle au moteur que par `/api/<route>`, qui sert la même chose ; le
pont de la fenêtre ne fait que sa liste `FENETRE`.

**Chaque route porte sa fiche dans son code**, brève, d'où `moteur routes` tire le catalogue. **Rien du métier ne
s'écrit à côté** : un manque, où qu'on le voie, est un manque de la route. Rien ne se débranche : gardé par
`tests/fichiers/structure.rs`, sans liste d'exceptions, et prouvé en jouant (tout cliquer ; une compagnie créée depuis
zéro).

**Le moteur rend, la page lit la réponse** et n'a que ses règles d'affichage. **Où chaque route
se voit, c'est sa fiche qui le dit** (`Fiche::ecran`, obligatoire, `routes/src/catalogue.rs`) : `Dessine(place)` (un écran
fait main de cette place l'appelle), `Auto(place, bouton)` (sous « Avancé » de sa place, `js/avance.js` la dessine
depuis sa fiche : son nom n'est écrit dans aucun JS), `Lecture` (elle nourrit un écran, n'écrit rien). Les pages sont
`logiciel/interface/` et `atelier/sources/moteur/src/interface/pages/` (l'accueil). On ne cherche donc jamais une route
par son nom dans le JS : on lit sa fiche.

## La preuve d'un lot

- **Un lot dit ce qu'il touche** : chaque fonction, clé, format changés, et **chaque lecteur** trouvé par `grep`.
- **La preuve joue chaque lecteur** (route ou banc, avant et après), par l'orchestrateur à l'arrivée, notée avec ce
  qu'elle n'a pas joué. **Ce n'est pas un test de plus.**
- Le rejeu des fictives (`banc.js verifier`) suit chaque fusion qui touche un domaine ; avant, un lot n'est pas « fini ».
- **Aucune attente fixe** : on attend un état ; une lenteur se cherche à la cause.

## Une séance, par tickets

- **Un ticket** : le reproduire, corriger dans le public, prouver, le **commenter** (quoi, où, comment le voir) et le
  **fermer** dès que c'est sur la branche : « ce sera réglé dans la nouvelle version ». Ce qu'il demande se fait entier.
- **Un manque va dans un ticket du privé**, jamais dans un plan ni une consigne.
- **Un retour se trie avant de se corriger** (`journal/methodes/TRIER-LES-RETOURS.md` du privé) : ne se corrige que
  ce qui est reproduit et **que les textes autorisent** ; une question de métier va d'abord à Léandre.
- **Une consigne ne donne pas de règle sans texte** : elle pose la question, l'exécutant cherche le texte.
- **À Léandre**, une décision se présente entière, avec ses contre-arguments, et s'explique par un exemple ; réponses
  courtes ; un point de passation ne se répète qu'une fois vu.
- **La branche** part de `main`, PR en brouillon, Léandre fusionne. **Il dit la fin de séance** ; puis la suite, le
  journal, `ETAT.md` remplacé.

## Les sous-agents : des Opus

- **Opus par défaut**, pour coder comme pour juger ; Sonnet ou Haiku seulement quand une méthode le dit (le tamis de
  `methodes/FONDER-UN-TAUX.md`) ; au plus quatre Opus en même temps.
- **Un lot de code** : un Opus dessine chaque forme neuve sur une ou deux routes en modèle, puis on propage par lots
  disjoints, par crate et non par route (un Opus tient 150 à 200 appelants), en commitant à chaque étape.
- **L'orchestrateur** regarde chaque worktree toutes les 30 minutes, relit à la fusion les tests modifiés, décide seul
  du relais (contexte proche de 500 000 jetons, ou une heure sans commit) ; jamais de limite de temps dans une consigne,
  ni de message à un Opus qui a fini.
- **Un agent qui travaille ne se dérange pas** : l'orchestrateur ne lui écrit que si Léandre le demande. Quelques mots
  de Léandre ne sont pas une consigne : l'orchestrateur reformule et fait confirmer avant de transmettre, car l'agent
  prend sa parole pour sûre. Un choix de l'agent qui surprend se comprend en lui demandant sa logique, jamais en la
  supposant.
- **En tête de chaque consigne** : un test de l'ancien parcours qui casse se supprime ; aucune donnée destinée à être
  remplacée ne se convertit ; aucun test de plus de 30 secondes (sauf le rejeu d'une fictive entière,
  `tests/rejeu.rs`) ; rien en fond ; aucun acte réel lancé à la main.
- **Les cibles Cargo**, l'orchestrateur les choisit, une par worktree, gardée d'un lot à l'autre (une cible neuve coûte
  une longue compilation) ; **deux chaînes au plus**, deux worktrees gardés où les lots se suivent. Une cible qui change
  de checkout se nettoie d'abord (`cargo clean --profile essais -p <crate>`), sinon elle masque le code présent.
- **Ne se délègue pas** : une décision de conception, un acte qui engage, ce qui toucherait au coffre. Un sous-agent **ne
  réécrit jamais l'historique**.
- **Aucun Python, ni code ni outil, aucun script de patch** : Rust, Edit/Write, Playwright en Node. Un script ne montre
  jamais une requête d'authentification ni son corps.

## Où sont les choses

**Le code n'est que dans le public** ; le privé a `journal/`, `reference-privee/` et les tickets ; les données de la
compagnie, aucun dépôt (règle 9). `logiciel/` est **exactement ce qu'une compagnie reçoit** et ce que publie la mise à
jour ; `fictives/` est livrée par « Publier », hors Git ; `atelier/sources/` est l'espace Cargo (`docs/ARCHITECTURE.md`,
« Les parties » ; `declarations` prend à `documents`, jamais l'inverse) ; les textes : « Les sources ».

**On développe dans le nuage, et seulement là** : le poste de Léandre est une installation ordinaire.

**Ce que les gens lisent** : rien ne promet le futur ; le bridé se dit « démo » ; un champ n'apparaît que si un fait
l'exige ; le nom le plus court ; la page reste en HTML et JS, sans TypeScript. **On ne coupe jamais un texte : mieux vaut laid que tronqué** ; une phrase trop longue
alerte le développeur et se raccourcit dans le moteur.

## Les sources : tout texte qu'on lit se dépose

**Toute règle vient d'un texte déposé dans `atelier/reference/sources/` avant d'entrer dans le code**, sans exception ;
une règle sans texte déposé est un souvenir.

- Le texte brut tel que servi (`<émetteur>-<objet>-<AAAAMMJJ>.<ext>`), sa transcription s'il ne se lit pas, **un
  `LISEZ-MOI.md` par dossier** (URL, date, **où le moteur s'en sert**) ; une inférence se dit telle ; le code **cite le
  fichier déposé** ; chaque taux livré porte sa `source { fichier, article, citation }`, qu'un test vérifie dans le
  texte. Les codes se relèvent par `moteur-atelier releve-codes`, jamais à la main.
- **Un fondement** : un texte déposé qui dit la valeur, ou la convergence d'au moins deux textes d'émetteurs distincts,
  raisonnement écrit (`methodes/FONDER-UN-TAUX.md` du privé). L'usage du secteur ou la pratique d'un autre logiciel ne
  fondent rien. Un texte que le nuage n'atteint pas se demande à Léandre, qui le relève.
- **Ce qui ne se republie pas** : un document d'organisme privé entre par sa transcription et son URL, l'original va
  dans `reference-privee/` du privé ; une pièce réelle (bulletin, courrier, relevé) ne va **que** là, le public n'en
  garde que l'attendu, sans nom ni NIR. `atelier/reference/` ne se convertit jamais.

## Les fichiers qu'on ne lit pas

```
prive/config/coffre.bin          les mots de passe net-entreprises, Urssaf et de la boîte courriel
prive/config/google-jeton.bin    le jeton du compte Google
prive/config/maj-cle-privee.bin  la clé qui signe les mises à jour (éditeur)
```

**Ne jamais lire, afficher, copier ni recopier leur contenu**, ni en extraire une valeur, ni en les décodant ni par
`coffre.lire()`. C'est une **convention, pas une garantie** : la clé maîtresse est scellée par le mot de passe du
logiciel et par la clé du poste (`cle-poste.bin`) ; sur le poste, un agent *pourrait* les ouvrir sans mot de passe. Ce
qui tient la règle, c'est qu'elle est écrite ici et qu'aucune commande n'affiche jamais un mot de passe. `moteur coffre`
dit ce qui est posé, et cela suffit.

## Ce qu'un agent peut faire, et à quelle condition

| | condition |
|---|---|
| lire, calculer, produire un mois, sauvegarder, relever les retours | aucune |
| **déposer en test** (`--test`) | aucune, **sans demander d'aval** |
| **déposer une DSN ou une DPAE en réel, envoyer, faire signer** | **le mot de passe du logiciel, tapé par un humain au moment de l'acte**, à l'écran comme en ligne de commande |
| **approuver une publication** (environnement `publication`) | **jamais un agent** : les sessions Claude agissent sous l'identité de Léandre ; seul Léandre approuve |

**Deux mots de passe, deux chemins, pas d'autre.** Ouvrir le coffre = la séance (`moteur coffre seance --fenetre`, 12 h,
avant la première commande qui touche au coffre ; jamais `getpass` ; sans terminal, refus). Un acte réel = le popup, à
chaque fois ; la séance n'est jamais une preuve. **Ne pas contourner** : un agent ne tape pas le mot de passe, ne pose
pas `PUPPETPAIE_MOTDEPASSE` ni la variable d'un organisme, n'ouvre pas le coffre dans un script ; le LLM ne voit ni le
mot de passe ni le coffre. Si la clé du poste n'ouvre pas le coffre, le dire et donner la commande.

**Un dépôt de test ne conserve rien et c'est le seul validateur du projet** (règle 10). Dans le nuage, un dépôt réel est
mécaniquement impossible (`PreuveInsuffisante`) ; un dépôt de test passe par la compagnie d'essai si Léandre a posé
`NET_ENTREPRISES_MOTDEPASSE` : un agent ne la pose, ne la modifie ni ne l'affiche.

## Faire tourner le code dans le conteneur, et les tests

**Pour reproduire, on ne compile pas** : `echo '{"route":"…","charge":{…}}' | PUPPETPAIE_CODE=logiciel/donnees
atelier/sources/moteur/bin/linux/moteur route --racine <copie d'une fictive>`.

**Une PR dont la suite est rouge ne se fusionne pas.** La suite complète (`cargo nextest run --cargo-profile essais`,
dans `atelier/sources`) ne se lance qu'**une fois par séance, à la fin, par l'orchestrateur** ; à l'arrivée d'un lot,
il ne lance que les binaires que le lot touche (`-E 'binary(routes)'`). Avant, `rm -rf "${CARGO_TARGET_DIR:-target}/tmp"`
(les dossiers de travail des tests, ~3 Go).

- **On compile peu** (Léandre : « on code et après on compile », pas « tout le temps parce qu'on peut ») : l'exécutant,
  `cargo check` seulement ; l'orchestrateur, une compilation à l'arrivée du lot ; après une correction, seul ce qui
  était rouge se relance. Une référence déjà fabriquée ne se refait pas.
- **Le nombre de tests ne monte pas** : un test par règle d'un texte déposé, un par bug réel échappé, un rejeu par
  fictive ; **les tests de structure sont les bienvenus** ; un test qui fige un détail ne s'écrit pas. Les tests
  éprouvent des fonctionnements d'ensemble, ils ne rattrapent pas les erreurs des agents. Un test monte sa
  propre compagnie (`outils::dossier_de_test`).
- **Aucun test ne sort de la machine** : gardé par `tests/fichiers/garde_reseau.rs`.
- **L'atelier** (la crate `atelier`, son binaire du nuage `moteur-atelier`, jamais livré : les relevés des textes, les
  outils, les scénarios du banc) est hors de `default-members` : la suite ci-dessus ne le construit pas. Il allume la
  feature `atelier` de ce qu'il prend, donc il se construit et se teste dans sa propre cible (`<cible>-atelier`,
  `-p atelier`) ; ce qui est sous `#[cfg(feature = "atelier")]` y compile sans avertissement. **Le dépôt le garde
  compilé** (`atelier/sources/atelier/bin/linux/moteur-atelier`) : le garde avant commit et la publication s'en servent
  sans rien compiler. Un lot qui change l'atelier le refait (`cargo build --release -p atelier`, puis copier le binaire)
  dans le même commit.

## Les règles de fond du logiciel

0. **Ce logiciel n'est écrit pour aucune compagnie en particulier** : celle du poste n'est que le banc d'essai ; un cas
   qu'aucune donnée n'éprouve reste à porter.
1. **Rien ne se devine.** Un taux de PAS, un identifiant DGFiP se *lisent* ; un bordereau Urssaf se *calcule*. Le moteur
   refuse plutôt que d'inventer ; on ne « débogue » pas un refus avec une valeur plausible.
2. **Toute décision vit dans le moteur, jamais dans un front** : un garde-fou dans un front se contourne par l'autre.
3. **Les modules de transport ne cherchent jamais un secret.** Ils reçoivent un mot de passe ; ils n'ouvrent pas le coffre.
4. **Ne jamais régénérer une déclaration déjà déposée** pour la redéposer : les numéros de contrat se renumérotent. Un
   numéro de contrat ne se recalcule pas, il se lit.
5. **Deux canaux de retours** : la DSN sur net-entreprises, la DPAE sur `consultation.dpae-edi.urssaf.fr`, services,
   mots de passe et archives distincts. Un 404 sur une plage veut dire « rien » ; un accusé `OK` n'est pas une déclaration
   acceptée, le bilan tranche.
6. **Une nature de contrat n'est pas un détail** : `donnees/contrats-base.json` dit ce que la DSN écrit par nature ; la
   nature est toujours **dite**, jamais devinée : une carte neuve s'ouvre en CDDU, déclaré, et la nature ne se choisit
   que là où la compagnie emploie des permanents ; le numéro d'objet spectacle est **interdit** hors des natures 02 et 10.
7. **Deux contrôles avant tout dépôt** : les usages et les formats. Un contrôle faux refuserait du licite : c'est lui
   qu'on retire, jamais la déclaration.
8. **Un barème a trois étages** (le socle livré, la convention, la compagnie : son taux AT/MP, lu dans sa configuration,
   l'effectif des retours 118),
   empilés à un seul endroit (`droit::contexte`) ; une convention sans couche refuse.
9. **Trois destinations, et les données de la compagnie dans aucun dépôt.** `logiciel/`, `fictives/`, `atelier/` : le
   public. `journal/`, `reference-privee/`, les tickets : le privé. `prive/` : aucun dépôt (le poste, la clé USB, le
   Drive). Rien de réel n'entre dans le public (`moteur-atelier verifier-avant-commit`, avant chaque commit).
   a) **Les tests** ne lisent rien de `prive/` (gardé par `atelier/tests/atelier/donnees_reelles.rs`). D'une compagnie réelle on
      ne garde que ses attendus, sans nom ni NIR, dans un test fictif.
   b) Un conflit sur un journal JSON se résout enregistrement par enregistrement, sans rien perdre.
10. **Test = réel.** Un dépôt de test accepté (`OK, 0 anomalie`) vaut un dépôt réel : même moteur, même fichier, même
    circuit ; un acte « prouvé en test, pas en réel » ne se présente jamais comme un risque ; reste ouvert ce que le
    test ne montre pas (AER, taux de PAS). Le garde-fou test → réel protège d'un envoi par erreur, il ne met pas le
    moteur en doute.
11. **La copie de développement ne se voit pas dans l'interface** : `prive-dev/` reste en ligne de commande.

## Les fins de ligne

Code et docs en **LF**, `.cmd` et `.bat` en CRLF (`.gitattributes`). Les formats à largeur fixe (AEM, DPAE, DSN, retours)
et ce dont on vérifie une empreinte **ne se convertissent jamais**.

## Publier une version

Depuis GitHub seulement (`journal/methodes/PUBLIER.md` du privé) : suite verte, PR sur `main`, Léandre fusionne ; le
numéro est la date. **Le rythme est à Léandre : lui seul lance « Publier »**, avec `notes`, et `calcul` **seulement** si
un calcul de paie change ; **Léandre approuve** (environnement `publication`), le moteur signe avec la clé de GitHub.
La forme de l'archive de mise à jour (`compagnie/src/maj.rs`) **est un contrat avec toutes les installations
posées et ne change plus**. Le manifeste publié (`derniere.json`) non plus : une installation le lit en strict, avec la
forme de sa version, et une clé de plus la couperait des mises à jour ; ce qui est neuf va dans l'archive. Un besoin
contraire se présente d'abord à Léandre, expliqué. Jamais pendant que l'interface tourne ; jamais republier un numéro.
