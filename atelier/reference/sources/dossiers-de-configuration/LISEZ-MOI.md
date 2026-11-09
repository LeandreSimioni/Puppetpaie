# Le dossier de configuration d'une personne, selon son système (relevé du 09/11/2026)

Où vit la clé du poste quand rien ne la déplace (`PUPPETPAIE_CLE`) et qu'aucun dossier portable ne la garde :
`moteur/src/compagnie/coffre.rs`, `dossier_cle_de`. Le système dit où une application range sa configuration (une
variable d'environnement) et, quand la variable ne le dit pas, quel dossier il lui donne par défaut : c'est ce défaut que
le moteur prend, et rien d'autre.

Pages relevées le 09/11/2026 par `curl -L`, puis transcrites par `pandoc -f html -t plain --wrap=none` (le texte entier de
la page ; aucune retouche). Les originaux HTML vont au dépôt privé (`reference-privee/sources/dossiers-de-configuration/`) ;
leurs condensats SHA-256 sont donnés ici.

| fichier | d'où | date du texte | SHA-256 de l'original HTML |
|---|---|---|---|
| `freedesktop-basedir-spec-20210508.txt` | https://specifications.freedesktop.org/basedir-spec/0.8/ | XDG Base Directory Specification, version 0.8, 08/05/2021 | `87baabbcfb63f544bde60c461f327497704e6de61cb16337635cde80ed19fd73` |
| `microsoft-knownfolderid-20230830.txt` | https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid | mise à jour de la page : 30/08/2023 | `441bf797989b85c3d0505d29e67cc1b9071fef558527d8d53252630bf63229ac` |

## Ce qu'on en lit

- **Linux et les autres Unix**, `freedesktop-basedir-spec-20210508.txt` l. 68 : « $XDG_CONFIG_HOME defines the base
  directory relative to which user-specific configuration files should be stored. If $XDG_CONFIG_HOME is either not set or
  empty, a default equal to $HOME/.config should be used. » ; l. 62 : « All paths set in these environment variables must
  be absolute. If an implementation encounters a relative path in any of these variables it should consider the path
  invalid and ignore it. » Le moteur prend donc `XDG_CONFIG_HOME` s'il est un chemin absolu, `$HOME/.config` sinon (absent,
  vide ou relatif).
- **Windows**, `microsoft-knownfolderid-20230830.txt` l. 1120-1127 : « FOLDERID_RoamingAppData […] Default Path %APPDATA%
  (%USERPROFILE%\AppData\Roaming) ». Le moteur prend `APPDATA` s'il est dit, `%USERPROFILE%\AppData\Roaming` sinon.
