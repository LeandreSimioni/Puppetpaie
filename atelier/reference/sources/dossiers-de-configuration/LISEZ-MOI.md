# Le dossier de configuration d'une personne, selon son système (relevé du 09/11/2026)

Où vit la clé du poste quand rien ne la déplace (`PUPPETPAIE_CLE`) et qu'aucun dossier portable ne la garde :
`compagnie/src/coffre.rs`, `dossier_cle_de` ; et où vit ce qui appartient à la personne sur ce poste (le profil
du navigateur de la fenêtre, son jeton) : `moteur/src/interface/navigateur.rs`, `dossier_utilisateur`. Le système dit où une application range sa configuration (une
variable d'environnement) et, quand la variable ne le dit pas, quel dossier il lui donne par défaut : c'est ce défaut que
le moteur prend, et rien d'autre.

Pages relevées le 09/11/2026 par `curl -L`, puis transcrites par `pandoc -f html -t plain --wrap=none` (le texte entier de
la page ; aucune retouche). Les originaux HTML vont au dépôt privé (`reference-privee/sources/dossiers-de-configuration/`) ;
leurs condensats SHA-256 sont donnés ici.

| fichier | d'où | date du texte | SHA-256 de l'original HTML |
|---|---|---|---|
| `freedesktop-basedir-spec-20210508.txt` | https://specifications.freedesktop.org/basedir-spec/0.8/ | XDG Base Directory Specification, version 0.8, 08/05/2021 | `87baabbcfb63f544bde60c461f327497704e6de61cb16337635cde80ed19fd73` |
| `microsoft-knownfolderid-20230830.txt` | https://learn.microsoft.com/en-us/windows/win32/shell/knownfolderid | mise à jour de la page : 30/08/2023 | `441bf797989b85c3d0505d29e67cc1b9071fef558527d8d53252630bf63229ac` |
| `apple-macos-library-directory-details-20180409.txt` (relevé le 09/11/2026, par `curl`) | https://developer.apple.com/library/archive/documentation/FileManagement/Conceptual/FileSystemProgrammingGuide/MacOSXDirectories/MacOSXDirectories.html | « macOS Library Directory Details », File System Programming Guide (archive d'Apple), « Updated: 2018-04-09 » | `2d3e6a8b2b314f593c0172390e3867a2214161fb35cbde98698d9f9db52756e8` |

## Ce qu'on en lit

- **Linux et les autres Unix**, `freedesktop-basedir-spec-20210508.txt` l. 68 : « $XDG_CONFIG_HOME defines the base
  directory relative to which user-specific configuration files should be stored. If $XDG_CONFIG_HOME is either not set or
  empty, a default equal to $HOME/.config should be used. » ; l. 62 : « All paths set in these environment variables must
  be absolute. If an implementation encounters a relative path in any of these variables it should consider the path
  invalid and ignore it. » Le moteur prend donc `XDG_CONFIG_HOME` s'il est un chemin absolu, `$HOME/.config` sinon (absent,
  vide ou relatif).
- **Windows**, `microsoft-knownfolderid-20230830.txt` l. 1120-1127 : « FOLDERID_RoamingAppData […] Default Path %APPDATA%
  (%USERPROFILE%\AppData\Roaming) ». Le moteur prend `APPDATA` s'il est dit, `%USERPROFILE%\AppData\Roaming` sinon.
- **Le profil de la fenêtre, Linux et les autres Unix**, `freedesktop-basedir-spec-20210508.txt` l. 66 : « $XDG_DATA_HOME
  defines the base directory relative to which user-specific data files should be stored. If $XDG_DATA_HOME is either
  not set or empty, a default equal to $HOME/.local/share should be used. » Le moteur prend `XDG_DATA_HOME` s'il est un
  chemin absolu, `$HOME/.local/share` sinon.
- **Le profil de la fenêtre, Windows**, `microsoft-knownfolderid-20230830.txt` l. 614-618 : « FOLDERID_LocalAppData […]
  Default Path %LOCALAPPDATA% (%USERPROFILE%\AppData\Local) ». Le moteur prend `LOCALAPPDATA` s'il est dit,
  `%USERPROFILE%\AppData\Local` sinon.
- **macOS, la clé du poste et le profil de la fenêtre**, `apple-macos-library-directory-details-20180409.txt` l. 26 :
  « Application Support | Contains all app-specific data and support files. » ; l. 28 : « By convention, all of these
  items should be put in a subdirectory whose name matches the bundle identifier of the app. » L'exemple de la page place
  ces fichiers dans `~/Library/Application Support/com.example.MyApp/`. Le moteur prend donc
  `~/Library/Application Support/fr.puppetpaie.fenetre` (l'identifiant de la fenêtre, `fenetre/tauri.conf.json`), pour
  la clé du poste (`coffre.rs`, `dossier_cle_de`) comme pour le profil (`navigateur.rs`, `dossier_utilisateur`).

## Empreintes

Le SHA-256 de chaque fichier de ce dossier, tel que déposé (`moteur-atelier empreintes`).

- `apple-macos-library-directory-details-20180409.txt` : `7b617c8046f50c6710c439c67169af72fb124309015ab7219cbe266c235f7e61`
- `freedesktop-basedir-spec-20210508.txt` : `b377df12ebe6f17bf3a3f09fd794d7d521f20b4419a186cd9713c30a4b384742`
- `microsoft-knownfolderid-20230830.txt` : `3fc71956835439acd2922960a279e6be46e7ded1992ff94a2c4d0573b8070ccf`
