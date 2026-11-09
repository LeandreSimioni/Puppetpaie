//! L'ATELIER : les outils de l'editeur et des seances, a cote du produit. Le releve des textes (LEGI, KALI, la veille
//! des sources), les referentiels extraits des textes deposes, les tables Unicode generees, le garde avant commit, le
//! guet du contexte, la publication des versions (la cle de l'editeur), et leurs commandes (`commandes`). Le scenario
//! d'une compagnie fictive, que le logiciel livre rejoue, n'en est pas : il est au niveau des routes (`scenario`).
//!
//! Seuls la redirection vers de faux serveurs locaux, les bulletins fictifs et les outils sur une compagnie
//! (`commandes::outils` : les taux de PAS, les AER, la reprise des DPAE, l'enquete des retours, le depot de test a un
//! ordre) n'existent que dans le binaire de l'atelier (`--features atelier`, jamais livre).

pub mod commandes;
pub mod extraire;
pub mod generer_tables;
pub mod guet;
pub mod publier;
pub mod releve;
pub mod sources;
pub mod verifier_avant_commit;

#[cfg(feature = "atelier")]
pub mod bulletins_fictifs;
#[cfg(feature = "atelier")]
pub mod fabriquer_bulletins;
#[cfg(feature = "atelier")]
pub mod redirection;

/// UN OBJET DONT L'ORDRE COMPTE (un referentiel ecrit dans l'ordre de son texte depose, un resume dans l'ordre ou il
/// s'est lu) : ses paires `(cle, valeur)`, lues (`socle::forme::en_ordre`, une cle repetee refuse) et ecrites
/// (`socle::ecriture::en_ordre`) dans leur ordre. Une cle posee de nouveau garde sa place et prend la valeur neuve.
#[derive(Debug, Clone, PartialEq)]
pub struct EnOrdre<V>(pub Vec<(String, V)>);

impl<V> EnOrdre<V> {
    pub(crate) fn new() -> EnOrdre<V> {
        EnOrdre(Vec::new())
    }

    /// La valeur de la cle `k`, si elle est posee.
    pub(crate) fn valeur(&self, k: &str) -> Option<&V> {
        self.0.iter().find(|(c, _)| c == k).map(|(_, v)| v)
    }

    /// Pose `v` sous la cle `k` : a sa place si elle l'est deja, a la fin sinon.
    pub(crate) fn poser(&mut self, k: String, v: V) {
        match self.0.iter_mut().find(|(c, _)| *c == k) {
            Some((_, deja)) => *deja = v,
            None => self.0.push((k, v)),
        }
    }

    /// La valeur de la cle `k`, posee d'abord par `neuve` si elle ne l'est pas.
    pub(crate) fn entree(&mut self, k: String, neuve: impl FnOnce() -> V) -> &mut V {
        let i = match self.0.iter().position(|(c, _)| *c == k) {
            Some(i) => i,
            None => {
                self.0.push((k, neuve()));
                self.0.len() - 1
            }
        };
        &mut self.0[i].1
    }

    pub(crate) fn len(&self) -> usize {
        self.0.len()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl<V: serde::Serialize> serde::Serialize for EnOrdre<V> {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        socle::ecriture::en_ordre(&self.0, s)
    }
}

impl<'de, V: serde::Deserialize<'de>> serde::Deserialize<'de> for EnOrdre<V> {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<EnOrdre<V>, D::Error> {
        socle::forme::en_ordre(d).map(EnOrdre)
    }
}
