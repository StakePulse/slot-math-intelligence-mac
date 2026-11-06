//! La loi d'un mode de jeu : ce que peut payer un round et avec quel poids.
//!
//! Même convention que les lookup tables de Stake Engine : un paiement est un entier en
//! **centièmes de mise de base** (100 = 1×), un poids est un entier ≥ 0, et
//! `P(issue i) = w_i / W` avec `W = Σ w_j`. Toutes les sommes sont faites en entiers (`u128`) :
//! RTP, probabilités et contributions sont exacts.
//!
//! Chaque issue appartient à une **classe de résultat** (pas de gain, gains du jeu de base, bonus,
//! max win…). Les classes forment une partition : leurs probabilités se somment à 1 et leurs points de
//! RTP au RTP du mode.

use serde::{Deserialize, Serialize};

/// Un paiement de 1× la mise de base vaut `CENTS`.
pub const CENTS: u64 = 100;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ClassKind {
    NoWin,
    Base,
    Bonus,
    MaxWin,
    Other,
}

impl ClassKind {
    /// Les classes qui comptent comme « bonus » dans la répartition base / bonus.
    pub fn is_bonus(self) -> bool {
        matches!(self, ClassKind::Bonus | ClassKind::MaxWin)
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ClassDef {
    pub key: String,
    pub label: String,
    pub kind: ClassKind,
}

impl ClassDef {
    pub fn new(key: &str, label: &str, kind: ClassKind) -> Self {
        Self { key: key.into(), label: label.into(), kind }
    }
}

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Atom {
    /// Paiement en centièmes de mise de base.
    pub payout: u64,
    /// Poids entier (probabilité = poids / poids total).
    pub weight: u64,
    /// Indice dans `Distribution::classes`.
    pub class: u16,
    /// Nombre de books (issues distinctes) regroupés sous ce paiement.
    pub books: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Distribution {
    pub name: String,
    /// Prix d'un round en mises de base (1 pour le jeu de base, 100 pour un achat à 100×…).
    pub cost: f64,
    /// Plafond de gain par round, en centièmes de mise de base.
    pub cap: Option<u64>,
    pub classes: Vec<ClassDef>,
    /// Triées par (paiement, classe), sans doublon.
    pub atoms: Vec<Atom>,
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum DistError {
    #[error("la loi est vide ou tous ses poids sont nuls")]
    Empty,
    #[error("ligne {line} : {msg}")]
    Line { line: usize, msg: String },
    #[error("classe {0} inconnue")]
    UnknownClass(u16),
    #[error("prix du mode invalide : {0}")]
    BadCost(f64),
}

/// Informations de lecture d'une lookup table `id,weight,payout`.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct LutInfo {
    pub rows: u64,
    pub zero_weight_rows: u64,
    pub distinct_payouts: u64,
    pub first_id: u64,
    pub last_id: u64,
    /// Les ids se suivent sans trou ni doublon.
    pub ids_contiguous: bool,
}

impl Distribution {
    pub fn new(name: &str, cost: f64, cap: Option<u64>, classes: Vec<ClassDef>, atoms: Vec<Atom>) -> Result<Self, DistError> {
        if !(cost.is_finite() && cost > 0.0) {
            return Err(DistError::BadCost(cost));
        }
        let mut d = Self { name: name.into(), cost, cap, classes, atoms };
        for a in &d.atoms {
            if a.class as usize >= d.classes.len() {
                return Err(DistError::UnknownClass(a.class));
            }
        }
        d.normalize();
        if d.total_weight() == 0 {
            return Err(DistError::Empty);
        }
        Ok(d)
    }

    /// Les deux classes par défaut d'une table sans information de bonus.
    pub fn default_classes() -> Vec<ClassDef> {
        vec![
            ClassDef::new("no_win", "Pas de gain", ClassKind::NoWin),
            ClassDef::new("win", "Gain (origine non publiée)", ClassKind::Other),
        ]
    }

    /// Loi à partir de couples (paiement en centièmes, poids) : classe `no_win` pour 0, `win` sinon.
    pub fn from_pairs(name: &str, cost: f64, cap: Option<u64>, pairs: impl IntoIterator<Item = (u64, u64)>) -> Result<Self, DistError> {
        let atoms = pairs
            .into_iter()
            .map(|(payout, weight)| Atom { payout, weight, class: u16::from(payout > 0), books: 1 })
            .collect();
        Self::new(name, cost, cap, Self::default_classes(), atoms)
    }

    /// Lit une lookup table Stake Engine (`id,weight,payout` par ligne, en-tête facultatif).
    /// Les poids négatifs, non entiers ou les lignes incomplètes sont refusés avec leur numéro de ligne.
    pub fn from_lut_csv(name: &str, cost: f64, cap: Option<u64>, text: &str) -> Result<(Self, LutInfo), DistError> {
        let mut info = LutInfo { ids_contiguous: true, ..Default::default() };
        let mut agg: std::collections::BTreeMap<u64, (u64, u32)> = std::collections::BTreeMap::new();
        let mut prev_id: Option<u64> = None;
        for (i, raw) in text.lines().enumerate() {
            let line = raw.trim().trim_start_matches('\u{feff}');
            if line.is_empty() {
                continue;
            }
            let parts: Vec<&str> = line.split(',').map(str::trim).collect();
            if parts.len() < 3 {
                return Err(DistError::Line { line: i + 1, msg: format!("3 colonnes attendues (id,weight,payout), {} lues", parts.len()) });
            }
            let parse = |s: &str, what: &str| -> Result<u64, DistError> {
                s.parse::<u64>().map_err(|_| DistError::Line { line: i + 1, msg: format!("{what} « {s} » n'est pas un entier ≥ 0") })
            };
            let id = match parts[0].parse::<u64>() {
                Ok(v) => v,
                Err(_) if info.rows == 0 && i == 0 => continue, // en-tête
                Err(_) => return Err(DistError::Line { line: i + 1, msg: format!("id « {} » invalide", parts[0]) }),
            };
            let weight = parse(parts[1], "le poids")?;
            let payout = parse(parts[2], "le paiement")?;
            if info.rows == 0 {
                info.first_id = id;
            }
            if let Some(p) = prev_id {
                if id != p + 1 {
                    info.ids_contiguous = false;
                }
            }
            prev_id = Some(id);
            info.last_id = id;
            info.rows += 1;
            if weight == 0 {
                info.zero_weight_rows += 1;
            }
            let e = agg.entry(payout).or_insert((0, 0));
            e.0 += weight;
            e.1 += 1;
        }
        info.distinct_payouts = agg.len() as u64;
        let atoms = agg
            .into_iter()
            .map(|(payout, (weight, books))| Atom { payout, weight, class: u16::from(payout > 0), books })
            .collect();
        let d = Self::new(name, cost, cap, Self::default_classes(), atoms)?;
        Ok((d, info))
    }

    /// Trie, fusionne les doublons (même paiement, même classe) et retire les poids nuls
    /// (une issue de poids 0 ne peut jamais être tirée).
    pub fn normalize(&mut self) {
        self.atoms.sort_by_key(|a| (a.payout, a.class));
        let mut out: Vec<Atom> = Vec::with_capacity(self.atoms.len());
        for a in self.atoms.drain(..) {
            if a.weight == 0 {
                continue;
            }
            match out.last_mut() {
                Some(l) if l.payout == a.payout && l.class == a.class => {
                    l.weight += a.weight;
                    l.books += a.books;
                }
                _ => out.push(a),
            }
        }
        self.atoms = out;
    }

    pub fn total_weight(&self) -> u128 {
        self.atoms.iter().map(|a| a.weight as u128).sum()
    }

    /// Paiement maximal parmi les issues tirables (centièmes).
    pub fn max_payout(&self) -> u64 {
        self.atoms.iter().filter(|a| a.weight > 0).map(|a| a.payout).max().unwrap_or(0)
    }

    /// Même support, autres poids (sert à lire une simulation avec les mêmes outils que la loi exacte).
    pub fn with_weights(&self, weights: &[u64]) -> Self {
        assert_eq!(weights.len(), self.atoms.len());
        let atoms = self.atoms.iter().zip(weights).map(|(a, &w)| Atom { weight: w, ..*a }).collect();
        Self { atoms, ..self.clone() }
    }

    /// Loi conditionnelle : les seules issues dont la classe vérifie `keep` (même prix, même plafond).
    pub fn conditional(&self, name: &str, keep: impl Fn(&ClassDef) -> bool) -> Option<Self> {
        let atoms: Vec<Atom> = self.atoms.iter().filter(|a| a.weight > 0 && keep(&self.classes[a.class as usize])).copied().collect();
        if atoms.is_empty() {
            return None;
        }
        Some(Self { name: name.into(), atoms, ..self.clone() })
    }

    /// Indices des classes de bonus (Bonus + MaxWin).
    pub fn has_bonus_classes(&self) -> bool {
        self.classes.iter().any(|c| c.kind.is_bonus())
    }

    /// Empreinte FNV-1a 64 bits de la loi (support, poids, classes, prix) : identifie une version.
    pub fn fingerprint(&self) -> String {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut eat = |v: u64| {
            for b in v.to_le_bytes() {
                h ^= b as u64;
                h = h.wrapping_mul(0x0100_0000_01b3);
            }
        };
        eat(self.cost.to_bits());
        eat(self.cap.unwrap_or(u64::MAX));
        for a in &self.atoms {
            eat(a.payout);
            eat(a.weight);
            eat(a.class as u64);
        }
        format!("{h:016x}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lut_parsing_aggregates_and_reports() {
        let csv = "id,weight,payout\n1,10,0\n2,5,150\n3,5,150\n4,0,9999\n5,1,5000\n";
        let (d, info) = Distribution::from_lut_csv("t", 1.0, None, csv).unwrap();
        assert_eq!(info.rows, 5);
        assert_eq!(info.zero_weight_rows, 1);
        assert!(info.ids_contiguous);
        assert_eq!(d.atoms.len(), 3); // 0, 150, 5000 (9999 a un poids nul)
        assert_eq!(d.total_weight(), 21);
        assert_eq!(d.atoms[1], Atom { payout: 150, weight: 10, class: 1, books: 2 });
    }

    #[test]
    fn lut_rejects_negative_weight() {
        let err = Distribution::from_lut_csv("t", 1.0, None, "1,-3,100\n").unwrap_err();
        assert!(matches!(err, DistError::Line { line: 1, .. }));
    }

    #[test]
    fn empty_is_rejected() {
        assert_eq!(Distribution::from_pairs("t", 1.0, None, [(0, 0)]).unwrap_err(), DistError::Empty);
    }
}
