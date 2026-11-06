//! Moteur de règles générique : rouleaux à bandes, gains en ways ou en lignes, wild, scatter,
//! tours gratuits avec relances et multiplicateur, plafond de gain, achat de bonus.
//!
//! Trois méthodes, de la plus sûre à la moins sûre :
//! 1. **Espérance exacte** du jeu de base (formules fermées, rouleaux indépendants, arrêts uniformes) :
//!    - ways : `E = Σ_s Σ_L pay_s(L) · (Π_{r<L} E[a_r] − Π_{r<L} E[w_r]) · P(a_L = 0)` où `a_r` = nombre de
//!      cases du rouleau r portant s ou wild, `w_r` = nombre de wilds (les ways faites QUE de wilds ne
//!      paient pas) ;
//!    - lignes : même formule avec les probabilités par case de chaque ligne ;
//!    - scatters : loi exacte du nombre de scatters visibles (convolution des rouleaux) → gains de
//!      scatter et probabilité de déclenchement EXACTES.
//! 2. **Énumération** de toutes les combinaisons d'arrêts quand Π longueurs ≤ `enumeration_limit` :
//!    loi EXACTE complète du gain d'un spin de base (hors tours gratuits).
//! 3. **Simulation** (MONTE CARLO, graine, flux par lot) des rounds complets, tours gratuits compris :
//!    elle produit une loi empirique (paiement → nombre de rounds, par classe) analysée avec les mêmes
//!    outils que la loi exacte, et comparée aux valeurs exactes des points 1 et 2.

