//! L'IMPORT : le passe d'une compagnie, tenu dans un autre logiciel, tel que ses pieces le disent. Les DSN et les AEM
//! deposees (`dsn_aem`), les bulletins (`bulletin_lu`), les certificats Congés Spectacles (`certificat_lu`) et les AEM en
//! PDF (`aem_lu`), ce qu'une piece atteste une seule fois (`attestes`), leur tri dans la boite d'import (`tri`), les
//! fiches que les pieces proposent (`proposee`), la reponse de l'import telle que la page la lit (`reponse`), le mois
//! importe tel que la page le montre (`mois`), les ecarts de calcul (`ecarts`) et le caviardage de ce qui part chez
//! l'editeur (`caviardage`) ; le guide de ce qu'on y met (`guide`). Ce que l'import compose et ecrit pour une route est
//! dans `routes::importation`.

pub mod aem_lu;
pub mod attestes;
pub mod bulletin_lu;
pub mod certificat_lu;
pub mod caviardage;
pub mod dsn_aem;
pub mod ecarts;
pub mod guide;
pub mod mois;
pub mod proposee;
pub mod releve;
pub mod reponse;
pub mod tri;
