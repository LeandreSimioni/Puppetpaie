# Movinmotion, aide en ligne : « Votre numéro de congés spectacles n'est pas valide, comment faire ? »

Transcription d'une capture d'écran prise par Léandre sur son téléphone le 19/11/2026, de l'aide en ligne de
Movinmotion Salarié (`…support.movinmotion.com`, rubrique « Mes informations » ; l'adresse entière n'est pas sur la
capture ; « Mise à jour : il y a 4 ans »). Movinmotion est un éditeur de paie privé : la capture est au privé
(`reference-privee/exemples/`), seule cette transcription est ici. Le texte ci-dessous est celui de la capture, mot
pour mot.

---

Le format du numéro de **congés spectacles** attendu sur votre espace salarié Movinmotion est : 1 lettre, suivie de 7
chiffres.

- *Exemple : M784501*

Si votre numéro de congés spectacles est composé d'1 lettre et de 6 chiffres, il faut ajouter un 0 entre la lettre et
les chiffres (la plateforme le fera automatiquement au moment de l'enregistrement).

- *Exemple : M784501 -> saisir M0784501*

Si malgré ces vérifications votre numéro de congés spectacles n'est pas accepté par la plateforme, nous vous invitons à
contacter la caisse des congés spectacles au 01 73 17 34 34 afin de vérifier avec eux que ce numéro est le bon.

---

## Ce qu'on en lit, et ce que cela vaut

Ce n'est pas un texte de la caisse : c'est la pratique d'un autre logiciel, qui ne fonde aucune règle (AGENTS, « Les
sources »). Elle explique seulement l'écart entre deux choses déposées : le mode d'emploi de la caisse dit le matricule
« formé d'une lettre suivie de six chiffres » (`audiens-conges-spectacles-mode-emploi-employeur-202304-20261021.txt`,
l. 169), et des matricules réels ont une lettre et sept chiffres (Léandre, le 19/11/2026, du sien : « Le mien aussi est
comme ça »). Les deux formes existent ; que la forme à sept chiffres soit celle à six précédée d'un zéro est ce que
dit cet éditeur, pas la caisse.

## Où le moteur s'en sert

Nulle part : la case « Numéro Congés Spectacles » de la fiche du salarié reste un texte libre, que le moteur imprime
tel qu'il est saisi (`logiciel/donnees/champs-base.json`, `num_conges_spectacles`, exemple `E0123456`). Il n'ajoute ni
n'ôte un zéro.
