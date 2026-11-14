# Architecture

Ce fichier fait foi pour l'architecture : il dit comment le logiciel est fait, et pourquoi. Ce qui change l'architecture
le change dans le même commit.

## Le corps et ses portes

**Tout est en Rust**, et le logiciel est un corps solide sous une peau solide, percée de quelques portes seulement : les
routes (la page comme la ligne de commande), les fichiers, les imports. À la porte, on choisit ce qui entre ; partout ailleurs, rien
n'entre. Ce qui passe la porte est lu **une fois** et devient un type Rust ; dedans ne circulent que des types.

- **Une donnée lue est une forme, déclarée une fois.** Fichier, barème, charge ou réponse d'une route : une structure
  `serde` (`deny_unknown_fields`), lue par `socle::forme`, écrite par `socle::ecriture`. Le JSON brut ne se touche
  qu'au bord, ces deux modules : tout autre passe par eux (la charge et la réponse d'une route y sont un
  `socle::forme::Objet`, qui ne se lit qu'en une forme ; le JSON ASCII s'écrit dans `socle::json_ascii`). Aucune lecture par clé
  texte, aucune clé libre, aucun repli (une donnée facultative absente est `None`, une donnée exigée absente se refuse),
  aucune erreur avalée. Les états impossibles ne s'écrivent pas : une nature de contrat est une `enum`
  (`droit::natures::NatureDeContrat`), dont `contrats-base.json` dit les propriétés ; un motif de recours ou de rupture
  et la modalité du temps de travail aussi (`droit::natures::MotifDeRecours`, `MotifDeRupture`,
  `ModaliteDuTempsDeTravail`), et le code d'une base assujettie (`droit::base_assujettie::BaseAssujettie`, que le
  barème nomme en `bloc78` et dont la déduction forfaitaire spécifique abat certaines), et celui d'une cotisation d'établissement (`droit::cotisation_etablissement::CotisationDEtablissement`,
  S21.G00.82.002 : l'OETH, la taxe d'apprentissage, la CSA), et la nature d'une déclaration
  (`droit::nature_declaration::NatureDeLaDeclaration`, S20.G00.05.001 : le fichier l'écrit, le journal des envois la
  garde par son code ; un signalement d'arrêt, `declarations::signalement::NatureSignalement`, n'en est que 04 ou 05),
  et son type (`droit::type_declaration::TypeDeLaDeclaration`, S20.G00.05.002, avec ses libellés) :
  chacune est la liste entière de sa rubrique dans la norme (`ListeDeLaNorme`), tenue aux
  listes relevées des versions livrées par un test de structure ; un code propre à une version le dit (`depuis`,
  `jusqu_a`), et la DSN ne l'écrit que dans un fichier de cette version (`dsn::Fichier::ecrire_code`, classe
  `HorsDeLaVersion`). Le fichier, lu contre elles, dit ceux que chaque nature permet. Une assiette du calcul aussi
  (`droit::assiette::Assiette`) : un barème la nomme (`base`, `base78`) et une assiette inconnue le refuse à sa lecture ;
  elle indexe les bases d'une paie, calculée, écrite ou importée ; ce qui est propre à une assiette (par rubrique de
  salaire, sa forme abattue) est une de ses propriétés.
- **La porte des tiers : les réponses de Google.** On ne lit que des clés connues ; seule une réponse de Google peut
  porter en plus des clés qu'on ignore à la porte. Sa forme se déclare elle-même réponse d'un tiers
  (`impl socle::forme::ReponseDUnTiers`, à côté de sa structure, sans `deny_unknown_fields`) et ne se lit que par
  `socle::forme::lire_reponse_tiers` : stricte sur les clés qu'elle déclare (une clé exigée qui manque, ou d'un autre
  type, refuse la réponse) ; une clé que Google ajoute et qu'on ne lit pas n'est ni lue, ni gardée, ni transmise. Rien
  d'inconnu n'entre dans le logiciel. **Pourquoi** : Google ajoute des clés à ses réponses sans prévenir ; en strict, la
  connexion du compte ou le Drive tomberaient jusqu'à une nouvelle version. Ne l'empruntent que le jeton, l'erreur, le
  compte et le Drive (`envois::google`, `google_autorisation`, `drive`). Nos propres formes et les retours de la norme
  (net-entreprises, Urssaf) restent stricts : un format de la norme ne change pas sans nouvelle version de la norme.
- **Pourquoi** : aucun calcul ne se demande si une case est vide ou mal écrite ; le compilateur oblige chaque calcul à
  traiter chaque cas, et suit chaque lecteur quand une forme change. Et la vitesse : avant, ouvrir une page prenait
  jusqu'à 5 s parce que la même carte se relisait 40 à 50 fois ; lue une fois à la porte, elle se passe ensuite en type
  (`socle::forme::partagee`, le journal lu une fois par geste : `envois::journal::lu`). C'est la structure qui fait la
  vitesse, pas Rust seul.
- **La peau protège la forme, les sources protègent le contenu** : chaque valeur livrée cite le texte déposé qui la
  fonde (« Les sources », `AGENTS.md`). Il faut les deux.

## Les garde-fous sont dans la structure