use crate::dist::{Atom, ClassDef, ClassKind, Distribution, CENTS};
use crate::rng::Xoshiro256pp;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct SymbolDef {
    pub name: String,
    /// Gain (en multiples de la mise totale) pour k symboles alignés : `pays[k]`, k = 0..=rouleaux.
    pub pays: Vec<f64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PayMode {
    Ways,
    /// Une ligne = l'indice de rangée sur chaque rouleau ; les gains de ligne sont en multiples de la mise totale.
    Lines { lines: Vec<Vec<usize>> },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScatterDef {
    pub symbol: usize,
    /// Gain de scatter (× mise totale) pour k scatters visibles.
    pub pays: Vec<f64>,
    /// Tours gratuits accordés pour k scatters au jeu de base.
    pub free_spins: Vec<u32>,
    /// Tours ajoutés pour k scatters pendant les tours gratuits.
    pub retrigger: Vec<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReelGame {
    pub name: String,
    pub rows: Vec<usize>,
    pub symbols: Vec<SymbolDef>,
    pub wild: Option<usize>,
    pub scatter: Option<ScatterDef>,
    pub pay_mode: PayMode,
    pub reels_base: Vec<Vec<usize>>,
    #[serde(default)]
    pub reels_free: Option<Vec<Vec<usize>>>,
    /// Multiplicateur appliqué à tous les gains des tours gratuits.
    #[serde(default = "one")]
    pub free_spins_multiplier: f64,
    #[serde(default = "default_max_fs")]
    pub max_free_spins: u32,
    /// Plafond par round en multiples de la mise.
    pub cap: f64,
}

fn one() -> f64 {
    1.0
}
fn default_max_fs() -> u32 {
    100
}

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum ReelError {
    #[error("configuration invalide : {0}")]
    Invalid(String),
}

impl ReelGame {
    pub fn validate(&self) -> Result<(), ReelError> {
        let n_reels = self.rows.len();
        let bad = |m: String| Err(ReelError::Invalid(m));
        if n_reels < 2 {
            return bad("au moins 2 rouleaux".into());
        }
        if self.reels_base.len() != n_reels {
            return bad(format!("{} bandes de base pour {} rouleaux", self.reels_base.len(), n_reels));
        }
        if let Some(f) = &self.reels_free {
            if f.len() != n_reels {
                return bad("bandes des tours gratuits : nombre de rouleaux incorrect".into());
            }
        }
        for strips in std::iter::once(&self.reels_base).chain(self.reels_free.iter()) {
            for (r, s) in strips.iter().enumerate() {
                if s.len() < self.rows[r] {
                    return bad(format!("bande du rouleau {} plus courte que la fenêtre", r + 1));
                }
                if let Some(x) = s.iter().find(|&&x| x >= self.symbols.len()) {
                    return bad(format!("symbole {x} inconnu sur le rouleau {}", r + 1));
                }
            }
        }
        for s in &self.symbols {
            if s.pays.len() != n_reels + 1 {
                return bad(format!("le symbole {} doit avoir {} gains (k = 0..{n_reels})", s.name, n_reels + 1));
            }
        }
        if let PayMode::Lines { lines } = &self.pay_mode {
            for l in lines {
                if l.len() != n_reels || l.iter().zip(&self.rows).any(|(&r, &h)| r >= h) {
                    return bad("une ligne sort de la fenêtre".into());
                }
            }
        }
        if let Some(sc) = &self.scatter {
            let need = self.rows.iter().sum::<usize>() + 1;
            if sc.pays.len() < need.min(n_reels + 1) || sc.free_spins.len() != sc.pays.len() || sc.retrigger.len() != sc.pays.len() {
                return bad("scatter : pays, free_spins et retrigger doivent avoir la même longueur (k = 0..max)".into());
            }
        }
        if self.cap <= 0.0 {
            return bad("plafond ≤ 0".into());
        }
        Ok(())
    }

    fn is_wild(&self, s: usize) -> bool {
        self.wild == Some(s)
    }

    fn is_scatter(&self, s: usize) -> bool {
        self.scatter.as_ref().is_some_and(|sc| sc.symbol == s)
    }

    /// Gain (× mise) des lignes ou ways d'une fenêtre `win[r][row]`.
    pub fn evaluate(&self, win: &[Vec<usize>]) -> f64 {
        let n = self.rows.len();
        let mut total = 0.0;
        match &self.pay_mode {
            PayMode::Ways => {
                for (s, def) in self.symbols.iter().enumerate() {
                    if self.is_wild(s) || self.is_scatter(s) || def.pays.iter().all(|&p| p == 0.0) {
                        continue;
                    }
                    let (mut prod_a, mut prod_w, mut len) = (1.0f64, 1.0f64, 0usize);
                    for col in win.iter().take(n) {
                        let a = col.iter().filter(|&&x| x == s || self.is_wild(x)).count();
                        if a == 0 {
                            break;
                        }
                        let w = col.iter().filter(|&&x| self.is_wild(x)).count();
                        prod_a *= a as f64;
                        prod_w *= w as f64;
                        len += 1;
                    }
                    if len > 0 {
                        total += def.pays[len] * (prod_a - prod_w);
                    }
                }
            }
            PayMode::Lines { lines } => {
                for line in lines {
                    let cells: Vec<usize> = line.iter().enumerate().map(|(r, &row)| win[r][row]).collect();
                    let Some(&s) = cells.iter().find(|&&x| !self.is_wild(x)) else { continue };
                    if self.is_scatter(s) {
                        continue;
                    }
                    let len = cells.iter().take_while(|&&x| x == s || self.is_wild(x)).count();
                    total += self.symbols[s].pays[len];
                }
            }
        }
        total
    }

    fn scatters(&self, win: &[Vec<usize>]) -> usize {
        match &self.scatter {
            Some(sc) => win.iter().flatten().filter(|&&x| x == sc.symbol).count(),
            None => 0,
        }
    }

    fn window(&self, strips: &[Vec<usize>], stops: &[usize]) -> Vec<Vec<usize>> {
        strips.iter().zip(stops).zip(&self.rows).map(|((s, &st), &h)| (0..h).map(|i| s[(st + i) % s.len()]).collect()).collect()
    }

    fn spin(&self, strips: &[Vec<usize>], rng: &mut Xoshiro256pp) -> Vec<Vec<usize>> {
        let stops: Vec<usize> = strips.iter().map(|s| rng.below(s.len() as u64) as usize).collect();
        self.window(strips, &stops)
    }

    /// Valeur d'une session de tours gratuits (× mise), plafonnée.
    fn free_spins(&self, initial: u32, rng: &mut Xoshiro256pp, already: f64) -> (f64, u32) {
        let strips = self.reels_free.as_ref().unwrap_or(&self.reels_base);
        let sc = self.scatter.as_ref();
        let mut left = initial.min(self.max_free_spins);
        let mut played = 0u32;
        let mut total = 0.0;
        while left > 0 {
            left -= 1;
            played += 1;
            let win = self.spin(strips, rng);
            let k = self.scatters(&win);
            let mut w = self.evaluate(&win) * self.free_spins_multiplier;
            if let Some(sc) = sc {
                w += sc.pays.get(k).copied().unwrap_or(0.0) * self.free_spins_multiplier;
                let add = sc.retrigger.get(k).copied().unwrap_or(0);
                left = (left + add).min(self.max_free_spins.saturating_sub(played));
            }
            total += w;
            if already + total >= self.cap {
                return (self.cap - already, played);
            }
        }
        (total, played)
    }

    /// Un round complet (× mise). `buy` : nombre de scatters imposé (achat de bonus).
    pub fn play_round(&self, rng: &mut Xoshiro256pp, buy: Option<usize>) -> RoundOutcome {
        if let Some(k) = buy {
            let fs = self.scatter.as_ref().and_then(|s| s.free_spins.get(k).copied()).unwrap_or(0);
            let (v, played) = self.free_spins(fs, rng, 0.0);
            return RoundOutcome { win: v.min(self.cap), bonus: true, free_spins: played, scatters: k };
        }
        let win = self.spin(&self.reels_base, rng);
        let k = self.scatters(&win);
        let mut total = self.evaluate(&win);
        let mut bonus = false;
        let mut played = 0;
        if let Some(sc) = &self.scatter {
            total += sc.pays.get(k).copied().unwrap_or(0.0);
            let fs = sc.free_spins.get(k).copied().unwrap_or(0);
            if fs > 0 && total < self.cap {
                bonus = true;
                let (v, p) = self.free_spins(fs, rng, total);
                total += v;
                played = p;
            }
        }
        RoundOutcome { win: total.min(self.cap), bonus, free_spins: played, scatters: k }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RoundOutcome {
    pub win: f64,
    pub bonus: bool,
    pub free_spins: u32,
    pub scatters: usize,
}

// ---------------------------------------------------------------------------------------------
// Calculs exacts du jeu de base
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ExactBase {
    /// Espérance des gains de lignes / ways d'un spin de base (× mise), EXACT.
    pub line_or_ways_rtp: f64,
    /// Espérance des gains de scatter (× mise), EXACT.
    pub scatter_rtp: f64,
    /// Loi exacte du nombre de scatters visibles : P(k).
    pub scatter_dist: Vec<f64>,
    /// Probabilité exacte de déclencher les tours gratuits.
    pub trigger_prob: f64,
    /// Nombre de combinaisons d'arrêts (Π longueurs des bandes).
    pub combinations: f64,
    /// Loi exacte complète du gain d'un spin de base hors tours gratuits, si l'énumération a été faite.
    pub enumerated: Option<EnumeratedBase>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct EnumeratedBase {
    pub rtp: f64,
    pub hit_rate: f64,
    pub trigger_prob: f64,
    /// (gain en centièmes, nombre de combinaisons).
    pub law: Vec<(u64, u64)>,
}

/// Comptes par rouleau : pour chaque arrêt, la fenêtre ; puis les moyennes nécessaires.
pub fn exact_base(g: &ReelGame, enumeration_limit: f64) -> Result<ExactBase, ReelError> {
    g.validate()?;
    let n = g.rows.len();
    let strips = &g.reels_base;
    let windows: Vec<Vec<Vec<usize>>> = (0..n)
        .map(|r| (0..strips[r].len()).map(|st| (0..g.rows[r]).map(|i| strips[r][(st + i) % strips[r].len()]).collect()).collect())
        .collect();

    // Espérance des lignes / ways.
    let mut ev = 0.0;
    match &g.pay_mode {
        PayMode::Ways => {
            for (s, def) in g.symbols.iter().enumerate() {
                if g.is_wild(s) || g.is_scatter(s) {
                    continue;
                }
                let mut ea = vec![0.0; n];
                let mut ew = vec![0.0; n];
                let mut p_empty = vec![0.0; n];
                for r in 0..n {
                    let m = windows[r].len() as f64;
                    for w in &windows[r] {
                        let a = w.iter().filter(|&&x| x == s || g.is_wild(x)).count() as f64;
                        let wc = w.iter().filter(|&&x| g.is_wild(x)).count() as f64;
                        ea[r] += a / m;
                        ew[r] += wc / m;
                        if a == 0.0 {
                            p_empty[r] += 1.0 / m;
                        }
                    }
                }
                for len in 1..=n {
                    let pay = def.pays[len];
                    if pay == 0.0 {
                        continue;
                    }
                    let pa: f64 = ea[..len].iter().product();
                    let pw: f64 = ew[..len].iter().product();
                    let stop = if len < n { p_empty[len] } else { 1.0 };
                    ev += pay * (pa - pw) * stop;
                }
            }
        }
        PayMode::Lines { lines } => {
            for line in lines {
                // p[r][x] = P(case de la ligne sur le rouleau r = x).
                let p: Vec<Vec<f64>> = (0..n)
                    .map(|r| {
                        let mut v = vec![0.0; g.symbols.len()];
                        let m = windows[r].len() as f64;
                        for w in &windows[r] {
                            v[w[line[r]]] += 1.0 / m;
                        }
                        v
                    })
                    .collect();
                for (s, def) in g.symbols.iter().enumerate() {
                    if g.is_wild(s) || g.is_scatter(s) {
                        continue;
                    }
                    let psw = |r: usize| p[r][s] + g.wild.map_or(0.0, |w| p[r][w]);
                    let pw = |r: usize| g.wild.map_or(0.0, |w| p[r][w]);
                    for len in 1..=n {
                        let pay = def.pays[len];
                        if pay == 0.0 {
                            continue;
                        }
                        let a: f64 = (0..len).map(psw).product();
                        let b: f64 = (0..len).map(pw).product();
                        let stop = if len < n { 1.0 - psw(len) } else { 1.0 };
                        ev += pay * (a - b) * stop;
                    }
                }
            }
        }
    }

    // Loi exacte du nombre de scatters.
    let mut sdist = vec![1.0];
    if let Some(sc) = &g.scatter {
        for r in 0..n {
            let m = windows[r].len() as f64;
            let mut per = vec![0.0; g.rows[r] + 1];
            for w in &windows[r] {
                per[w.iter().filter(|&&x| x == sc.symbol).count()] += 1.0 / m;
            }
            let mut next = vec![0.0; sdist.len() + per.len() - 1];
            for (i, a) in sdist.iter().enumerate() {
                for (j, b) in per.iter().enumerate() {
                    next[i + j] += a * b;
                }
            }
            sdist = next;
        }
    }
    let (scatter_rtp, trigger_prob) = match &g.scatter {
        Some(sc) => (
            sdist.iter().enumerate().map(|(k, p)| p * sc.pays.get(k).copied().unwrap_or(0.0)).sum(),
            sdist.iter().enumerate().filter(|(k, _)| sc.free_spins.get(*k).copied().unwrap_or(0) > 0).map(|(_, p)| p).sum(),
        ),
        None => (0.0, 0.0),
    };

    let combinations: f64 = strips.iter().map(|s| s.len() as f64).product();
    let enumerated = if combinations <= enumeration_limit {
        let mut law: BTreeMap<u64, u64> = BTreeMap::new();
        let mut trig = 0u64;
        let mut stops = vec![0usize; n];
        loop {
            let win: Vec<Vec<usize>> = (0..n).map(|r| windows[r][stops[r]].clone()).collect();
            let k = g.scatters(&win);
            let mut x = g.evaluate(&win);
            if let Some(sc) = &g.scatter {
                x += sc.pays.get(k).copied().unwrap_or(0.0);
                if sc.free_spins.get(k).copied().unwrap_or(0) > 0 {
                    trig += 1;
                }
            }
            *law.entry((x.min(g.cap) * CENTS as f64).round() as u64).or_insert(0) += 1;
            // Incrément de l'odomètre.
            let mut r = 0;
            loop {
                if r == n {
                    break;
                }
                stops[r] += 1;
                if stops[r] < windows[r].len() {
                    break;
                }
                stops[r] = 0;
                r += 1;
            }
            if r == n {
                break;
            }
        }
        let total: u64 = law.values().sum();
        let rtp = law.iter().map(|(p, c)| *p as f64 * *c as f64).sum::<f64>() / total as f64 / CENTS as f64;
        let hit = law.iter().filter(|(p, _)| **p > 0).map(|(_, c)| *c).sum::<u64>() as f64 / total as f64;
        Some(EnumeratedBase { rtp, hit_rate: hit, trigger_prob: trig as f64 / total as f64, law: law.into_iter().collect() })
    } else {
        None
    };

    Ok(ExactBase { line_or_ways_rtp: ev, scatter_rtp, scatter_dist: sdist, trigger_prob, combinations, enumerated })
}

// ---------------------------------------------------------------------------------------------
// Simulation des rounds complets
// ---------------------------------------------------------------------------------------------

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ReelSimResult {
    pub rounds: u64,
    pub seed: u64,
    pub buy_scatters: Option<usize>,
    pub price: f64,
    /// Loi empirique des rounds (classes : pas de gain, base, bonus, plafond) — nature MONTE CARLO.
    pub distribution: Distribution,
    pub bonus_rounds: u64,
    pub free_spins_played: u64,
    pub trigger_rate: f64,
    pub elapsed_ms: u64,
}

pub fn simulate(g: &ReelGame, rounds: u64, seed: u64, threads: usize, buy: Option<(usize, f64)>, ctl: &crate::sim::Control) -> Result<ReelSimResult, ReelError> {
    g.validate()?;
    let started = std::time::Instant::now();
    let chunk: u64 = 100_000;
    let n_chunks = rounds.div_ceil(chunk);
    let next = AtomicU64::new(0);
    let merged: Mutex<(BTreeMap<(u64, u16), u64>, u64, u64)> = Mutex::new((BTreeMap::new(), 0, 0));
    let cap_c = (g.cap * CENTS as f64).round() as u64;
    ctl.done.store(0, Ordering::Relaxed);
    std::thread::scope(|scope| {
        for _ in 0..threads.clamp(1, 256) {
            scope.spawn(|| {
                let mut local: BTreeMap<(u64, u16), u64> = BTreeMap::new();
                let (mut nb, mut fs) = (0u64, 0u64);
                loop {
                    let ci = next.fetch_add(1, Ordering::Relaxed);
                    if ci >= n_chunks || ctl.cancel.load(Ordering::Relaxed) {
                        break;
                    }
                    let len = chunk.min(rounds - ci * chunk);
                    let mut rng = Xoshiro256pp::stream(seed, ci);
                    for _ in 0..len {
                        let o = g.play_round(&mut rng, buy.map(|b| b.0));
                        let c = (o.win * CENTS as f64).round() as u64;
                        let class = if c >= cap_c { 3 } else if o.bonus { 2 } else if c > 0 { 1 } else { 0 };
                        *local.entry((c, class)).or_insert(0) += 1;
                        if o.bonus {
                            nb += 1;
                        }
                        fs += o.free_spins as u64;
                    }
                    ctl.done.fetch_add(len, Ordering::Relaxed);
                    while ctl.pause.load(Ordering::Relaxed) && !ctl.cancel.load(Ordering::Relaxed) {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                    }
                }
                let mut m = merged.lock().unwrap();
                for (k, v) in local {
                    *m.0.entry(k).or_insert(0) += v;
                }
                m.1 += nb;
                m.2 += fs;
            });
        }
    });
    let (law, nb, fs) = merged.into_inner().unwrap();
    let done: u64 = law.values().sum();
    let classes = vec![
        ClassDef::new("no_win", "Pas de gain", ClassKind::NoWin),
        ClassDef::new("base", "Gains du jeu de base", ClassKind::Base),
        ClassDef::new("bonus", "Tours gratuits", ClassKind::Bonus),
        ClassDef::new("max", "Plafond atteint", ClassKind::MaxWin),
    ];
    let atoms = law.into_iter().map(|((payout, class), w)| Atom { payout, weight: w, class, books: 1 }).collect();
    let price = buy.map_or(1.0, |b| b.1);
    let distribution = Distribution::new(&format!("{} (simulation)", g.name), price, Some(cap_c), classes, atoms).map_err(|e| ReelError::Invalid(e.to_string()))?;
    Ok(ReelSimResult {
        rounds: done,
        seed,
        buy_scatters: buy.map(|b| b.0),
        price,
        distribution,
        bonus_rounds: nb,
        free_spins_played: fs,
        trigger_rate: nb as f64 / done.max(1) as f64,
        elapsed_ms: started.elapsed().as_millis() as u64,
    })
}

/// Jeu de démonstration 5×3, 243 ways, wild, scatter (3/4/5 → 8/12/20 tours, ×2 en tours gratuits).
/// C'est une CONFIGURATION d'exemple pour éditer ses propres règles, pas un jeu réglé.
pub fn demo_game() -> ReelGame {
    let names = ["H1", "H2", "H3", "H4", "L1", "L2", "L3", "L4", "L5", "WILD", "SCATTER"];
    let pays: [[f64; 6]; 11] = [
        [0.0, 0.0, 0.0, 1.0, 2.5, 10.0],
        [0.0, 0.0, 0.0, 0.75, 2.0, 6.0],
        [0.0, 0.0, 0.0, 0.5, 1.5, 4.0],
        [0.0, 0.0, 0.0, 0.4, 1.0, 3.0],
        [0.0, 0.0, 0.0, 0.2, 0.5, 1.5],
        [0.0, 0.0, 0.0, 0.2, 0.5, 1.5],
        [0.0, 0.0, 0.0, 0.1, 0.3, 1.0],
        [0.0, 0.0, 0.0, 0.1, 0.3, 1.0],
        [0.0, 0.0, 0.0, 0.1, 0.2, 0.8],
        [0.0; 6],
        [0.0; 6],
    ];
    let symbols = names.iter().zip(pays).map(|(n, p)| SymbolDef { name: (*n).into(), pays: p.to_vec() }).collect();
    // Bandes : fréquences par rouleau (H1..L5, W, S), mélangées de façon déterministe.
    let counts: [[usize; 11]; 5] = [
        [2, 3, 3, 4, 6, 6, 7, 7, 8, 0, 2],
        [2, 3, 3, 4, 6, 6, 7, 7, 8, 2, 2],
        [2, 3, 3, 4, 6, 6, 7, 7, 8, 2, 2],
        [2, 3, 3, 4, 6, 6, 7, 7, 8, 2, 2],
        [2, 3, 3, 4, 6, 6, 7, 7, 8, 2, 2],
    ];
    let mut rng = Xoshiro256pp::seed_from_u64(5);
    let reels_base: Vec<Vec<usize>> = counts
        .iter()
        .map(|c| {
            let mut s: Vec<usize> = c.iter().enumerate().flat_map(|(sym, &k)| std::iter::repeat_n(sym, k)).collect();
            for i in (1..s.len()).rev() {
                let j = rng.below(i as u64 + 1) as usize;
                s.swap(i, j);
            }
            s
        })
        .collect();
    let max_k = 16;
    let mut scat_pays = vec![0.0; max_k];
    let mut fs = vec![0u32; max_k];
    let mut rt = vec![0u32; max_k];
    for k in 3..max_k {
        scat_pays[k] = match k {
            3 => 2.0,
            4 => 10.0,
            _ => 50.0,
        };
        fs[k] = match k {
            3 => 8,
            4 => 12,
            _ => 20,
        };
        rt[k] = 5;
    }
    ReelGame {
        name: "Démo 243 ways".into(),
        rows: vec![3; 5],
        symbols,
        wild: Some(9),
        scatter: Some(ScatterDef { symbol: 10, pays: scat_pays, free_spins: fs, retrigger: rt }),
        pay_mode: PayMode::Ways,
        reels_base,
        reels_free: None,
        free_spins_multiplier: 2.0,
        max_free_spins: 100,
        cap: 5_000.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sim::Control;
    use crate::stats::{compute, StatsOptions};

    /// Petit jeu énumérable : 3 rouleaux de 6 arrêts, 1 rangée, 1 ligne.
    fn tiny(mode: PayMode, rows: usize) -> ReelGame {
        ReelGame {
            name: "tiny".into(),
            rows: vec![rows; 3],
            symbols: vec![
                SymbolDef { name: "A".into(), pays: vec![0.0, 0.0, 1.0, 5.0] },
                SymbolDef { name: "B".into(), pays: vec![0.0, 0.0, 0.0, 2.0] },
                SymbolDef { name: "W".into(), pays: vec![0.0; 4] },
                SymbolDef { name: "S".into(), pays: vec![0.0; 4] },
            ],
            wild: Some(2),
            scatter: Some(ScatterDef { symbol: 3, pays: vec![0.0, 0.0, 1.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0, 3.0], free_spins: vec![0; 10], retrigger: vec![0; 10] }),
            pay_mode: mode,
            reels_base: vec![vec![0, 1, 2, 3, 0, 1], vec![0, 0, 1, 2, 3, 1], vec![1, 0, 2, 3, 0, 0]],
            reels_free: None,
            free_spins_multiplier: 1.0,
            max_free_spins: 10,
            cap: 1_000.0,
        }
    }

    #[test]
    fn closed_form_matches_enumeration_lines() {
        let g = tiny(PayMode::Lines { lines: vec![vec![0, 0, 0]] }, 1);
        let e = exact_base(&g, 1e7).unwrap();
        let en = e.enumerated.unwrap();
        assert!(((e.line_or_ways_rtp + e.scatter_rtp) - en.rtp).abs() < 1e-12, "{} vs {}", e.line_or_ways_rtp + e.scatter_rtp, en.rtp);
    }

    #[test]
    fn closed_form_matches_enumeration_ways() {
        let g = tiny(PayMode::Ways, 2);
        let e = exact_base(&g, 1e7).unwrap();
        let en = e.enumerated.unwrap();
        assert!(((e.line_or_ways_rtp + e.scatter_rtp) - en.rtp).abs() < 1e-12, "{} vs {}", e.line_or_ways_rtp + e.scatter_rtp, en.rtp);
    }

    #[test]
    fn simulation_matches_exact_base() {
        let g = tiny(PayMode::Ways, 2);
        let e = exact_base(&g, 1e7).unwrap();
        let r = simulate(&g, 1_000_000, 4, 4, None, &Control::default()).unwrap();
        let s = compute(&r.distribution, &StatsOptions::default());
        let z = (s.rtp - e.enumerated.as_ref().unwrap().rtp) / (s.sd / 1_000_000f64.sqrt());
        assert!(z.abs() < 4.5, "z = {z}");
    }

    #[test]
    fn demo_game_runs_and_is_reproducible() {
        let g = demo_game();
        g.validate().unwrap();
        let e = exact_base(&g, 0.0).unwrap();
        assert!(e.trigger_prob > 0.0 && e.trigger_prob < 0.05);
        let a = simulate(&g, 200_000, 9, 1, None, &Control::default()).unwrap();
        let b = simulate(&g, 200_000, 9, 4, None, &Control::default()).unwrap();
        assert_eq!(a.distribution, b.distribution);
        assert!((a.trigger_rate - e.trigger_prob).abs() < 5.0 * (e.trigger_prob / 200_000.0).sqrt());
    }
}