Un garde-fou écrit dans le code ou dans une consigne finit contourné (un agent au long contexte l'oublie) ; un
garde-fou que le compilateur ou un test de structure tient ne s'oublie pas. On ne garde donc une règle d'architecture
que par la structure :

- **Le verrou des dépendances** : `serde_json` n'est au `Cargo.toml` que du `socle`, la porte (`moteur` et `atelier`
  l'ont pour leurs essais ; `fabrique-pdf`, hors de l'espace, l'a aussi : à ranger, #419) ; une autre crate qui
  voudrait lire du JSON brut ne compile pas (`une_crate_ne_prend_qu_a_celles_du_dessous` le lit dans les `Cargo.toml`).
- **L'ordre des crates** : chaque crate ne prend qu'à celles du dessous (« Les parties ») ; le calcul n'est que dans
  `engagement`.
- **Les pièces d'une fiche** : une case qu'un seul genre de calcul lit est rangée, dans le type, dans sa pièce
  (`droit::fiche::PieceFermee`), qui ne s'ouvre que par sa clé. Les cases d'un salarié au mois
  (`ChampsSalarie::permanent`), et celles de l'entreprise que seul son calcul lit (`ChampsEntreprise::permanent`), ne
  s'ouvrent que par `droit::natures::AuMois`, que seule la nature d'un engagement payé au mois donne : un calcul qui n'a
  pas cette nature en main ne les lit pas, il ne compile pas. « Au moins cinquante salariés »
  (`ChampsEntreprise::avec_permanents`) ne s'ouvre que par la case « emploie des permanents » cochée. Les cases du
  métier (l'échelon de la CCNEAC, le poste, l'artiste stagiaire, l'expérience de la 3241) ne s'ouvrent que par
  `droit::minima::ParLaGrille`, que seule une grille trouvée donne (`grille_au`, et la prime d'ancienneté d'un
  technicien de l'annexe I) ; la jauge de la salle de l'annexe I, sur la fiche Entreprise, par cette clé et
  `AuMois` ensemble (`ChampsEntreprise::metier`). La représentation du personnel ne s'ouvre que par
  `droit::contexte::UnCsePossible`, que seul l'effectif donne quand un comité peut exister (`Effectif::cse`) ;
  l'exemption de la CASC-SVP, par `droit::fiche::ExemptionCasc`, que seuls cinquante salariés
  (`Effectif::cinquante`) et un CSE dit donnent. Ce que la fiche
  montre s'en déduit (`routes::fiches::montres`) : la pièce d'un permanent, sur la fiche d'un salarié, par un de ses
  engagements payés au mois (`engagement::permanents::cle_du_salarie`) ; celle du métier, case par case, par la grille
  du métier de chacun de ses engagements ; celles de l'effectif, par lui ; une pièce qu'aucun engagement ni l'effectif
  ne peut ouvrir ne pose aucune question. Le fichier reste à plat ; la pièce d'une case se lit dans sa forme, jamais dans
  `champs-base.json`. Une case d'une pièce n'est pas un `droit::fiche::NomDeCase` : `fiche!` la déclare à part
  (`droit::fiche::CaseDePiece`), et une saisie (`declarations::champs::Saisie`) ne la lit pas par `case`, qui ne prend
  qu'un `NomDeCase`, mais par `piece`, qui prend une `CaseDePiece` ; la mécanique de toutes les cases (la table,
  l'écriture datée) les nomme ensemble (`droit::fiche::CaseDeFiche`). Une rubrique livrée que seul le calcul d'un salarié au mois paie est rangée à part
  (`au_mois` de `rubriques-base.json`, `droit::catalogue::Rubrique::piece`) ; un contrat d'organisme des permanents
  l'est par son rôle, prévoyance ou santé (`droit::organismes::Attendu::piece`). Aucune étiquette ne dit une pièce. Le
  rejeu des deux fictives le vérifie (`moteur/tests/rejeu.rs`, `aucune_piece_fermee_ne_se_montre`) : le Fil de fer,
  sans permanent, et un intermittent de l'Hôtel de Bourgogne ne voient aucune case d'une pièce qu'aucun de leurs
  engagements n'ouvre.
- **Les niveaux du compilateur** (`[workspace.lints.rust]` de `atelier/sources/Cargo.toml`, repris par chaque crate ;
  les mêmes dans `fabrique-pdf/` et `fenetre/`) : aucun avertissement, et le code mort, les imports et variables
  inutiles, le déprécié interdits (`forbid`). Aucun attribut ne lève un niveau `forbid` : le compilateur le refuse ;
  une clé connue qu'on ne lit pas est une `socle::forme::NonLue`, qui n'en garde rien ; un code mort se supprime ; un
  code propre à une cible se range sous son `cfg`.
- **Le verrou simple** : un verrou qui lit le texte du code se contourne en réécrivant le repli sans changer ce qu'il
  fait ; le garde-fou est le type et le compilateur. Restent donc : le compilateur sans appel (les niveaux ci-dessus) ;
  la visibilité (les lectures privées au `socle`) ; les `Cargo.toml` ; trois tests courts, sans liste, qui lisent les
  sources des crates ; et la relecture du diff de chaque lot à l'arrivée, qui tient tout le reste.
- **Les tests de structure** (`moteur/tests/fichiers/structure.rs`), sans liste d'exceptions ; les tests de structure
  sont les bienvenus, un test qui fige un détail ne s'écrit pas :
  - `aucun_flottant_dans_le_code` : aucune source d'une crate n'écrit `f32` ni `f64` ;
  - `chaque_forme_lue_refuse_une_cle_inconnue` : chaque `derive(Deserialize)` porte `deny_unknown_fields`, sauf une
    réponse d'un tiers, qui se déclare telle par son type (« Le corps et ses portes ») ;
  - `aucune_valeur_par_defaut_si_absent` : ni `unwrap_or`, `unwrap_or_default`, `unwrap_or_else`, `map_or`,
    `map_or_else` dans les sources des crates ;
  - `une_crate_ne_prend_qu_a_celles_du_dessous` : l'ordre des crates, et seul le `socle` prend `serde_json`, lus dans
    les `Cargo.toml` ;
  - `les_niveaux_des_lints_tiennent` : chaque crate reprend les niveaux de l'espace, et aucune configuration de Cargo
    ne les abaisse ;
  - `le_catalogue_est_range` : une lecture n'écrit pas, une route sous « Avancé » n'a pas de champ objet, un module a
    une place, une route inconnue se refuse ;
  - `l_atelier_reste_hors_du_logiciel` : aucun manifeste du logiciel ne prend l'atelier ;
  - `chaque_route_dit_brievement_ce_qu_elle_fait` : chaque route du catalogue a sa fiche, une seule ;
  - `l_exemple_de_chaque_fiche_est_accepte` : l'exemple de chaque fiche, joué par `moteur route`, est accepté et rend sa
    phrase ;
  - `ce_que_l_ecran_affiche_est_fait_pour_l_ecran` : les boutons, libellés, aides et choix du catalogue et de
    `champs-base.json` sont brefs et en mots ;
  - `champs_base_et_les_fiches_ont_les_memes_cases` : `champs-base.json` et la forme des fiches disent les mêmes choix
    et longueurs ;
  - `la_carte_valide_a_une_variante_par_nature` : une variante par nature de `contrats-base.json` ;
  - `une_liste_de_la_norme_est_celle_des_versions_livrees` : chaque `ListeDeLaNorme` est l'union des listes relevées
    des versions livrées ;
  - `aucun_fichier_livre_ne_porte_une_cle_en_souligne` : aucune clé en `_` dans les données livrées ;
  - `chaque_script_de_la_page_se_charge` : chaque script de la page passe `node --check`, sans nom déclaré deux fois ;
  - `la_page_ne_coupe_aucun_texte` : le CSS de la page ne cache aucun texte ;
  - `chaque_type_du_glossaire_existe` : chaque type que « Le glossaire » cite est déclaré dans un des fichiers du code
    que sa ligne nomme.
- **Un texte ne devient un jour, un mois ou un instant qu'à sa porte**, et c'est la visibilité qui le garde : les
  lectures d'un texte (`Mois::lire`, `Jour::du_texte`, les `lire` de `socle::paris`) sont privées à `socle`. Hors de
  `socle`, il entre par la forme qui le porte (`Deserialize` d'une charge, d'un fichier, d'une pièce), ou par une porte
  qui nomme son origine dans son type, une par origine : le nom d'un fichier ou d'un dossier lu sur le disque
  (`socle::fichiers::nommes_d_un_mois`), un argument du processus (`socle::ligne::Argument` : seul `socle::ligne`
  lit les arguments du processus et en fabrique, les parseurs de la ligne de commande les reçoivent), une zone d'une
  pièce lue, fabriquée avec le format que sa norme donne à cette zone (`socle::piece::Zone`, un type par format dans
  `socle::piece::format`, que le lecteur de chaque pièce passe : la lecture typée de ce qu'on lit, pas une preuve
  d'origine), la clé d'une lecture notée dans une empreinte (`socle::fichiers::CleDeLecture`, et sa `LectureDatee`). Une charge dit son mois
  par sa forme : la déclaration d'un mois se désigne par `mois`, jamais par un chemin (`routes::depots::resoudre`).

## Compiler peu

Le découpage en crates sert aussi le temps : on code, et on compile après. Pendant le travail, `cargo check` seulement
(types, quelques secondes) ; à l'arrivée de chaque lot, l'orchestrateur compile ce que le lot touche et ce qui en
dépend, et lance les tests de ces crates seules (plus le rejeu des fictives si le lot touche au calcul ou aux
déclarations) ; la suite complète, **une fois avant chaque PR**. Compiler toutes les trente secondes ne coûte que du
temps ; ne compiler qu'à la fin laisse s'empiler les rouges. La règle de travail est dans `AGENTS.md` (« Une
seule vérification, à l'arrivée », « On compile peu »).

## Le glossaire

**Une notion, un type ; une façon d'écrire n'est pas un type** : elle se dit dans la forme du fichier qui la porte (la
DSN, le journal, la saisie, l'UTC du filet), et son lecteur rend le type de la notion. Un type neuf est une notion de
ce glossaire, sinon Léandre décide. Des notions, pas des fonctions. Les chemins sont relatifs à `atelier/sources/`.
« À unifier (#419) » : le code a encore plusieurs types pour la notion, le type nommé est la cible. « À créer
(#419) » : la notion circule encore en `String`, `Decimal` ou entier nu.

**Le temps**

| notion | son type | déclaré dans |
|---|---|---|
| instant | `Moment` | `socle/src/paris.rs` |
| instant ou jour | `InstantOuJour` | `socle/src/paris.rs` |
| jour | `Jour` | `socle/src/date.rs` |
| mois | `Mois` | `socle/src/mois.rs` |
| mois ou jour | `MoisOuJour` | `socle/src/mois.rs` |
| année | `Annee` ; à unifier (#419) : des `i32` | `socle/src/mois.rs` |
| période | à créer (#419) : `Periode` | — |
| durée de droit | `droit::maintien::Duree` | `droit/src/maintien.rs` |
| durée mesurée | `std::time::Duration` ; à unifier (#419) : `envois::ligne::Durees` | la bibliothèque standard ; `envois/src/ligne.rs` |

**Le détail**

- **instant** : un point du temps, à la nanoseconde ; aussi « à la minute près ». Ses écritures (à la seconde, à la
  minute, la saisie, la française, l'UTC du filet, le bloc de la plate-forme DSN, les secondes locales) sont les modules
  de `socle::paris::ecrit`, dits par l'attribut serde du champ ; « pas encore » : `Option<Moment>`.
- **instant ou jour** : l'instant d'un geste, ou le jour seul (un bilan qui ne dit que sa date, un geste de scénario dit
  d'un jour, une obligation d'un arrêt dite faite ou une connaissance sans heure). Écrit en un champ
  (`AAAA-MM-JJTHH:MM:SS` ou `AAAA-MM-JJ`), ou en deux, le jour et l'heure `HH:MM` (`socle::paris::ecrit::jour_et_heure` :
  la fiche de suivi des obligations d'un arrêt et ce que ses routes rendent).
- **jour** : un jour civil ; les marqueurs de `socle::piece::format` sont des formats de zone.
- **mois** : un mois civil, clé d'une paie ; `Graphie` dit comment s'écrit son nom.
- **mois ou jour** : le mois d'un acte ou le jour d'un événement.
- **année** : une année civile. Écrite en texte (`"2025"`, la clé de `paies_annuelles`), ou en nombre là où la forme le
  dit (`socle::mois::en_nombre` : le nombre de paies parti chez l'éditeur, ce que le serveur du filet lit et rend).
- **période** : deux jours bornes, du … au … ; aujourd'hui `(Jour, Jour)`, `documents::etats_paie::Periode` et d'autres.
- **durée de droit** : des jours ou des mois de calendrier.
- **durée mesurée** : un temps écoulé ; `envois::ligne::Durees`, à unifier, est des millisecondes : une écriture.

**L'argent et les quantités**

| notion | son type | déclaré dans |
|---|---|---|
| montant | à créer (#419) : `Montant` | `socle` |
| taux | à créer (#419) : `Taux` | — |
| quantité | à créer (#419) : `Heures`, `Cachets`, `Jours` | — |
| nombre lu | pas une notion : `socle::forme::Court`, `Nombre`, `NombreEcrit`, `NombreLu` | `socle/src/forme.rs` |

**Le détail**

- **montant** : une somme en euros, au centime ; aujourd'hui `Decimal`, `engagement::paie_ecrite::Montant`,
  `droit::fiche::Montant` ; `engagement::ligne::Montant` est une ligne au montant, à renommer. Une réponse qui rend un
  montant ou une quantité en texte garde le `Decimal` (au centime : `socle::num::q2`) et l'écrit par
  `socle::ecriture::decimal_en_texte`.
- **taux** : un pourcentage appliqué à une base ; aujourd'hui `Decimal`, `droit::forme_bareme::Taux` (une forme) ; le
  taux de PAS est sa notion à lui, `droit::pas::TauxDePas`.
- **quantité** : des heures, des cachets, des jours : trois grandeurs indépendantes ; aujourd'hui `Decimal`, `i64`,
  `engagement::ligne::Chiffre`.
- **nombre lu** : un nombre tel qu'un fichier l'écrit ; ce sont des écritures, lues à la porte.

**Les identifiants**

| notion | son type | déclaré dans |
|---|---|---|
| NIR | `droit::fiche::Nir` | `droit/src/fiche.rs` |
| NTT | `Ntt` | `declarations/src/ntt.rs` |
| identifiant d'un individu | `IdentifiantDIndividu` | `declarations/src/ntt.rs` |
| case d'identité | `CaseDIdentite` | `declarations/src/changements_individu.rs` |
| SIREN, NIC, SIRET | `Siren`, `Nic`, `Siret` | `droit/src/fiche.rs` |
| IBAN, BIC | `Iban`, `Bic` | `droit/src/fiche.rs` |
| numéro de contrat | `NumeroContrat` | `engagement/src/numeros.rs` |
| clé d'une paie versée | `CleDePaieVersee` | `engagement/src/importe.rs` |
| numéro de bulletin | `NumeroDeBulletin` | `engagement/src/numeros.rs` |
| numéro d'objet spectacle | `NumeroObjetSpectacle` | `droit/src/fiche.rs` |
| IDCC | `Idcc` | `droit/src/fiche.rs` |
| identifiant d'organisme | `IdentifiantDOrganisme` | `droit/src/organismes.rs` |
| code caisse | `CodeCaisse` | `droit/src/organismes.rs` |
| salarié | `IdSalarie` | `droit/src/contexte.rs` |
| salarié désigné | `SalarieDesigne` | `droit/src/contexte.rs` |
| spectacle | `IdSpectacle` | `droit/src/contexte.rs` |
| compagnie | `IdDeCompagnie` | `compagnie/src/installation.rs` |
| couleur d'une compagnie | `CouleurDeCompagnie` | `compagnie/src/installation.rs` |
| PCS-ESE | `CodePcsEse` | `droit/src/fiche.rs` |
| code emploi déclaré | `CodeEmploiFt` | `droit/src/fiche.rs` |
| matricule | `Matricule` | `droit/src/fiche.rs` |
| licence | du texte : la norme ne lui donne qu'une longueur | — |
| nationalité déclarée | `CodeNationalite` | `droit/src/fiche.rs` |
| code NAF, code postal | `CodeNaf`, `CodePostal` | `droit/src/fiche.rs` |
| pays | `CodePays` | `socle/src/pays.rs` |
| lieu de mission | `LieuDeMission` | `droit/src/missions.rs` |
| lieu d'une ligne de frais | `LieuDeFrais` | `engagement/src/ligne.rs` |
| monnaie | `CodeMonnaie` | `droit/src/missions.rs` |
| commune | `CodeInsee` | `droit/src/fiche.rs` |
| courriel | `Courriel` | `droit/src/fiche.rs` |
| code de demande | `CodeDeDemande` | `documents/src/fiche_questionnaire.rs` |
| identifiant de courriel | `IdDeCourriel` | `envois/src/envoi.rs` |
| courriel envoyé | `envois::envoi::Message<Pour>` | `envois/src/envoi.rs` |
| expéditeur d'un courriel lu | `envois::courriel_lecture::Expediteur` | `envois/src/courriel_lecture.rs` |
| version du logiciel | `compagnie::maj::VersionDuLogiciel` | `compagnie/src/maj.rs` |
| échec d'une mise à jour | `EchecDeMiseAJour` | `compagnie/src/maj.rs` |
| système | `Systeme` | `compagnie/src/maj.rs` |
| clé d'archive | `CleDArchive` | `compagnie/src/maj.rs` |
| état du bridage | `EtatDuBridage` | `compagnie/src/bridage.rs` |
| sorte d'une copie | `SorteDeCopie` | `compagnie/src/installation.rs` |
| clé d'incident | `CleDIncident` | `compagnie/src/incidents.rs` |
| canal d'un incident de la page | `CanalDeLaPage` | `compagnie/src/incidents.rs` |
| origine d'un appel | `OrigineDeLAppel` | `compagnie/src/diagnostic.rs` |
| numéro de poste | `NumeroDePoste` | `compagnie/src/incidents.rs` |
| secret | `Secret` | `socle/src/secret.rs` |
| numéro de retour | `NumeroDeRetour` | `compagnie/src/editeur.rs` |
| identifiant d'un message | `IdDeMessage` | `compagnie/src/problemes.rs` |
| étiquette de caviardage | `EtiquetteDeCaviardage` | `compagnie/src/incidents.rs` |
| identifiant de la DILA | `IdDila` | `atelier/src/releve/entrepot_dila.rs` |
| livraison de la DILA | `LivraisonDila` | `atelier/src/releve/entrepot_dila.rs` |
| état d'un texte de la DILA | `EtatDuTexteDila` | `atelier/src/releve/entrepot_dila.rs` |
| type d'une capture | `TypeDeCapture` | `compagnie/src/problemes.rs` |
| rubrique de la norme | `RubriqueDeLaNorme` | `droit/src/natures.rs` |
| nom de fichier | `NomDeFichier` | `socle/src/fichiers.rs` |
| chemin relatif | `CheminRelatif` | `socle/src/fichiers.rs` |
| empreinte | `Empreinte` | `socle/src/fichiers.rs` |
| clé de lecture | `CleDeLecture<S, L, P>` | `socle/src/fichiers.rs` |
| valeur lue | `ValeurLue` | `socle/src/fichiers.rs` |
| empreinte d'une saisie | `EmpreinteDeSaisie` | `socle/src/fichiers.rs` |
| empreinte courte | `EmpreinteCourte` | `socle/src/fichiers.rs` |
| empreinte MD5 | `EmpreinteMd5` | `envois/src/drive.rs` |
| identifiant de flux | `IdDeFlux` | `envois/src/retours.rs` |
| identifiant d'un envoi | `IdDEnvoi` | `envois/src/journal.rs` |
| nature d'un retour | `NatureDuRetour` | `envois/src/retours.rs` |
| canal des retours | `CanalDesRetours` | `envois/src/releve_retours.rs` |
| plate-forme de net-entreprises | `EnvironnementNetEntreprises` | `envois/src/releve_retours.rs` |
| verdict | `Verdict` | `envois/src/retours.rs` |
| compte rendu d'un test de dépôt | `CompteRenduDuTest` | `envois/src/retours.rs` |
| statut d'un retour | `StatutDuRetour` | `envois/src/retours.rs` |
| catégorie d'une anomalie | `CategorieDAnomalie` | `envois/src/retours.rs` |
| type d'un rapport de dépôt | `TypeDeRapport` | `envois/src/retours.rs` |
| codes d'un compte rendu | huit listes de codes, nommées dans le détail | `envois/src/retours.rs` |
| type d'un paramètre NEOReS | `TypeDeParametreNeores` | `droit/src/contexte.rs` |
| identifiant Drive | `IdDrive` | `envois/src/drive.rs` |
| adresse | `Url` | `socle/src/garde_reseau.rs` |
| dépôt des mises à jour | `DepotDesMisesAJour` | `compagnie/src/maj.rs` |
| version de la norme | `VersionDeLaNorme` | `droit/src/natures.rs` |
| fraction | `Fraction` | `droit/src/natures.rs` |
| numéro d'ordre | `NumeroDOrdre` | `droit/src/natures.rs` |
| identifiant de déclaration | `IdentifiantDeDeclaration` | `droit/src/natures.rs` |
| code risque AT/MP | `CodeRisqueAt` | `droit/src/bareme.rs` |
| identifiant du taux de PAS | `IdentifiantDuTauxPas` | `droit/src/pas.rs` |
| clé d'un contrat d'organisme | `CleDeContrat` | `droit/src/organismes.rs` |
| accord de salaires | `AccordDeSalaires` | `droit/src/minima.rs` |
| classe d'un échec noté | `ClasseNotee` | `socle/src/echec.rs` |

**Le détail**

- **NIR** : le numéro de sécurité sociale ; treize caractères, saisi il se range sans aucun blanc et en capitales
  (`Nir::rangee`). Celui d'une pièce importée se lit par `Nir::d_une_piece` (hors forme : aucun), coupé de sa clé par
  `Nir::et_sa_cle` ; le sexe qu'il dit, `Nir::sexe`, et celui d'une fiche, `ChampsSalarie::sexe` ; sa clé, `CleNir`
  (`droit/src/fiche.rs`), celle d'un `Nir` par `CleNir::du_nir`. Que portent la fiche, l'identité qu'une DSN partie a
  déclarée, une DSN lue, une AEM, une AER, un certificat, un bulletin importé (après sa porte), la paie versée d'un
  engagement importé, les retours ; un NIR en cours de saisie reste son texte.
- **NTT** : le numéro technique temporaire d'un individu sans NIR (S21.G00.30.020). Il se fabrique pour un salarié
  (`ntt::numero`, contrôlé avec le SIREN) ou se lit dans une DSN (`Ntt::lu`, sa forme seule), ou dans un retour de la
  norme (`Ntt::lire`, strict : RO.B009.R002 de NEOReS). Que portent l'identité qu'une DSN partie a déclarée
  (`changements_individu::Declaree`), l'individu d'une DSN lue et l'identité en écart d'un compte rendu NEOReS
  (`envois::retours::IdentiteNeores`, à côté du NIR déclaré, R001).
- **identifiant d'un individu** : ce qui identifie un individu dans une DSN : son NIR, ou, sans NIR, son NTT (« Un
  individu s'identifie par son NIR **ou** par son NTT »). `Nir(Nir)`, `Ntt(Ntt)`, écrit dans aucun fichier, trié NIR
  d'abord ; celui d'un individu d'une DSN lue, `IndividuDeclare::identifiant` (le NIR, sinon le NTT). Que portent
  l'effectif que le simulateur des organismes compte et le contrôle des doublons (CCH-14).
- **case d'identité** : un des quatre identifiants que le bloc « Changements individu » reporte quand il change : NIR,
  nom de famille, prénoms, date de naissance (S21.G00.31.008 à .011). Une liste de la norme : `TOUTES` dans l'ordre du
  bloc 31, sa rubrique (`r31`), son libellé, la case de la fiche qui la porte ; sa valeur telle que le bloc 30 l'écrit
  est privée au fichier.
- **SIREN, NIC, SIRET** : l'entreprise, l'établissement, les deux.
- **IBAN, BIC** : un compte bancaire ; leurs contrôles, `socle/src/iban.rs`.
- **numéro de contrat** : S21.G00.40.009 : il se lit, ne se recalcule pas. Ni `Deref` ni comparaison à un texte : il se
  compare à un numéro, s'imprime par `Display` (`numero_dit` quand il peut manquer ; le contrat d'un salarié dans un
  refus, `numeros::le_contrat`), s'écrit par `as_str` à la porte ; celui qu'imprime une pièce d'autrui se lit par
  `d_une_piece`.
- **clé d'une paie versée** : ce qui reconnaît une paie versée d'un engagement importé d'un import à l'autre, et ce qui
  l'a déclarée : le numéro de contrat qu'une DSN déposée déclare ; sinon un bulletin seul, par le numéro qu'il imprime
  (le texte d'autrui), ou par sa copie et sa page. `Dsn(NumeroContrat)`, `Bulletin(String)`,
  `Copie { fichier: NomDeFichier, page }`, écrite avec son origine (`{"dsn": …}`, `{"bulletin": …}`,
  `{"copie": {…}}`) ; ce que la carte d'un engagement importé en montre, `dite()` ; que porte `PaieVersee.contrat`.
- **numéro de bulletin** : le numéro qu'un bulletin du logiciel porte, `N/AAAA` : son rang dans la suite de l'année, puis
  l'année. `numeroter` en donne un, et le départ des documents d'un mois sur la suite qu'il réserve
  (`routes::gestes::depart::fabriquer_documents`) ; l'aperçu d'un bulletin en montre un (`du_rang`) ; il se lit
  strictement. Que portent la paie, sa forme écrite, la paie écrite (`bulletin`, `rectifie`), le bulletin imprimé et son
  rectificatif, le dernier état déclaré d'un contrat ; celui qu'un bulletin d'autrui imprime est du texte.
- **numéro d'objet spectacle** : S21.G00.40.050, natures 02 et 10 ; l'année de sa demande, ses deux premiers chiffres
  (`annee_de_la_demande`).
- **IDCC** : l'identifiant d'une convention collective : la notion « convention ». Quatre chiffres, il s'écrit et se trie
  comme son texte ; la clé des tables par convention (minima, clauses, organismes, prévoyance, emplois, barème, plan
  comptable) ; une convention que le moteur nomme est une constante (`Idcc::du_moteur` : `IDCC_3090`…,
  `clauses::IDCC_CCNEAC`). Dans l'atelier, le suivi KALI d'une source, la fiche d'une convention que la veille garde, un
  changement de l'entrepôt KALI, l'IDCC que `releve-conventions` prend ; `minima::Convention`, `clauses::Convention`,
  `forme_bareme::Convention` sont des formes de fichiers.
- **identifiant d'organisme** : l'organisme de protection sociale que la DSN paie (S21.G00.20.001), d'une cotisation
  (81.002), d'une adhésion de prévoyance (15.002) : le SIRET de l'Urssaf, de France Travail, de la caisse Congés
  Spectacles, le code d'une institution (`P0983`), `DGFIP`. Que portent un versement de `organismes-base.json`, un
  contrat qu'un texte exige, une adhésion, la `org` d'une cotisation DSN du barème, les lignes 81 et les ordres de
  paiement de la DSN, ce que la lecture d'une DSN garde des blocs 15 et 20, l'organisme et le délégataire qu'un retour
  de la plate-forme ou un compte rendu d'organisme complémentaire nomme (`envois::retours`) ; `AUDIENS`,
  `SIRET_FRANCE_TRAVAIL`, `DGFIP` sont des constantes. Le code caisse d'un mois sans individu (S20.G00.08.001) est une
  autre notion.
- **code caisse** : l'organisme qui n'a plus d'individu rattaché, dans une DSN sans individu (S20.G00.08.001),
  « X [2,6] » de la table ODRT : Agirc-Arrco 88, France Travail 93, Congés Spectacles 97 ; que porte `sans_individu` de
  `organismes-base.json`.
- **salarié** : la clé de sa fiche.
- **salarié désigné** : le salarié tel qu'une personne ou une charge le désigne : la clé de sa fiche, son nom, son
  prénom, son NIR. Que portent les charges des imprévus et des obligations d'un arrêt ; il se résout en `IdSalarie` à un
  seul endroit, `Contexte::cle_du_salarie`.
- **spectacle** : la clé de sa fiche.
- **compagnie** : l'identifiant d'une compagnie de l'installation, celui de sa tuile. `Reelle`, `Cie(NomDeFichier)`,
  `Fictive(NomDeFichier)`, écrit `reelle`, `cie:<dossier>`, `fictive:<dossier>`. Que portent une tuile de l'accueil, les
  charges de l'accueil (couleur, remise à zéro, compagnies à garder), une compagnie gardée, le tableau de
  l'administrateur, le choix d'une tuile dans la fenêtre.
- **couleur d'une compagnie** : la couleur qu'une compagnie choisit pour sa tuile et sa page. `#RRGGBB`, gardée telle que
  lue ; `poser_couleur` l'écrit en majuscules ; celle de la charte quand rien n'est posé. Que portent
  `config/affichage.json`, une tuile, la charge et la réponse de `compagnie-couleur`, le tableau de l'administrateur. La
  couleur d'une pastille (`socle::ecran`) est une autre notion.
- **PCS-ESE** : la profession et catégorie socioprofessionnelle d'un emploi (S21.G00.40.004), « X [4,4] » de la table de
  l'Insee. Que portent la fiche du salarié, la carte, l'engagement et sa paie écrite, le métier du catalogue, ce qu'un
  bulletin importé imprime, ce que la lecture d'une DSN garde du bloc 40 (`declarations::dsn_lue::EmploiDeclare`), le
  contrat de la liste nominative d'une régularisation CASC-SVP.
- **code emploi déclaré** : le complément PCS-ESE d'un emploi (S21.G00.40.005), « X [2,6] » : pour le spectacle, un code
  de la nomenclature des emplois du spectacle, « 999SPT » pour un emploi permanent, « 50 », « 51 » (CCH-18) ; il ne
  nomme pas toujours un métier. Que portent la fiche du salarié, la carte, l'engagement et sa paie écrite, ce qu'un
  bulletin importé imprime, ce que la lecture d'une DSN garde du bloc 40 ; le métier qu'il nomme : `emplois::metier` ;
  un code d'un métier se déclare tel quel (`From<&CodeEmploi>`).
- **matricule** : le numéro d'un salarié dans la compagnie (S21.G00.30.019, « X [1,30] », que l'employeur choisit). Que
  portent la fiche du salarié (`code_salarie`), l'identité qu'une DSN partie a déclarée
  (`changements_individu::Declaree`), l'individu d'une DSN lue ; ce qu'un bulletin d'autrui imprime reste son texte.
- **pays** : un code ISO 3166 à deux lettres, que porte aussi le pays d'une ville de la table des missions
  (`villes.json`).
- **lieu de mission** : un pays, ou une ville que la table DGFiP des indemnités de mission à l'étranger distingue de son
  pays (« NY », New York). Deux lettres capitales, le `code_pays` de `indemnites.csv` et de `change.csv`, les clés de
  `villes.json` ; il s'écrit et se trie comme son texte. Que portent les tables lues, la clé `mission <lieu> <jour>`
  d'une empreinte de paie (`socle::fichiers::LectureDatee`, générique sur le lieu) ; celui d'une ligne de frais à
  l'étranger (`LieuDeFrais::Etranger`), que `missions::lieu` trouve dans la table.
- **lieu d'une ligne de frais** : où une nuitée ou un repas a lieu hors de la métropole (le choix « Lieu » ; absent : la
  métropole) : un territoire hors métropole, ou un lieu à l'étranger. `HorsMetropole(TerritoireHorsMetropole)` (`dom`,
  `pacifique`, `monaco`, les clés de `frais_hors_metropole` du barème, et `taaf`, sans forfait) ou
  `Etranger(LieuDeMission)` ; écrit par son texte, lu à sa porte (une clé de territoire, sinon un code de lieu). Que
  portent une ligne de frais (`territoire`), la ligne qu'un appelant donne, ce que le choix « Lieu » propose
  (`TerritoireHorsMetropole::TOUS`).
- **monnaie** : la monnaie d'une indemnité de mission à l'étranger (« EUR », « USD ») ; trois lettres capitales,
  `code_monnaie` et `monnaie_source` des tables DGFiP ; que porte une indemnité d'un jour (`Indemnite`, son empreinte).
- **commune** : son code INSEE (versement mobilité, S21.G00.81.005).
- **courriel** : une adresse de courriel. Que portent la fiche, et au journal des envois l'expéditeur, les adresses
  servies (`partis`), celle où un envoi a échoué (`echec_a`), celle d'une demande (`adresse`) et celle d'où sa réponse
  est venue (`de`) ; l'expéditeur d'un courriel **reçu** est un `Expediteur`.
- **code de demande** : le code qu'une demande de signature ou de fiche porte, et qu'on cherche dans la réponse : `SIG-`
  ou `FIC-` (la sorte de demande, `SorteDeDemande`) et six signes de `ALPHABET_CODE`. Seul `code_au_hasard` le fait, il
  se lit strictement. Que portent un contrat parti, un avenant parti, ce qui en revient, le contrat contresigné, une
  demande de fiche et sa réponse, une signature attendue et relevée ; une réponse reçue est le texte d'un tiers, on y
  cherche `texte()`.
- **identifiant de courriel** : le `Message-ID` d'un courriel que le logiciel envoie, `<gauche@droite>` (RFC 5322,
  § 3.6.4). Écrit par `nouveau_message_id` et la boîte simulée, rendu par l'envoi (`Courriel::envoyer`). Que portent un
  contrat parti, une demande de fiche, une signature ou une fiche attendue (`envoyes`) ; un identifiant **reçu** (l'UID
  d'un message de la boîte, un en-tête) est le texte d'un tiers, qui se compare à `texte()`.
- **courriel envoyé** : un courriel qu'un geste envoie, et celui à qui il s'adresse, que l'envoyeur retrouve après
  l'envoi. `Pour` du type de ce qu'il est : `IdSalarie` (les documents du mois, le dossier, les contrats à signer et le
  contrat contresigné), le nom court de l'organisme (la régularisation CASC-SVP : celui du `.eml` gardé), `()` quand
  personne n'est à retrouver (la fiche demandée, la réponse d'une personne fictive) ; l'envoi ne le lit pas.
- **expéditeur d'un courriel lu** : une adresse, rien, ou un texte qui n'en est pas une.
- **version du logiciel** : le numéro d'une publication, des nombres séparés de points. Il garde le texte lu et s'écrit
  par lui (`2026.11.01` reste `2026.11.01` : `VERSION`, `derniere.json`, `prete.json`, les noms des archives,
  `maj/<version>/`) ; deux versions sont la même si elles s'écrivent pareil, « plus récente » compare les nombres
  (`plus_recente_que`) ; `lue()` la dit `JJ.MM.AAAA`. `VERSION` ne se lit que par `maj::courante` ; celle d'un incident
  est vide quand `VERSION` ne se lit pas.
- **échec d'une mise à jour** : ce qui a fait échouer une recherche de mise à jour, dit en un mot à l'écran : hors ligne,
  introuvable, dépôt illisible, signature refusée, archive altérée, refusée au contrôle, réglage illisible. Une liste du
  logiciel ; `mot()`.
- **système** : le système dont une archive porte les programmes, et ceux d'une installation : `windows`, `linux`,
  `mac`, le nom du dossier de ses programmes dans `logiciel/`. Une liste du logiciel : `TOUS`, `DE_CE_PROGRAMME` (celui
  qu'on a compilé), `lu()` (« Windows », « Linux », « macOS »). Que portent la liste `systemes` de l'archive de tous dans
  `derniere.json`, ce que `portable-completer` ajoute ou ne trouve pas, le système d'un fichier de `logiciel/`. Ce que
  `diagnostic` dit de la machine (`macos`, la constante du compilateur) est une description.
- **clé d'archive** : la clé d'une archive de mise à jour publiée dans `derniere.json` : celle d'un système, ou
  `tout-systeme` (le dossier portable, qui les porte tous). `Du(Systeme)` ou `DeTous`, écrite par son texte ; que
  portent les archives prises d'une version prête (`prete.json`), les noms des archives d'une version.
- **état du bridage** : bridé ou débloqué : ce que disent `.edition.json` et `logiciel/EDITION.json`. Une liste du
  logiciel : écrit `bride`, `debloque` ; `demo` et `complete` se lisent aussi (le contrat avec les installations
  posées) ; que portent la machine d'un incident (`edition`) et sa ligne en mots.
- **sorte d'une copie** : ce qu'une copie de compagnie est : une fictive (ses envois sont joués), la compagnie d'essai
  (ses dépôts partent en test), une copie de développement. Une liste du logiciel ; le marqueur
  (`COPIE-DE-DEVELOPPEMENT.json`) ne l'écrit pas à part : elle se lit au début de sa phrase `copie_de` (« compagnie
  fictive … », « compagnie d'essai … »), une fois, par `Marqueur::sorte`.
- **clé d'incident** : ce qui fait que deux incidents sont le même : la nature, l'endroit, le genre. Les douze premiers
  hexadécimaux minuscules du SHA-256 de `nature|ou|quoi`, que seul `incidents::empreinte` fait, lue strictement ; pas
  une `Empreinte`. Que portent une ligne du journal des incidents, un paquet, ce qui part chez l'éditeur, les clés
  reçues et écartées que le serveur du filet rend.
- **canal d'un incident de la page** : par où la page a vu casser (`js/tuyau.js`) : une erreur, une promesse rejetée,
  une ressource absente. Une liste du logiciel, écrite `erreur`, `promesse`, `ressource` ; que portent le rapport de la
  page (`incident-navigateur`) et le détail de l'incident.
- **origine d'un appel** : d'où vient l'appel d'une route servie par la fenêtre : la page, ou le fond (la veille, le
  filet, l'état des mois). Une liste du logiciel, écrite `page`, `fond` dans le fil du diagnostic.
- **numéro de poste** : le numéro que le serveur du filet donne à ce poste (`POST /poste`) ; 32 hexadécimaux minuscules
  (`RE_POSTE` du serveur). Que portent `config/diagnostic.json`, l'identité rendue par le serveur, le client du filet, ce
  qui part (`poste`) et une année de paies partie. La clé du poste, à côté, est un secret.
- **secret** : un mot de passe (du logiciel, d'un organisme, de la boîte, de l'administrateur), un jeton (d'une session
  net-entreprises, de Google, de la fenêtre), une clé (celle du poste chez le serveur du filet, le secret de
  l'application Google, la graine de publication). Opaque ; `{:?}` écrit « [secret] », ni `Display` ni `Serialize` ; il
  se lit d'une charge, d'une réponse, d'une saisie ; `devoiler()` aux seules portes qui l'emploient (le transport, le
  coffre, la page servie à la fenêtre), `egal_a` le compare en temps constant. Là où le logiciel en garde un dans un
  fichier qu'il écrit (la séance gardée, le jeton Google scellé, la clé du poste dans `config/diagnostic.json`, la
  charge d'un dépôt groupé), l'écriture est nommée (`socle::secret::ecrit`, `ecrit_option`). Les enveloppes et les
  secrets scellés du coffre sont du chiffré en base64, pas des secrets ; le nom de l'algorithme de la dérivation reste
  le texte lu (`deriver` refuse un autre que scrypt en le nommant).
- **numéro de retour** : le numéro que le serveur du filet donne à un message au développeur, celui que l'utilisateur
  lit ; `R-` et six hexadécimaux majuscules (`RE_NUMERO_RETOUR` du serveur). Que portent la réponse à `POST /retour`, les
  réponses relevées, la boîte des messages ; le ticket qui suit un message est autre chose (son adresse, une `Url` ; son
  numéro, un entier).
- **identifiant d'un message** : l'identifiant d'un message au développeur, propre au poste, qui nomme son fichier
  d'attente. `AAAAMMJJTHHMMSS-` et douze hexadécimaux minuscules, la forme que seul `nouvel_id` fait, lue strictement ;
  que portent la boîte des messages (`journal/signalements.json`), le nom de `journal/signalements/<id>.json`, la
  réponse de `signalement-envoyer`.
- **étiquette de caviardage** : ce que le caviardage a retiré d'un texte, jamais ce que c'était (`[NIR]`, `<NOM>`) :
  nom, prénom, NIR, date de naissance, adresse, numéro Congés Spectacles, courriel, téléphone, IBAN, code salarié
  (l'import) ; BIC, jeton de session (les incidents). Une liste du logiciel : écrite par son mot en capitales (`NOM`,
  `CS`, `JETON`), elle se trie comme lui ; `dit()`, ses mots pour la personne. Que portent une ligne du journal des
  incidents, un paquet, ce qui part chez l'éditeur (`caviarde`), le compte de ce que l'import a enlevé d'un bulletin
  (`import::caviardage::Caviarde::enleves`).
- **identifiant de la DILA** : un texte, un article ou une convention des bases LEGI, KALI, JORF
  (« KALICONT000028157262 », « LEGITEXT000006072050 ») : sa famille (`LEGITEXT`, `LEGIARTI`, `KALITEXT`, `KALIARTI`,
  `KALICONT`, `JORFTEXT`) et douze chiffres. L'atelier seul : il s'écrit et se trie comme son texte ; un identifiant que
  l'atelier écrit en Rust, `IdDila::du_moteur`. Que portent le suivi KALI d'une source et une convention à relever
  (`sources.json`), le texte d'un code relevé (`releve::codes::textes`), les articles et les changements des entrepôts
  (`legi-entrepot/`, `kali-entrepot/`), un relevé (`releve.json`), les événements de la veille KALI.
- **livraison de la DILA** : une livraison quotidienne des bases LEGI et KALI, nommée par son estampille
  `AAAAMMJJ-HHMMSS` (`KALI_20260922-214912.tar.gz`), qui commence par son jour. L'atelier seul : elle s'écrit et se trie
  comme son estampille, `jour()` dit son jour. Que portent les livraisons appliquées d'un entrepôt et ses changements, la
  dernière livraison d'un relevé, ce que la veille KALI a lu.
- **état d'un texte de la DILA** : l'élément `ETAT` d'un article, d'un texte ou d'une convention (`VIGUEUR_ETEN`,
  `ABROGE`…). Une liste relevée des bases (les entrepôts et les pièces de `atelier/reference/sources/`), écrite par son
  nom ; un autre nom se refuse. `de_vigueur()` : les états dont le nom commence par `VIGUEUR`. Que portent un article
  de l'entrepôt KALI et ses changements, la fiche d'une convention et un texte modifié de la veille KALI.
- **type d'une capture** : le type d'image d'une capture jointe à un message au développeur : PNG ou JPEG. Une liste du
  logiciel, écrit `image/png`, `image/jpeg` ; que portent une capture de la page, celle qui part et celle que l'aperçu
  montre.
- **rubrique de la norme** : l'identifiant d'une rubrique de la DSN, « structure, groupe, bloc, rubrique »
  (`S21.G00.40.021`, cahier technique, chapitre 3, règles de nommage). Que portent la rubrique d'une liste de la norme
  (`ListeDeLaNorme::RUBRIQUE`), les clés des tables d'usages et de formats, les `codes`, `champs`, `absents` et
  `interdits` d'une nature de `contrats-base.json`, la rubrique d'une case de `champs-base.json`, ce que la lecture
  d'une DSN garde ; une rubrique que le moteur nomme se vérifie à la compilation (`droit::rubrique!`). Les rubriques que
  l'écrivain de la DSN passe à `dsn::Fichier::add` restent des littéraux.
- **nom de fichier** : le nom d'un fichier seul, sans son dossier : le fichier parti que le journal des envois note, le
  modèle d'un contrat, une table de la norme livrée, l'archive d'une version, la copie d'une pièce importée. Ni vide, ni
  `.` ni `..`, sans `/`, `\` ni caractère nul ; il se fait d'un chemin par `NomDeFichier::du`, qui refuse un chemin sans
  nom ; un nom que le moteur écrit en Rust, `NomDeFichier::du_moteur`.
- **chemin relatif** : un chemin sous une racine que son champ dit (la compagnie, le dossier de la sauvegarde) : la trace
  d'un dépôt, un fichier de la sauvegarde, une pièce du mois que la fenêtre ouvre, la copie d'un bulletin repris. Des
  `NomDeFichier` séparés par des barres droites sur tous les systèmes, ni vide ni avec une racine ; il se fait d'un
  chemin par `CheminRelatif::sous(racine, chemin)` ; la clé d'une lecture notée s'écrit avec la même écriture. Un chemin
  absolu du disque est un `PathBuf` ; rendu par une route, il s'écrit par `socle::fichiers::ecrire_chemin`.
- **empreinte** : le SHA-256 entier d'un contenu : une source déposée, une archive de mise à jour et les fictives
  (`derniere.json`, `.livraison.json`, `.livree.json`), un exercice clos, un état gardé. 64 caractères hexadécimaux
  minuscules ; elle ne se fait que par `Empreinte::des_octets` (ou `du_hachage`, un contenu haché par morceaux ;
  `du_fichier`), elle se lit par `Empreinte::lire` ; que porte aussi ce qu'une lecture notée valait
  (`ValeurLue::Empreinte`).
- **clé de lecture** : ce dont une paie dépend, une lecture que son calcul a notée : un fichier lu, un chemin dont il a
  demandé s'il existe ou s'il est un dossier, un dossier parcouru, la marque du code qui calcule, la date, aujourd'hui,
  les fiches d'une paie, les imprévus d'un salarié, une lecture datée (le taux de PAS, une indemnité de mission, les
  mensuels d'avant, les paies validées d'un mois, la saisie d'un mois). Générique sur le salarié, le lieu d'une mission
  et le spectacle (`droit::contexte::CleLue` : `IdSalarie`, `LieuDeMission`, `IdSpectacle`) ; le chemin qu'elle nomme,
  `CheminLu` (`racine:`, `code:`, `abs:`) ; une lecture datée, `LectureDatee<S, L>`. Écrite par son texte
  (`fichier racine:config/effectifs.json`, `fiches ["2026-11","FULLER",null]`), triée comme lui ; le registre
  (`socle::fichiers`, un par fil) garde le texte qu'il note et ne rend que des clés lues (`rendre`). Que porte
  l'empreinte d'une paie écrite (`engagement::paie_ecrite::Empreinte.entrees`, écrite par `socle::fichiers::lectures`).
- **valeur lue** : ce qu'une lecture notée valait : une empreinte, absent, illisible (et la raison), oui ou non, un jour,
  la marque du code, la saisie d'un mois (`ValeurLue::Saisie`, une `EmpreinteDeSaisie`), l'empreinte composée d'une
  donnée (le taux de PAS, l'indemnité de mission d'un jour). Écrite par son texte, relue selon la sorte de sa clé,
  strictement.
- **empreinte d'une saisie** : ce dont la saisie d'un mois est faite pour une paie : ce que le mois pose pour tous (le
  taux du cachet), et ses engagements valides (ceux d'un salarié, pour sa paie), deux empreintes ; la raison d'un
  recalcul dit laquelle a changé. `mois`, `engagements`, écrite `<mois>:<engagements>` et relue strictement ; seul
  `engagement::stock::empreinte_de_saisie` la fait. Que portent une lecture notée d'une paie, la saisie d'un mois que
  l'écran garde, les empreintes des saisies de l'année qu'un export compta note au journal des écritures
  (`EmpreinteNotee::Annee`).
- **empreinte courte** : les 16 premiers caractères du SHA-256 : ce que le journal des envois écrit d'un fichier parti et
  d'un modèle figé, une paie importée déjà comparée, une DPAE due, l'état d'un mois de l'horloge ; on n'en retrouve pas
  l'entière. Elle ne se fait que par `Empreinte::courte`.
- **empreinte MD5** : le `md5Checksum` que Drive publie d'un fichier, et celui que la sauvegarde et la restauration
  calculent pour s'y comparer ; 32 caractères hexadécimaux minuscules ; un `md5Checksum` vide se lit absent, à la porte.
- **identifiant de flux** : l'identifiant que la plate-forme donne à un dépôt (`idflux`), et par lequel elle range ses
  retours : celui de net-entreprises, celui du canal DPAE, le `FICTIF-…` du simulateur. Son fichier est la porte des
  retours : chiffres, lettres, `.`, `-`, `_`, de 1 à 50 caractères (guide de l'API DSN v6.8 ; la classe est une
  inférence). Que portent une ligne du journal des envois (`""` au fichier quand la plate-forme n'en a pas donné :
  `socle::forme::vide_si_aucun`), l'ouverture d'un dépôt réel, un retour listé, un dépôt listé, un bilan, la reprise ;
  `IdDeFlux::est_fictif` reconnaît un dépôt joué.
- **identifiant d'un envoi** : l'`id` qu'un envoi porte au journal des envois, et que `a` reprend pour ce qui lui revient.
  Privé au journal : l'empreinte courte du texte de la ligne, suivie de `-<rang>` (à partir de 2) quand une ligne au
  même texte la porte déjà ; la forme que le seul écrivain fait (`noter`), lue strictement.
- **nature d'un retour** : ce qu'est un retour d'organisme, des deux canaux : un accusé, un bilan, un compte rendu, une
  AER, le certificat des DPAE (« 10 », « 11 », « 46 », « 118 », « 41 »…). `droit::codes_fermes!` sur son texte : la
  liste du guide de l'API DSN v6.8 (l. 573-788, chaque code à sa ligne) et le retour 41 de la FAQ de l'API DPAE (p. 5).
  Que portent un retour listé, un retour rangé au journal des envois, son complément, l'incident d'un retour, le nom
  d'une archive (`archive_de_nature`) ; les natures qu'une compagnie reçoit ont leur fiche (libellé, organisme, format).
- **canal des retours** : le canal d'un retour d'organisme (règle 5) : la DSN sur net-entreprises, la DPAE sur l'Urssaf,
  services, mots de passe et archives distincts. Une liste du logiciel : `Dsn`, `Dpae` ; il ne s'écrit dans aucun
  fichier ; `dossier()` : ses pièces dans `retours/` ou `retours/dpae/`. Que portent la relève, l'archive d'une pièce, le
  journal des retours vus, l'incident d'un retour (sa nature au journal des incidents), la pièce que l'Historique relit.
- **plate-forme de net-entreprises** : la plate-forme à laquelle le client de la DSN parle : la production, ou la
  plate-forme d'essai de l'éditeur, où rien n'est déclaré (un dépôt réel n'y part pas). Une liste du logiciel :
  `Production`, `Editeur`, nommés `production`, `editeur` (`NOMS`, ce que la ligne de commande propose, lu à sa porte
  par `lire`) ; elle choisit les adresses et le service. Que portent le client de la DSN, le dépôt d'une DSN, la relève.
- **verdict** : ce que la pièce d'un retour dit d'un envoi, d'une déclaration ou des données analysées : OK, KO, ANO.
  `etat_type`, `envoi_etat_type` et `ok_ko` du schéma des bilans, RO.B003.R002 de NEOReS ; il se lit sans casse, à sa
  seule porte (`Verdict::lu`), et s'écrit en capitales. Que portent un bilan, une déclaration qu'il identifie, un
  rapport, un NEOReS, un retour d'organisme au journal des envois et l'état de la déclaration qu'une ligne reprise
  garde. Ce que la pièce en dit au journal (le verdict, les anomalies en phrases, les personnes d'une DPAE) :
  `ligne::VerdictDeLaPiece`.
- **compte rendu d'un test de dépôt** : ce que le test conforme d'un fichier dit à l'heure où on le lit : son bilan
  attendu, conforme, refusé (`attendu`, `ok`, `ko`, en minuscules). Que portent la répétition d'un fichier
  (`etats::Repetition`, `null` sans test conforme) et l'ouverture d'un dépôt réel au journal des envois
  (`ligne::Ouverture`), qui garde ce qu'il valait au départ du dépôt.
- **statut d'un retour** : ce que la liste des retours dit d'un retour (`flux.retour.statut`) : OK, KO, ANO, EN COURS,
  ANOBLOQUANTE (France Travail). `droit::codes_fermes!` sur son texte : guide de l'API DSN v6.8, l. 1733-1745, et « De
  la DSN à l'attestation employeur » v3.2, l. 235-243. Que portent un retour listé, un retour d'organisme au journal des
  envois ; le simulateur écrit OK, et KO au bilan d'anomalies d'une DSN refusée (guide v6.8, § 5.2.3) ; un retour joué
  se reconnaît à son flux (`IdDeFlux::est_fictif`).
- **catégorie d'une anomalie** : bloquante ou non, ce qu'un bilan dit d'une anomalie (`categorie`). `codes_fermes!` sur
  `categorie_anomalie_type` du schéma des bilans, l. 839-855 : « bloquant », « non-bloquant ». Que portent une anomalie
  en codes, une anomalie d'un rapport, l'anomalie d'un incident de retour (les bloquantes d'abord).
- **type d'un rapport de dépôt** : ce qu'est le bilan d'un dépôt (`rapport/@type`) : l'accusé d'enregistrement, le
  certificat de conformité, le bilan d'anomalies. `codes_fermes!` sur le guide de l'API DSN v6.8, l. 4825-4828 : AEE,
  CCO, BAN ; que porte un bilan. Un rapport d'un autre type n'est pas un bilan de dépôt, et l'Historique le lit sous son
  type tel qu'écrit.
- **codes d'un compte rendu** : ce que les comptes rendus NEOReS et des organismes complémentaires disent par un code, et
  dont l'Historique fait ses phrases. Chacun par `codes_fermes!` sur son texte, avec son libellé. NEOReS (cahiers
  techniques 2026.1.2 et 2027.1.0) : le déclencheur `DeclencheurNeores` (RO.B001.R001), le type de retour
  `TypeDeRetourNeores` (RO.B002.R004 : 02 annule et remplace), le type d'action `TypeDActionNeores` (RO.B004.R003), le
  résultat du contrôle `ResultatDuControleNeores` (RO.B004.R006, RO.B007.R009), le caractère substituable
  `SubstituableNeores` (RO.B004.R016, RO.B005.R005). CRM OC (schéma V1.1.4) : `TypeDeCrmOc` (`TypCRM`), `RetourGlobalOc`
  (`CodeRetourDecla`), `CriticiteOc` (`CriticiteRetour`).
- **type d'un paramètre NEOReS** : l'effectif qu'un compte rendu 118 communique (`RO.B008.R001` : A01 l'effectif de
  sécurité sociale, A02 à A05, A07), et la clé d'un effectif de `config/effectifs.json`. `codes_fermes!` sur la table
  que net-entreprises sert, `neores/gip-mds-neores-table-types-de-parametres-20230703.csv`, avec son libellé. Que portent
  un paramètre d'un NEOReS, un effectif mesuré d'un 118, les clés de `config/effectifs.json` (par `Annee`, puis par
  type).
- **identifiant Drive** : l'identifiant que Drive donne à un fichier ou à un dossier : ce qu'un listage rend, le dossier
  où l'on dépose, ce que l'index de la sauvegarde garde de chaque dossier. Un texte opaque, non vide (Google n'en publie
  pas la forme) ; lu à la porte de la réponse de Google ; `IdDrive::RACINE`, le dossier racine du compte (`root`). Le
  nom de la racine de la sauvegarde (`PuppetPaie`) est un nom, pas un identifiant.
- **adresse** : une adresse que le moteur appelle ou montre : un service (Google, net-entreprises, l'Urssaf, le serveur
  du filet, le dépôt des mises à jour), un texte (Légifrance, le CASC-SVP). Son fichier est la porte du réseau : elle
  commence par `http://` ou `https://`, rien d'autre n'est vérifié ; elle s'écrit et s'envoie au caractère près. Une
  adresse que le code compose, `Url::joindre` ; une que le moteur écrit en Rust, `Url::du_moteur` ; une adresse que rend
  un tiers (un lien du Drive, une redirection) se lit à sa porte. Une adresse de courriel est un `Courriel`.
- **dépôt des mises à jour** : d'où une installation reçoit ses versions (`maj/reglage.json`, `depot`), ou ce que la
  publication relit. `Adresse(Url)` ou `Dossier(PathBuf)`, choisi à sa lecture par le début du texte ; il s'écrit tel
  qu'on l'a lu.
- **version de la norme** : la version de la norme DSN d'un fichier (S10.G00.00.006 : `P26V01`, `P27V01`). Une liste de
  la norme, que porte `declarations::norme::Norme` ; une DSN lue d'une version non livrée n'en a pas.
- **fraction** : le numéro de fraction d'une déclaration (S20.G00.05.003), « nd » : la fraction n sur d, n au plus d ;
  « 11 » pour une entreprise non fractionnée. Que portent les options de la DSN mensuelle, la DSN lue, le journal des
  envois et le bilan d'un dépôt ; `Fraction::NON_FRACTIONNEE`.
- **numéro d'ordre** : le rang de constitution d'une déclaration (S20.G00.05.004, et RO.B000.R012 de NEOReS),
  `0|[1-9][0-9]*`, au plus quinze chiffres. Que portent les options des déclarations, la DSN lue, le journal des envois,
  les retours, les charges des signalements ; « aucun » est `None`, le suivant `NumeroDOrdre::suivant` ; écrit en nombre
  par `droit::natures::en_nombre` là où la forme le dit.
- **identifiant de déclaration** : la déclaration qu'un annule-et-remplace ou un annule vise (S20.G00.05.006) : sa date
  de constitution `JJMMAAAA` et son numéro d'ordre ; que portent les options des signalements, la DSN lue, les charges
  des signalements.
- **licence** : la licence d'entrepreneur de spectacles (S21.G00.40.049) reste du texte : la norme ne lui donne qu'une
  longueur (`X [1,100]`), aucun texte déposé ne dit sa forme.
- **nationalité déclarée** : la codification UE du salarié (S21.G00.30.013 : 01 France, 02 Union européenne, 03 EEE hors
  Union, 04 reste du monde), un choix de la fiche. Elle se déduit du pays de nationalité de la liste, ou se choisit pour
  un pays hors de la liste (`droit/src/nationalite.rs` : `codification`, `codification_ue`, la liste servie à la page,
  `PaysDeNationalite`). Que portent la fiche, la DSN, l'état du personnel.
- **code risque AT/MP** : S21.G00.40.040, `X [5,6]` de la table RAT : trois chiffres, deux caractères, et le « B » du
  taux bureau ; `SANS_NOTIFICATION` (999ZZ). Il s'écrit comme la DSN l'écrit ; l'arrêté de tarification l'écrit avec un
  point (`92.3AD` : `de_l_arrete`, `a_l_arrete`) ; une saisie se range sans point ni blanc, en capitales. Que portent la
  case du taux AT/MP, un risque de l'arrêté, le taux du barème, la DSN lue (hors forme : vide), la notification de la
  CARSAT (cinq signes, `avec_bureau`), ce que le simulateur annonce.
- **identifiant du taux de PAS** : S21.G00.50.008, `-1|0|[1-9][0-9]{0,17}` ; `SansCrm` (« -1 », un contrat court sans
  compte rendu) ou le numéro que la DGFiP donne, écrit comme lu. Que portent un taux reçu de la fiche, le compte rendu
  PAS relevé (strict), le taux qui vaut, le prélèvement d'un versement, la paie versée d'un engagement importé, la DSN
  lue (hors forme : vide).
- **clé d'un contrat d'organisme** : un contrat de `organismes-base.json` (`prevoyance_permanents`, `tsintermittents`),
  la CASC-SVP (`CASC`), une contribution de branche (`contribution_<nom>`, `de_la_contribution`). Une clé du logiciel,
  non vide ; que portent une adhésion saisie (`config/organismes.json`), un contrat attendu, la ligne de l'écran, la
  saisie d'`organismes-enregistrer`. Le contrat d'organisme lui-même est `droit::organismes::Contrat`.
- **accord de salaires** : l'`id` d'un accord de `minima-conventionnels.json`, que nomme une valeur du barème datée par
  lui (`accord`). Une clé du logiciel, non vide.
- **classe d'un échec noté** : ce que la mémoire des échecs dit d'un échec : sa classe, un refus ordinaire (`-`), une
  panique (`panique`) ; ce que le diagnostic montre.

**Les codes de la norme** : une liste fermée par rubrique (`droit::natures::ListeDeLaNorme`) : `NatureDeContrat`,
`MotifDeRecours`, `MotifDeRupture`, `ModaliteDuTempsDeTravail`, `BaseAssujettie`, `CotisationDEtablissement`,
`NatureDeLaDeclaration`, `TypeDeLaDeclaration`, `CodeCotisation81`… ; les choix d'une fiche (`StatutConventionnel`, `Civilite`, `Sexe`…)
aussi. Le CTP Urssaf (S21.G00.23.001) se lit dans la table datée de l'Urssaf, pas dans la norme : `CodeCtp`
(`droit/src/organismes.rs`), que portent une ligne de la table des CTP, une ligne du bordereau, les CTP de la réduction
générale et du régime local ; son format dans la table (A, B, C, D, E, F, K, R, V, Y, Z : guide Urssaf de la
régularisation, § 1.3), `FormatDeCtp` (même fichier), lu à la porte de la table, dit comment une ligne se valorise. La
fin du contrat : le type de préavis (S21.G00.63.001), `TypeDePreavis`, la transaction
en cours (62.008), `TransactionEnCours`, le mois de rattachement d'un FCTU (50.020), `RattachementDuVersement`
(`droit/src/preavis.rs`, `fctu-base.json` en dit les propriétés) ; le type d'une prime ou d'une indemnité
(52.001), `TypeDePrime` (`droit/src/type_de_prime.rs`), que portent la rubrique (`dsn_bloc52`), ses lignes de paie et
`permanents-base.json`. L'arrêt (bloc 60) : son motif (60.001), `MotifDArret`, le motif de la reprise (60.011),
`MotifDeReprise`, la subrogation (60.004), `SubrogationDeLArret` (`droit/src/arrets.rs`, `arrets-base.json` en dit
les propriétés ; la case « subrogation » d'un permanent, `fiche::Subrogation`, est une autre notion). Le motif d'une
suspension (65.001), `MotifDeSuspension` (`droit/src/permanents.rs`, ses propriétés : `ProprietesDeSuspension`). Le
contrat et son activité (`droit/src/natures.rs`, `contrats-base.json` dit celles de chaque nature) : l'unité de la
quotité (40.011), `UniteDeQuotite` ; la modalité de déclaration de la fin (62.017, le circuit dérogatoire),
`ModaliteDeDeclarationDeFin` ; le type d'une activité (53.001), `TypeDActivite`, et son unité de mesure (53.003),
`UniteDeMesure`, que portent aussi les activités d'une DSN lue. Le type d'un composant de base assujettie (79.001),
`TypeDeComposant` (`droit/src/base_assujettie.rs`), que porte la santé du barème (`composant_79`). Le qualifiant
d'assiette d'une ligne du bordereau (23.002), `QualifiantDAssiette` (`droit/src/organismes.rs`), que portent un CTP de
`organismes-base.json`, la réduction du barème et la ligne du bordereau. Le type d'un bénéficiaire externe de l'OETH
(13.002), `TypeDeBoethExterne` (`droit/src/cotisation_etablissement.rs`). L'envoi d'essai ou réel (S10.G00.00.005),
`EssaiOuReel`, que reprennent un bilan, un rapport, un NEOReS (RO.B000.R013) et un compte rendu d'organisme
complémentaire (`CodeEnvoi`), et la nature de l'événement déclencheur d'un amorçage (S20.G00.05.011), `NatureDEvenementDeclencheur`
(`droit/src/nature_declaration.rs`). Le code d'une taxe du bloc 44 (44.001), `CodeTaxe` (`droit/src/apprentissage.rs`) ;
son motif de non-assujettissement (44.004, table MNATA) est le choix de la fiche `MotifNonAssujettissement`. Le type
de rémunération (51.011), `TypeDeRemuneration` (`droit/src/natures.rs`), n'a que les quatre codes que nos natures
écrivent : une DSN lue d'un autre code laisse la case vide. La période d'affectation d'un composant de versement
(55.004, « AAAAPNN »), `PeriodeDAffectation` (`declarations/src/dsn.rs`) : le trimestre, le semestre ou l'année que le
moteur écrit. La nature du contrat déclarée (40.007), `NatureDuContratDsn`, et le dispositif de politique publique
(40.008), `DispositifDePolitiquePublique` (`droit/src/natures.rs`), que `contrats-base.json` dit par nature
(`Nature::nature_dsn`, `Nature::dispositif`) et dont le calcul lit le CDD de droit privé et le « non concerné »
(CPF-CDD, majoration du chômage). Le code d'option de prévoyance (70.004) reste du texte : « les valeurs de code sont
fournies par l'organisme de prévoyance » (`X [1,30]`).

**Le métier**

| notion | son type | déclaré dans |
|---|---|---|
| engagement | `Engagement` | `engagement/src/lib.rs` |
| contrat | à unifier (#419) : `ContratEcrit`, `ContratParti`, `ContratLu`, `ContratAtteste`, `ContratDeclare`… | — |
| acte d'un contrat parti | `envois::ligne::ActeDuContrat` | `envois/src/ligne.rs` |
| acte qu'une case exige | `declarations::champs::ActeExigeant` | `declarations/src/champs.rs` |
| marqueur | `droit::marqueurs::Marqueur` | `droit/src/marqueurs.rs` |
| modèle de contrat choisi | `IdDeModele` | `engagement/src/carte.rs` |
| modèle du bulletin | `ModeleBulletin` | `droit/src/fiche.rs` |
| catégorie d'un document | `documents::CategorieDeDocument` | `documents/src/lib.rs` |
| état d'export | `EtatDExport` | `documents/src/plan_comptable.rs` |
| format d'export | `FormatDExport` | `documents/src/plan_comptable.rs` |
| regroupement d'un état | `RegroupementDEtat` | `documents/src/plan_comptable.rs` |
| logiciel du comptable | `LogicielComptable` | `documents/src/plan_comptable.rs` |
| période d'export | `PeriodeDExport` | `documents/src/plan_comptable.rs` |
| colonne d'un état | `ColonneDEtat` ; à unifier (#419) : `tableaux_export::Colonne.cle` | `documents/src/plan_comptable.rs` |
| code journal | `CodeJournal` | `documents/src/ecritures_comptables.rs` |
| numéro de compte | `NumeroDeCompte` | `documents/src/ecritures_comptables.rs` |
| poste analytique | `PosteAnalytique` | `documents/src/ecritures_comptables.rs` |
| formalité d'une DPAE | `declarations::dpae::FormaliteDpae` | `declarations/src/dpae.rs` |
| code retour d'une DPAE | `declarations::dpae::CodeRetourDpae` | `declarations/src/dpae.rs` |
| code retour du lot | `declarations::dpae::CodeRetourDuLot` | `declarations/src/dpae.rs` |
| type de contrat de la DPAE | `TypeContratDpae` | `droit/src/natures.rs` |
| lieu d'une obligation hors DSN | `declarations::obligations::LieuHorsDsn` | `declarations/src/obligations.rs` |
| origine du retour d'un contrat | `envois::ligne::OrigineDuRetourDeContrat` | `envois/src/ligne.rs` |
| paie | `engagement::calcul::Paie` ; à unifier (#419) : `PaieEcrite`, `PaieImportee`, `PaieLue`… | `engagement/src/calcul.rs`, `engagement/src/paie_ecrite.rs`, `engagement/src/importe.rs` |
| population | `droit::natures::Paie` | `droit/src/natures.rs` |
| ligne de paie | `engagement::ligne::LignePaie` | `engagement/src/ligne.rs` |
| salarié | `IdSalarie`, `droit::fiche::FicheSalarie` | `droit/src/contexte.rs`, `droit/src/fiche.rs` |
| compagnie | `compagnie::installation::Compagnie` | `compagnie/src/installation.rs` |
| spectacle | `IdSpectacle`, `droit::fiche::Spectacle` | `droit/src/contexte.rs`, `droit/src/fiche.rs` |
| fiche | `droit::fiche::NomDeFiche`, `Datee<T>` | `droit/src/fiche.rs` |
| fiche nommée | `FicheNommee` | `droit/src/fiche.rs` |
| fiche visée par l'import | `FicheVisee` | `import/src/reponse.rs` |
| fiche saisie (une fiche que la table des cases juge) | `FicheSaisie` | `droit/src/fiche.rs` |
| faute d'une case saisie | `FauteDeCase` | `droit/src/fiche.rs` |
| fiche à juger (l'une des trois, typée) | `FicheAJuger` | `declarations/src/champs.rs` |
| ce qu'une case écrit | `MotEcrit` | `socle/src/ecriture.rs` |
| valeur d'une case d'une fiche | `ValeurDeCase` | `socle/src/ecriture.rs` |
| liste lue élément par élément | `socle::forme::Liste` | `socle/src/forme.rs` |
| clé d'échéance | `CleDEcheance` | `routes/src/rappels.rs` |
| échéance de l'année | `CleDAnnuelle` | `routes/src/rappels.rs` |
| genre d'une échéance | `GenreDEcheance` | `routes/src/rappels.rs` |
| ce que vise l'échéance d'un salarié | `QuoiDuSalarie` | `routes/src/rappels.rs` |
| case | `NomDeCase`, `CaseDePiece`, toutes deux `CaseDeFiche` | `droit/src/fiche.rs` |
| rubrique | `droit::catalogue::Rubrique` | `droit/src/catalogue.rs` |
| unité d'une rubrique | `Unite` | `droit/src/catalogue.rs` |
| sens d'une rubrique | `Sens` | `droit/src/catalogue.rs` |
| ce qu'une rubrique compte pour les droits | `ComptePour` | `droit/src/catalogue.rs` |
| case qu'une rubrique fait saisir | `CaseSaisie` | `droit/src/catalogue.rs` |
| origine d'une rubrique | `OrigineDeRubrique` | `droit/src/catalogue.rs` |
| cotisation du bulletin | `IdDeCotisation` ; à unifier (#419) : `LigneCotisation.id` | `droit/src/forme_bareme.rs` |
| famille d'une cotisation | `FamilleDeCotisation` | `droit/src/forme_bareme.rs` |
| regroupement de la comparaison | `RegroupementDeComparaison`, `TotalDuBulletin` | `engagement/src/ecarts.rs` |
| origine du taux AT/MP comparé | `OrigineDuTauxAtmp` | `engagement/src/ecarts.rs` |
| logiciel d'un bulletin importé | `LogicielDUnBulletin` | `engagement/src/importe.rs` |
| raison d'un recalcul | `RaisonDuRecalcul` | `engagement/src/etats.rs` |
| cotisation de la DSN | `CodeCotisation81` | `droit/src/code_cotisation.rs` |
| clé de totalisation | `CleDeTotalisation` | `droit/src/code_cotisation.rs` |
| assiette | `droit::assiette::Assiette` | `droit/src/assiette.rs` |
| barème | `droit::forme_bareme::Pile` | `droit/src/forme_bareme.rs` |
| emploi | à unifier (#419) : `droit::emplois::FicheDuMetier`, `EmploiFt` et d'autres | `droit/src/emplois.rs` |
| code d'un métier | `CodeEmploi` | `droit/src/emplois.rs` |
| annexe du règlement d'assurance chômage | `AnnexeChomage` | `droit/src/emplois.rs` |
| clé de grille des minima | `CleDeGrille` | `droit/src/minima.rs` |
| catégorie d'une clause | `CategorieDeClause` | `droit/src/clauses.rs` |
| annexe de la convention 3090 | `AnnexeConvention` | `droit/src/fiche.rs` |
| statut | `droit::fiche::Statut` | `droit/src/fiche.rs` |
| colonne de la répartition de branche | `ColonneDeRepartition` | `droit/src/bareme.rs` |
| répartition de la tranche 1 | `RepartitionDeLaTrancheUne` | `droit/src/bareme.rs` |
| part de la tranche 1 | `PartDeLaTrancheUne` | `droit/src/bareme.rs` |
| majoration de la caisse des congés | `MajorationCaisseConges` | `droit/src/forme_bareme.rs` |
| catégorie du barème | `CategorieDuBareme` | `droit/src/forme_bareme.rs` |
| rôle d'un contrat d'organisme | `RoleDuContrat` | `droit/src/organismes.rs` |
| contribution de branche | `ContributionDeBranche` | `droit/src/forme_bareme.rs` |
| niveau de garantie santé | `NiveauDeGarantie` | `droit/src/forme_bareme.rs` |
| organisation patronale | `OrganisationPatronale` | `droit/src/forme_bareme.rs` |
| catégorie de la DFS | `CategorieDfs` | `droit/src/forme_bareme.rs` |
| formule de la réduction générale | `FormuleDeReduction` | `droit/src/forme_bareme.rs` |
| affectation d'une cotisation de la DSN | `AffectationDeCotisation` | `droit/src/code_cotisation.rs` |
| établissement | son `Nic` | `droit/src/fiche.rs` |
| effectif | `droit::contexte::Effectif` | `droit/src/contexte.rs` |

**Le détail**

- **engagement** : ce qu'une compagnie engage d'un salarié, et ses calculs ; sa forme écrite est une forme.
- **contrat** : ce que l'engagement fait signer et déclare : son numéro et ses termes. `ContratEcrit`, `ContratParti`,
  `ContratLu`, `ContratAtteste`, `ContratDeclare`… sont des lectures, à ranger par origine ;
  `droit::organismes::Contrat` est un contrat d'organisme, à renommer.
- **acte d'un contrat parti** : ce qu'est une pièce partie à la signature qui n'est pas le contrat lui-même : un avenant,
  le contrat refait qui annule et remplace celui qui est parti ; aucun : le contrat. `avenant`, `remplace` : une liste
  du logiciel, qu'il écrit lui-même. Que portent un contrat parti, le détail d'un envoi à la signature, ce qui en
  revient, une signature attendue, une réponse relevée par courriel.
- **acte qu'une case exige** : un acte qui ne se fait pas sans une case de fiche (`exige_par` de `champs-base.json`), ou
  qu'elle sert sans le bloquer (`utile_pour`) : la DSN, la DPAE, le contrat, le bulletin, le certificat Congés
  Spectacles, toute paie, l'envoi par courriel. Une liste du logiciel, écrit `dsn`, `dpae`, `contrat`, `bulletin`,
  `certificat`, `paie`, `envoi`. Que portent une ligne et une case de `champs-base.json`, leur description, ce qu'une
  case sert (`Sert`), le refus d'un acte quand une fiche n'a pas ce qu'il exige (`refus_des_manques`). La sorte d'un
  envoi au journal (`envois::ligne::Acte`) est une autre notion.
- **marqueur** : un champ qu'un modèle de document cite (`{{employeur.siret}}`) ; aussi une clause d'un contrat parti.
  Une liste du logiciel : il s'écrit, se lit et se trie par son nom (`nom()`). Il est dans `droit` pour que la table des
  natures (`contrats-base.json`, ses mentions obligatoires) le lise à sa porte. Que portent une mention d'une nature, les
  valeurs d'un document, un modèle lu (`documents::marqueurs`), les clauses d'un contrat parti au journal des envois (`ContratParti.clauses`, rangées dans l'ordre de leurs
  noms) ; ce qu'un refus ou la liste des champs d'un modèle écrivent entre accolades reste leur texte.
- **modèle de contrat choisi** : un modèle de contrat propre à la compagnie, qu'une carte choisit (`modele_contrat`) ;
  aucun : le défaut de sa nature. `m` et un entier, gardé tel qu'écrit ; seul `IdDeModele::de` en fait un neuf (la copie
  d'une base, `routes::modeles_contrat`). Que portent la carte, le contrat écrit d'un engagement, un modèle spécial, le
  défaut d'une nature, les supprimés (`gabarits/contrats/modeles.json`), et les charges des routes de modèles, lues à
  leur porte. Le modèle du bulletin (« simplifié », « complet ») est une autre notion.
- **modèle du bulletin** : le bulletin que la compagnie remet, le simplifié (le modèle de l'arrêté du 25/02/2016, art.
  1, I) ou le complet (le même, suivi d'une page annexe) ; un choix de la fiche (`modele_bulletin`, écrit `simplifié`,
  `complet`), celui du salarié l'emportant ; sans choix, le simplifié, que l'arrêté donne
  (`documents::bulletin::modele_de_la_compagnie`, qui le cite). Que portent les fiches Entreprise et Salarié, la charge
  et la réponse des routes du modèle ; l'adresse d'un aperçu l'écrit sans accent (`dans_l_adresse`).
- **catégorie d'un document** : une sorte de pièce du dossier d'un salarié : bulletins, contrats, contrats signés,
  certificats Congés Spectacles, attestations, documents de fin, fiche récapitulative. Une liste du logiciel, dans
  l'ordre où le dossier se lit : `TOUTES` ; écrite `bulletins`, `contrats-signes`… Que portent la charge du dossier, le
  dossier rendu, les catégories proposées, le dossier parti au journal des envois (`DetailDossier.categories`).
- **état d'export**, **format d'export**, **regroupement d'un état**, **logiciel du comptable**, **période d'export** :
  les listes du logiciel que les tables des exports livrent (`exports_tables.json`) : les états que la page propose
  (`livre-de-paie`…), leurs formats (les six des tableaux, `csv`… `json`, puis ceux des logiciels du comptable, `fec`…
  `pennylane`), leurs regroupements (`mois`, `organisme`, `salarie`, `spectacle`), les logiciels du comptable
  (`oxygene`… `autre`, chacun avec son format ; `FormatBorne` dit ceux dont la notice borne les zones), les périodes
  qu'un état propose (`annee`, `mois`, `toutes`). Chacune s'écrit, se lit et se trie par son texte ; une autre se refuse
  à sa lecture (les tables, la charge d'`exporter`, de `retenir-logiciel` et des libellés courts, `config/compta.json`).
  Que portent la demande d'un état (`etats_paie::Demande`), un tableau rendu, le livre de paie, les limites d'un format.
- **colonne d'un état** : une colonne qu'un état totalise ; aujourd'hui celles du livre de paie (`brut`…
  `cout_employeur`), sous lesquelles son total les nomme et compte ce qu'un bulletin ne déclare pas. Les colonnes des
  autres tableaux (`tableaux_export::Colonne.cle`) sont encore la clé texte d'une ligne.
- **code journal**, **numéro de compte**, **poste analytique** : les codes d'une écriture comptable que le plan
  comptable dit (`plan-comptable-base.json`, recouvert par `config/plan-comptable.json`) : le journal (`OD`), les comptes
  (du brut, du net, des frais, du prélèvement à la source, des retenues, des lignes, des tiers, de la provision de
  congés ; le sous-compte individuel qu'un modèle fait pour chaque salarié, `compte_du_salarie`), le poste analytique
  d'Oxygène (la zone CODAGE). Des clés du logiciel (`droit::cle_opaque!`), non vides, gardées telles que lues ; chaque
  logiciel du comptable borne leur largeur et leurs caractères à l'écriture (`plan_comptable::controler_plan`). Que
  portent le plan comptable et une ligne d'écriture (`ecritures_comptables::Ecriture`), ce que rend `export-compta`. Le
  modèle du sous-compte (`net.par_salarie`, « 421{code:03} ») n'est pas un compte ; le code analytique d'un spectacle
  (`sections`) est une autre notion.
- **formalité d'une DPAE** : ce que le certificat de conformité d'une DPAE dit fait : CERTIF, IMMA, MT, PMF5 (Format 120,
  I_CERTIF à I_PMF5). Une liste de la norme, que portent une ligne du certificat et la personne certifiée au journal des
  envois.
- **code retour d'une DPAE** : ce que le certificat de conformité dit de chaque DPAE (C_RETOUR_AR, positions 430-431 du
  Format 120) : 00 enregistrée, 98 déjà existante, 99 autre erreur, les codes d'anomalie. `codes_fermes!` sur l'onglet
  « Codes retour » du Format 120, avec son libellé mot pour mot, PDF p. 9-10 ; `enregistree()` : 00 et 99. Un code hors
  de la liste, ou la zone à blanc, refuse le certificat ; que portent une ligne du certificat et la personne certifiée au
  journal des envois.
- **code retour du lot** : ce que le certificat de conformité dit du traitement du message (C_RET_AR_LOT, positions
  76-77) : 00 correct, 11, 15. `codes_fermes!`, avec son libellé ; un code hors de la liste refuse le certificat ; à
  blanc, aucun ; que porte une ligne du certificat (un lot autre que correct est une anomalie de la pièce).
- **lieu d'une obligation hors DSN** : où une obligation qu'un arrêt fait naître se fait soi-même : `courrier`,
  `net-entreprises`, `net-entreprises-ecran`, `net-entreprises-edi`. Une liste du logiciel, que portent les lieux
  d'`obligations-base.json` (`ou`), la charge d'`accident-declare` et d'`attestation-declare` (`depot`), l'obligation
  dite faite de la fiche de suivi.
- **origine du retour d'un contrat** : d'où vient la réponse à un contrat parti : un « Je signe » relevé par courriel ;
  aucune : la mention posée à la main. `courriel` : une liste du logiciel, que porte ce qui revient d'un contrat parti.
- **paie** : ce qui part avec le bulletin d'un mois ; `droit::natures::Paie` est une autre notion (la population), à
  renommer.
- **population** : la face de paie d'une nature : à l'activité (l'intermittent) ou au mois (le permanent). `Activite`,
  `Mensuelle`, écrits `activite`, `mensuelle` dans `contrats-base.json` (`paie`), et `intermittent`, `permanent` là où
  les fichiers livrés nomment la population (`natures::population`) : les sections `par_nature` du barème et de ses
  couches, la `nature` d'un effet de seuil, la section d'une nature (`bareme`), la `nature` d'une ligne du bordereau, la
  `population` d'une grille des minima (absente : toutes).
- **ligne de paie** : une ligne datée : cachet, heures, prime, frais ; une date de ligne, `DateDeLigne`.
- **salarié** : une personne que la compagnie emploie ; `SalarieDuContrat`, la fiche figée d'un contrat.
- **compagnie** : l'employeur et son installation.
- **spectacle** : une production.
- **fiche** : entreprise, salarié ou spectacle, datée ; `routes::catalogue::Fiche`, la fiche d'une route, est une autre
  notion, à renommer.
- **fiche nommée** : une fiche et ce qui la désigne : l'entreprise (le fichier entier, sans clé), un salarié ou un
  spectacle par la clé de sa fiche. `Entreprise`, `Salarie(IdSalarie)`, `Spectacle(IdSpectacle)`, que porte une fiche
  que l'import complète ou vise. La clé d'une sorte de fiche, `SorteDeFiche::Cle` (`()` pour l'entreprise), que portent
  l'écriture d'une fiche, sa charge (`enregistrer-fiche`, `activer-fiche`), la fiche que la configuration montre et ce
  que l'écriture rend.
- **fiche visée par l'import** : la fiche qu'une ligne de l'import vise : une fiche nommée (qui existe, ou que l'import
  vient d'écrire), ou la sorte d'une fiche encore à créer, sans clé. `Nommee(FicheNommee)`, `ACreer(NomDeFiche)` ;
  écrite en deux champs, `nom` et `cle` (`null` pour l'entreprise et une fiche à créer) ; que porte une fiche de la
  réponse de l'import (`FicheImportee`).
- **fiche saisie** : la saisie d'une fiche est la fiche typée elle-même (`ChampsEntreprise`, `ChampsSalarie`,
  `ChampsSpectacle`), ses fautes à côté ; chaque case se nettoie et se type une fois, à l'entrée (`lire_cases`, la seule
  porte d'un formulaire ; `Changements::lire` pour `enregistrer-fiche`), et personne ne relit le texte tapé. La table
  (`declarations::champs`) la juge par ce trait : ce qu'une case écrit (`ecrite`), si elle est remplie (`dite` : zéro
  l'est, une case à cocher non cochée ne l'est pas, une structure l'est par une de ses cases).
- **faute d'une case saisie** : la case, le chemin sous elle dans une structure, la phrase de ce que son type refuse ;
  la case reste vide dans la fiche typée, la faute devient une anomalie non forçable (`champs::des_fautes`), dite avec
  toutes les autres.
- **ce qu'une case écrit** : vide, un mot (un texte, un nombre écrit), oui ou non, une structure ; la valeur typée dans
  l'écriture de son fichier (`socle::ecriture::mot`), ce que lisent les règles de `champs-base.json` (`si`, `exige_si`,
  `avance`, la norme, la longueur).
- **valeur d'une case d'une fiche** : la valeur typée d'une case, écrite comme le fichier l'écrit ; que portent la boîte
  « Ce qui change » (`ChangementDit`) et l'historique d'une fiche.
- **liste lue élément par élément** : une liste d'une charge dont chaque élément se lit seul en sa forme, une faute ne
  faisant pas taire les suivants (`chacun`) ; les changements d'`enregistrer-fiche`.
- **clé d'échéance** : ce qui nomme une échéance des rappels d'un rappel à l'autre : une échéance de l'année
  (`quoi:AAAA`), la DSN d'un mois (`dsn:AAAA-MM`), les DPAE d'un jour (`dpae:AAAA-MM-JJ`), une obligation qui vise un
  salarié (`<quoi>:<salarié>:<jour>` : une fin de contrat, ce qu'un arrêt fait naître).
  `DeLAnnee { quoi: CleDAnnuelle, .. }`, `Dsn`, `Dpae`, `DuSalarie { quoi: QuoiDuSalarie, .. }` ; écrite par son texte,
  lue strictement par lui (`quoi` jusqu'au premier deux-points, le jour après le dernier), triée comme lui. Que portent
  une échéance et ce que les rappels en rendent, la charge et la réponse d'`echeance-faite`, les échéances cochées
  (`config/echeancier.json`), une déclaration du tableau de l'administrateur.
- **échéance de l'année** : une échéance de calendrier : la `cle` d'une échéance d'`echeancier-base.json`, ou l'export
  comptable (une clé du logiciel). Une clé non vide, sans deux-points, écrite comme son texte ; celles que le moteur
  lit, `CLOTURE`, `REVUE_SPECTACLES`, `AT_MP` (des clés de l'échéancier livré) et `EXPORT_COMPTA` (qui n'en est pas
  une), tenues par un test de structure.
- **genre d'une échéance** : ce qu'est une échéance des rappels : la DSN d'un mois, les DPAE d'un jour, une obligation
  due, une échéance de l'année. `Dsn`, `Dpae`, `Obligation`, `DeLAnnee(CleDAnnuelle)` ; écrit `dsn`, `dpae`,
  `obligation` ou la clé ; que porte une échéance, que les rappels rendent (`quoi`) et que lit le tableau de
  l'administrateur.
- **ce que vise l'échéance d'un salarié** : la fin de son contrat à signaler, ou une obligation qu'un arrêt fait naître.
  `FinContrat` (écrit `fin-contrat`), `Obligation(CleDObligation)` (écrit son nom) ; le `quoi` d'une clé d'échéance
  `<quoi>:<salarié>:<jour>`.
- **case** : un champ d'une fiche ; `engagement::ligne::Case` (un `Option<Decimal>`) est mal nommé.
- **rubrique** : une ligne du catalogue de paie ; `droit::catalogue::Rubrique` est sa définition. Son code,
  `CodeDeRubrique` (`droit/src/catalogue.rs`), que portent une ligne de paie (`engagement::ligne`), une ligne du brut
  (`LigneBrut.code`), les frais d'une profession au barème (`frais_hors_assiette`), une prime saisie, une ligne imprimée
  d'un bulletin importé ; un code que le moteur compose est une constante (`CodeDeRubrique::du_moteur`).
- **unité d'une rubrique** : ce que la quantité d'une ligne compte : cachet, heure, jour, semaine, mois, service,
  forfait, pourcentage (`rubriques-base.json`). Que portent la rubrique, ses cases `unite`, `plafond` et `aem`, une ligne du brut (`LigneBrut`,
  sa forme écrite), une ligne de rémunération d'un mois, une grille des minima (`droit::minima::Grille::unite`, ce que
  sa valeur paie).
- **sens d'une rubrique** : ce que sa ligne fait au brut, l'ajouter ou l'en retirer : `Plus`, `Moins`, écrits `+`, `-`
  (`sens` de `rubriques-base.json`) ; `signe()` donne 1 ou -1. Que portent la rubrique, sa case, la rubrique rendue à la
  page.
- **ce qu'une rubrique compte pour les droits** : les heures et le montant (`droits_et_montant`), ou les heures pour
  l'ouverture des droits seulement (`ouverture_des_droits`, l'enseignement de l'annexe X). Une liste du logiciel ; que
  portent la rubrique, sa case, la rubrique rendue.
- **case qu'une rubrique fait saisir** : une case de la ligne d'une rubrique (`saisie` de `rubriques-base.json`) :
  `quantite`, `taux`, `heures`, `jours`, `base`, `taux_majoration`, `montant`, `genre`. Une liste du logiciel ; la
  forme d'une ligne s'en déduit (`engagement::ligne::FormeSaisie::de`). Que portent la rubrique, sa case, la rubrique
  rendue.
- **origine d'une rubrique** : livrée (`base`) ou propre à la compagnie (`compagnie`) ; le catalogue la pose à sa
  construction, la rubrique rendue l'écrit.
- **cotisation du bulletin** : une ligne de cotisation du barème (`rubriques_bulletin`), par sa clé : `maladie`,
  `retraite_t1`. Que nomment les modifications d'une couche, `rubriques_retirees`, `apres`, `du_corps`, les `lignes`
  d'une cotisation DSN ou d'une contribution, `rubriques_abattues` de la DFS et les rubriques d'une DFS calculée ; une
  clé que le moteur nomme est une constante (`MUTUELLE`, `RETRAITE_T1`…). À unifier : `LigneCotisation.id` et ce qui la
  relit, en `String`. Ce n'est ni une rubrique du catalogue, ni une cotisation de la DSN.
- **famille d'une cotisation** : la famille sous laquelle le bulletin range une cotisation (arrêté du 25/02/2016 :
  `RETRAITE`, `SANTÉ`, `ASSURANCE CHÔMAGE`…). Une clé du logiciel (`droit::cle_opaque!`), gardée telle que lue : le
  barème écrit l'AT/MP avec un tiret court ou un tiret long, la table de lecture des bulletins écrit ses familles en
  minuscules (`conges_spectacles`) ; celles que le moteur lit ou pose, `FAMILLE_RETRAITE`, `FAMILLE_ALLEGEMENTS`. Que
  portent une rubrique du barème, sa modification par une couche, une contribution conventionnelle, une ligne de
  cotisation calculée et sa forme écrite, une ligne qu'un bulletin importé imprime, une ligne de cotisation d'une paie
  importée, le rang d'une famille que le bulletin ne range pas.
- **regroupement de la comparaison** : ce que la comparaison d'un bulletin importé au calcul nomme, au grain où un
  bulletin l'imprime : une cotisation du barème (`Cotisation(IdDeCotisation)`), les contributions que l'employeur paie
  seul (`autres`), celles de la convention (`convention`), les tranches de l'Apec (`apec`), ou un total du bulletin
  (`Total(TotalDuBulletin)` : `cotisations_salariales`, `cotisations_patronales`, `net_social`, `net_imposable`).
  Écrit par son texte, trié comme lui ; la clé de `cotisations` de la table de lecture des bulletins. Que portent un
  écart, la ligne de cotisation qu'un bulletin importé imprime, l'incident d'un écart d'import ; la ligne de cotisation
  d'une paie importée en garde le nom pour clé de cotisation (`cle_de_cotisation`).
- **origine du taux AT/MP comparé** : d'où vient le taux AT/MP du calcul d'une comparaison : la compagnie, ou le
  bulletin importé (l'AT/MP n'est alors pas comparé). Une liste du logiciel, écrite `compagnie`, `bulletin`.
- **logiciel d'un bulletin importé** : le logiciel de paie qu'un bulletin importé est reconnu avoir fait, à sa signature
  (une clé de `logiciels` de `bulletins-lecture-base.json`). Une clé du logiciel (`droit::cle_opaque!`) ; aucun : un
  autre. Que portent un bulletin lu, le bulletin d'une paie versée, ce qu'une page dit d'elle-même dans le compte rendu
  de l'import, une comparaison ; l'incident d'un écart d'import l'écrit « autre » quand il n'y en a pas
  (`import::ecarts::LogicielDuRapport`, une écriture).
- **raison d'un recalcul** : pourquoi une paie déjà écrite se recalcule : le taux de PAS reçu, une règle datée (`RegleDu`,
  un jour), le code qui calcule corrigé, un autre engagement du salarié, la saisie du mois, un arrêt ou une absence, la
  paie d'un autre mois, la configuration de la compagnie. Une liste du logiciel ; écrite par sa phrase (`Display` :
  « calcul corrigé », « règle du JJ/MM/AAAA »…), que la paie écrite garde (`Recalcul::raison`, du texte). Le code et une
  règle datée sont des décisions de l'éditeur (`decision_de_l_editeur`) : elles seules refont une paie partie.
- **cotisation de la DSN** : une ligne du bloc 81, par son code S21.G00.81.001. Que portent `cotisations_dsn` (`code81`,
  `apres`), `cotisations_dsn_retirees`, `du_corps`, les codes de la réduction générale, la CET patronale d'une version
  de la norme ; une couche la nomme par son code et, quand plusieurs le partagent, son affectation (`059:203`) :
  `CleDeCotisationDsn` (même fichier).
- **clé de totalisation** : le versement (bloc S21.G00.20) dont le montant (20.005) compte une cotisation de la DSN : son
  code 81.001 et, quand ce code va à deux versements, qui le recouvre, écrits `code` ou `code:recouvreur`
  (`018:urssaf`, `018:chomage`, `040:urssaf`) ; une cotisation que le barème ne clé pas se totalise sous son code. Son
  recouvreur, `Recouvreur`. Que portent les `codes81` d'un versement de `organismes-base.json`, `cle_total` d'une
  cotisation DSN du barème, `cle_total_urssaf` et `cle_total_chomage` de la réduction générale, les totaux des
  versements de la DSN.
- **assiette** : ce sur quoi une cotisation se calcule.
- **barème** : les taux d'un jour, en trois étages.
- **emploi** : ce que fait le salarié.
- **code d'un métier** : un code de la nomenclature des emplois du spectacle de France Travail
  (`nomenclature-emplois-ft.json`), qui désigne un métier : du catalogue (`emplois-base.json`, dont chaque clé en est
  un), sinon de la nomenclature. Que portent le métier qu'une carte choisit (`metier`), la clé du catalogue, un métier et
  celui d'un déroulant, les `codes` d'une grille des minima, les métiers retenus par la compagnie ; son métier :
  `CodeEmploi::metier`. Le code emploi qu'une fiche déclare (S21.G00.40.005) est une autre notion, `CodeEmploiFt`, qui
  peut n'en nommer aucun (« 999SPT »).
- **annexe du règlement d'assurance chômage** : l'annexe VIII (les ouvriers et techniciens) ou X (les artistes) dont
  relève un intermittent. `Viii`, `X` ; écrite par son nom (`X`, `VIII` : l'annexe d'une rubrique, celle d'une source)
  ou par son numéro (`8`, `10` : `emplois-base.json`, la nomenclature, un métier rendu, `emplois::annexe_en_nombre`).
  Que portent un métier (`Metier::annexe`), la rubrique, une source citée, les annexes en jeu d'un conseil.
- **clé de grille des minima** : ce qui nomme une ligne ou une colonne des grilles de `minima-conventionnels.json` (une
  grille de cachets `theatre.role`, une fonction de la 2642 ou de la 3097, un groupe de la 1285, un niveau de la 3090,
  une jauge, une période, une catégorie de grille, le groupe d'une majoration) et ce qui la désigne : la ligne de minima
  d'un métier et sa classification 3090 (`emplois-base.json`), les niveaux et les grilles de la prime d'ancienneté
  (`clauses-conventionnelles-base.json`). Une clé du logiciel, non vide ; celles que le moteur lit sont des constantes
  (`TECHNIQUE`, `ADMINISTRATIF`, `ACTEUR`, `CIRQUE`). Le nom d'une grille (`nom`, que `remplace` cite) et les
  classements de ses `valeurs` restent leur texte. Un test de structure tient que chaque clé citée existe là où elle
  désigne (`chaque_cle_citee_existe_dans_le_fichier_qu_elle_designe`).
- **catégorie d'une clause** : la catégorie d'un salarié dans les clauses d'une convention (`categories.valeurs` de
  `clauses-conventionnelles-base.json` : `artiste`, `cadre`, `non_cadre`, `cadre_de_direction`…), que nomment le
  préavis par catégorie, la catégorie d'un niveau 3090 et le profil d'un salarié (`clauses::Profil`). Une clé du
  logiciel, non vide ; constantes `ARTISTE`, `CADRE`, `NON_CADRE`, `CADRE_DE_DIRECTION`, `AGENT_DE_MAITRISE` ; les six
  choix de la case « Catégorie dans la convention » en sont (`From<CategorieConventionnelle>`).
- **annexe de la convention 3090** : l'annexe (`1` à `6`) de la convention du spectacle vivant privé dont relève
  l'entreprise, un choix de sa fiche. Que portent la fiche Entreprise, les annexes des minima, la ligne de minima 3090
  d'un métier, les clauses d'annexe de l'indemnité de licenciement, la prime d'ancienneté, la prévoyance par annexe.
- **statut** : artiste ou technicien ; écrit `artiste`, `technicien`. Que portent la fiche, les sections `par_statut` du
  barème (d'une catégorie aussi), la section d'un effet de seuil, les `statuts` de la réduction générale, le `statut`
  d'une ligne du bordereau et d'une grille des minima (absent : tous).
- **colonne de la répartition de branche** : la colonne de la répartition de la retraite complémentaire en tranche 1
  d'une branche (tableau n° 3 de la fiche Audiens « Taux et répartitions ») : le permanent, ou l'intermittent artiste
  ou technicien. `Permanent`, `Intermittent(Statut)` ; elle lit sa colonne dans `retraite_t1_repartition.par_idcc` du
  socle (`permanent`, `artiste`, `technicien`) et s'écrit par ce nom dans le refus d'une branche sans répartition.
- **répartition de la tranche 1** : la part employeur et la part salariée de la retraite complémentaire en tranche 1
  d'une branche (`retraite_t1_repartition` du socle : `avant_1999`, chaque colonne de `par_idcc`). Deux écritures,
  gardées telles que lues et réécrites ainsi : en pourcentages (`EnPourcentages`, « 60/40 »), en fractions
  (`EnFractions`, « 2/3-1/3 ») ; ce que le calcul partage, ses deux parts irréductibles (`parts`,
  `PartDeLaTrancheUne`, même fichier). Ce n'est pas une fraction de la norme (`natures::Fraction`, S20.G00.05.003).
- **majoration de la caisse des congés** : ce par quoi le coefficient de la réduction générale d'un salarié déclaré à
  la caisse des Congés Spectacles se multiplie (`majoration_caisse_conges` du barème, « 100/90 ») : son numérateur et
  son dénominateur tels qu'écrits, sans les réduire (le calcul reste `c × 100 / 90`) ; réécrite comme lue.
- **catégorie du barème** : la section d'un cadre dans le barème (`par_categorie` : `cadre`, `cadre_permanent`). Une clé
  du logiciel (`droit::cle_opaque!`), qui se trie comme son texte ; celles que le moteur lit, `CADRE`, `CADRE_PERMANENT`.
- **établissement** : un lieu d'emploi ; son `Nic` a cinq chiffres. Que portent la fiche entreprise et ses autres
  établissements (`nic_principal`, `nics`, `etablissement`), la carte et l'engagement, la fiche d'un contrat
  (`SalarieDuContrat`), la DSN fabriquée et lue, le journal des envois, la désignation d'un dépôt ; aucun (`None`) : le
  siège.
- **effectif** : l'effectif que l'Urssaf calcule.
- **type de contrat de la DPAE** : C_TYPE_CONTRAT du Format 120 (1 CDD, 2 CDI, 3 CTT), `codes_fermes!` avec son libellé ;
  que portent la DPAE d'une nature (`contrats-base.json`) et une DPAE relue.
- **rôle d'un contrat d'organisme** : la prévoyance ou la santé d'un salarié payé au mois (`role` de
  `organismes-base.json`, écrit `prevoyance`, `sante`) ; une liste du logiciel. Il range le contrat dans la pièce d'un
  permanent (`Attendu::piece`) et ordonne son rattachement. Ceux qu'un contrat couvre (`beneficiaires`) sont une
  population (`intermittents`, `permanents` : `natures::population::au_pluriel`).
- **contribution de branche** : une contribution conventionnelle déclarée sous une adhésion Audiens (FNAS, FCAP,
  PARITARISME, CCHSCT, APPAV), gardée telle que le barème l'écrit (`contributions_dsn`) ; deux noms sont la même sans
  égard à la casse (`est`), une adhésion la tient aussi par son libellé (`est_le_libelle`). Que portent une contribution
  DSN du barème, une adhésion saisie, un contrat attendu.
- **niveau de garantie santé** : le niveau de la complémentaire santé d'un permanent (`1`, `2`, `3`) : la clé des
  niveaux du relevé santé du barème, des options d'un contrat d'organisme, la case `sante_svp.niveau` ; une clé du
  logiciel, non vide.
- **organisation patronale** : la clé d'une couche du barème par organisation (`par_organisation_patronale`, son
  sigle). Celle que la compagnie nomme dans sa fiche est son texte, que la couche reconnaît par son sigle ou un alias
  (`reconnait`).
- **catégorie de la DFS** : la catégorie d'une profession de la déduction forfaitaire spécifique (`20`, `25` : art. 5
  de l'annexe IV du CGI), la clé des taux de l'année ; que portent une profession du barème, la DFS d'un contrat et sa
  forme écrite. La profession elle-même est le choix de la fiche `ProfessionDfs`.
- **formule de la réduction générale** : celle que le barème nomme (`formule`) : `fillon`, au coefficient `t` ; sans
  elle, le coefficient se calcule par `tmin`, `tdelta` et `puissance`. Une liste du logiciel.
- **affectation d'une cotisation de la DSN** : la place de l'adhésion sous laquelle une cotisation du barème se déclare
  (`202`, `203`), quand plusieurs partagent un code 81.001 ; elle distingue leur clé (`059:203`, `CleDeCotisationDsn`).

## Le principe

**L'argent est hors du logiciel.** Il dit combien et à qui ; la compagnie vire ; la DSN porte les ordres de
prélèvement.

**Un seul logiciel : le moteur et ses routes.** Cliquer dans la page ou appeler `moteur route` en ligne de commande,
c'est le même geste : la même route, la même réponse, les mêmes phrases, les mêmes refus. La route rend ce qui se lit
(phrase, état, geste) ; la page n'est qu'une façon de lire cette réponse : elle la met en page, ne décide rien et n'a rien en propre. Il n'y a donc **ni « côté écran », ni
« côté LLM »**, et on ne les distingue jamais, que ce soit dans un ticket, une consigne, un plan ou un commit. Une chose
est faite quand la route la fait ; un manque, où qu'on le voie, est un manque de la route. Chaque route porte sa fiche
dans son code (`routes/src/catalogue.rs`) ; `moteur routes` tire le catalogue de ces fiches.

**Tout est daté.** Barèmes, configuration entière de la compagnie, engagements et leurs versions, journal : chaque
valeur porte sa date, et le moteur lit à la date de la période payée, jamais « à aujourd'hui ». Une donnée non datée
est un trou : elle rend le passé faux dès qu'elle change. Une compagnie se rejoue donc depuis son premier jour en
avançant l'horloge (« L'horloge des compagnies fictives »).

**Le calcul est du code, pas du langage.** Un modèle de langage ne calcule jamais
une cotisation. La crate `engagement` applique les barèmes datés ; elle est la seule
du moteur à calculer. Claude ne fait que traduire une phrase en saisie et appeler
une route.

**Les formats à largeur fixe partent d'un enregistrement.** La DPAE se fabrique sur
un gabarit de 990 caractères (`donnees/modeles/due_tel_gabarit.txt`, un enregistrement
neutre, ou celui du `gabarits/` de la compagnie) dont seule l'en-tête constante reste :
chaque zone variable est réécrite (`declarations/src/dpae.rs`).

**Rien n'est réputé juste sans preuve.** Une règle se prouve par un test qui cite
le texte déposé qui la fonde ; une déclaration, par le dépôt de test, seul
validateur (règle de fond 10).

## Les parties

L'espace Cargo `atelier/sources/` a une crate par partie ; chaque flèche descend, jamais l'inverse
(`atelier/sources/Cargo.toml`, en tête).

```
                         atelier  le binaire du nuage `moteur-atelier`, jamais livré ; rien ne le prend
                           │
                         moteur   la ligne de commande et la page ; la publication par GitHub
                           │
                         routes   la porte, une route par geste, ce qu'elles font
                           │
                         import   les pièces d'un autre logiciel, telles qu'elles le disent
                           │
                        compagnie l'installation, le coffre, la mise à jour, la sauvegarde, les incidents
                           │
                         envois     ce qui part et revient ; le journal
                           │
                       declarations la DSN, la DPAE, la norme
                           │
                       documents    contrat, bulletin, certificat… (ne calcule rien)
                           │
                       engagement   la saisie, le calcul (le seul), la validation, le recalcul
                           │
                         droit      barèmes datés, conventions, natures de contrat, règles ; lit tout à une date
                           │
                         socle      dates, heure de Paris, temps simulé, nombres, texte, fichiers, garde-fou réseau
```

| crate | ce qu'elle fait | ce qu'elle ne fait pas |
|---|---|---|
| `socle` | dates, heure de Paris, temps simulé, nombres (`d28` : 28 chiffres significatifs, au pair sur le demi après chaque opération), texte, HTML et base64, fichiers, échecs, garde-fou réseau | aucun droit, aucun métier |
| `droit` | barèmes datés et leurs trois étages, conventions, natures de contrat, minima, congés, PAS, cumuls des annexes 8 et 10, effectif | ne connaît pas l'engagement |
| `engagement` | la saisie, le calcul de ses paies, la validation, la modification et ses versions, le recalcul de lui-même, l'engagement importé, l'engagement non actif, les écarts d'une paie importée avec le calcul du moteur ; reçoit ce qui est parti (`departs`) | ne fabrique aucun document, ne lit jamais le journal |
| `documents` | contrat, avenant, bulletin, certificat Congés Spectacles, AER, questionnaire, récapitulatif annuel, bordereau, prélèvement, états de paie, du personnel et de comptabilité, plan comptable | ne calcule rien |
| `declarations` | DSN mensuelle et signalements, DPAE, régularisations, bloc 44, CSA, OETH, CASC-SVP, et la norme qui les contrôle (usages, formats, natures, champs) ; prend à `documents` | ne calcule rien, n'envoie rien |
| `envois` | ce qui part et revient : net-entreprises, DPAE, signature, courriel, Drive, retours et bilans ; le journal de ce qui est parti ; le branchement (le monde, le simulateur, le message) ; prend à `declarations` et à `documents` ce qui part | ne fabrique rien, n'ouvre jamais le coffre |
| `compagnie` | ce qui garde la compagnie et la fait vivre à côté de ses engagements : l'installation qui la porte sur le poste (`installation`, `instance`), le coffre et ses deux mots de passe, la preuve d'un acte réel (`preuve`) et sa demande (`demande`), la sauvegarde et la restauration, la mise à jour (`maj`), le bridage, les incidents et les signalements, le journal des pièces écrites, l'état de la DSN d'un mois (`etat_dsn`), les organismes et les taux de PAS reçus | ne prend rien aux routes |
| `import` | le passé d'une compagnie tenu dans un autre logiciel, tel que ses pièces le disent : le tri de la boîte aux lettres, les DSN et AEM (`dsn_aem`), le lecteur unique des bulletins (`bulletin_lu`), les certificats et les AEM en PDF, ce qu'une pièce atteste (`attestes`), les fiches proposées, le mois importé, les écarts de calcul, le caviardage | n'écrit rien pour une route : c'est `routes/src/importation/` |
| `routes` | `moteur route` : la porte (`lib.rs` : l'aiguillage ; `enveloppe`, `catalogue`), une route par geste de l'écran, et ce qu'elles font ; seules à décider, la ligne de commande et la page ne font que les appeler (règle 2) | ne prend rien à l'interface, aux commandes ni à l'atelier |
| `moteur` | la ligne de commande (`moteur`), la page, la publication par GitHub, en dossiers (ci-dessous) | ne touche pas le JSON brut (la porte est aux routes) ; ne connaît aucune commande de l'atelier |
| `fabrique-pdf` | la fabrique des PDF : les modèles, la mise en page, le marquage, la lecture ; hors de l'espace Cargo (`exclude`), prise par `documents` et ce qui est au-dessus | ne prend aucune crate de l'espace |
| `atelier` | tout ce qui n'est pas le logiciel, dans le binaire du nuage `moteur-atelier` (la ligne de commande du moteur, plus la sienne) : le relevé des textes (`releve/` : LEGI, KALI, les missions de la DGFiP), les référentiels extraits (`extraire/`), les tables Unicode (`generer_tables`), la vérification avant commit, le guet, la veille des sources (`sources`), la fabrique des bulletins fictifs, les outils sur une compagnie (`commandes/outils/`), la redirection vers de faux serveurs, et les commandes `scenario` du banc | n'est jamais livré ; rien ne le prend, et il est hors de `default-members` (il allume la feature `atelier` de ce qu'il prend) |

À part : `fenetre/` (la fenêtre Tauri, programme `puppetpaie`, qui ne fait qu'afficher la page).

**Ce que porte `routes/src/`**, à côté d'une route par fichier :

| dossier | ce qu'il porte |
|---|---|
| `gestes/` | les gestes de la compagnie qui appellent des routes : les portes d'un dépôt réel (`depot_acte`), le départ des pièces (`depart`), le passage de la sauvegarde, l'ouverture, l'administrateur, la clôture et l'export compta (`exercice_ecrit`) |
| `importation/` | ce que l'import compose et écrit pour les routes : les engagements importés (`ecrite`), la nature à dire, la CASC-SVP des mois importés, le document pas lu |
| `accueil/` | ce que l'accueil fait des compagnies de l'installation (`accueil.rs` : ses routes, l'ajout ; `suppression`) |
| `metier/` | le métier que des routes partagent (`imprevus`) |

À sa racine, à côté des routes : `service.rs` (la demande et la réponse de `moteur route`), `etat_garde.rs` (l'état
d'un mois et de l'ouverture tels que l'écran les lit, refaits en fond), `veille.rs` (ce que la veille dit à la page),
`horloge.rs` (l'horloge des compagnies fictives), `scenario.rs` (le scénario d'une compagnie et son rejeu, le circuit
des fictives), et ce que l'écran lit d'un mois : `apercu_dsn.rs`, `historique_mois.rs`, `obligations.rs`,
`regularisation.rs`, `simulation_courriels.rs`.

**Les dossiers de `moteur/src/`** :

| dossier | ce qu'il porte |
|---|---|
| `interface/` | le serveur local de la page (127.0.0.1, un jeton par appel), l'accueil et ses compagnies, une fenêtre par installation, la veille qui tourne |
| `publication/` | ce que le flux « Publier » appelle sur le `moteur` qu'il vient de compiler (`moteur maj --publier-github`, `--cle-github`, `--marquer-fictives`, `--poser-fictives`) : les archives, la signature par la graine de GitHub, les fictives livrées |
| `commandes/` | ce qui n'est pas un geste de l'écran : le coffre, la restauration, la mise à jour ; l'aiguillage de la ligne de commande (`ligne_de_commande`), que `moteur-atelier` reprend avec ses commandes ; les gestes passent par `moteur route` |

À la racine : `lib.rs`, `main.rs`.

## Le flux

```
   la page (logiciel/interface/) ou la ligne de commande
              │  moteur route
              ▼
   routes/ ── un geste : saisir, valider, modifier, envoyer…
              ▼
   engagement ── engagements/AAAA-MM.json et paies/AAAA-MM.json ; lit config/ (datée)
              │   et les barèmes au jour de la période ; calcule et écrit les paies à la validation
              ▼
   documents, declarations ── contrat, DPAE, bulletin, certificat, DSN…
              │                fabriqués au départ, depuis les paies écrites
              ▼
   envois ── dépôt par l'API, signature, courriel, contre le mot de passe du logiciel
              │
              ▼
   journal/depots.jsonl ─ ce qui est parti, sur quelle version de chaque
                          engagement, et ce que l'organisme en a dit
```

Plus d'AEM à produire : la compagnie est en 100 % DSN, France Travail édite l'attestation lui-même et la renvoie
(la relève des retours) ; l'import relit les AEM d'un ancien logiciel (`import/src/dsn_aem.rs`, `lire_aem`). La source :
le courriel de France Travail du 28/05/2026 (`reference-privee/sources/francetravail-100-dsn/
francetravail-courriel-fin-phase-pilote-100-dsn-20260528.pdf`, dépôt privé) — fin des AEM, AE Cinéma Spectacle remise
au salarié, `S21.G00.40.025` vide ; la date vaut pour la compagnie qui l'a reçu, une autre reçoit la sienne.

## L'engagement : un objet, ses paies validées dans le fichier de leur mois

L'engagement est au centre (crate `engagement`, `atelier/sources/engagement/`). Il vit dans
`engagements/AAAA-MM.json`, le fichier **du mois où il commence**, et nulle part ailleurs ; un engagement de
novembre à janvier est un seul objet de `engagements/2026-11.json`. Ses paies n'y sont pas : chacune vit dans
`paies/AAAA-MM.json`, le fichier du mois payé (`engagement/src/paies.rs`, `{"mois", "paies": [...]}`), une entrée
par engagement désignée par son salarié et son numéro de contrat, et sous `versions` la paie de ce mois de chaque
version passée ; `engagement::paies` seul lit et écrit ce fichier, dans le même brouillon que l'engagement.

- **Sa forme** (`engagement/src/stock.rs`) : son premier mois en clair (salarié, spectacle, `debut`, `fin`, lignes de
  paie datées…). Un terme du contrat (nature, salaire, durée du travail, emploi, spectacle… : `engagement::saisie::Groupe::Terme`) qui change
  ensuite s'écrit sous `termes`, une liste datée par terme (`[{"depuis": "AAAA-MM-JJ", "valeur"}]`, la forme de la
  configuration ; `null` : retiré), et vaut de son jour au changement suivant : le mois de la carte qui le change est le
  « à partir de » (le 1er, ou le jour dit). Ce qu'un mois d'après dit de lui seul (lignes, congés pris, primes, rupture)
  s'écrit sous `faits`, à ce mois. Le salarié, les bornes et le numéro de contrat sont ceux de l'engagement dans tous
  ses mois (`saisie::Groupe::Partage`). La vue d'un mois se lit à la date, sans relire les mois d'entre-deux : chaque terme au dernier
  jour du mois, les faits du mois ; un contrat d'usage sans faits ce mois-là prend les jours que le planning du dernier
  mois qui en dit y met ; un contrat au mois dont un terme change dans le mois y porte l'avenant daté
  (`date_modification`, et `avant` quand il change en cours de mois), déduits. Le fichier d'un mois porte aussi les
  défauts du mois (`taux_cachet`, `date_paiement`).
- **Le mois de l'écran** (`stock::vue_du_mois`) : le fichier du mois et chaque engagement d'un mois d'avant qui court
  encore, vu ce mois-là. L'écran lit et écrit cette vue (`stock::ecrire_la_carte`, `stock::poser_la_saisie`) ; chaque engagement y retrouve
  son objet (numéro, puis salarié). Un engagement validé ne change pas par ce chemin.
- **Ses états** (`engagement/src/etats.rs`), chacun son type : **en saisie** (`EnSaisie` : calculé à mesure pour
  l'écran, rien ne part de lui) ; **validé** (`Valide` : « Valider l'engagement » écrit sous `valide` la date, le numéro
  de sa version et sa saisie telle qu'elle était, et dans `paies/` une paie par mois couvert, avec son prélèvement à la source et
  l'empreinte de ce dont le calcul dépend : fichiers lus, fiches, saisie des mois lus, code du moteur, règles datées) ;
  **documents partis** pour un mois (`Partis` : le bulletin, le certificat Congés Spectacles et l'AER du mois sont
  partis au salarié). Seul un engagement validé donne une paie à qui fabrique un document : tout document se fabrique
  depuis les paies écrites (`etats::validees_du_mois`, `etats::paies_du_mois`, qui refuse un mois dont un engagement est
  en saisie) ; aucun ne recalcule. Deux états de plus, à part :
  - **importé** (`engagement/src/importe.rs`, la clé `importe`) : tiré par l'import des pièces d'un autre logiciel,
    validé et documents partis (ce qui en est parti, c'est l'import lui-même). Il se lit, ne se modifie pas et ne se
    recalcule jamais : il ne touche aucun mois de la saisie (`stock::Objet::couvre`), donc aucun calcul, aucune
    déclaration ni aucun document ne le prend, et l'écran ne peut ni le réécrire ni en poser un double. Son numéro de
    contrat est celui que la DSN déposée lui a donné (S21.G00.40.009). Il compte dans les cumuls des annexes 8 et 10
    (`engagement/src/cumul.rs`) et dans les numéros pris.
  - **non actif** (`etats::NonActif`, la clé `non_actif`) : un engagement dont quelque chose est parti ne se supprime
    plus ; « Retirer cet engagement » (route `retirer-engagement`) le rend non actif. Il garde ses versions, ne se calcule
    plus, n'entre dans aucun document ni aucune déclaration ; le moteur dit ce que sa désactivation rend faux (un mois
    déclaré à corriger). Un engagement dont rien n'est parti se supprime, même validé, et rend son numéro.
- **Les versions datées** : tant que rien n'est parti d'un engagement, il se modifie librement, sans historique. Dès
  qu'un envoi est parti sur sa version, une revalidation qui le change garde la version d'avant dans l'engagement
  (`versions`, sa saisie), datée, ses paies sous son numéro dans le fichier de leur mois ; le présent prend le numéro suivant. Ce qui est à refaire (`etats::a_refaire` : avenant, DPAE,
  bulletin rectificatif, DSN) se tire de la comparaison entre le présent et la version sur laquelle chaque envoi est
  parti. L'engagement ne lit jamais le journal : le moteur lui passe ce qui est parti (`engagement/src/departs.rs`).
- **Chaque document se fabrique à son départ** (`routes/src/gestes/depart.rs`) : la DSN à son dépôt (`fabriquer_dsn`, route
  `depot-apercu`, la DSN désignée par son mois), dès qu'un engagement du mois est validé, sans attendre DPAE ni contrat ; le bulletin et le certificat
  Congés Spectacles à l'envoi des documents (`fabriquer_documents`), qui écrit dans la paie le numéro du bulletin
  (`N/AAAA`, la suite de l'année, après le plus haut que porte une paie de l'année, importée comprise) et le premier
  N° CE (`etats::poser_numeros_des_documents`). Un bulletin parti dont la paie change ensuite a son rectificatif, sous
  le numéro suivant, l'original gardé ; un engagement dont aucun bulletin n'est parti a son bulletin ordinaire, du mois
  d'origine. Après la veille de l'échéance, un mois déclaré ne se redépose plus (`AnnuleRemplaceHorsDelai`) : sa
  correction part dans la DSN d'un mois suivant (`declarations::produire_regularisations::regularisations_pour`). La DSN d'un mois déposée par l'ancien logiciel ne
  se refait pas ici (règle 4) : ce que la saisie ajoute à ce mois se déclare dans la DSN suivante. L'AER ne se fabrique
  pas : France Travail la renvoie, et elle se vérifie contre les paies écrites.
- **Une pièce partie dont le fichier manque se refait pour se lire** (`depart::piece_partie`, route `piece-partie`, que
  la page appelle avant d'ouvrir le chemin rendu ; l'Historique liste ce que le journal dit parti, fichier présent ou non, et un mois y lit aussi
  les signalements de ses jours). Elle se refait par la fonction qui l'a fabriquée, depuis ce que sa ligne du journal
  porte : les engagements dans la version qu'elle a emportée (`versions`), au jour et à l'heure où elle a été écrite (le
  journal des écritures, `dsn-ecrites.json`, sinon l'instant de la ligne ; `socle::temps::pendant`), sous le numéro
  d'ordre de sa ligne. Se refont ainsi la DSN mensuelle (avec les arrêts qu'elle reporte et les corrections qu'elle
  portait), la DPAE (son `.zip`, son récapitulatif), les signalements d'amorçage, d'arrêt, de reprise et de fin de
  contrat, le contrat et l'avenant (et leur description gardée), le bulletin, le rectificatif et le bulletin d'origine
  gardé à côté de lui, le certificat Congés Spectacles, et l'archive du mois. Ce qui a une empreinte au journal (DSN,
  DPAE, signalements, contrat et avenant partis à la signature) se vérifie par elle, un contrat aussi par les clauses que
  sa ligne a notées : une pièce refaite qui ne dit pas ce qui est parti s'efface et la page dit que le fichier manque.
  L'exemplaire contresigné est le contrat parti, octet pour octet, plus la mention de l'artiste (une mise à jour
  incrémentale, `fabrique_pdf::marquage::contresigner`) : il se refait du contrat parti et de la mention notée. La fabrique des PDF est déterministe (date de
  création fixe), rien n'y dépend de l'heure de la machine. **Ne se refait jamais ce qui vient de dehors** : les bilans et comptes rendus ; l'AER se relit dans `retours/` (l'archive du retour
  46) et se range comme à sa relève. Règle 4 : une pièce refaite se lit, elle ne repart jamais ; ce qui repart se
  fabrique à son départ. Un annule-et-remplace de signalement ne se refait pas : sa ligne ne dit pas ce qu'il remplaçait.
- **Un contrat rompu n'a pas d'avenant** (code du travail L. 1243-1, `reference/sources/code-du-travail/`) : la rupture
  d'un CDD avant son terme met fin au contrat signé, elle ne le modifie pas. Une saisie qui ne diffère du contrat parti
  que par sa rupture (le motif, la fin et le préavis qu'il appelle, les jours d'après la fin retirés) ne le rend pas à
  refaire signer (`engagement::etats::seule_la_rupture`, lu par `routes::etapes::a_refaire_du_mois`) ; ce qui change en
  même temps (un taux) en fait un avenant.
- **Un contrat signé hors du logiciel a ses avenants** (`engagement::signe_ailleurs` : commencé avant le logiciel, ou
  repris d'un ancien logiciel ; code du travail R. 1221-40). Il ne se fait pas signer ici ; ce qu'il disait est ce que dit
  le premier mois de l'engagement, puis la version de son dernier avenant parti. Un terme daté depuis (`termes` de la
  saisie) en fait un avenant à signer (`etats::a_refaire`, `ARefaire::Avenant { depuis: None }`), qui ne cite aucun
  article : chaque terme qui change se dit en clair, l'ancienne valeur, la nouvelle, à partir de quand
  (`versions::termes_changes`, `documents::avenant::changements_en_clair`, le paragraphe `{{modification.phrase}}` du
  modèle d'avenant, `{{avenant.reference}}` : « commencé le 01/03/2022 »). Le PDF du contrat signé, déposé par la
  compagnie dans `salaries/<clé>/contrat-du-<début>/` (le chemin que rendent `avenant-apercu` et
  `etat-par-engagement`, la fenêtre l'ouvre), part joint à chaque avenant envoyé ; le logiciel ne le lit pas.
- **Le recalcul de lui-même** (`etats::rafraichir`, à chaque lecture d'un mois, `etats::du_mois`) : une paie validée
  dont l'empreinte ne tient plus et dont les documents ne sont pas partis se recalcule, d'un seul calcul du mois. Si un
  chiffre change, elle dit pourquoi : « taux de PAS reçu », « règle du JJ/MM/AAAA », « calcul corrigé » (le code ou ce
  qu'il livre), « un autre engagement du salarié a changé », « un arrêt ou une absence a changé », « configuration de la compagnie modifiée ». L'écran le montre
  (« Recalculé le JJ/MM/AAAA : … »). Sans chiffre changé, rien ne se dit. Un recalcul que le moteur refuse
  garde la paie validée et se note en incident pour l'éditeur (le témoin des recalculs refusés, `compagnie::incidents`).
- **Modifier…** (route `modifier-engagement`) écrit ses `changements` sur la carte d'un engagement validé et le
  revalide. Si ce qui est parti le demande, la route rend un avertissement et n'écrit rien sans `confirme` ; la
  revalidation reprend telles quelles les paies des mois partis si la saisie de leur mois n'a pas changé, et en fait une
  version si quelque chose est parti sur elle.
- **Le numéro de contrat** (`engagement/src/numeros.rs`, règle 4) s'écrit dans l'engagement à sa validation
  (`numeros::a_la_validation`) : le suivant du plus haut que portent, pour le même salarié et la même année, les
  engagements et ce qui est parti. Tout ce qui part (le contrat signé ou imprimé, la DPAE, la DSN, le bulletin) le lit
  là. Tant que rien n'est parti sous lui, un engagement dont le salarié ou le début change en reçoit un nouveau ; dès
  qu'un envoi l'a porté, il ne change plus. La norme n'exige aucune suite continue (S21.G00.40.009, CCH-12).

## La ligne de paie datée

Le fichier de saisie ne décrit pas un engagement par ses dates et son lieu,
mais par ses **lignes de paie** : « 4 cachets à 130 € au Théâtre 71 le 14, 15,
17, 18 ». Une ligne est un groupe de cachets, à un taux, à un endroit, à des
dates. Deux villes, deux lignes — et c'est une seule paie, donc un contrat, un
bulletin, une AEM. Le format est dans le `README.md`.

Ce que ça impose au moteur :

- `jours_des_lignes` (`engagement/src/lib.rs`) est le **seul endroit** qui sache où
  chercher les jours d'un engagement : dans ses lignes de travail (une ligne de frais n'en fait
  aucun, `LignePaie::frais`), et nulle part
  ailleurs. La lecture d'un engagement (`Engagement::nouveau`) passe par lui, et le décompte des droits (`cumul.rs`)
  lit ce qu'elle a lu.
- **Une seule écriture.** `Engagement` exige
  `lignes` ; `cachets` et `heures` en sont **dérivés** (l'AEM, le bloc 53 de
  la DSN, le plafond et `cumul` les lisent toujours), `taux_cachet` est le
  taux de la première ligne `cachet`. Un
  engagement portant `dates`, `cachets`, `heures` ou `taux_horaire` est
  refusé, avec ce que le moteur a vu et ce qu'une saisie porte.
- Une date de ligne est une `engagement::ligne::DateDeLigne` : nue (un jour), ou détaillée `{"jour": …, "lieu": …}` ;
  une date détaillée est une date, tout ce qui parcourt des jours continue de marcher sans le savoir.
- La construction d'un `Engagement` (`engagement/src/lib.rs`) réunit les deux écritures et n'en
  laisse qu'une : `dates_travail` reste une liste de dates triées, le lieu part
  dans `lieux` (`lieu_de()`, `lieux_joues()`). Le contrat groupe les dates par
  lieu ; le lieu ne se déclare nulle part ailleurs (ni DSN, ni AEM, ni DPAE).
- **Une autre écriture ne se lit pas** (`dates`, `cachets`, `heures` ou
  `taux_horaire` sur l'engagement). Le moteur refuse un tel engagement et dit ce qu'une
  saisie porte (`engagement/src/lib.rs`, à la
  lecture d'un engagement sans `lignes` et à la lecture du mois). Ce qui reste
  lu tel quel, ce sont les dates d'une ligne écrites sans lieu (une chaîne) à
  côté des dates détaillées.

## Les métiers, les minima, et les sources qu'on date

Trois fichiers de données livrés avec le moteur, et une règle : **toute
donnée réglementaire part d'une source officielle téléchargée, décrite,
datée, vérifiable**.

- `logiciel/donnees/nomenclature-emplois-ft.json` — les **1 236 codes emploi** France
  Travail (annexes 8 et 10), **générés** depuis le PDF de la nomenclature.
- `logiciel/donnees/emplois-base.json` — **129 métiers du spectacle vivant** en onze
  familles. Chaque métier : libellé (et féminin), `emploi` officiel
  (S21.G00.40.006), `pcs_ese` (40.004, depuis les fiches Insee), `statut`
  (déduit de l'annexe, jamais saisi), `cadre_par_defaut`,
  `statut_conventionnel` (40.002 : cadre → 04, artiste et agent de maîtrise
  → 05, administratif non cadre → 06, employé technique → 07),
  `classification_3090` et `minima`. `droit::emplois::resoudre`
  rend sept champs : les six déclarés et le libellé lisible
  (`emploi_affichage`, au féminin si la fiche dit sexe 2). La lecture de l'engagement les
  pose tous les sept sur l'engagement, sauf ce que l'engagement écrit
  lui-même (un `emploi` écrit à la main ne garde pas le libellé du métier) ;
  `emplois::emploi_de` les reporte sur la fiche de cet engagement, et le contrat lit
  le libellé de là. Le cadre se combine au statut.
- `logiciel/donnees/minima-conventionnels.json` — les grilles de la **3090** (annexes
  1, 2, 4, 5 et clauses communes) et de la **1285** (cachet, répétition,
  cirque, musiciens, lyrique, chœur ; les accords suivants sous `accords`, chacun avec sa grille entière et la date où il lie la compagnie, `droit::minima::lie_depuis`).
  `engagement::minima::avertissements` compare chaque ligne à son plancher et
  rend une phrase : un salaire sous son minimum ne se refuse pas. **Toute inconnue tire vers le bas** :
  jauge minimale, palier du mois pour un barème trimestriel, la plus basse
  des annexes quand la fiche entreprise ne dit pas la sienne
  (`annexe_convention`). Quand une grille distingue les jauges, le métier
  pointe la ligne la plus basse et la grille porte les autres pour mémoire.
  Sauf l'échelon et l'ancienneté, que seule la compagnie connaît : l'échelon
  CCNEAC d'un emploi autre qu'artistique (`echelon_ccneac`, un entier de 1
  à 12, attribué par l'employeur, X.4.1) et l'ancienneté dans le poste
  (`poste_depuis`) partout où un minimum en dépend (la grille administrative
  de la 3090, les conditions `anciennete` des grilles) ; sans la case, le
  calcul refuse en la nommant, jamais « aucune ancienneté ».
- `logiciel/donnees/sources.json` — le **manifeste** : pour chaque source, titre,
  éditeur, URL, description, version et d'où on la lit, date de relevé,
  fichier de la copie (dans `atelier/reference/sources/`, versionnée dans le
  dépôt ; l'original d'un organisme privé n'y est pas, seule
  sa transcription), SHA-256, mode de vérification, et
  quels fichiers de données elle nourrit. `moteur-atelier sources --verifier`
  (`atelier/src/sources.rs`) retélécharge et compare ; une source `changee` est à RELIRE
  (`--relever` pose la nouvelle version à côté). Légifrance refuse les
  robots (403) : la copie ne se revérifie pas en ligne, mais la source
  porte un bloc `kali` (IDCC, conteneur, texte) et `atelier/src/releve/kali.rs` lit
  les **livraisons open data KALI** de la DILA
  (`echanges.dila.gouv.fr/OPENDATA/KALI/`, une archive par soir) depuis
  le relevé : fiche de la convention relivrée avec des textes nouveaux,
  texte modifié (état, arrêté d'extension). Un texte dont le titre parle
  de salaires, ou le texte cité, rend la source `changee` ; le reste est
  montré en second. Lecture incrémentale, résumée dans
  `atelier/reference/sources/veille-kali.json` (livraisons lues, liste des
  textes de chaque convention, trous de l'index) ; à la première veille,
  l'index est remonté jusqu'à la dernière fiche de chaque convention.
  L'ouverture d'une compagnie ne fait pas cette vérification
  (`routes/src/gestes/ouverture.rs`) : elle se lance dans le nuage,
  avant chaque publication.
- `moteur-atelier releve-codes` (`atelier/src/releve/codes.rs`)
  — **les articles de loi eux-mêmes**, lus dans la base **LEGI**
  de la DILA (même voie que KALI) plutôt qu'à la main sur Légifrance, qui
  refuse les robots. Tous les textes en un seul passage
  (`atelier/src/releve/codes/textes.rs`) : travail, sécurité sociale, collectivités
  territoriales (le versement mobilité), construction, impôts, et les arrêtés
  et la loi que la paie lit. Trois verbes, sur le dossier des sources :
  - `moissonner` tient à jour l'entrepôt `legi-entrepot/`
    (`atelier/src/releve/entrepot_dila.rs`) : la première fois, ou avec
    `--reconstruire`, l'archive globale défile (1,1 Go, en flux, jamais
    écrite sur le disque) puis **chaque livraison quotidienne depuis sa
    date** ; ensuite seules les livraisons nouvelles se téléchargent. Il garde
    la version *en vigueur* de chaque article — `<ETAT>` à VIGUEUR et
    `<DATE_FIN>` à 2999-01-01, vérifié, parce que le dossier
    `code_en_vigueur` porte aussi les versions périmées —, note au passage
    quel article a changé de version, et depuis quelle livraison, puis écrit
    `releve.json` et `xml/` de chaque texte ;
  - `rapport` écrit le document lisible d'un code à partir de ce qui a été
    moissonné, chaque article cité avec l'identifiant de **sa** version ;
  - `comparer` répond à la question que `moissonner` pose. « Huit articles
    ont changé de version » ne dit pas si le *droit* a changé : il faut
    relire chacun contre sa version d'avant, et **cette version-là ne se
    trouve que dans l'archive globale** — ni le relevé, qui ne garde que la
    version en vigueur, ni l'historique Git, où un relevé précédent portait
    déjà celle d'après. `comparer` va l'y chercher **par son identifiant**
    (jamais par le numéro : l'archive porte plusieurs rédactions du même
    article), la range dans `<code>/xml-anterieur/` et écrit le diff,
    alinéa par alinéa. Il s'arrête dès qu'il les a toutes : comptez un quart
    d'heure. Il compare le `BLOC_TEXTUEL` seul, et le dit.
- `moteur-atelier releve-conventions` (`atelier/src/releve/conventions.rs`) — le texte
  complet des conventions que cite `sources.json`, tenu dans l'entrepôt
  `kali-entrepot/` : `mettre-a-jour` (les seules livraisons KALI nouvelles ;
  la base entière, 180 Mo, la première fois ou pour une convention nouvelle),
  `article <IDCC> <numéro>…`, `chercher <IDCC> <mot>`.

## Le barème, lu sur un bulletin

Un bulletin d'artiste au cachet, brut 260,00 €, dans la convention 3090 :

| Rubrique | Base | Patronal | Salarial |
|---|---|---|---|
| Maladie maternité invalidité décès | brut | 9,100 | — |
| Prévoyance Audiens | brut | 1,110 | 0,120 |
| AT/MP | brut | 1,050 | — |
| Vieillesse plafonnée | brut | 5,990 | 4,830 |
| Vieillesse déplafonnée | brut | 1,480 | 0,280 |
| Retraite complémentaire T1 | brut | 5,740 | 5,300 |
| Famille | brut | 3,680 | — |
| Chômage | brut | 9,750 | 2,400 |
| Autres contributions | brut | 2,836 | — |
| Autres contributions | brut × 1,115 | 0,070 | — |
| Autres contributions | brut × 1,10 | 0,590 + 0,090 | — |
| Congés Spectacles | brut | 15,500 | — |
| Cotisations statutaires CCN | brut | 0,500 | — |
| CSG déductible | assiette CSG | — | 6,800 |
| CSG/CRDS non déductible | assiette CSG | — | 2,900 |

Les taux de vieillesse et de famille sont les taux de droit commun réduits de
30 % : c'est l'abattement applicable aux artistes du spectacle (8,55 × 0,70 =
5,985 ≈ 5,990 ; 6,90 × 0,70 = 4,83 ; 5,25 × 0,70 = 3,675 ≈ 3,680).

**Le plafond de Sécurité sociale est dans le barème** (`plafond_securite_sociale`
: `{"mensuel": 4005, "horaire": 30}`, source BOSS). Le module
(`droit/src/plafond.rs`) ne porte que la règle, et
`calcul::plafond_de_l_engagement` en est la seule source : le bas du bulletin,
les bases du calcul, la DSN et le bordereau la suivent. La règle, telle que
le module la dit avec ses textes (`atelier/reference/sources/`) :

- un **artiste** engagé moins de cinq jours calendaires : 12 × le PSS horaire
  par journée de travail de la paie (arrêté du 24/01/1975, art. 3, al. 2) ;
- tout **autre intermittent** (artiste engagé cinq jours ou plus, technicien) :
  le PSS mensuel au prorata des jours rémunérés de la paie, mois par mois (la
  tolérance du BOSS, § 1090, appliquée : le BOSS est opposable) ;
- un **permanent** : le PSS au prorata des jours de sa période de paie
  couverts par le contrat (CSS R. 242-2) ; à **temps partiel**, proratisé
  aussi à sa quotité (CSS L. 242-8), sauf si la fiche entreprise dit que
  l'établissement y renonce (`plafond_temps_partiel` : « entier ») ; un
  artiste à temps partiel garde le plafond entier (BOSS § 1090). Le choix se
  déclare en `S21.G00.40.084` à partir de P27V01 (`calcul::plafond_temps_partiel`,
  `dsn::proratisation_du_plafond`).

Sur ce plafond s'assoient, pour un intermittent, la vieillesse plafonnée, le
FNAL (`brut_plafonne`, `brut_x115_plafonne`) et le chômage, dans la limite de
quatre plafonds (`brut_chomage`) ; pour un permanent, les tranches et le
chômage, dans la limite de quatre plafonds de la période (`brut_chomage`).

**Une valeur du barème peut être datée** : le SMIC (`smic_horaire`, liste de `{depuis, valeur}`), et toute autre
valeur — un taux, un plafond —, parce que le droit change en cours d'année (AGS 0,20 → 0,25 au 01/07/2024,
chômage 11,45 → 11,40 au 01/06/2025) :

```json
"taux": [{"depuis": "2024-01-01", "valeur": 0.20},
         {"depuis": "2024-07-01", "valeur": 0.25}]
```

`Contexte::bareme(jour)` rend le barème **résolu à cette date** ; le calcul et
la DSN lisent des nombres et n'en savent rien. Une date antérieure à toute
entrée est un refus, pas un zéro.

**La configuration de la compagnie est datée de même.** Chaque valeur d'une
fiche — `config/entreprise.json` (convention, annexe, adresse, établissements…),
chaque salarié de `salaries.json`, chaque spectacle de `spectacles.json` —
s'écrit `[{"valeur": …}, {"depuis": "AAAA-MM-JJ", "valeur": …}]` : la première
valeur, sans `depuis`, vaut **de tout temps** (un contrat commencé avant la
saisie de la fiche, un mois repris d'un autre logiciel la trouvent), chacune des
suivantes à partir de sa date ; les clés `_…` sont des notes. Les organismes
et contrats saisis par la compagnie (`config/organismes.json`, le reste se composant des textes à la date lue :
`droit::organismes`, `Contexte::organismes` ; enregistrer pour un mois écrit à partir de son premier
jour) et les rubriques propres à la compagnie (`config/rubriques-compagnie.json`,
lues par `catalogue::catalogue(racine, jour)`) sont datés de même. Une seule
lecture, `droit::fiche::valeur_au` ; une valeur non datée ne passe pas la forme
`Datee`. Le
`Contexte` garde les fiches datées et les lit à `lue_au` ; `Contexte::au(jour)`
et `du_mois(mois)` donnent le contexte d'une période. Le calcul d'une période
lit tout à sa fin (`calculer_periode`), la DSN d'un mois et ses documents à son
dernier jour (`dsn::generer`, `entreprise_du_mois`, et toute route qui porte un
`mois`, `routes::dans_la_fenetre`), et le barème d'un jour applique la
convention de ce jour. On écrit par `droit::fiche::dater`, et seulement par lui : la
première valeur d'un champ s'y écrit de tout temps, quelle que soit la date
donnée ; changer une valeur existante exige sa date d'effet
(`ecrire_datee`, `routes/src/fiches/ecriture.rs` ; l'écran la demande alors) ; ce qui précède
garde l'ancienne valeur. Ce qu'une case datée dit à un jour (sa valeur en vigueur, la dernière d'avant, la prochaine à
venir, chacune avec sa note, et toutes) se lit par `fiche::temps_au`, seule (la valeur d'avant dit un changement : jamais une case vide, ni la même valeur
réécrite) : la route de chaque fiche le rend à la date du jour sous chaque case (`historique` de la fiche, les autres
établissements case par case), les contrats de la DSN saisis (`organismes-contrats`) et le taux AT/MP (`taux-atmp`) de
même, mis en phrases par `routes::temps`.

Trois subtilités, chacune trouvée en cherchant pourquoi un centime manquait.

**L'assiette CSG n'est pas 98,25 % du brut.** Elle vaut
`arrondi(brut × 0,9825) + part patronale de prévoyance`, soit
255,45 + 2,89 = 258,34 pour 260 € de brut, et 510,90 + 5,77 = 516,67 pour 520 €.

**Une ligne du bulletin peut cacher deux cotisations.** La ligne
« autres contributions 0,680 % » sur 286,00 € affiche 1,95 € alors que
286 × 0,68 % = 1,9448 → 1,94. Ce sont en réalité deux contributions arrondies
séparément : 0,590 % → 1,69 et 0,090 % → 0,26. Même mécanisme pour la
cotisation prévoyance `059` de la DSN (0,420 + 0,810 ; « Cotisation individuelle Prévoyance-Assurance-Mutuelle », versée à Audiens Santé Prévoyance, P0983).

**Tous les arrondis au centime sont au demi supérieur** : `eur`
(`socle/src/num.rs`) arrondit un `Decimal` au demi qui s'éloigne de zéro
(`MidpointAwayFromZero`) ; un arrondi au pair se tromperait d'un centime régulièrement.

## Le SMIC déclaré en DSN (composant 01 de la base 03)

Ce n'est pas un plafond. `S21.G00.79.001 = '01'` est le « Montant du SMIC retenu pour le
calcul de la Réduction générale », rattaché à une base 03 (cahier technique
2026.1, p. 299-300 ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.79.txt`).

Dans des DSN acceptées d'une compagnie, il vaut **SMIC horaire × 7 h × nombre de
cachets**. Vu sur soixante-dix contrats de 2021 à 2026 : la valeur par
cachet suit exactement les revalorisations du SMIC (10,25 → 10,48 → 10,57 →
10,85 → 11,07 → 11,27 → 11,65 → 11,88 → 12,02 → 12,31), et pour les contrats
payés à l'heure c'est SMIC × nombre d'heures — 52 h en octobre 2025, 140 h en
novembre 2023.

Le moteur (`dsn::bases_et_cotisations`) écrit sous chaque base 03 le SMIC de
référence de la réduction générale quand la paie en a une
(`ReductionGenerale::smic_reference`), sinon `Paie::plafond` — qui, malgré
son nom, n'est pas un plafond de Sécurité sociale mais le SMIC horaire × les
heures de la paie (`engagement/src/calcul.rs`, « 7.5 » ; les heures d'un intermittent
bornées à `heures_par_cachet` × jours) : la valeur que ces DSN montrent. Un artiste n'a
pas de réduction générale et reçoit quand même ce composant. Le plafond de Sécurité sociale est
décrit plus haut, avec le barème.

## L'AEM V5

Le moteur n'émet pas d'AEM : il relit, à l'import, celles qu'un ancien logiciel a déposées
(`import/src/dsn_aem.rs`, `lire_aem`). Enregistrements de 1000 caractères, latin-1, séparés par CRLF. La
queue numérique est celle-ci :

| Position | Contenu |
|---|---|
| 650-653 | nombre d'**heures** effectuées |
| 653-655 | nombre de **cachets** |
| 655-659 | nombre de **jours** travaillés |
| 659-668 | brut en centimes |
| 668-677 | assiette des contributions chômage en centimes |
| 677-681 | taux de contribution chômage × 100 (1215 = 12,15 %) |
| 681-690 | montant de contribution, **arrondi à l'euro** puis × 100 |
| 690-712 | zéros |
| 712-721 | même montant |

Ce schéma se vérifie sur 189 enregistrements déposés de 2021 à 2026 : 189/189. La contribution est arrondie à
l'euro, non au centime (119,79 → 120, 7,29 → 7, 36,30 → 36).

### Les positions 650-659

Le formulaire officiel de l'AEM V5 donne les trois champs accolés : **Nombre d'HEURES effectuées**, **Nombre de
CACHETS**, **Nombre de JOURS travaillés**, en 3 / 2 / 4 caractères. Un contrat au cachet pur a autant de jours que
de cachets (`000` `02` `0002`) ; un contrat mixte les distingue : `012` `01` `0004` = 12 heures, 1 cachet, 4 jours
travaillés, pour un brut de 370 € = 130 € de cachet + 12 h à 20 €.

`lire_aem` lit 650-653 en heures, 653-655 et 655-657 en cachets (les deux zones sommées) et 657-659 en jours : sur
une AEM où les jours tiennent en deux chiffres, c'est la même lecture.

Un contrat mixte se saisit avec une ligne `cachet` **et** une
ligne `heure`, chacune à son taux : le taux du cachet ne s'applique pas aux
heures.

### Heures, cachets et jours sont trois grandeurs indépendantes

Et elles se lisent toutes au même endroit, sur l'engagement. Les **cachets**
sont la somme des quantités des lignes en unité cachet, les **heures** celles
des autres lignes, et les **jours** sont un fait qui se saisit (`jours` sur
l'engagement) : on peut jouer un cachet réparti sur trois jours, ou deux
cachets le même jour. À défaut, les jours valent le nombre de dates jouées
**distinctes**.

Ces trois valeurs partent dans la DSN (bloc
`S21.G00.53` : unité 12 journée, 37 cachets, 35 heures) et dans les droits
(`cumul.rs`). Aucune de ces destinations ne les recalcule. Un engagement qui porte des
lignes de paie et **zéro jour** est refusé : l'unité 12 ouvre les droits du
salarié, et un mois payé en compte au moins un. Un mois du contrat sans paye
(`lignes: []`) et un contrat mensuel, qui compte en heures, en sont exclus.

## Les droits (annexes 8 et 10), et d'où viennent les contrats

`droit/src/cumul.rs` compte les 507 heures sur 12 mois (un cachet = 12 h, les
heures d'enseignement plafonnées) sur les lignes qu'on lui passe (`Declare`) ; il
ne sait pas ce qu'est un engagement. Ces lignes, c'est l'engagement qui les lit
(`engagement/src/cumul.rs`, `lire_engagements`), dans **une seule source, la
compagnie** :

| | quoi |
|---|---|
| les engagements importés | les contrats que l'import atteste (une AEM, un bulletin : dates, numéro d'objet, cachets, heures, jours, brut), lus par `importe::contrats_lus` |
| les paies écrites des engagements validés | une ligne par paie d'intermittent, mois par mois (`etats::validees_du_mois`) : mêmes cachets, heures et brut que le bulletin et la DSN ; ce qui est en saisie n'est pas payé et n'entre dans aucun cumul |

Un mois qui a des paies écrites est lu là, et les engagements importés de ce mois
se taisent : rien ne se compte deux fois. Les bornes d'une ligne tirée d'une paie
sont celles de sa **période de paie** (le contrat borné au mois) : un contrat
d'octobre à décembre compte ses cachets d'octobre en octobre.

Le bulletin se compte lui-même : `documents::bulletin` passe les paies qu'il
imprime à `Passe::engagements(Some(paies))` (`engagement::cumul`), qui les substitue à celles de
leur mois. Les conseils (`routes/src/gestes/conseils.rs`) et la position des droits
(`routes/src/archives.rs`) lisent la même chose.

### Le modèle d'un contrat, et le contrat parti

Un modèle de contrat est un texte `.modele` : ses `ARTICLE:` sont les paragraphes du contrat, dans l'ordre ; un alinéa
qui commence par `[si un.champ]` ne s'écrit que si ce champ a une valeur (`fabrique_pdf::modele::retenu`), ce qui
remplace tout morceau de phrase vide calculé dans le code (le numéro d'objet, la personne remplacée, le hors-brut, les
clauses particulières) ; un paragraphe dont aucun alinéa ne s'écrit se tait, titre compris. Un modèle est **un contrat
et son avenant**. Chaque nature a sa **base**, livrée (`contrat_ecrit.modele` et `.avenant` de `contrats-base.json`),
qui suit les mises à jour et ne se modifie pas ; la compagnie en copie des **modèles spéciaux**
(`gabarits/contrats/modeles.json` : chacun sa nature, son contrat et son avenant, chacun avec le texte de la base au
jour de la copie, d'où la route dit quels paragraphes de la base ont changé depuis), choisit le défaut de chaque
nature, et un engagement en choisit un (sa case `modele_contrat`). Un spécial s'écrit **paragraphe par paragraphe**,
contrat ou avenant (`partie`), une route par geste (écrire, ajouter, déplacer, supprimer un paragraphe ; renommer) ;
chaque geste passe par le vérificateur de sa partie (`documents::contrat_ecrit::manques`, `manques_de_l_avenant`), qui
dit, article par article, le champ inconnu, la condition fausse ou la mention exigée qui manque : un geste qui
laisserait le modèle faux n'écrit rien. Un spécial supprimé garde son id dans `supprimes` : un id ne sert qu'une fois,
et l'engagement qui l'avait choisi suit le défaut de sa nature. **Une seule façon de personnaliser** : la compagnie
écrit ses clauses dans un paragraphe de son spécial ; celles d'un engagement (sa case `clauses_particulieres`) ne
s'impriment que par un alinéa `[si contrat.clauses_particulieres]`, que le vérificateur exige de tout modèle comme une
mention : elles ne se perdent jamais en silence. L'aperçu fait le contrat ou l'avenant d'un modèle, rempli avec un
engagement de la compagnie (l'avenant cite le premier article du contrat sans le changer). **Un contrat parti garde son texte** : au
départ, son contrat et son avenant se figent sous une empreinte (`gabarits/contrats/partis/`), que la ligne du journal
note (`ContratParti::modele`) ; le refaire ou lui faire un avenant relit ces textes-là ; l'avenant d'un contrat signé
ailleurs suit le modèle que son engagement suit (`routes::modeles_contrat`).

### Les conseils « Le saviez-vous ? »

Une **option de la compagnie** (fiche Entreprise, `conseils`, décochée par défaut) : sans elle, aucune route ne rend
rien. Le texte, ses chiffres et ses sources sont des données (`logiciel/donnees/conseils.json`, chaque citation lue à
l'identique dans un texte déposé, `tests/fichiers/sources.rs`) ; `routes/src/gestes/conseils.rs` n'a que les conditions.
Trois familles, qui parlent à l'employeur :

| | où | ce qui décide |
|---|---|---|
| le conseil du jour (`conseil-du-jour`, « un autre » : `decalage`) | l'accueil (`js/conseils.js`) | `quand` = « toujours », hors `suspendu` ; rang = `jour × pas mod N`, `pas` premier avec N : N jours de suite font le tour sans répétition, rien ne s'écrit |
| le conseil de saisie (champ `conseil` d'une paie de `apercu`) | sous la carte d'engagement, au plus un | `ordre_saisie` : plus de 208 h d'un technicien, plus de 28 cachets, mois sans allocation (art. 30), 507 heures assurées chez nous, cachet de répétition de la 3090, heures qu'une même somme couvre au minimum horaire, enseignement, suspension, cachets et heures |
| ce que la compagnie a fait (`conseils-salarie`, `conseils-resume`) | la fiche du salarié ; l'accueil | `cumul::position_droits` sur la fenêtre (depuis `anniversaire_droits`, sinon 12 mois glissants) : heures comptées chez nous et part des 507, cachets perdus d'un mois, ce que les conseils de saisie auraient changé |

Un conseil de saisie n'est **pas un avis** (`engagement::Avis`) : il ne passe ni dans les avertissements de la paie, ni au
bulletin ; un engagement validé n'en a pas. Il tient en **une phrase** (`texte`) ; son
`detail` (les deux faces, les chiffres) et sa référence se déplient derrière « pourquoi ? » : le moteur rend les deux, la
page n'affiche que ce qu'elle reçoit. Le salarié s'y nomme **par son seul prénom** (`{prenom}`, `conseils::prenom`), jamais
par un pronom qui le genre. Les deux conseils « plus d'heures » ne se montrent que si
la somme couvre le minimum conventionnel (`minima::cachet_de_repetition`, `minima::minimum_de_la_ligne`) et disent leurs
deux faces : précieux s'il manque des heures pour les 507, à éviter si elles sont assurées un mois indemnisé (le coût se
dit en jours d'allocation, art. 30, au demi-jour le plus proche, jamais en euros ; un coût qui s'arrondit à zéro se dit
« moins d'un demi-jour », variante `-minime` de la banque). « Heures pour la même somme » ne parle qu'à partir d'un gain
du poids d'un cachet (12 heures, `conseils::gain_heures`, la justification y est) : hors du seuil des 507 heures, une
heure de plus coûte plus qu'elle ne rapporte. **Le seul cas de certitude** : nos contrats font à eux seuls au moins
507 heures sur la fenêtre ; ses droits sont ouverts, et « intense plutôt que dilué » prend la place des conseils « plus
d'heures ».

## L'import : le passé d'une compagnie

Une compagnie qui arrive d'un autre logiciel met tout ce qu'elle a dans sa boîte aux lettres `import/` (DSN, AEM,
bulletins en PDF, zip, doublons) ; l'import en tire ses fiches et ses **engagements importés**
(`import/src/`, routes `importation-apercu`, `importer`, `importation-etat`).

- **Le tri** (`import/tri.rs`) : chaque fichier reçoit une sorte, par son contenu, jamais par son nom (`dsn`, `aem`,
  `pdf_texte`, `pdf_sans_texte`, `archive`…). Une DSN d'une autre entreprise que la compagnie n'est pas lue.
- **Les DSN et les AEM d'abord** (`import/dsn_aem.rs`) : l'identité, l'emploi, les numéros d'objet, l'entreprise, ce
  qui a été déclaré et versé ; les AEM, les contrats. La dernière DSN où un salarié figure fait foi pour son identité ;
  l'annule-et-remplace d'un mois passe après l'original. Un numéro d'objet sans titre devient `objet-<numéro>`, titre
  « A COMPLETER », et l'année se lit dans le numéro (`266Z` = 2026).
- **Puis les bulletins, par un lecteur unique** (`import/bulletin_lu.rs`), le même pour tout logiciel : il ne connaît
  aucune mise en page. Il trouve les mentions que tout bulletin imprime (art. R. 3243-1 du code du travail) par leurs
  libellés, et les colonnes du tableau par leurs en-têtes, d'après la table livrée `bulletins-lecture-base.json`, qui dit
  aussi la rubrique du catalogue de chaque libellé de ligne ; un logiciel connu se reconnaît à sa signature, pour le
  compte rendu seulement. Rien ne se devine : une page sans texte, sans en-têtes, qui se lit de deux façons
  (`ambigu`), ou dont ce qui identifie le bulletin ne se lit pas (`incomplet`) est refusée, et le compte rendu dit
  pourquoi. Deux contrôles sans correction (le brut vaut la somme des lignes soumises ; le net se retrouve) : un écart
  avertit, le bulletin est repris avec ce qu'il imprime. Chaque bulletin est rapproché de la paie que sa DSN déclare
  (NIR, début, fin) et la complète (`importation/bulletins.rs`) ; sans DSN, il fait foi seul. Un bulletin d'un salarié
  qu'un engagement saisi paie le même mois n'est pas repris, sauf pour compléter une paie déclarée.
- **L'aperçu n'écrit rien** ; `importer` écrit ce qu'il annonce (`importation/ecrite.rs`) : les fiches à créer, par le même
  chemin que l'écran (`routes::fiches::ecrire_fiche`) ; dans une fiche qui existe, seulement ce qui y est vide ou
  « A COMPLETER » (`importation/completer.rs`), le reste étant signalé (`ecarts`), jamais appliqué ; les engagements
  importés, chacun dans `engagements/AAAA-MM.json`, le fichier du mois où il commence (le contrat qu'une AEM ou un
  bulletin atteste, et ce qui a été versé, mois par mois) ; la copie de chaque pièce reprise dans
  `historique/bulletins/`, nommée par son empreinte (rien de personnel dans un nom). Un import se rejoue sans dommage.
- **La nature à dire** (`importation/nature.rs`) : un bulletin sans marque du spectacle est celui d'un permanent ; la page
  demande sa nature (CDI ou CDD), la route `importation-nature` l'écrit.
- **Le mois importé** (`import/mois.rs`) : un mois qu'aucun engagement de la saisie ne touche et que des engagements
  importés touchent. La page Paie le montre par ses engagements, une carte fermée « importé » chacun, sans geste ;
  une case qu'aucune pièce ne porte reste vide et dit pourquoi. Sa DSN est celle de l'ancien logiciel (le journal des
  dépôts, à défaut la DSN importée) : rien ne propose de la refaire.
- **La CASC-SVP des mois importés** (`importation/casc.rs`) : un mois tenu dans un autre logiciel a pu partir sans elle ;
  `exporter regularisation-casc-svp` prépare la régularisation depuis les engagements importés, `casc-regularise`
  note l'année faite.
- **Les écarts de calcul** (`import/ecarts.rs`, `engagement/src/ecarts.rs`) : à la fin de chaque import, et à
  l'ouverture de la compagnie pour les paies importées pas encore comparées (`journal/ecarts-import.json`), chaque paie
  importée se compare à ce que le moteur aurait calculé de la même saisie, avec les barèmes et la configuration à la
  date de la période. Rien ne change dans l'engagement importé. Les écarts partent chez l'éditeur comme un incident de
  fonctionnement (nature `logiciel`, `ecarts_import`, un incident par sorte d'écart), sans rien afficher ni demander :
  ils ne désignent personne (ni nom, NIR, naissance, adresse, ni salaire ou montant d'une paie ; seulement la
  cotisation et ses libellés, le mois, l'IDCC et l'annexe, l'organisation patronale, la nature et le statut, d'où vient
  le taux AT/MP, le logiciel reconnu, les deux taux et l'écart relatif), et le texte passe quand même par le
  caviardage. Une paie se compare une fois. Ce qui ne se compare pas le dit (une période sans barème, une convention
  sans couche, une ligne sans rubrique).
- **Le document accepté mais pas lu** (`importation/incident.rs`) : à chaque import, un PDF à texte que l'import n'a pas repris
  part en incident, en silence, un seul par famille (même suite de mots du vocabulaire sur sa première page, même
  logiciel) ; son texte, masqué par liste blanche (les mots de la table de lecture et des pièces, les codes de rubrique
  DSN restent ; toute autre lettre devient `X`, tout chiffre `9`), jamais le fichier. Un scan et un fichier qui n'est
  ni PDF, ni DSN, ni AEM se refusent à l'écran avec leur raison (`import/tri.rs`), sans incident.

## La DSN

Norme NEODeS P26V01, puis P27V01 : la version se choisit **par fichier**
depuis `logiciel/donnees/norme-dsn-base.json` — le mois
principal déclaré pour une mensuelle, le jour de constitution pour un
signalement (`declarations/src/norme.rs`). Cette règle de bascule est une **inférence**
(la `note` de la donnée) : le cahier technique ne la donne pas. Chaque
version a ses tables d'usages et de formats ; `usages` et `formats` prennent
celles de la version que le fichier porte (`S10.G00.00.006`), et le fichier en
fabrication décide par elles des rubriques qu'une version a retirées (`06.003`,
`11.002` en P27V01) ou ajoutées (`Fichier::norme`, `Norme::admet`). Les
tables P27V01 sont relevées des classeurs 2027.1.2 de net-entreprises (fiches
3357 et 3358) par `moteur-atelier extraire usages` et `moteur-atelier extraire formats` (`atelier/src/extraire/`).
La part patronale de la CET se déclare en `146` en P26V01 et en `148` en P27V01,
sous la base 78 du `131` (type 23), à l'Urssaf (fiche net-entreprises 2556 ;
`cotisation_cet_patronale` de la donnée). Le code APE de la fiche entreprise se
saisit en NAF rév. 2 ou en NAF 2025 (`logiciel/donnees/naf-base.json`, relevé
de l'INSEE) ; un fichier P27V01 refuse un code NAF rév. 2 et dit vers quels codes
NAF 2025 il passe, sans choisir (`declarations/src/naf.rs`).
`declarations/src/dsn.rs` émet les blocs dans l'ordre que la norme donne, et que des DSN acceptées montrent.

Ce que le moteur écrit, d'après la norme déposée (`atelier/reference/norme-dsn/texte/`) :

- `S21.G00.70.012` et `S21.G00.78.005` sont l'« Identifiant technique
  Affiliation » : « interne au message », unique pour chaque affiliation du
  salarié, le 78.005 d'une base 31 citant le 70.012 de son affiliation (cahier
  technique 2026.1, p. 286 et p. 297 ; `atelier/reference/norme-dsn/texte/bloc-S21.G00.70.txt`,
  `bloc-S21.G00.78.txt`). Ce n'est ni une affectation ni une « contribution
  formation ». La norme laisse la numérotation libre ; le moteur prend 202 + la
  place de la référence d'adhésion, décalée de `places_par_contrat` × (rang du
  contrat − 1), où `places_par_contrat` = max(3, 2 + contributions de branche
  déclarées) : 3 en 3090, 4 en 1285 (`dsn.rs` : `places_par_contrat`,
  `bloc_contrat_fctu`, `bases_et_cotisations`).
- `S21.G00.40.043` est le taux AT/MP notifié à l'établissement (`taux_atmp`,
  lu dans la configuration ; refus s'il manque). Il
  n'a aucun lien avec le bloc 54.
- `S21.G00.54.002` est un **montant** en euros (p. 258). Le moteur écrit un
  bloc 54 par ligne du bulletin qui porte `dsn_autres_bruts`, le type `10` pour
  la déduction forfaitaire spécifique, et les types `92` (santé) et `93`
  (prévoyance et retraite supplémentaire) : `cotisation_sante_patronale` et
  `cotisation_prevoyance_retraite_supp` du barème (les lignes patronales du
  bulletin, ou un taux sur une base), arrondis à l'euro (`dsn::autre_revenu`).
- `S21.G00.53` : `12` et `37` sont des **unités** (53.003), pas des types :
  `12` = journée (les jours distincts travaillés), `37` = cachets isolés (le
  nombre de cachets), `35` = heures d'intermittent hors cachets ; un permanent
  déclare `10` (heure), et ses absences en type `02` (p. 256-257 ; `activites`
  de chaque nature dans `logiciel/donnees/contrats-base.json`, `dsn::activites`).
- `S21.G00.86` (type 07, unité 01) = l'ancienneté en jours depuis
  `date_anciennete` de la fiche jusqu'à la fin du mois déclaré, jamais zéro
  (CCH-12, p. 322 ; `dsn::anciennete_du_mois`). Une fiche qui ne la porte pas
  refuse.
- `S21.G00.23` : une ligne par CTP des organismes de la compagnie (composés depuis
  `ctp_urssaf` de `logiciel/donnees/organismes-base.json`), sur l'assiette que
  la ligne nomme (`brut`, `csg`, `brut_artiste`, `brut_x115_artiste`,
  `ta_principale`, `chomage_permanent`… : une `droit::bordereau::AssietteDuBordereau`,
  ou un `MontantDuBordereau`, lus strictement ; celles qui somment une assiette du
  calcul sur toutes les paies le disent, `de_toutes_les_paies`), sommée sur les paies du mois
  (`dsn::assiettes_ctp`, plafonnée sous le qualifiant 921) et arrondie à
  l'euro ; le montant se calcule depuis la table des CTP (`bordereau.rs`). La
  norme ne définit pas ces assiettes, elle renvoie au guide Urssaf (p. 42 et
  p. 168).
- `S21.G00.50.008` est l'identifiant du taux porté par le compte rendu métier
  (CRM) du PAS que transmet la DGFiP (p. 244) : il se lit dans les retours
  (`retours.rs`), ne se calcule pas, et ne s'écrit qu'avec un taux de type 01
  (`dsn::bloc_versement`).

**Un contrat qui déborde du mois.** `Engagement` connaît le mois de la saisie qui le porte
(`mois`) et en déduit s'il en est le premier ou le dernier (`premier_mois`, `dernier_mois`),
et `debut_periode`/`fin_periode` (le contrat borné au mois). La DSN en tire :

- bloc 40 identique chaque mois — 40.001 début, 40.009 numéro, 40.010 fin
  prévisionnelle sont les identifiants du contrat (fiche net-entreprises
  n° 3344) ;
- blocs 50, 51, 54, 58, 78 sur la **période de paie du mois**, pas sur le
  contrat : la norme veut « a minima » la période de paie (§ 2.1.3.2, p. 28),
  et une date au plus le dernier jour du mois principal (§ 4.4.12, p. 99) ; les
  CCH-11 de 51.001 et CCH-13 de 51.002 (p. 246-247) bornent, elles, par le
  contrat. En P27V01, le 58 d'un versement du mois n'est plus daté :
  il hérite de la datation du 50 (fiche net-entreprises 2983,
  `dsn::dates_du_58_du_mois`) ;
- bloc 62 (fin de contrat) **le dernier mois seulement** ;
- bloc 53 : l'activité du mois, comme toujours ;
- date de constitution (S20.G00.05.007) : le jour où le fichier est fabriqué, comme pour les signalements et la FCTU
  (`dsn::generer`) ; un fichier refait un autre jour porte une autre date.

`Engagement.planning` (jours des mois suivants, lus sous `planning` sur les
lignes, avec leur lieu dans `planning_lieux`) ne compte ni dans
`dates_travail`, ni dans les cachets, ni dans la DSN : il donne `fin` par
défaut, `dates_du_contrat()` pour le contrat imprimé, et les lignes du
mois suivant (`lignes_du_planning`). Le contrat entier recomposé pour le
certificat CS (`paie_du_contrat_entier`) retire le planning des lignes :
chaque mois a payé le sien.

Le barème et le SMIC (`engagement::calcul`) sont ceux de `fin_periode`, pas de
`fin` : un contrat de décembre à février se paie en décembre au barème de
décembre. Sur un contrat qui tient dans son mois, ces propriétés valent
`debut`/`fin`. La quotité 40.012/013 est celle du **contrat entier** (jours
signés : dates du premier mois + planning, `Engagement.jours_contrat`),
identique chaque mois et jamais zéro : le cahier technique 2026.1 interdit
le zéro en unité journée (bloc 40, CCH-11). Comme elle ne bouge pas d'un mois
à l'autre, le CDDU n'a pas de changement de quotité à déclarer en bloc 41 ;
le bloc 41 (41.007 pour la quotité) s'écrit quand une modalité d'un contrat
déjà déclaré change (`changements_contrat.rs`). Plafond 31 en journée
(CCH-12), 250 en heure (CCH-13), sur 40.012 comme sur 40.013 : au-delà,
`dsn::bloc_contrat_fctu` refuse (`unite_quotite.plafond` de `contrats-base.json`).

**Un mois du contrat sans aucun cachet** (`lignes: []`, `Engagement.sans_paie`)
a quand même sa DSN : le contrat en cours avec un versement à zéro — bloc 50,
quatre 51, 53 à 0 jour, 58, et par affiliation prévoyance une base 78 « 31 »
avec sa cotisation 81 « 059 », tous à `0.00` ; ni 54, ni autre base. Pas de
bulletin.

### L'échéance de dépôt : le 5 ou le 15, reportée au jour ouvré, à midi

**Un seul endroit** la donne : `declarations/src/echeance_dsn.rs` (`du_mois`, `de_la_compagnie`).
Le jour est celui de R. 243-6, II (le 5 du mois suivant à partir de cinquante salariés quand la paie est faite dans le
mois, le 15 sinon ; `Contexte::echeance_dsn` choisit selon l'effectif) ; R. 133-14, I le **reporte** : « Si le délai
imparti pour effectuer la déclaration sociale nominative expire un jour férié ou non ouvré, il est prorogé jusqu'au jour
ouvrable suivant » ; l'heure limite est **midi** (cahier technique 2026.1, § 1.4.2), et l'annule-et-remplace intégral
s'arrête la veille à minuit (`Echeance::veille`). Les textes et le raisonnement :
`atelier/reference/sources/echeances-dsn/` (`RAPPORT-20261015.md`, le calendrier de net-entreprises : samedi
05/12/2026 → lundi 07/12, dimanche 15/11/2026 → lundi 16/11).

Trois **inférences**, écrites dans le module et rendues dans le champ `inference` des réponses : « non ouvré » se lit
sur le calendrier du point de dépôt (samedi, dimanche, les onze fêtes légales de `feries::feries`), jamais sur les
fériés chômés de la compagnie (`feries::chomes`) ; « jour ouvrable suivant » se lit « premier jour ouvré » (un vendredi
férié reporte au lundi, aucune pièce officielle ne montre ce cas) ; la veille de l'annule-et-remplace est celle de
l'échéance reportée.

Tout ce qui parle de la date limite de dépôt passe par lui : l'état du mois et l'annule-et-remplace
(`Moteur::limite`), les régularisations, le bilan des mois, l'échéancier, les rappels, le mois
hors délai, l'horloge des compagnies fictives, et la date butoir d'un **signalement d'arrêt subrogé** (R. 133-14, II :
avec la DSN du mois de l'événement, donc avec son report). Les routes rendent
`echeance_depot` (`echeance_dsn` sur une obligation) : `jour`, `heure` (`12:00`), `nominale`, `reportee`, `pourquoi`,
`phrase`, `veille_annule_et_remplace`, `textes`, `inference`. **Une seule date n'en dépend pas** : la date de paiement
du bloc 20 (`S21.G00.20.011`, `dsn.rs`, `echeance_mois_suivant`) reste le 5 ou le 15 de R. 243-6, qui ne prévoit
aucun report, et aucun texte déposé ne dit qu'un prélèvement se reporte.

### Un individu s'identifie par son NIR **ou** par son NTT

`S21.G00.30.001` (le NIR) est **conditionnelle**, et c'est ce qui permet de
déclarer quelqu'un qui n'est pas encore immatriculé — une première embauche,
un artiste étranger. En son absence, la norme exige le **numéro technique
temporaire** `S21.G00.30.020` (CCH-12), et le moteur le fabrique :
`sexe + SIREN + matricule`, jamais saisi (`declarations/src/ntt.rs`).

| | |
|---|---|
| ce qui l'écrit | `ntt::poser_nir` / `ntt::poser_ntt`, appelés par les émetteurs de bloc 30 — `dsn.rs`, `fctu.rs`, `signalement.rs`, `amorcage.rs` |
| les règles | CSL-11 (commence par 1 ou 2), CSL-12 (aucun espace), CCH-13 (caractères 2 à 10 = le SIREN), longueur [11,40] — trois d'entre elles ne sont **pas** dans le relevé des formats, comme pour le NIR |
| quand les deux partent ensemble | la déclaration du mois où le NIR a pris effet : « le NTT doit être renseigné sur la première DSN où le NIR est attribué », pour raccorder les deux identités |
| ce qui le date | l'arrivée du NIR sur une fiche qui existait est un changement daté (`changements_a_dater`, `routes/src/fiches/ecriture.rs`) : la fiche dit « sans NIR » jusqu'à la veille — seule case qui, en se remplissant, demande sa date |
| ce qu'il n'est pas | une donnée de gestion : il n'ouvre **aucun droit**, et ne s'imprime ni sur un bulletin, ni sur un contrat, ni sur un certificat |

Deux conséquences hors DSN : la **DPAE** part alors avec sa zone NIR à blanc,
ce qui vaut demande d'immatriculation (formalité `IMMA`) ; et le **taux de
PAS** ne viendra jamais, la DGFiP n'ayant personne à qui répondre — c'est la
grille de l'article 204 H du CGI qui s'applique (`droit/src/pas.rs`, la section
`pas_taux_par_defaut` du barème de l'année).

Un salarié sans NIR, identifié par son seul NTT, a été déposé en test :
**0 anomalie bloquante, 0 non bloquante**, certificat de conformité délivré.

### Le montant Urssaf : un calcul

Le bordereau Urssaf d'une DSN (22.005, recopié en 20.005) est la somme de ses
lignes 23, chacune valorisée au taux de la table des CTP de l'Urssaf pour la
période (plafonné pour le qualifiant 921, déplafonné pour le 920, le taux AT
arrondi à part, les réductions de format F en déduction), arrondie à l'euro
ligne par ligne : `documents/src/bordereau.rs`, table
`logiciel/donnees/urssaf-codes-types-personnel.csv`.
Rejouée sur 75 DSN mensuelles acceptées d'une compagnie, la règle en retrouve 74 au centime
(octobre 2025 : 394 pour 393). Un code absent de la table fait refuser : le
bloc est un **ordre de télépaiement**, un montant faux fait un prélèvement faux.

### Les cotisations se calculent rubrique par rubrique

Le moteur n'applique pas les taux au brut global : il les applique à **chaque
rubrique de salaire**, arrondit chaque montant, puis somme (`engagement/src/calcul.rs`, « les assiettes, par rubrique
de salaire »). L'assiette CSG suit la même règle ; les cotisations CSG, en revanche, se calculent une seule fois
sur l'assiette agrégée.

La différence ne se voit pas sur un contrat au cachet pur, qui n'a qu'une rubrique. Sur un bulletin d'un cachet à
130 € et de 12 h à 20 €, elle se voit, et c'est le calcul par rubrique que les DSN acceptées montrent :

| | rubrique par rubrique | brut global | DSN acceptée |
|---|---|---|---|
| Assiette CSG | 367,63 | 367,64 | **367,63** |
| Charges patronales | 212,99 | 213,00 | **212,99** |
| Coût employeur | 582,99 | 583,00 | **582,99** |

L'arrondi est **au demi supérieur** (`socle::num::eur`) : c'est ce que montrent les 144 cas ambigus des cotisations
`S21.G00.81` de ces DSN, 144 sur 144.

## La nature du contrat, et les salariés permanents

Le calcul, la déclaration, le signalement de fin de contrat et l'écran d'un permanent sont ceux d'un intermittent,
avec ce que la nature du contrat change. La prévoyance des permanents est un taux que **la convention fixe, donc le
logiciel le livre**.

**La case « emploie des permanents » ne règle que ce que le moteur propose.** Elle est datée (l'historique de la fiche
Entreprise) et se lit par une seule fonction, à la date qui compte : `Contexte::emploie_des_permanents_pendant(debut,
fin)`. Ce que le moteur propose et ce qui est dû sur une période, une seule réponse le dit,
`engagement::permanents::Permanents::de` : `proposes` (la case sur la période : un CDI ou un CDD neuf s'y propose) et
`dus` (les engagements payés au mois qui y courent et ont commencé la case cochée). Les natures d'une carte neuve
(route `saisie` du mois), les modèles de contrat, les cases des fiches (`montres`), les contrats des organismes,
« Pour démarrer » et les états d'export la lisent. **Ce qui est dû se montre toujours** : un permanent engagé avant
que la case se décoche se paie, se termine et reçoit ses documents (la portabilité de ses garanties comprise) comme
avant ; seul un contrat payé au mois qui **commence** sans la case se refuse (au calcul, à son début ; l'avenant qui y
ferait passer un contrat, à l'écriture).

### Le taux d'une branche n'est pas une donnée de compagnie

`logiciel/donnees/prevoyance-conventionnelle-base.json` porte la prévoyance des
permanents telle que la fixe la convention collective, et `droit/src/bareme.rs` (module `prevoyance`)
la lit. 1,20 % de la tranche A est vrai pour **toutes** les entreprises de
l'IDCC 3090 : le mettre dans le barème d'une compagnie, c'est le ranger là où
aucune autre ne le trouvera. La ligne de partage, et elle vaut pour la suite :

| | où ça vit | qui tranche |
|---|---|---|
| ce que la loi ou la convention **impose** | catalogue livré avec le code | personne — le moteur applique |
| ce que l'entreprise **choisit** | `config/entreprise.json` | la compagnie |
| ce que le moteur **ne peut pas savoir** | la fiche du salarié | il pose la question |

Concrètement, pour l'IDCC 3090 : le moteur applique **1,20 % de la tranche A,
100 % patronal** (art. 12.6), **porté à 1,50 %** pour un cadre (ANI du
17/11/2017) — sans rien demander, parce que c'est obligatoire. Une compagnie
qui n'a rien saisi est donc correctement couverte. Elle n'écrit
`prevoyance_permanents` dans sa fiche que pour ce qu'elle a **en plus** :
l'extension facultative à la tranche 2 conventionnelle (2,32 % pour tous,
2,62 % pour les cadres — assiette : du plafond à **quatre** plafonds, pas huit,
d'où la base `t2_conventionnelle`), ou un contrat d'entreprise plus favorable.
Un contrat **sous** le minimum de branche est refusé : ce n'est pas un choix.

Et ce qui ne se devine pas se demande : la prévoyance d'un **non cadre**
s'ouvre après un an d'ancienneté **dans la branche**, pas dans l'entreprise.
Quand le salarié a moins d'un an chez nous et que sa fiche ne dit rien, le
moteur refuse et réclame `prevoyance_branche_depuis` — supposer qu'il vient de
la branche ferait cotiser pour rien, supposer le contraire laisserait un
salarié découvert pendant un an.

Le catalogue porte **dix** conventions : 1285, 1790, 1922, 2121, 2412, 2642, 3090, 3097, 3241 et 3252. Une convention
qui n'y est pas refuse, et une convention relevée mais pas confirmée (`applicable: false`) refuse aussi, en disant ce
qui a été lu (`prevoyance::regime`, dans `droit/src/bareme/prevoyance.rs`). La **1285 (CCNEAC)**, le secteur
subventionné, y porte — non cadres 0,95 % T1,
cadres 1,50 % T1 + 0,81 % T2, 100 % patronal, dus dès l'embauche (art.
XII.2.1.2 et XII.2.1.5), la tranche 2 allant du plafond à **quatre**
plafonds. Le texte se lit dans la **base KALI de la DILA** (open data,
sans compte, là où Légifrance refuse les robots) : la base complète, puis
toutes ses livraisons quotidiennes jusqu'à la veille, pour lire la version
**en vigueur** de chaque article. Elle porte aussi la **santé** des
permanents, qui est un taux de la convention et non un contrat à niveaux :
0,81 % du PMSS en 2026, 0,89 % au 01/01/2027, au moins pour moitié patronal
(XII.2.1.10 et 11) — `droit::bareme::prevoyance::sante_conventionnelle`.

Un écart est à connaître : la CCNEAC ajoute à l'interbranche, **pour ses
intermittents cadres techniciens seulement** (XII.2.2.1), 0,45 % en TA et
0,95 % en TB — 2,14 patronal en TA au lieu de 1,69, et une tranche B qu'une
compagnie privée n'a pas. Ses artistes cadres restent à l'interbranche
(XII.2.2.4). La couche 1285 du barème le porte sous
`par_categorie.cadre.par_statut.technicien`, et la tranche B s'assied sur
`t2_prevoyance_journaliere` (du plafond journalier à quatre fois lui).

### Ce qu'un permanent déclare, et ce qui se réclame salarié par salarié

La prévoyance des permanents :
1,20 % de la tranche A pour la 3090, porté à 1,50 % pour un cadre, 100 %
patronal ; la CASC-SVP d'un permanent est celle de l'article 4.22 (0,50 % de la
tranche A mensuelle, 100 % patronale) ; les blocs 54 (92 santé, 93 prévoyance)
et les bases 31 (059) se lisent sur les lignes du bulletin. La section
`par_nature.permanent` du barème est complète en 2025 et en 2026.

Ce qui reste est **propre à chaque compagnie** et la fait refuser, elle seule :
la référence de son contrat de prévoyance (`adhesions_prevoyance` de `config/organismes.json`, composée en `affiliations_par_nature.permanent`,
dans l'ordre prévoyance puis santé). **Deux faits se réclament salarié par
salarié** : le niveau de garantie santé (`sante_svp`) et, pour un non cadre de
moins d'un an dans l'entreprise, son ancienneté **dans la branche**
(`prevoyance_branche_depuis`), qui ouvre sa prévoyance.

### Un cadre permanent n'est pas un cadre intermittent

`par_categorie.cadre` du barème décrit un cadre **intermittent** : tranches
journalières, prévoyance de l'accord interbranches. Appliquée à une
administratrice en CDI, elle lui donnerait des plafonds à la journée et la
prévoyance de quelqu'un d'autre, en silence.
`droit::bareme::bareme_cadre` lit `par_categorie.cadre_permanent` quand le contrat est
mensuel, et refuse si la section n'est pas vérifiée (`verifie`) : mêmes taux Agirc-Arrco (le régime est unifié), mais l'Apec sur
quatre plafonds **du mois** et la CET sur des tranches mensuelles. Sa
prévoyance, elle, vient de la convention.

`logiciel/donnees/contrats-base.json` porte ce que la DSN écrit de la **structure** d'un
contrat, par nature. Trois
natures : `cddu_spectacle` (l'intermittent, et le défaut d'un engagement qui
ne dit rien), `cdi`, `cdd`. Chaque code y porte sa valeur, sa phrase et sa
source ; `droit/src/natures.rs` les lit et ne décide rien.

Ce qui sépare un permanent d'un intermittent, en une page :

| | intermittent | permanent |
|---|---|---|
| ce qu'il vend | des cachets, des heures, **à des dates** | une **durée du travail** |
| quotité (40.011) | journée (12), ≤ 31 (CCH-12 du bloc 40) | heure (10), ≤ 250 (CCH-13 du bloc 40, p. 189 ; 3250 est le plafond de la mesure 53.002) |
| fin prévisionnelle | obligatoire | absente sur un CDI |
| numéro d'objet (40.050) | oui | **INTERDIT** — CCH-11 ne l'autorise que pour les natures 02 et 10 |
| congés payés | caisse des Congés Spectacles (40.022, cotisation 15,5 %), sauf un CDDU de douze mois ou plus | chez l'employeur : compteur, indemnité, solde ; sauf le CDD de droit commun de moins de douze mois d'un artiste, déclaré à la caisse (`caisse_conges::affilie`) |
| assiettes FNAL / taxe d'apprentissage | brut **majoré** de 11,5 % et de 10 % (CSS L. 243-1-3, D. 243-0-2), pour un contrat déclaré à la caisse des Congés Spectacles | brut nu ; tranche 1 majorée de 11,5 % (FNAL) et brut majoré de 11,5 % (taxe d'apprentissage) pour un contrat déclaré à la caisse (`par_caisse_conges.affilie` du barème) |
| plafond de Sécurité sociale | par paie : 12 × le PSS horaire par journée pour un artiste engagé moins de cinq jours, sinon le PSS au prorata des jours rémunérés (`plafond.rs`) | la période de paie (R. 242-2), proratisé à la quotité d'un temps partiel sauf renonciation de l'établissement (`40.084` en P27V01) |
| chômage | 9,75 / 2,40 dans la limite de quatre plafonds, recouvré par **France Travail** | 4,00 + AGS 0,25 dans la limite de quatre plafonds de la période, recouvré par l'**Urssaf** (CTP 772, 937) |
| retraite complémentaire | taux Audiens, tranches **journalières** | droit commun, tranches **mensuelles** |
| formation professionnelle | 2,00 % (CTP 983) | 0,55 % (CTP 959) |
| mutuelle santé | aucune | **obligatoire** (ANI, art. L911-7) |
| fin du contrat | à son terme (031) : DSN mensuelle, circuit dérogatoire 62.017 = 01 (la norme le réserve au contrat infra-mensuel, p. 277) ; un FCTU si le salarié le demande. Rompu avant son terme (036, 037, 084…) : 62.017 = 02 et **signalement nature 07** (CCH-12, fiche 2062 ; `Nature::signalement_fctu`) | **signalement nature 07** (FCTU) |
| blocs 51 | les quatre valent le brut | 001 brut, 002 assiette chômage, 003 salaire rétabli, 010 salaire de base |
| bloc 53 | jours (12), cachets (37), heures (35) | heures (10), absences en type 02 |

### Le barème a trois étages : le socle livré, la couche de convention, la surcouche de la compagnie

Les taux de **droit commun** — Urssaf, Agirc-Arrco, chômage, CSG, plafond, SMIC, et les taux de la **branche** du
spectacle — sont les mêmes pour tout le monde ; ce qui est **propre à une entreprise** tient en un nombre : son taux
AT/MP notifié, lu dans sa configuration. Les premiers sont livrés : une compagnie qui installe PuppetPaie ne relève pas
elle-même les taux nationaux.

```
logiciel/donnees/baremes/2026.json      le socle : droit commun + tout le spectacle
  └─ section `par_convention`           la couche de SA convention, choisie par l'IDCC
config/baremes/2026.json                la surcouche de la compagnie, ce qu'elle corrige du socle
config/effectifs.json                   son effectif, écrit par le moteur depuis
                                        les retours de nature 118 (envois/src/releve_retours.rs)
```

**Trois étages, et le deuxième est une bifurcation.** Le corps du socle porte
le droit commun (Urssaf, Agirc-Arrco, CSG, plafond, SMIC) **et** ce qui vaut
pour tout le spectacle : Congés Spectacles 15,50 % (la même caisse pour la
3090 et la 1285), chômage 9,75 / 2,40 des annexes 8 et 10, prévoyance des
intermittents **non cadres** (0,12 / 1,11 — identique dans les deux
conventions), formation 2,00 %, Thalie Santé 0,42 %. Ce qu'une **convention**
change est dans sa couche : la cotisation conventionnelle (CASC-SVP 0,50 %
pour la 3090, **FNAS 1,45 %** pour la CCNEAC) et la prévoyance des
intermittents **cadres** (1,69 interbranche, **2,14** en CCNEAC, plus 0,95 en
tranche B). C'est `idcc` de la fiche entreprise qui choisit — pas le code.

**Une convention sans couche fait refuser le moteur**, et c'est le point :
payer un salarié d'une autre branche avec la CASC-SVP de la 3090 — une
cotisation qu'il ne doit pas, un FNAS qu'il doit et qui manque — ne se
verrait sur aucun bulletin. Une couche `applicable: false` refuse aussi :
ses nombres sont relevés, pas confirmés, et le refus dit ce qu'il reste à relever.

**Ce qu'une couche peut porter en plus de rubriques à fondre** :

- `contributions_conventionnelles` : des rubriques posées par
  `droit::bareme::bareme_convention` **après** le statut et la nature du contrat,
  donc pour tout le monde — la section `technicien` du socle recopie la
  CASC-SVP de la 3090 dans son `ccn`, et le FNAS doit la remplacer là aussi.
  Un taux peut y dépendre de la **représentation du personnel** de la fiche
  entreprise (`representation_personnel`, lue par
  `droit::bareme::representation_du_personnel`, confrontée à l'effectif) et de la
  nature du contrat : c'est le FNAS, 1,45 % sous onze salariés ou sans
  représentation, 0,825 % (permanents) / 1,325 % (intermittents) avec un CSE
  conventionnel ou un CSE adhérent, rien avec un CSE qui n'adhère pas.
- `contributions_dsn` : les contributions que la DSN déclare sous une
  **adhésion Audiens** de la compagnie, comme la CASC-SVP de la 3090 — le
  FNAS et le FCAP de la CCNEAC. `dsn::contributions_dsn` les résout mois par
  mois (l'adhésion, `"contribution": "FNAS"`, se compose du barème du mois, `droit::organismes`,
  sans rien de la compagnie), et `dsn::places_par_contrat` réserve autant
  d'identifiants d'affiliation de plus : trois pour la 3090, quatre pour la 1285.

`Contexte::empiler` (`droit/src/contexte.rs`) empile les trois, sous `Contexte::bareme`, et c'est
le seul endroit qui le fait : le reste du moteur lit un barème déjà fondu et
n'a pas à savoir d'où vient quel nombre.

**La règle de fusion a une subtilité.** Un dictionnaire se fond clé par clé.
Une **liste d'objets nommés** — les rubriques du bulletin (`id`), les
cotisations DSN (`code81` + `affectation`) — ne se remplace pas en bloc : la
surcouche reprend celles qu'elle nomme et laisse les autres. Une entreprise
de plus de cinquante salariés corrige son FNAL en une ligne :

```json
{"annee": 2026, "socle": 2026,
 "rubriques_bulletin": [{"id": "autres_2", "taux_pat": 0.35}]}
```

Une liste de **valeurs** (les paliers du SMIC, les composants d'une
cotisation), elle, se remplace en entier.

**Le marqueur `socle` est explicite, et c'est voulu.** Un fichier qui ne le
porte pas est un barème **complet**, pris tel quel (`droit/src/contexte.rs`, la lecture du marqueur). Deviner serait
dangereux dans les deux sens — une surcouche prise pour un barème complet paierait avec trois taux,
un barème complet pris pour une surcouche hériterait de taux qu'il a de
bonnes raisons de ne pas avoir.

**Le taux AT/MP se lit dans la configuration** : la fiche entreprise (`taux_atmp`, datée comme toute la configuration)
pour le siège, la ligne de chaque autre établissement (`etablissements[].taux_atmp`) ; `droit::contexte` le pose sur la
pile, la surcouche ne le porte pas. Chaque ligne de la case dit le code, le taux, la date à partir de laquelle il vaut
et d'où il vient (l'import, la notification reçue, la saisie, le taux collectif de l'arrêté pour un risque « TC », inscrit
à l'ouverture : `routes::atmp`) ; la paie ne lit qu'elle (`droit::bareme::taux_atmp_de_l_annee`). Les rubriques du socle
portent leur **facteur** (`_facteur_atmp`) — 0,70 pour un artiste, dont les taux Urssaf sont abattus de 30 % (arrêté
du 24/01/1975), 1 pour tout le monde d'autre — et `droit::bareme::bareme_atmp` fait la multiplication.

**Trois valeurs du socle supposent une entreprise de moins de onze
salariés** : le FNAL à 0,10 % (0,50 % à partir de 50), la formation
professionnelle des permanents à 0,55 % (1,00 % à partir de 11), et le
`tdelta` de la réduction générale, 0,3781 au FNAL 0,10. Elles sont écrites et
nommées dans `_ce_qui_depend_de_l_entreprise`, en tête du socle.

### L'effectif, ses seuils, et ce que le moteur refuse

**Ce que l'effectif change est écrit pour être LU** : la
section `seuils_effectif` du socle dit, chemin par chemin, où vit chaque valeur
qui dépend de l'effectif et ce que le socle y porte (formation des permanents à
onze salariés ; FNAL, `tdelta` et échéance de la DSN à cinquante).
`droit/src/contexte.rs` la confronte à l'effectif **calculé par l'Urssaf** — il ne se saisit
pas : il arrive en CRM de nature 118,
`envois/src/releve_retours.rs` l'écrit seul dans `config/effectifs.json` à chaque relevé, et
un seuil n'est franchi qu'après cinq années consécutives (L. 130-1, II) — et
**refuse de payer** si la surcouche ne suit pas le palier atteint.

- **Une compagnie neuve** n'a qu'à renseigner `premiere_embauche` sur sa fiche :
  avant elle, aucun salarié, et pendant cinq ans aucun seuil ne peut être franchi.
- **Une compagnie plus ancienne** dont les retours ne servent plus les vieux 118
  déclare dans `effectifs.json`
  `"jamais_au_dessus_de": {"seuil": 11, "jusqu_a": AAAA, "source": "..."}` — la
  seule clé de ce fichier qui s'écrit à la main.
- Ces seuils-là (formation, FNAL, échéance de la DSN) sont ceux du code de la
  sécurité sociale ; ceux du CSE (CCNEAC, 11 et 50) relèvent du code du travail,
  et le moteur ne les tranche pas avec l'effectif de l'Urssaf : la représentation
  déclarée fait foi (`droit::bareme::representation_du_personnel`).
- Une valeur surchargée dans la surcouche est tenue pour décidée.
- Le même module refuse une **adhésion à une organisation patronale** dont la
  couche n'est pas relevée (`par_organisation_patronale`) et un **second
  établissement** incomplet (adresse, taux AT/MP) ; complet, il a sa propre DSN
  (`droit/src/contexte.rs`, `declarations/src/dsn.rs`). Le contrôle (`contexte::controler`) est appelé dans
  `Contexte::pile_lue`, par où passe toute lecture du barème.

### Le barème a deux axes, pas un

Le corps d'un barème **décrit un intermittent**. `par_statut` dit s'il est
artiste ou technicien — c'est-à-dire si ses taux Urssaf sont abattus de 30 %
(arrêté du 24/01/1975). `par_nature` dit si son contrat est un CDDU ou un
contrat permanent. **Les deux sont indépendants** : un comédien en CDI garde
ses taux abattus *et* perd les Congés Spectacles. La pile est toujours la
même, dans cet ordre, et elle ne s'écrit qu'à un endroit, `droit::bareme::bareme_applique` :

```
établissement du salarié → statut (bareme_du_statut) → taux AT/MP → nature (bareme_de_la_nature) → caisse des
Congés Spectacles → convention → CASC → santé → retraite en tranche 1 → catégorie (bareme_cadre) →
prévoyance → régime local → versement mobilité
```

La pile finie se lit une fois, typée (`droit::forme_bareme::Pile`, qui refuse une clé inconnue) ; le calcul du bulletin
et la DSN (`dsn::cotisations`, le bloc 54) lisent ces types ; en sauter un étage,
ce serait déclarer les cotisations de quelqu'un d'autre.

La section `par_nature` empile par identifiant (`rubriques` remplace,
`rubriques_retirees` enlève, `rubriques_ajoutees` insère) ; seule la
**réduction générale se complète** au lieu d'être remplacée — sa formule vient
du statut, et la nature ne change que la destination de la part chômage
(`018:urssaf` et CTP 668 pour un permanent, `018:chomage` et CTP 671 pour un
intermittent).

**La section `par_nature.permanent`** des barèmes 2025 et 2026 est vérifiée (`verifie`). La fiche de paramétrage
Audiens (version 139) porte les permanents : la **santé** (1,44 / 1,84 / 2,04 % du PMSS selon le niveau) et la
**CASC-SVP** (0,50 % de la tranche A). La **prévoyance des permanents** vient de la convention, pas du barème (voir
plus haut), et la rubrique porte `rempli_par` comme la mutuelle. Les identifiants techniques des deux
**affiliations DSN 059** viennent de la fiche de paramétrage de l'entreprise, qui n'existe qu'une fois l'entreprise
adhérée au régime de sa branche : ce sont ses références d'adhésion, saisies dans `config/organismes.json`. La santé ne
se relève pas dans un barème : c'est un **forfait** dont le niveau se lit sur
le bulletin d'adhésion du salarié (`sante_svp` sur sa fiche), et sans ce
choix le moteur refuse, salarié par salarié.

### Le bordereau Urssaf d'un permanent

Les CTP viennent de la **table des CTP de l'Urssaf** (open data,
`open.urssaf.fr`, re-téléchargeable et donc re-vérifiable) : 100 (RG cas
général, avec le taux AT), 635 et 430 (compléments maladie et allocations
familiales — le CTP 100 porte encore les anciens taux réduits 7,00 et 3,45),
332 (FNAL plafonné), 772 (chômage), 937 (AGS), 959 (CFP), 987 (CPF-CDD, 1 % du
brut d'un CDD de droit commun, C. trav. L. 6331-6 : la somme des cotisations 129 du
nominatif ; le barème l'ajoute par `bareme::bareme_cpf_cdd`), 668 (réduction
générale **étendue**, celle qui inclut le chômage : la somme des 018 de l'Urssaf
du nominatif, parts sécurité sociale et chômage, guide déclaratif Urssaf § 8.2 ;
`dsn::reduction_a_l_urssaf`, la même lecture pour le bordereau courant et pour
celui d'une correction). Une correction qui fait
**baisser** la réduction l'écrit sous son CTP de régularisation, 669 (801 pour
le 671 d'un technicien intermittent), en assiette (23.004) et sans signe, et
non en montant négatif (guide § 8.2, schéma ; `reduction_generale.dsn.ctp_regularisation`
du barème). Un mois qui porte un
permanent sans aucun CTP marqué `"nature": "permanent"` **refuse** : le
bordereau est un ordre de prélèvement.

Le CTP 983 de l'intermittent est conditionnel (`"nature": "intermittent"`) ;
les compléments abattus 432 et 636 ne portent que le brut des **artistes**
(`"statut": "artiste"`), et un technicien intermittent relève des compléments au
taux plein 430 et 635 (`"statut": "technicien"` : « Les
techniciens ne sont pas concernés » par les taux réduits, page Urssaf déposée dans
`atelier/reference/sources/urssaf-taux-reduits-artistes/`). Une ligne de
`ctp_urssaf` peut dépendre de l'effectif (`effectif_au_moins`, `effectif_sous`, lus
par `dsn::Seuils` dans `config/effectifs.json`) : le FNAL des artistes passe du 334
plafonné au 316 « totalité » à cinquante salariés. L'assiette de la taxe d'apprentissage se
compose — brut majoré de 10 % pour les intermittents, brut nu pour les
permanents, un seul CTP 992.

### Ce que la loi doit à un permanent

`logiciel/donnees/permanents-base.json` porte les nombres, chacun avec son article ;
`droit/src/permanents.rs` porte la règle ; les routes
(`routes/src/imprevus.rs`)
décident. Les articles sont relevés et datés dans
`atelier/reference/sources/code-du-travail/`.

- **congés payés** : 2,5 jours ouvrables par mois, 30 au plus (L3141-3) ;
  période de référence au 1er juin (R3141-4 — le 1er avril du même article
  est celui des *caisses* de congés, donc des intermittents : les deux dates
  cohabitent dans une compagnie de spectacle) ; indemnité **calculée deux
  fois**, le dixième (L3141-24 I) et le maintien (II), le plus favorable
  retenu ; indemnité compensatrice à la fin, **due quelle que soit la cause
  de la rupture** (L3141-28). Le compteur se **dérive des saisies**, jamais
  stocké — même règle que les 507 heures d'un intermittent ;
- **précarité** : 10 % (L1243-8), **pas due pour un contrat d'usage**
  (L1243-10 1°, qui renvoie au 3° de L1242-2) ;
- **préavis** : L1234-1, et **rien sous six mois d'ancienneté** — la loi y
  renvoie à la convention ou aux usages, et le moteur demande plutôt que
  d'inventer ;
- **indemnité de licenciement** : huit mois d'ancienneté (L1234-9), un quart
  puis un tiers de mois (R1234-2), salaire de référence le plus favorable des
  deux formules (R1234-4). Un montant **supra-légal est refusé** : son régime
  social est autre, et le moteur ne sait pas l'assujettir ;
- **les trois autres fins d'un CDI** — et c'est le
  **motif de rupture** qui choisit, jamais la ressemblance :
  - **rupture conventionnelle** (motif 043) : son indemnité spécifique ne
    peut pas être inférieure à celle du licenciement (L1237-13), et c'est
    tout ce que la loi dit — le montant est celui de la **convention
    signée**, il se saisit. Le moteur calcule le plancher, refuse en dessous,
    et **n'écrit rien** tant que le montant n'est pas dit. Pas de préavis :
    la convention *fixe* la date de rupture ;
  - **mise à la retraite** (038) : au moins l'indemnité de licenciement
    (L1237-7), préavis de L1234-1 (L1237-6). La **condition d'âge n'est pas
    vérifiée** — L1237-5 renvoie au code de la sécurité sociale — et si elle
    n'est pas remplie, « la rupture constitue un licenciement » (L1237-8) :
    le moteur le rappelle à chaque fois ;
  - **départ à la retraite** (039) : **un palier, pas un cumul** (D1237-1).
    Un demi-mois après dix ans, un mois après quinze, un et demi après vingt,
    deux après trente — douze ans valent un demi-mois, pas trois —, et
    **rien du tout sous dix ans**. Salaire de référence de D1237-2, les deux
    mêmes formules que R1234-4 ;

  **Ce qu'elles supportent ne se lit pas dans le code du travail**, qui n'en
  dit pas un mot : le régime social sort de **L242-1 II 7° du code de la
  sécurité sociale**, et lui-même renvoie au régime fiscal de **l'article 80
  duodecies du code général des impôts** — est exclu des cotisations, dans la
  limite de deux plafonds annuels, ce qui n'est pas imposable. Les deux
  articles se relèvent avec le code du travail (`moteur-atelier releve-codes`). Il en sort trois choses :
  - l'indemnité **légale** de licenciement et celle de mise à la retraite ne
    sont **ni soumises ni imposables** (80 duodecies 3° b et 4° b, qui
    exonèrent « le montant prévu […] par la loi ») ;
  - l'indemnité de **départ volontaire** à la retraite, hors plan de
    sauvegarde de l'emploi, n'est dans aucune exonération : elle est
    **imposable, donc soumise**, et se traite comme un salaire. C'est la
    seule des quatre ;
  - celle de **rupture conventionnelle** dépend d'un fait du *salarié* :
    l'exonération d'impôt du 6° ne joue que s'il **n'est pas en droit de
    bénéficier d'une pension**. S'il l'est, elle est imposable — et reste
    hors cotisations (L242-1 II 7°, second alinéa, « y compris lorsqu'elles
    sont imposables »). D'où **deux rubriques** pour une seule indemnité, et
    un moteur qui refuse de calculer tant que le fait n'est pas dit.

  La **contribution patronale de 40 %** sur la part exonérée d'une rupture conventionnelle ou d'une mise à la
  retraite (L137-12) n'est dans aucune section du barème : le solde de tout compte avertit, à chaque fois, de ce
  qu'elle vaut (`engagement/src/solde.rs`). Son taux **se lit** dans `permanents-base.json`
  (`regime_des_indemnites_de_rupture.contribution_patronale`, d'après la version de L137-12 en vigueur au
  31/12/2025).

  Chacune a **son** code d'indemnité au bloc S21.G00.52 — 001, 003, 005 —,
  et le cahier technique interdit de les intervertir (CCH-21 à CCH-24). Le
  contrôle est au point d'écriture du bloc, dans `dsn.rs` (`permanents::code_52_permis`) : c'est le seul
  endroit par où passent toutes les lignes ;
- **suspensions** (bloc S21.G00.65) : congé sans solde, parental, sabbatique…
  Saisies une fois comme **imprévus** du salarié (`engagement/src/imprevus.rs`) : chaque mois
  traversé en tire sa ligne et son bloc 65, entier, sans recopie. Un **congé payé n'en est pas une** — il est payé, le contrat n'est pas
  suspendu, et le moteur refuse d'écrire le motif « 200 - COP ».

  **Un mois entier sans rémunération** (textes et rapport :
  `atelier/reference/sources/suspension-non-remuneree/`) :
  - la **retenue** d'une absence (arrêt comme suspension) se calcule **au réel du mois** : le salaire du mois
    fois les heures d'absence sur les heures de travail du mois (Cass. soc. 11/02/1982), au taux exact,
    jamais arrondi avant de multiplier ; les heures se comptent au réel, jours ouvrés (lundi à vendredi,
    moins les fériés chômés) fois la durée hebdomadaire sur cinq (fiche DSN 2640 : l'hypothèse « classique »
    quand le planning n'est pas connu ; fiche 2693 : septembre 2026 entier = 22 × 7 = 154 h). Un mois
    entier retire le salaire entier : le brut est 0,00, l'activité rémunérée (53 type 01) 0.00, l'absence
    (53 type 02) les heures réelles ;
  - la **complémentaire santé** : en 3090 les garanties sont suspendues (accord du 19/07/2024, art. 4.2) —
    pas de mutuelle sur le bulletin, pas de rattachement santé dans la DSN ce mois-là (inférence pour la DSN :
    aucune fiche ne le dit) ; ailleurs, la fiche entreprise dit ce que fait le contrat de santé,
    `sante_suspension_non_indemnisee` = « suspendue » ou « maintenue », et sans réponse le mois refuse
    (`calcul::sante_de_la_suspension`) ;
  - un **net fiscal négatif** (une part patronale de santé maintenue et réintégrée) se déclare tel quel en
    50.002, et le montant soumis au PAS (50.013) comme le PAS (50.009) à 0.00 (fiche DSN 1810,
    `pas::du_versement`) ; le net social négatif s'affiche (BOSS, FAQ Q37) ;
  - les **congés payés** ne s'acquièrent pas sur les jours d'une suspension qui n'acquiert pas
    (`acquiert_conges`, L3141-5 ; `imprevus::jours_de_conges_non_acquis`) ; l'**ancienneté** compte pour moitié
    le congé parental (L1225-54) et ne compte pas, en l'avertissant, une autre suspension non rémunérée
    (`imprevus::anciennete_depuis`).

- **jours fériés** : les onze fêtes légales de L3133-1, dont trois qui
  dépendent de Pâques — le seul calcul du logiciel qui remplace un relevé,
  aucune table officielle ne publiant cette date. Mais **férié n'est pas
  chômé** : la loi n'impose que le 1er mai (L3133-4), les dix autres relèvent
  de la convention ou de l'usage. `socle/src/feries.rs` ne retire donc du
  décompte des jours ouvrables que ce que `config/entreprise.json` déclare
  chômer (`jours_feries_chomes`), et **dit** ce qu'il a compté comme
  ouvrable. À noter : L3133-3 exclut nommément les **intermittents** du
  maintien de salaire un jour férié ;
- **maintien de salaire** pendant un arrêt (L1226-1, D1226-1 à D1226-8) : un
  an d'ancienneté appréciée au premier jour de l'absence, sept jours de
  carence — **aucune** pour un accident du travail, mais l'accident de
  *trajet* garde la sienne —, trente jours à 90 % puis trente aux deux tiers,
  allongés de dix jours par tranche de cinq ans et plafonnés à quatre-vingt-dix,
  le tout imputé sur douze mois glissants. Le moteur rend le **brut avant
  déduction** : les indemnités journalières viennent de la caisse et se
  saisissent (D1226-5) ;
- **certificat de travail** (L1234-19, D1234-6) et **reçu pour solde de tout
  compte** (L1234-20, D1234-7, D1234-8) : produits par
  `routes/src/imprevus.rs`, leur texte en donnée dans `logiciel/donnees/modeles/*.modele` (ou le modèle propre de la compagnie, `socle::fichiers::modele_de`).
  Le certificat est le seul document du logiciel dont le contenu soit
  **plafonné** par la loi — D1234-6 dit « exclusivement ».

## Les signalements d'événements (DSN événementielle)

Le moteur en écrit quatre, dans trois modules :

| `S20.G00.05.001` | message | écrit par |
|---|---|---|
| `04` | signalement d'arrêt de travail | `declarations/src/signalement.rs` |
| `05` | signalement de reprise suite à arrêt de travail | `declarations/src/signalement.rs` |
| `07` | **fin de contrat de travail unique** (FCTU) | `declarations/src/fctu.rs` |
| `08` | **amorçage des données variables** (SADV) | `declarations/src/amorcage.rs` |

**Trois modules et non un**, parce qu'ils ne portent pas la même chose. Les
deux premiers ne portent pas de paie ; le FCTU ne porte que ça — il déclare le
**solde de tout compte** (cahier technique p. 107, « y compris les données du
mois en cours calculées dans le cadre du solde de tout compte »). Le quatrième ne porte
ni paie ni événement de contrat : il **demande** — c'est la seule façon
d'obtenir de la DGFiP le taux de prélèvement à la source d'un salarié sans
attendre une DSN mensuelle, depuis que TOPAze est fermé. Un signalement
d'amorçage vaut pour **un unique individu** (base de connaissances DSN, fiche
2117) : cinq salariés à amorcer, c'est cinq fichiers.

**Le bloc 40 est le même dans les quatre**, et c'est voulu : `fctu.rs` et
`amorcage.rs` appellent `dsn::bloc_contrat` (ou `bloc_contrat_fctu`) avec leur nature de déclaration,
et `si_admise` retire ce que cette nature n'admet pas **en lisant le tableau
des usages** de la version du fichier (`usages-dsn-base.json` pour P26V01,
`usages-dsn-p27v01-base.json` pour P27V01) au lieu de recopier la
différence. Une seule vérité sur la façon de décrire un contrat. Ce que la
nature 08 retire par rapport à une mensuelle : le spectacle (`40.049`,
`40.050`, `40.051`), l'accident du travail (`40.039`, `40.040`, `40.043`),
l'identifiant technique d'affiliation (`70.012`) et le bloc 71 — et à
l'inverse, elle **exige** le code APET (`11.002`), que le signalement d'arrêt
interdit — en P26V01 : la norme P27V01 le retire.

**Un CDDU du spectacle n'a pas besoin de FCTU** : la même page donne une
dérogation aux artistes et techniciens du spectacle, et la fin d'un contrat
d'usage part dans la **DSN mensuelle** avec le circuit dérogatoire
`S21.G00.62.017 = '01'` — une rubrique justement **interdite** dans un FCTU.
Le moteur en émet un quand même si le salarié le demande (la norme le permet,
p. 68 et p. 107), en le disant par un avertissement (`fctu.rs`, « La
dérogation du spectacle ») ; la mensuelle qui suit écrit alors encore
62.017 = 01.

**Ce que l'arborescence tranche, et que le tableau des usages ne dit pas.**
Le tableau dit qu'une rubrique a le droit d'être là ; il ne dit pas si son
bloc s'applique. Les cardinalités du modèle (p. 122, recopiées dans
`logiciel/donnees/fctu-base.json`) le disent :

```
S21.G00.30 - Individu               (1,1)   UN individu, UN contrat :
S21.G00.40 - Contrat                (1,1)   c'est le mot « unique » du nom
S21.G00.62 - Fin du contrat         (1,1)   obligatoire
S21.G00.63 - Préavis                (1,*)   OBLIGATOIRE AUSSI — même sans
                                            préavis, code « 90 »
S21.G00.78 / 81 - bases, cotisations (0,*)  facultatives
```

Et deux contrôles lui sont propres, tenus par le moteur avant tout dépôt :
**CCH-14** (la valeur 31 — base assujettie de prévoyance — interdite en
`S21.G00.78.001`) et **CCH-15** (au moins trois blocs 51, types 001, 002 et
010, par versement).

Un FCTU déposé en test : **0 anomalie bloquante, 0 non bloquante**,
certificat de conformité délivré.

**Un signalement d'arrêt n'est pas une petite DSN mensuelle.** Il ne porte ni
paie, ni cotisation, ni versement, ni bordereau. (Un FCTU, lui, porte la paie
du solde de tout compte — mais pas plus de bordereau ni d'ordre de
télépaiement : les blocs 20, 22 et 23 lui sont interdits aussi.) Son arborescence tient en huit blocs —
envoi, déclaration, contact, entreprise, établissement, adhésions prévoyance,
individu, contrat, arrêt — et le *tableau des usages* du cahier technique
(2026.1, p. 363-371) **interdit** dans une nature 04 ou 05 la quasi-totalité
des rubriques qu'une mensuelle écrit : le code APE, l'adresse de l'entreprise,
le lieu et le pays de naissance du salarié, son adresse, et tout le bloc 40
sauf trois rubriques.

```
S21.G00.40.001   date de début du contrat      O
S21.G00.40.009   numéro du contrat             O
S21.G00.40.019   identifiant du lieu de travail  C
tout le reste du bloc 40                       I  (interdit)
```

Le contrat est donc **désigné**, jamais redécrit : c'est la DSN mensuelle qui
l'a déclaré, et ce sont ces deux identifiants-là qui doivent correspondre.
D'où `contrats_au` (`routes/src/imprevus.rs`), qui lit les contrats des engagements
plutôt que d'en fabriquer un.

Une rubrique interdite écrite par mégarde, c'est une anomalie bloquante
**CST-04** côté plate-forme, et le fichier ne déclare rien. Tout signalement
produit se confronte, avant dépôt, au tableau des usages de la norme
(`logiciel/donnees/usages-dsn-base.json`, natures 01, 04 et 05 ;
`declarations::usages::verifier_fichier`, appelé par `declarations/src/signalement.rs`).

### Le bloc 60, et ses trois visages

Le même bloc « Arrêt de travail » s'écrit à quatre endroits, avec quatre jeux
de rubriques — et c'est `dsn::bloc_60` qui les tient tous, pour qu'il n'y ait
pas deux vérités sur ce qui part. Il décide de ce qu'il écrit par le tableau
des usages, pour la nature du fichier et jamais une autre (`dsn::usage_admet` ;
`atelier/reference/norme-dsn/texte/tableaux-des-usages.txt`, bloc S21.G00.60).

| rubrique | mensuelle (01) | arrêt (04) | reprise (05) | fin de contrat (07) |
|---|---|---|---|---|
| `60.001` motif de l'arrêt | O | O | O | O |
| `60.002` dernier jour travaillé | O | O | O | O |
| `60.003` fin prévisionnelle | O | O | O | O |
| `60.004` subrogation | C | **O** | **I** | **I** |
| `60.005/.006` dates de subrogation | C | C | I | I |
| `60.007/.008` IBAN / BIC | C | C | I | I |
| `60.010` date de la reprise | C | C | **O** | C |
| `60.011` motif de la reprise | C | C | **O** | C |
| `60.012` date de l'accident | **I** | C | C | **I** |

Trois conséquences qui ne se devinent pas :

- la **subrogation est obligatoire** dans un signalement d'arrêt. Elle ne se
  déduit de rien : répondre *oui* engage la compagnie à percevoir les
  indemnités journalières et à maintenir le salaire. Le moteur refuse un arrêt
  saisi sans réponse ;
- la **date de l'accident** est interdite en DSN mensuelle. Elle ne part que
  dans le signalement, et seulement pour les motifs 04, 05 et 06 (CCH-11) ;
- la **date de reprise** ne se renseigne que si la reprise est **anticipée**
  par rapport à la fin prévisionnelle. Connue mais non anticipée, *« elle ne
  doit pas être valorisée »*.

### Le report dans la DSN mensuelle

Cahier technique §2.5.2 : *« il est nécessaire de reporter les informations
d'un signalement dans la DSN mensuelle consécutive »* — la mensuelle est la
« photo » de fin de mois, et c'est elle qui constitue l'historique de vie
professionnelle sur lequel s'exploitent les signalements suivants. Et bloc
60 : *« en cas de suspension supra-mensuelle, la suspension doit être déclarée
dans TOUTES les DSN mensuelles dont le mois principal déclaré présente une
période commune »*.

`dsn::generer` le fait : le bloc 60 s'écrit sous le bloc 40 du
contrat suspendu, entre le contrat et la fin de contrat (l'ordre de
l'arborescence : 41, 60, 62, 65, 70, 71). La fabrication de la DSN du mois (`routes/src/gestes/depart.rs`,
`fabriquer_dsn`) lit les arrêts qui croisent le mois et les annonce dans les avertissements — y compris quand
l'arrêt n'a **jamais été signalé**, parce que la mensuelle ne remplace pas le
signalement : elle le répète.

Si le contrat suspendu n'est déclaré dans aucune paie du mois, la DSN
**refuse** plutôt que de laisser tomber l'arrêt en silence : le bloc 60 est
fils du bloc 40, il n'aurait rien où s'accrocher.

### Le magasin, et la trace des envois

`imprevus/AAAA-MM.json` sous les données (`engagement/src/imprevus.rs`) :
le magasin des **imprévus** d'un salarié, arrêts de travail (`"genre": "arret"`, au mois du
dernier jour travaillé) et suspensions du bloc 65 (`"genre": "suspension"`, au mois de leur
premier jour). Un imprévu se saisit une fois ; le moteur en tire la ligne de paie d'un
permanent (`imprevus::lignes_du_mois`, le seul endroit qui la tire), le bloc 60 ou 65 de chaque
DSN dont le mois le croise, et nomme les cachets d'un intermittent datés pendant l'imprévu.
Chaque arrêt garde ses `envois` : les
signalements déposés pour de vrai (nature, type, numéro d'ordre, identifiant
rendu par le point de dépôt) et les DSN mensuelles qui l'ont reporté. C'est
cette liste qui donne son rang au prochain annule-et-remplace — le cahier
technique (§1.4.3.4) exige une séquence croissante par événement — et un
dépôt de **test** n'y écrit rien : il n'a rien déclaré.

Le dépôt passe par la même route `deposer` qu'une mensuelle. Deux adaptations
seulement : un signalement n'a pas de mois principal déclaré (la rubrique y
est interdite), donc c'est le **jour de l'événement** qui s'affiche et se
recopie ; et le garde-fou « une DSN réelle est déjà partie pour ce mois » ne
s'applique pas — plusieurs signalements peuvent partir le même mois, un par
arrêt.

## La DPAE (ex-DUE)

Enregistrements de 990 caractères, **saut de ligne compris** : 989 caractères
puis `LF`, comme le dit le Format 120 (« 990 en incluant le saut de ligne »).
Un enregistrement par contrat, encodage latin-1. Zones dans `declarations/src/dpae.rs` :

| Position | Contenu |
|---|---|
| 121-133 | horodatage de l'envoi, `JJMMAAAAHHMM` — **sans secondes** : le dessin officiel (Format 120) place `C_URSSAF` juste après |
| 133-136 | `C_URSSAF`, code de l'Urssaf réceptrice (117 Île-de-France), lu dans `entreprise.json` → `code_urssaf` |
| 283-315 | nom (32 c.) |
| 315-347 | prénom (32 c.) |
| 347-360 | NIR (13 c., sans la clé) — **facultatif** : à blanc, la DPAE vaut demande d'immatriculation (formalité `IMMA`, transmise à la CPAM). C'est par elle que le NIR arrive, et la DSN déclare l'individu sous son NTT en attendant (`declarations/src/ntt.rs`) |
| 360-368 | date de naissance `JJMMAAAA` |
| 368-412 | lieu de naissance (44 c.) |
| 412-420 | date d'embauche `JJMMAAAA` |
| 420-424 | heure d'embauche `HHMM` |
| 730-731 | sexe `M` / `F` — lu sur le NIR quand il y en a un, sur la case `sexe` de la fiche sinon (`ntt::sexe_de` ; le sexe d'un NIR ne se lit que par `droit::fiche::Sexe::du_nir`, que `Nir::sexe` appelle) |
| 827-829 | département de naissance |
| 864-865 | `C_TYPE_CONTRAT` : `1` CDD, `2` CDI, `3` CTT — **la DPAE n'en connaît que trois**, quand la DSN en a une vingtaine. La valeur vient de la nature du contrat (`logiciel/donnees/contrats-base.json`, section `dpae`) |
| 865-873 | `D_FIN_CDD`, fin prévue du CDD `JJMMAAAA` — **à blanc pour un CDI**, qui n'a pas de terme |

Le gabarit (celui de la compagnie s'il en a un, sinon l'enregistrement neutre livré,
`logiciel/donnees/modeles/due_tel_gabarit.txt`) ne donne que l'en-tête constante : marqueur `DUE`, `912` (N_LG_MESS,
la longueur du message hors entête — pas un code Urssaf), `120` (C_VERSION). Les zones de l'employeur se lisent dans
la fiche entreprise (`dpae::employeur` : SIRET, raison sociale, adresse, téléphone, code NAF, et le code du service de
santé au travail, `medecine_travail`, qui ne se devine pas : sans lui, la DPAE refuse, sauf quand le gabarit est celui
de la compagnie, au même SIRET, et le porte).

`C_TYPE_CONTRAT` (position 864) suit le dessin officiel (catégorie V « CONTRAT »), qui donne les trois valeurs : un
CDI part à durée indéterminée, sans terme.

**Ce que l'Urssaf répond** (onglet « Codes retour » du Format 120, tables dans
`declarations/src/dpae.rs`) : le bilan de conformité (nature 11, XML) porte un `<code>`
par anomalie, et le certificat de conformité (nature 41) est **notre
enregistrement renvoyé** avec ses zones de retour remplies — `C_RET_AR_LOT`
(76-77, le traitement du message), `R_DOSSIER` (425-429, la référence sous
laquelle la DPAE est enregistrée), `C_RETOUR_AR` (430-431, le code par DPAE)
et les formalités déclenchées (`I_CERTIF`, `I_IMMA`, `I_MT`, `I_PMF5`,
971-976). `envois/src/etats.rs` lit le certificat (`dpae::lire_certificat`) et pose sur
chaque embauche sa référence de dossier.

Deux lectures à connaître, observées sur les certificats reçus :

- **`99` « autre erreur, données facultatives » est l'état nominal**, toujours
  avec un `R_DOSSIER` et `CERTIF` + `MT` levées. Le tableau de 2013 est plus
  sévère que le service ; une DPAE en `99` avec un dossier est enregistrée.
- **`98` « DPAE déjà existante »** : l'Urssaf n'a rien créé (formalités à `N`)
  parce qu'une DPAE identique existait — vu sur le second test des mêmes
  embauches. L'embauche est déclarée, par la première.

Le moteur écrit le `.txt` **et** le `.txt.zip` du même nom ; net-entreprises accepte les deux.
`DPAE_<mois>_recapitulatif.txt` (`dpae::recapitulatif`, écrit par `routes/depots.rs`) donne la même chose en clair
pour la saisie en ligne.

## Le dépôt, et le journal

Ces déclarations engagent l'employeur. Le moteur les dépose **par l'API**
(`envois/src/depot.rs` le prépare, `envois/src/transport.rs` l'envoie : net-entreprises
pour la DSN, l'Urssaf pour la DPAE), jamais
par un portail : un contrat écrit, des retours lus par programme, et une
erreur qui se voit (un code Urssaf faux, qu'un portail laisserait passer, fait refuser le dépôt).

Trois garde-fous, tous dans le moteur (`compagnie/src/preuve.rs`, `routes/src/gestes/depot_acte.rs`, `compagnie/src/demande.rs`) :

- **Une preuve par acte.** Rien ne part en réel sans le mot de passe du
  logiciel tapé au moment de l'acte (`README.md`) ; la séance qui ouvre le
  coffre n'en est jamais une. Sans mot de passe posé, rien ne part du tout.
  Une version exigée et pas encore installée bloque tout dépôt réel
  (`compagnie::preuve::exigee_en_attente`).
- **Pas deux fois.** Une DSN réelle déjà partie pour le mois ne repart pas
  (`DsnDejaDeposee`), une embauche déjà déclarée n'est plus à déclarer.
  Une déclaration **refusée** repart sans forcer —
  c'est la seule raison de la refaire.
- **Un dépôt n'est pas fini quand il est parti.** `envois/src/journal.rs` écrit une ligne
  par acte sortant à la fin de `journal/depots.jsonl`, sans réécrire ce qui y est (un enregistrement par ligne ; dépôt, courriel, demande de
  signature ou contrat imprimé pour la main propre, documents), avec le `flux` rendu par le point de dépôt et, pour chaque engagement qu'il
  couvre, le numéro de la version sur laquelle il est parti (`journal::departs`, que le moteur passe à l'engagement) ;
  les retours relevés
  à l'ouverture s'y rattachent. `verdict_du_retour` (`envois/src/etats.rs`) lit les bilans (natures
  10/11) : une DPAE est **passée quand son bilan est OK**, le certificat 41
  confirme, il ne décide pas. `resume` dit « REFUSÉ : motif », « bilan OK »,
  « en TEST ».

Déposer la DSN en test avant le dépôt réel, et garder l'ancien logiciel installé le temps
de faire tourner un ou deux mois en parallèle et de comparer.

### Un logiciel, un administrateur

Une installation (le dossier qui porte `logiciel/`, `prive/`, `compagnies/*`, `fictives/*`) appartient à une personne,
l'**administrateur**, qui peut y gérer plusieurs compagnies d'un seul mot de passe ; qui veut deux administrateurs
installe un second logiciel. `routes/src/gestes/administrateur.rs` :

- **Rien de neuf dans le chiffrement.** Chaque compagnie garde son coffre (`config/coffre.bin`) et sa clé maîtresse, scellée
  par la clé du poste et par le mot de passe du logiciel. Rattacher une compagnie, c'est sceller cette enveloppe par le mot
  de passe de l'administrateur ; la détacher, la resceller par un mot de passe à elle. Le **registre**
  (`administrateur.json`, à la racine de l'installation, sans aucun secret) dit lesquelles s'ouvrent ensemble.
- **Changer le mot de passe de l'administrateur** rescelle toutes ses compagnies : tout se prépare
  (`Coffre::preparer_ouverture`), puis tout se remplace ; un échec au milieu remet chacune dans son état d'avant.
- **La fenêtre** demande « Gérer cette compagnie avec le même administrateur ? » à la création d'une compagnie quand une
  autre a déjà un mot de passe ; retient le mot de passe de l'administrateur, une fois tapé, pour ouvrir ses autres
  compagnies sans le redemander (il ouvre, il n'autorise aucun acte) ; et l'accueil devient son **tableau de bord** : les
  déclarations obligatoires de ses compagnies (les rappels de chacune : DSN, signalements, obligations d'`obligations.rs`,
  DPAE), du plus pressé au moins pressé, lues de `config/.tableau.json` sans calcul, refaites en fond
  (route `administrateur-tableau`).
- **Le dépôt groupé** (route `administrateur-deposer`, `{"reel"}`) : ce qui part par la DSN et qui est prêt — en test, un
  fichier écrit pas encore essayé ; en réel, un fichier dont le test est revenu conforme — part compagnie par compagnie
  par la route `deposer` de chacune, avec ses contrôles et son bilan. En réel, un mot de passe tapé au moment de l'acte sert
  de preuve à chaque dépôt, qui le revérifie. Une compagnie qui refuse n'arrête pas les autres. Le bouton n'apparaît qu'à
  partir de deux compagnies qui ont quelque chose de prêt.
- **Une fenêtre par installation** (`compagnie/instance.rs`) : deux installations s'ouvrent côte à côte sur un poste (la
  clé USB arrive sur un poste qui a déjà son logiciel) ; la même, relancée, ramène sa fenêtre devant. Chacune tient un
  **créneau** de la plage 7330-7429 : un verrou (une prise d'écoute, rendue par le système à la mort du processus) et, au
  port suivant, sa page. Le créneau est **retenu** dans `.fenetre.json` à la racine de l'installation (il voyage avec
  elle) ; à défaut, tiré d'une empreinte de son chemin. Pourquoi un port stable : le navigateur retient un mot de passe
  par origine (`hôte:port`). Un créneau tenu par une autre installation (son verrou répond par son dossier, un autre),
  par une copie de celle-ci (même `.fenetre.json`, autre dossier) ou par un autre programme se saute, et le suivant est
  retenu ; aucun créneau libre : la fenêtre s'ouvre quand même, sans verrou. Ce qui est propre à une fenêtre sur le poste
  (le profil de WebView2/WebKit ou du Chromium, le jeton de sa page, son journal d'avant toute compagnie) va dans
  `<profil>/fenetres/<port>/` — en portable, dans `.fenetre/` de l'installation ; le cookie du jeton porte le port
  (`jeton-<port>`). Ce qui reste commun aux installations d'un poste : la clé du poste (hors mode portable), sans danger —
  elle ne scelle que des enveloppes, chacune d'une clé maîtresse tirée au hasard pour sa compagnie, et se pose une fois
  (`O_EXCL`) ; la séance gardée est **une par coffre** (`ouverture-gardee-<empreinte>.bin`). La mise à jour est déjà
  par installation (`maj/`) ; son relais attend la fin de **sa** fenêtre (le processus sous Windows, le créneau de son
  installation ailleurs), et `moteur maj --appliquer` ne refuse que si **cette** installation est ouverte.
- **Toutes les compagnies tenues à jour dès le lancement** (`Fenetre::prechauffer`, `etat_garde::Fil`) : chaque vraie
  compagnie de l'installation (pas les fictives) reçoit sa file des états sur **un seul fil de calcul** pour toute la
  fenêtre, la dernière ouverte d'abord (la date de son `journal/interface.log`), puis chacune son tour ; la compagnie
  choisie passe devant dès son choix (pendant qu'on tape son mot de passe). L'état d'ouverture, le tableau (compagnie de
  l'administrateur, sous l'empreinte de l'ouverture) et les mois se refont en fond ; ouvrir sert un état frais sans
  attendre ; minuit passé, tout se revérifie. **Avant le mot de passe, du calcul local seulement** : le fil est marqué
  (`fil_de_fond`), et `garde_reseau` y refuse toute sortie, le coffre y refuse la clé du poste et toute dérivation de mot
  de passe. La veille (réseau, coffre, relève, sauvegarde) ne démarre qu'à l'ouverture d'une compagnie ; le tableau de
  l'accueil ne relève les comptes rendus qu'une fois le mot de passe de l'administrateur tapé dans la fenêtre.


## L'horloge des compagnies fictives

Une compagnie fictive (livrée, ou ajoutée par le « + » puis rendue simulée au banc) vit en temps réel : à chaque
ouverture, et à chaque passage de la veille, son horloge (`routes/src/horloge.rs`, `rattraper`) joue ce qu'une compagnie
bien tenue aurait fait **avant aujourd'hui**, à sa date, et seulement ce qui n'est pas fait. Hors d'une compagnie fictive
elle ne touche à rien (`HorlogeFictive::verifier`, revérifiée avant chaque geste) ; ses dépôts passent par
`depot_acte` contre le simulateur, ses courriels par une boîte muette.

**Ce qu'elle joue, et à quel instant** (heure de Paris) :

| instant | pas |
|---|---|
| le jour de l'embauche, 5 h | les engagements qui commencent ce jour-là se valident |
| 6 h | les contrats partent à la signature : ceux qui ne sont jamais partis, et ceux que l'engagement a rendus faux avant leur retour signé (dans leur version présente) |
| 7 h | la DPAE (test puis réel), et le signalement d'amorçage qui part avec elle |
| 8 h | les contrats reviennent signés (imprimés, pour un salarié sans courriel) |
| l'instant de la dernière validation d'un engagement | l'avenant d'un contrat revenu signé que l'engagement a rendu faux part, puis revient signé |
| la date butoir que rend `obligations`, 9 h 30 | les signalements dus : arrêt (04), reprise anticipée (05), fin de contrat (07) |
| le 1er du mois suivant, 9 h | ce qui reste en saisie du mois se valide |
| l'échéance de la DSN (`echeance_dsn`), 10 h, puis 11 h | la DSN du mois se fabrique et part (test puis réel) ; puis les documents aux salariés, rectificatifs compris (ce que l'étape Documents dit à envoyer) |
| le 31 janvier, 10 h, puis 11 h | l'exercice d'avant se clôt, puis s'exporte pour la comptabilité |

Les pas se rassemblent avec leur instant, se trient, et se jouent du plus ancien au plus récent : une embauche du 3 avril
passe avant la DSN de mars du 15. Chaque pas regarde, à son instant, ce que le moteur en dit (le journal, l'aperçu des
signatures, les embauches à déclarer, les obligations, le bilan du mois) et ne fait rien si c'est fait. Les mois d'un
contrat sans terme se jouent comme les autres : `engagement::stock::mois_saisis` les mène jusqu'au mois d'aujourd'hui.

**Ce qui est « fait »** : un mois dont l'échéance est passée, dont toutes les étapes sont vertes
(`Moteur::etapes_restantes`, ce que l'écran montre) et dont aucun signalement n'est dû. La mémoire de l'horloge
(`config/horloge.json`) le garde avec l'empreinte de sa saisie et de ses imprévus : il ne se relit plus tant qu'elles ne
changent pas et que sa DSN n'est pas refusée. Une année est faite quand l'année d'avant est close et exportée. Une
échéance annuelle n'est faite que par son fait (le taux AT/MP posé, la revue des spectacles) : l'horloge n'en coche aucune.

**Ce qu'elle ne fait jamais** : saisir, ni corriger une saisie (un signalement que la saisie ne permet pas d'écrire, un
engagement que le moteur refuse de valider, restent à faire, et le pas le dit) ; redéposer les octets qu'un organisme a
refusés. Une DSN refusée laisse son mois à faire : à chaque ouverture l'horloge la refait, datée de son échéance, et ne la
redépose pas tant qu'elle est identique ; quand une personne a corrigé ce qui la faisait refuser, le fichier change, part à
l'instant où l'horloge le trouve, et les documents suivent. Ce qui vient après aujourd'hui reste au visiteur.

**Une fictive fraîchement installée** (sa configuration et ses engagements validés, ni journal, ni `sorties/`, ni
`retours/`) : sa première ouverture fabrique et joue tout le passé dû, dans l'ordre du calendrier ; la seconde ne fait rien.

## La version, et la mise à jour dans l'ombre

`VERSION` (une date, `AAAA.MM.JJ`, comparée en entiers). Ce qu'une installation reçoit, vérifie et applique est dans
`compagnie/src/maj.rs` ; ce que le flux « Publier » publie (les archives, la signature par la clé de GitHub) dans
`moteur/src/publication/`. **Une mise à jour par installation** : tout se passe dans `<installation>/maj/`, à
côté de `logiciel/`. Un dépôt est une adresse qui sert
`derniere.json` et les archives de `logiciel/`.

**Une version s'installe dès qu'elle est prête** : au lancement suivant, ou au clic « Redémarrer pour mettre à jour ».
Rien ne la retient, ni une DSN à déposer ni un mois en cours. Sa phrase `calcul` (la case de « Publier ») dit à la
compagnie ce qu'elle change d'un calcul de paie : l'indicateur la montre avec la version prête, puis, une fois, au
lancement qui suit son installation (« PuppetPaie X est en place. Elle change un calcul de paie : … » ;
`maj/appliquee.json`, qui passe à `vue: true`) ; une version sans phrase `calcul` ne dit rien de plus que « Logiciel à
jour ». Une version **exigée** bloque tout dépôt réel tant qu'elle n'est pas installée
(`compagnie::preuve::exigee_en_attente`), et l'indicateur le dit, sans survol.

**Quatre archives de mise à jour par version** : une par système
(`windows`, `linux`, `mac` : les fichiers communs et les programmes de ce
système) et une pour tous (`tout-systeme`). Dans `logiciel/`, les programmes de
Windows sont à la racine (`puppetpaie.exe`, `WebView2Loader.dll`), ceux de Linux
et de Mac dans `linux/` et `mac/` (`puppetpaie`, `moteur`), posés par le flux
seulement (`maj::dossier_programmes_de`, `maj::systeme_du_fichier`).
`derniere.json` porte `version`, `notes`, `calcul`, `exigee`, et sous `archives`
une entrée par système publié (`archive`, `sha256`, `signature` ; celle de tous
dit en plus les systèmes qu'elle porte, `systemes`). « Publier » choisit ce qu'il
fabrique (Windows seul, Windows et Linux, ou les trois) ; une installation dont
le système n'est pas publié ne télécharge rien et garde sa version.

**La forme de l'archive de mise à jour est un contrat** avec toutes les
installations posées : le contenu de `logiciel/` à sa racine, rien d'autre. Le
code déjà installé la déballe telle quelle dans `maj/<version>/logiciel/` et y
lit `VERSION` ; une autre forme couperait toutes les installations de leurs
mises à jour, sans retour possible. `derniere.json` peut gagner une clé à sa
racine (les installations posées ne lisent que `version` et `archives`),
jamais en perdre ni rien gagner sous `archives` qui ne soit un système.

**Deux autres sortes d'archives, à côté.** L'**archive des fictives**
(`puppetpaie-<version>-fictives.zip`, `publication::publier::archiver_fictives`), signée, que
`derniere.json` nomme sous `fictives` avec l'**empreinte** de leur contenu
(SHA-256 des fichiers livrés et de leurs noms, ni compression ni dates). Les
**archives d'installation** (`puppetpaie-<version>-<système>-installation.zip`,
Linux, Mac et tous ; `publication::publier::archiver_installation`) : `logiciel/` et
`fictives/`, ce qu'on décompresse pour installer ; ni signées ni nommées,
comme l'installeur Windows.

**Les fictives se mettent à jour à part.** Une
installation retient la livraison qu'elle a dans `fictives/.livraison.json`
(`empreinte`, et `posees` : le nom et l'empreinte de chaque fictive qu'elle a
posée) ; chaque fictive posée porte sa marque, `.livree.json` (son empreinte).
L'archive d'installation les porte, l'installeur Windows aussi (le flux les
pose avant, `moteur maj --marquer-fictives`). `maj::preparer_les_fictives` ne
télécharge l'archive des fictives que si l'empreinte publiée n'est pas celle
retenue (un dépôt de travail, qui porte `.git`, ne la reçoit jamais), la
vérifie comme une archive du logiciel et la range dans `maj/fictives-pretes/`.
Au lancement suivant, avant toute fenêtre, `maj::poser_les_fictives_pretes`
remplace ce que la livraison avait posé (un dossier dont le nom est dans
`posees` et dont la marque dit l'empreinte retenue ; sans livraison retenue,
un dossier au nom d'une fictive livrée dont `COPIE-DE-DEVELOPPEMENT.json` dit
« compagnie fictive <ce nom> »), joué ou non, photo d'origine comprise (elle
se reprend à la prochaine ouverture) ; retire ce qu'elle avait posé et ne
porte plus ; pose ce qui est nouveau. Un dossier qu'elle n'a pas posé n'est
jamais touché : si une fictive livrée en porte le nom, elle n'est pas posée,
et `maj/journal.log` le dit.

- `verifier` — au lancement, en fond : lit `derniere.json`, prend une version
  plus récente, télécharge l'archive de son système (`maj::SYSTEME`), ou celle
  de tous (si elle le porte) quand la sienne n'est pas nommée ou ne se
  télécharge pas, vérifie son
  condensat et sa **signature** de l'éditeur (Ed25519 ; une archive qui ne
  passe pas est refusée, sans repli), la déballe dans `maj/<version>/` (les
  programmes Unix redeviennent exécutables), la **contrôle** (son programme,
  `puppetpaie route`, lit la configuration de chaque compagnie de
  l'installation) et pose `maj/prete.json`.
- `appliquer` — au démarrage suivant, par un relais lancé avant toute
  fenêtre (`maj::au_lancement`, `maj::relais` : sous Windows, un programme ne
  se remplace pas pendant qu'il tourne) : `logiciel/` → `logiciel/.avant/`, la
  version prête prend sa place ; si ça casse en route, tout revient.
- `revenir` — `.avant/` reprend la place (`moteur maj --revenir`).
- `publier` — par le flux « Publier » du dépôt public
  (`.github/workflows/publier.yml`), qui compile les systèmes choisis (le Mac
  sur `macos-latest`), attend l'approbation de Léandre, signe
  (`publication::publier::publier_depuis_l_environnement`) et pose leurs archives, celle de
  tous et celle des fictives, les archives d'installation, `derniere.json` et
  l'installeur dans une Release ; jamais republier un numéro.


## L'exercice : rappels, clôture, export compta

`routes/src/gestes/exercice_ecrit.rs` (la clôture,
l'export compta) et `routes/src/rappels.rs` (les rappels). Trois choses qui regardent plus loin
que le mois affiché.

**Les rappels** (`rappels(ctx, aujourdhui)`) rendent une liste de `Rappel`,
dans l'ordre où ça se fait. Les mois dont la DSN manque d'abord : depuis le
mois qui suit la dernière DSN réelle passée au journal (ou en attente de son
bilan) jusqu'au mois d'avant aujourd'hui ; sans aucune DSN au journal, le
seul mois d'avant. Chaque rappel porte l'état du mois — `a_saisir` (pas de
fichier de paie : la DSN « néant » passe par l'écriture d'un mois vide),
`a_produire` (saisi, jamais écrit), `a_deposer` (le fichier DSN, choisi par
`fichiers_deposables`), `a_corriger` (bilan KO) — et `en_retard` passé
l'échéance de dépôt (`declarations/src/echeance_dsn.rs` : le 5 ou le 15 du mois suivant,
reporté au jour ouvré, à midi ; le rappel la rend dans `echeance_depot`). Puis, quand l'année d'avant n'est pas close et a des mois :
la clôture (seulement si la DSN de décembre est au journal — sinon c'est son
rappel qui parle), l'export compta (absent, ou plus vieux que la dernière
saisie de l'année), et au premier trimestre la revue des spectacles : lesquels
continuent ; les autres s'archivent (`revue_spectacles`, journal §167 : un
numéro d'objet ne se périme pas). Ce qui fait taire un rappel, c'est le
fait ; jamais un clic — ici, la revue enregistrée.

**La clôture** (`cloturer`) refuse une année déjà close, l'année en cours,
et une année dont un mois avec des paies n'a pas de DSN réelle acceptée
(`forcer` passe outre, et `exercices.json` le garde). Elle écrit l'export
compta si besoin, puis `sorties/exercices/Exercice_<année>.zip` : `engagements/` (le fichier de chaque mois de l'année, et celui du mois où commence un engagement qui court dans l'année),
chaque `sorties/<mois>/` en entier (les contrats signés sont dans `envoi/`),
les retours dont le nom commence par l'année ou par janvier suivant (le
bilan de la DSN de décembre arrive en janvier), `journal/depots.jsonl`, les
fiches de `config/` et le barème de l'année, `compta/`, un `LISEZ-MOI.txt`.
Elle note dans `config/exercices.json` la date, l'archive, son empreinte
SHA-256 et le nombre de fichiers. **Figer** vit dans le moteur
(`routes::mois::refuser_si_clos`), appelé par l'écriture d'une saisie et par
la fabrication de la DSN (`depart::fabriquer_dsn`) : un front qui l'esquiverait n'existe pas. `rouvrir_exercice`
retire la note et garde l'archive (avec une trace `_rouverts`).

**L'export compta** (`export_compta`) lit les paies écrites des engagements validés
(`documents::ecritures_comptables`) — les mêmes lignes, les mêmes arrondis que les bulletins — et
construit une écriture par mois, datée du dernier jour : au débit le brut
par spectacle (l'analytique) et la part patronale de chaque ligne de
bulletin ; au crédit un tiers par organisme (part patronale + part
salariale, la RGDU ventilée selon `Paie.reduction_generale`), le PAS s'il y
en a, et le net par salarié. Débit = crédit par construction (brut + charges
patronales = tiers + net), et l'écriture est refusée si ça ne tombe pas.
Les comptes viennent de `plan-comptable-base.json` (PCG : 641, 645x, 431,
437x, 421, 4421), recouvert clé par clé par `config/plan-comptable.json` ;
une ligne de bulletin sans compte fait refuser — on ne devine pas un compte.
Le fichier : `Compta_<année>.csv`, `;`, virgule décimale, UTF-8 avec BOM.
Pourquoi les bulletins et pas la DSN : le montant Urssaf de la DSN est un
bordereau recopié, pas un calcul, et les blocs 81 ne couvrent pas tout ce
que l'Urssaf facture — la comptabilité de la paie, c'est le bulletin ; le
prélèvement réel se rapproche ensuite du compte de tiers.

## Les tests, par partie

Chaque partie a ses tests et une cible qu'elle ne dépasse pas : pour en ajouter un, on en retire un. Un test s'écrit
pour une règle tirée d'un texte déposé (il le cite), pour un bug réel qui a échappé, ou pour un parcours ; jamais pour
une forme, ni pour comparer le moteur à ce qu'il faisait. Un test qui monte une compagnie passe par le moteur entier et
vit dans `moteur/tests/` ; un test qui n'appelle qu'une crate sur des valeurs vit dans cette crate.

| partie | où | cible | ce qu'ils prouvent |
|---|---|--:|---|
| socle | `socle/` | 10 | dates, heure de Paris, arrondis ; l'écriture des fichiers et le verrou |
| droit | `droit/` | 80 | une règle = un texte déposé cité ; une grille à graine fixe par barème |
| engagement | `engagement/`, `moteur/tests/calcul/` | 15 | rien ne part sans validation ; ce qui est parti fige ; daté ou corrigé |
| documents | `documents/` | 20 | la forme relue, les chiffres égaux à l'engagement |
| déclarations | `declarations/`, `moteur/tests/dsn/` | 90 | la norme : usages, formats, blocs, ordre, échéances |
| envois | `envois/` | 40 | la lecture des retours réels transcrits, le garde-fou réseau, aucun envoi réel sans mot de passe |
| routes | `routes/`, `moteur/tests/routes/`, `interface_fichiers.rs`, `interface_http.rs`, `moteur/src/interface/` | 20 | une route refuse et rien n'est écrit ; l'ouverture |
| compagnie | `moteur/tests/fichiers/` (coffre, mise à jour, sauvegarde, restauration, bridage, essai, installations, Drive, banque des conseils), `tests/routes/compagnie_supprimee.rs`, `demarrage.rs`, `tests/drive_jeton.rs`, `compagnie/` | 40 | le coffre et ses deux mots de passe, la signature des mises à jour et la forme des archives, la sauvegarde et la restauration, le bridage |
| import | `moteur/tests/fichiers/bulletin_lu.rs`, `caviardage.rs`, `tests/routes/import_neuve.rs`, `import/` | 15 | un passé importé se relit (bulletins, DSN) ; ce qui part chez l'éditeur ne désigne personne |
| atelier | `atelier/tests/atelier/` (`donnees_reelles.rs`, `verifier_avant_commit.rs`, `referentiels.rs` : sa propre cible), `moteur/tests/fichiers/garde_reseau.rs`, `moteur/tests/atelier.rs`, la garde de l'horloge | 15 | les garde-fous de `atelier/AGENTS.md` ; la fidélité des référentiels livrés aux textes déposés ; le binaire livré ne porte rien de l'atelier |
| parcours | `moteur/tests/rejeu.rs` | 4 | un scénario joué de bout en bout à l'horloge, ses sorties comparées à un oracle |

Total visé : environ 350, plafond 400. La commande : `atelier/AGENTS.md`, « Faire tourner le code dans le conteneur,
et les tests ».
