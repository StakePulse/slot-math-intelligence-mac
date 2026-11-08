//! Contrôles de conception d'un jeu TERMINÉ, mesurés sur ses books publiés.
//!
//! Origine : retours d'un concepteur de maths transmis par l'utilisateur le 2026-11-08 (DECISIONS.md D-17).
//! Chaque contrôle se mesure EXACTEMENT sur les poids de la lookup table (aucune simulation) :
//!
//! 1. symboles affichés : part des symboles hauts parmi les bas + hauts du jeu de base ;
//! 2. connexions gagnantes du jeu de base : part portée par les hauts (« pas que des connexions de bas ») ;
//! 3. plancher d'un hidden / achat / bonus : médiane assez loin du plancher, peu de rounds collés au plancher ;
//! 4. ordre des scatters : plus de scatters au déclenchement = plus de gain (moyenne ET médiane), tous
//!    bonus confondus et dans chaque bonus ;
//! 5. books par tranche de gain : ni trou ni tranche mince, jeu de base et bonus séparés ;
//! 6. doublons : même book, même multiplicateur, books au contenu identique ;
//! 7. profil : hit rate comparé au marché (et à la cible du studio si elle est réglée) ;
//! 8. fréquence et hauteur du max win ;
//! 9. part du RTP venant des bonus, en base et en Ante.
//!
//! Les seuils sont dans [`DesignRules`] (Settings), avec des valeurs de départ À CONFIRMER : ni le
//! concepteur ni le marché ne les fixent tous (voir D-17).

use crate::stats::{group, BAND_EDGES};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct DesignRules {
    /// Part minimale des symboles HAUTS parmi les symboles bas + hauts affichés en jeu de base.
    pub high_board_min: f64,
    /// Part minimale des connexions gagnantes du jeu de base portées par des symboles hauts.
    pub high_wins_min: f64,
    /// Un plancher n'est retenu que s'il vaut au moins cette fraction de la médiane (sinon : pas de plancher).
    pub floor_detect: f64,
    /// Médiane minimale, en multiples du plancher (exemple du concepteur : plancher 1 000×, médiane 1 100× = trop près).
    pub floor_median_ratio_min: f64,
    /// Part maximale des rounds payés à moins de +10 % du plancher.
    pub floor_near_max: f64,
    /// Nombre minimal de books par tranche de gain (jeu de base et bonus séparés).
    pub min_books_per_band: u64,
    /// Nombre minimal de books d'un groupe (nombre de scatters, bonus) pour le juger.
    pub min_group_books: u64,
    /// Chance maximale de revoir un bonus identique (même book) en 10 bonus.
    pub repeat_in_10_max: f64,
    /// Chance maximale que deux bonus paient exactement le même multiplicateur.
    pub same_multi_max: f64,
    /// Part maximale du RTP venant des bonus dans le mode de base.
    pub bonus_share_base_max: f64,
    /// Part maximale du RTP venant des bonus dans un Ante (mode à coût > 1 qui garde des rounds sans bonus).
    pub bonus_share_ante_max: f64,
    /// Gain maximal (× prix) au-delà duquel le barème Stake 2★ n'est plus au vert.
    pub max_win_2star: f64,
    /// Cible de hit rate du studio pour le mode de base (facultative), comparée en plus du marché.
    pub studio_hit_target: Option<f64>,
    /// Symboles classés à la main. Vides : règle automatique (H*, P* = hauts ; L*, A K Q J 10 9 = bas ; le reste = spéciaux).
    pub high_symbols: Vec<String>,
    pub low_symbols: Vec<String>,
    /// Valeurs du champ `criteria` des rounds SANS bonus.
    pub no_bonus_criteria: Vec<String>,
    /// Événements de déclenchement qui portent les positions des scatters (en plus de tout type contenant « trigger »).
    pub trigger_events: Vec<String>,
    /// Événements qui listent les gains (`wins[].symbol`, `wins[].amount`).
    pub win_events: Vec<String>,
}

impl Default for DesignRules {
    fn default() -> Self {
        Self {
            high_board_min: 0.20,
            high_wins_min: 0.20,
            floor_detect: 0.05,
            floor_median_ratio_min: 1.5,
            floor_near_max: 0.15,
            min_books_per_band: 10,
            min_group_books: 30,
            repeat_in_10_max: 0.01,
            same_multi_max: 0.05,
            bonus_share_base_max: 0.50,
            bonus_share_ante_max: 0.60,
            max_win_2star: 50_000.0,
            studio_hit_target: None,
            high_symbols: vec![],
            low_symbols: vec![],
            no_bonus_criteria: ["0", "basegame", "base", "sans_bonus", "zero", "nowin", "no_bonus"].iter().map(|s| s.to_string()).collect(),
            trigger_events: ["freeSpinTrigger", "freespinTrigger", "bonusTrigger", "scatterTrigger", "dossiers"].iter().map(|s| s.to_string()).collect(),
            win_events: vec!["winInfo".into()],
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Low,
    High,
    Special,
}

impl Tier {
    pub fn idx(self) -> usize {
        match self {
            Tier::Low => 0,
            Tier::High => 1,
            Tier::Special => 2,
        }
    }
    pub fn label(self) -> &'static str {
        match self {
            Tier::Low => "bas",
            Tier::High => "haut",
            Tier::Special => "spécial",
        }
    }
}

/// Rang d'un symbole : listes de [`DesignRules`] d'abord, sinon règle automatique.
pub fn tier_of(sym: &str, rules: &DesignRules) -> Tier {
    if rules.high_symbols.iter().any(|s| s == sym) {
        return Tier::High;
    }
    if rules.low_symbols.iter().any(|s| s == sym) {
        return Tier::Low;
    }
    let u = sym.to_ascii_uppercase();
    let digits_after = |p: &str| u.strip_prefix(p).is_some_and(|r| !r.is_empty() && r.chars().all(|c| c.is_ascii_digit()));
    if digits_after("H") || digits_after("HP") || digits_after("P") || digits_after("HIGH") {
        Tier::High
    } else if digits_after("L") || digits_after("LP") || digits_after("LOW") || ["A", "K", "Q", "J", "10", "9", "T10"].contains(&u.as_str()) {
        Tier::Low
    } else {
        Tier::Special
    }
}

/// Un book publié, réduit à ce que les contrôles utilisent.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookRow {
    pub weight: u64,
    /// Paiement en centièmes de la mise de base (comme la lookup table).
    pub payout: u64,
    pub bonus: bool,
    /// Bonus déclenché (champ `bonus` de l'événement de déclenchement, sinon `criteria`).
    pub group: String,
    /// Nombre de scatters au déclenchement (premier déclenchement du jeu de base).
    pub scatters: Option<u32>,
    /// Empreinte du contenu (événements sans leurs index), pour repérer deux books identiques.
    pub content: Option<u64>,
}

/// Un mode lu depuis ses books. Les sommes sont pondérées par les poids ENTIERS de la table.
#[derive(Clone, Debug, Default)]
pub struct ModeInput {
    pub name: String,
    pub cost: f64,
    pub rows: Vec<BookRow>,
    /// Σ poids × nombre de symboles de chaque rang affichés sur les plateaux du jeu de base.
    pub board: [u128; 3],
    /// [0 = jeu de base, 1 = bonus][rang] → (Σ poids × connexions, Σ poids × montant).
    pub wins: [[(u128, u128); 3]; 2],
    pub has_boards: bool,
    pub has_win_events: bool,
    pub has_triggers: bool,
    /// (critère, nombre de books, classé bonus ?)
    pub criteria: Vec<(String, u64, bool)>,
    /// (symbole, rang, apparitions sur les plateaux du jeu de base)
    pub symbols: Vec<(String, Tier, u64)>,
}

/// Référence du marché pour un mode (pairs au max comparable).
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
pub struct MarketRef {
    pub n: usize,
    pub hit_median: Option<f64>,
    pub hit_p25: Option<f64>,
    pub hit_p75: Option<f64>,
    pub max_one_in_median: Option<f64>,
    pub basis: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum DStatus {
    Pass,
    Warn,
    Fail,
    NotRun,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DesignCheck {
    pub key: String,
    pub label: String,
    pub status: DStatus,
    pub observed: String,
    pub target: String,
    pub detail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ScatterRow {
    /// « tous les bonus » ou le nom du bonus.
    pub group: String,
    pub scatters: u32,
    pub books: u64,
    pub one_in: Option<f64>,
    /// Gain moyen et médian du round, en multiples de la mise de base.
    pub mean: f64,
    pub median: f64,
    /// Groupe trop petit pour être jugé.
    pub small: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct BandRow {
    pub class: String,
    pub label: String,
    pub books: u64,
    pub prob: f64,
    /// "", "mince" ou "trou".
    pub flag: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct FloorRow {
    pub group: String,
    pub books: u64,
    /// En multiples de la mise de base.
    pub floor: f64,
    pub median: f64,
    pub ratio: Option<f64>,
    pub near_floor: f64,
    /// Faux quand le minimum est trop bas pour être un plancher voulu.
    pub has_floor: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Collisions {
    pub class: String,
    pub same_book: f64,
    pub repeat_in_10: f64,
    pub same_multi: f64,
    pub distinct_multis: u64,
    pub identical_content: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct ModeDesign {
    pub name: String,
    pub cost: f64,
    /// "base", "ante" ou "achat".
    pub kind: String,
    pub books: u64,
    pub rtp: f64,
    pub hit: f64,
    pub dead: f64,
    pub under_cost: f64,
    pub ge_cost: f64,
    pub sd_per_price: f64,
    /// Gain maximal en multiples du prix du mode.
    pub max_price: f64,
    pub max_one_in: Option<f64>,
    pub bonus_one_in: Option<f64>,
    pub bonus_share: Option<f64>,
    /// Parts bas / haut / spécial des symboles affichés en jeu de base.
    pub board_share: Option<[f64; 3]>,
    /// [contexte][rang] → (part des connexions, part des montants) parmi tous les rangs.
    pub wins_share: [Option<[(f64, f64); 3]>; 2],
    pub scatters: Vec<ScatterRow>,
    pub floors: Vec<FloorRow>,
    pub bands: Vec<BandRow>,
    pub collisions: Option<Collisions>,
    pub criteria: Vec<(String, u64, bool)>,
    pub symbols: Vec<(String, Tier, u64)>,
    pub market: Option<MarketRef>,
    pub checks: Vec<DesignCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct DesignReport {
    pub game: String,
    pub source: String,
    pub generated_at: String,
    pub engine_version: String,
    pub rules: DesignRules,
    pub modes: Vec<ModeDesign>,
    pub passed: usize,
    pub warnings: usize,
    pub failed: usize,
    pub not_run: usize,
}

impl DesignReport {
    pub fn new(game: &str, source: &str, generated_at: &str, rules: &DesignRules, modes: Vec<ModeDesign>) -> Self {
        let all = || modes.iter().flat_map(|m| m.checks.iter());
        let count = |s: DStatus| all().filter(|c| c.status == s).count();
        Self {
            game: game.into(),
            source: source.into(),
            generated_at: generated_at.into(),
            engine_version: crate::ENGINE_VERSION.into(),
            rules: rules.clone(),
            passed: count(DStatus::Pass),
            warnings: count(DStatus::Warn),
            failed: count(DStatus::Fail),
            not_run: count(DStatus::NotRun),
            modes,
        }
    }
}

fn pct(v: f64, d: usize) -> String {
    format!("{} %", fr(v * 100.0, d))
}

fn fr(v: f64, d: usize) -> String {
    let s = format!("{v:.d$}");
    let (int, dec) = match s.split_once('.') {
        Some((a, b)) => (a.to_string(), Some(b.to_string())),
        None => (s.clone(), None),
    };
    let neg = int.starts_with('-');
    let digits = int.trim_start_matches('-');
    let grouped = if let Ok(n) = digits.parse::<u64>() { group(n) } else { digits.to_string() };
    let mut out = if neg { format!("-{grouped}") } else { grouped };
    if let Some(d) = dec {
        out.push(',');
        out.push_str(&d);
    }
    out
}

fn xm(v: f64) -> String {
    let d = if v >= 100.0 { 0 } else if v >= 10.0 { 1 } else { 2 };
    format!("{}×", fr(v, d))
}

fn one_in(p: f64) -> Option<f64> {
    (p > 0.0).then(|| 1.0 / p)
}

fn one_in_s(v: Option<f64>) -> String {
    v.map_or("—".into(), |n| format!("1 sur {}", group(n.round() as u64)))
}

/// Médiane pondérée (paiement en centièmes) d'une liste (paiement, poids).
fn weighted_median(v: &mut [(u64, u64)]) -> u64 {
    v.sort_unstable();
    let tot: u128 = v.iter().map(|&(_, w)| w as u128).sum();
    let mut acc = 0u128;
    for &(x, w) in v.iter() {
        acc += w as u128;
        if acc * 2 >= tot {
            return x;
        }
    }
    v.last().map_or(0, |&(x, _)| x)
}

fn band_index(payout: u64) -> usize {
    BAND_EDGES.windows(2).position(|w| payout >= w[0] && payout < w[1]).unwrap_or(BAND_EDGES.len() - 2)
}

fn band_label(i: usize) -> String {
    let lo = BAND_EDGES[i] as f64 / 100.0;
    let hi = BAND_EDGES[i + 1];
    if hi == u64::MAX {
        format!("≥ {}", xm(lo))
    } else {
        format!("[{} ; {}[", xm(lo), xm(hi as f64 / 100.0))
    }
}

struct Group {
    books: u64,
    w: u128,
    s: u128,
    pays: Vec<(u64, u64)>,
}

fn scatter_table(rows: &[BookRow], total_w: u128, by_group: bool, min_books: u64) -> Vec<ScatterRow> {
    let mut m: BTreeMap<(String, u32), Group> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.weight > 0) {
        let Some(n) = r.scatters else { continue };
        let key = (if by_group { r.group.clone() } else { "tous les bonus".to_string() }, n);
        let g = m.entry(key).or_insert(Group { books: 0, w: 0, s: 0, pays: vec![] });
        g.books += 1;
        g.w += r.weight as u128;
        g.s += r.weight as u128 * r.payout as u128;
        g.pays.push((r.payout, r.weight));
    }
    m.into_iter()
        .map(|((group, scatters), mut g)| ScatterRow {
            group,
            scatters,
            books: g.books,
            one_in: one_in(g.w as f64 / total_w as f64),
            mean: g.s as f64 / g.w as f64 / 100.0,
            median: weighted_median(&mut g.pays) as f64 / 100.0,
            small: g.books < min_books,
        })
        .collect()
}

/// Inversions d'ordre dans un groupe : (moyennes non croissantes, médianes non croissantes).
fn inversions(rows: &[&ScatterRow]) -> (Vec<String>, Vec<String>) {
    let ok: Vec<&&ScatterRow> = rows.iter().filter(|r| !r.small).collect();
    let (mut mean_bad, mut med_bad) = (vec![], vec![]);
    for w in ok.windows(2) {
        let (a, b) = (w[0], w[1]);
        if b.mean <= a.mean {
            mean_bad.push(format!("{} : {} scatters {} ≤ {} scatters {} (moyenne)", a.group, b.scatters, xm(b.mean), a.scatters, xm(a.mean)));
        }
        if b.median <= a.median {
            med_bad.push(format!("{} : {} scatters {} ≤ {} scatters {} (médiane)", a.group, b.scatters, xm(b.median), a.scatters, xm(a.median)));
        }
    }
    (mean_bad, med_bad)
}

pub fn analyze(input: &ModeInput, rules: &DesignRules, market: Option<&MarketRef>) -> ModeDesign {
    let rows = &input.rows;
    let cost = if input.cost > 0.0 { input.cost } else { 1.0 };
    let tw: u128 = rows.iter().map(|r| r.weight as u128).sum::<u128>().max(1);
    let twf = tw as f64;
    let has_plain = rows.iter().any(|r| !r.bonus && r.weight > 0);
    let kind = if !has_plain {
        "achat"
    } else if cost > 1.0 + 1e-9 {
        "ante"
    } else {
        "base"
    };
    let price_cents = cost * 100.0;
    // Profil de la loi.
    let (mut s, mut s2, mut hit_w, mut under_w, mut ge_w) = (0u128, 0f64, 0u128, 0u128, 0u128);
    let mut max_p = 0u64;
    for r in rows.iter().filter(|r| r.weight > 0) {
        let w = r.weight as u128;
        s += w * r.payout as u128;
        let x = r.payout as f64 / price_cents;
        s2 += r.weight as f64 * x * x;
        if r.payout > 0 {
            hit_w += w;
            if (r.payout as f64) < price_cents {
                under_w += w;
            } else {
                ge_w += w;
            }
        }
        max_p = max_p.max(r.payout);
    }
    let mean = s as f64 / twf / price_cents;
    let sd = (s2 / twf - mean * mean).max(0.0).sqrt();
    let max_w: u128 = rows.iter().filter(|r| r.payout == max_p && r.weight > 0).map(|r| r.weight as u128).sum();
    let bonus_w: u128 = rows.iter().filter(|r| r.bonus).map(|r| r.weight as u128).sum();
    let bonus_s: u128 = rows.iter().filter(|r| r.bonus).map(|r| r.weight as u128 * r.payout as u128).sum();
    let bonus_share = (has_plain && s > 0).then(|| bonus_s as f64 / s as f64);
    let mut checks: Vec<DesignCheck> = vec![];
    let mut add = |key: &str, label: &str, status: DStatus, observed: String, target: String, detail: String| {
        checks.push(DesignCheck { key: key.into(), label: label.into(), status, observed, target, detail });
    };
    use DStatus::*;

    // 1. Symboles affichés en jeu de base.
    let board_tot: u128 = input.board.iter().sum();
    let board_share = (input.has_boards && board_tot > 0).then(|| {
        let t = board_tot as f64;
        [input.board[0] as f64 / t, input.board[1] as f64 / t, input.board[2] as f64 / t]
    });
    if kind != "achat" {
        match board_share {
            Some(b) if b[0] + b[1] > 0.0 => {
                let high = b[1] / (b[0] + b[1]);
                add(
                    "symboles_plateau",
                    "Symboles affichés : assez de hauts face aux bas",
                    if high >= rules.high_board_min { Pass } else { Warn },
                    format!("hauts {} / bas {} (spéciaux {} à part)", pct(high, 1), pct(1.0 - high, 1), pct(b[2], 1)),
                    format!("hauts ≥ {}", pct(rules.high_board_min, 0)),
                    "Pondéré par la probabilité de chaque book : ce que le joueur voit réellement sur les plateaux du jeu de base.".into(),
                );
            }
            _ => add("symboles_plateau", "Symboles affichés : assez de hauts face aux bas", NotRun, "plateaux absents des books".into(), format!("hauts ≥ {}", pct(rules.high_board_min, 0)), "Aucun événement de révélation avec un plateau (`board`) en jeu de base.".into()),
        }
    }

    // 2. Connexions gagnantes.
    let share_of = |ctx: usize| -> Option<[(f64, f64); 3]> {
        let c: u128 = input.wins[ctx].iter().map(|v| v.0).sum();
        let a: u128 = input.wins[ctx].iter().map(|v| v.1).sum();
        (c > 0).then(|| {
            let mut out = [(0.0, 0.0); 3];
            for (i, v) in input.wins[ctx].iter().enumerate() {
                out[i] = (v.0 as f64 / c as f64, if a > 0 { v.1 as f64 / a as f64 } else { 0.0 });
            }
            out
        })
    };
    let wins_share = [share_of(0), share_of(1)];
    if kind != "achat" {
        match wins_share[0] {
            Some(ws) if ws[0].0 + ws[1].0 > 0.0 => {
                let high = ws[1].0 / (ws[0].0 + ws[1].0);
                let amount_high = ws[1].1 / (ws[0].1 + ws[1].1).max(f64::MIN_POSITIVE);
                let bonus = wins_share[1].map_or(String::new(), |b| format!(" En bonus : hauts {} des connexions, {} des gains.", pct(b[1].0, 1), pct(b[1].1, 1)));
                add(
                    "connexions_hauts",
                    "Connexions gagnantes du jeu de base : pas que des bas",
                    if high >= rules.high_wins_min { Pass } else { Warn },
                    format!("hauts {} des connexions, {} des gains", pct(high, 1), pct(amount_high, 1)),
                    format!("hauts ≥ {} des connexions", pct(rules.high_wins_min, 0)),
                    format!("Connexions = gains listés par les événements de gain, pondérés par la probabilité du book.{bonus}"),
                );
            }
            _ => add("connexions_hauts", "Connexions gagnantes du jeu de base : pas que des bas", NotRun, "aucun gain détaillé".into(), format!("hauts ≥ {} des connexions", pct(rules.high_wins_min, 0)), format!("Aucun événement {:?} avec `wins[].symbol` en jeu de base.", rules.win_events)),
        }
    }

    // 3. Planchers.
    let mut groups: BTreeMap<String, Vec<(u64, u64)>> = BTreeMap::new();
    for r in rows.iter().filter(|r| r.weight > 0 && r.bonus) {
        let g = if kind == "achat" { input.name.clone() } else { r.group.clone() };
        groups.entry(g).or_default().push((r.payout, r.weight));
    }
    let mut floors = vec![];
    for (g, mut v) in groups {
        if (v.len() as u64) < rules.min_group_books {
            continue;
        }
        let floor = v.iter().map(|&(x, _)| x).min().unwrap_or(0);
        let med = weighted_median(&mut v);
        let tot: u128 = v.iter().map(|&(_, w)| w as u128).sum();
        let near: u128 = v.iter().filter(|&&(x, _)| (x as f64) <= floor as f64 * 1.1).map(|&(_, w)| w as u128).sum();
        let has_floor = floor > 0 && floor as f64 >= rules.floor_detect * med as f64;
        floors.push(FloorRow {
            group: g,
            books: v.len() as u64,
            floor: floor as f64 / 100.0,
            median: med as f64 / 100.0,
            ratio: (floor > 0).then(|| med as f64 / floor as f64),
            near_floor: if has_floor { near as f64 / tot as f64 } else { 0.0 },
            has_floor,
        });
    }
    let with_floor: Vec<&FloorRow> = floors.iter().filter(|f| f.has_floor).collect();
    if with_floor.is_empty() {
        add("plancher", "Plancher d'un bonus / achat : médiane assez loin", NotRun, if floors.is_empty() { "aucun bonus à juger".into() } else { "aucun plancher voulu".into() }, format!("médiane ≥ {} × plancher ; ≤ {} des rounds à +10 % du plancher", fr(rules.floor_median_ratio_min, 1), pct(rules.floor_near_max, 0)), format!("Plancher retenu s'il vaut au moins {} de la médiane (hidden, achat à gain minimal garanti).", pct(rules.floor_detect, 0)));
    } else {
        let bad: Vec<String> = with_floor
            .iter()
            .filter(|f| f.ratio.unwrap_or(0.0) < rules.floor_median_ratio_min || f.near_floor > rules.floor_near_max)
            .map(|f| format!("{} : plancher {}, médiane {} ({} × le plancher), {} des rounds à moins de +10 %", f.group, xm(f.floor), xm(f.median), fr(f.ratio.unwrap_or(0.0), 2), pct(f.near_floor, 1)))
            .collect();
        let shown = with_floor.iter().map(|f| format!("{} : {} → {} ({} à +10 %)", f.group, xm(f.floor), xm(f.median), pct(f.near_floor, 1))).collect::<Vec<_>>().join(" ; ");
        add(
            "plancher",
            "Plancher d'un bonus / achat : médiane assez loin",
            if bad.is_empty() { Pass } else { Warn },
            if bad.is_empty() { shown } else { bad.join(" ; ") },
            format!("médiane ≥ {} × plancher ; ≤ {} des rounds à +10 % du plancher", fr(rules.floor_median_ratio_min, 1), pct(rules.floor_near_max, 0)),
            "Exemple du concepteur : plancher 1 000×, médiane 1 100× → la moitié des rounds tombe entre 1 000× et 1 100×.".into(),
        );
    }

    // 4. Ordre des scatters.
    let mut scatters = vec![];
    if kind != "achat" {
        let all = scatter_table(rows, tw, false, rules.min_group_books);
        let per = scatter_table(rows, tw, true, rules.min_group_books);
        if all.is_empty() {
            add("ordre_scatters", "Plus de scatters = plus de gain (6 > 5 > 4 > 3)", NotRun, "aucun déclenchement lu".into(), "moyenne et médiane croissantes".into(), format!("Aucun événement de déclenchement avec des positions ({:?} ou type contenant « trigger »).", rules.trigger_events));
        } else {
            let (mut mean_bad, mut med_bad) = inversions(&all.iter().collect::<Vec<_>>());
            let mut names: Vec<String> = per.iter().map(|r| r.group.clone()).collect();
            names.dedup();
            for g in &names {
                let rs: Vec<&ScatterRow> = per.iter().filter(|r| &r.group == g).collect();
                let (a, b) = inversions(&rs);
                mean_bad.extend(a);
                med_bad.extend(b);
            }
            let judged = all.iter().filter(|r| !r.small).count() + per.iter().filter(|r| !r.small).count();
            let status = if !mean_bad.is_empty() {
                Fail
            } else if !med_bad.is_empty() {
                Warn
            } else if judged == 0 {
                NotRun
            } else {
                Pass
            };
            let observed = if judged == 0 {
                format!("groupes de moins de {} books : pas assez de rounds pour juger l'ordre", rules.min_group_books)
            } else if mean_bad.is_empty() && med_bad.is_empty() {
                all.iter().filter(|r| !r.small).map(|r| format!("{} scatters {}", r.scatters, xm(r.mean))).collect::<Vec<_>>().join(" < ")
            } else {
                mean_bad.iter().chain(med_bad.iter()).cloned().collect::<Vec<_>>().join(" ; ")
            };
            add(
                "ordre_scatters",
                "Plus de scatters = plus de gain (6 > 5 > 4 > 3)",
                status,
                observed,
                "moyenne ET médiane strictement croissantes, tous bonus confondus et dans chaque bonus".into(),
                format!("Groupes de moins de {} books non jugés (affichés à part). Moyenne inversée = échec ; médiane inversée = à revoir.", rules.min_group_books),
            );
            scatters.extend(all);
            scatters.extend(per);
        }
    }

    // 5. Books par tranche.
    let mut bands = vec![];
    let mut holes = vec![];
    let mut thin = vec![];
    for (class, sel) in [("jeu de base", false), ("bonus", true)] {
        let cls: Vec<&BookRow> = rows.iter().filter(|r| r.weight > 0 && r.payout > 0 && (r.bonus == sel || kind == "achat")).collect();
        if cls.is_empty() || (kind == "achat" && sel == false) {
            continue;
        }
        let class = if kind == "achat" { "achat" } else { class };
        let lo = band_index(cls.iter().map(|r| r.payout).min().unwrap_or(1));
        let hi = band_index(cls.iter().map(|r| r.payout).max().unwrap_or(1));
        let mut cnt = vec![(0u64, 0u128); BAND_EDGES.len() - 1];
        for r in &cls {
            let i = band_index(r.payout);
            cnt[i].0 += 1;
            cnt[i].1 += r.weight as u128;
        }
        for (i, &(n, w)) in cnt.iter().enumerate().take(hi + 1).skip(lo) {
            let flag = if n == 0 { "trou" } else if n < rules.min_books_per_band { "mince" } else { "" };
            let label = band_label(i);
            if flag == "trou" {
                holes.push(format!("{class} {label}"));
            } else if flag == "mince" {
                thin.push(format!("{class} {label} : {n} book(s)"));
            }
            bands.push(BandRow { class: class.into(), label, books: n, prob: w as f64 / twf, flag: flag.into() });
        }
    }
    add(
        "books_par_tranche",
        "Books par tranche : ni trou ni tranche mince",
        if !holes.is_empty() {
            Fail
        } else if !thin.is_empty() {
            Warn
        } else {
            Pass
        },
        if holes.is_empty() && thin.is_empty() { "toutes les tranches ont assez de books".into() } else { holes.iter().map(|h| format!("trou : {h}")).chain(thin.iter().cloned()).collect::<Vec<_>>().join(" ; ") },
        format!("≥ {} books par tranche, du plus petit gain au max", rules.min_books_per_band),
        "Tranches en multiples de la mise de base. Même rare, une tranche à 3 books rejoue toujours les 3 mêmes rounds.".into(),
    );

    // 6. Doublons.
    let pool: Vec<&BookRow> = rows.iter().filter(|r| r.weight > 0 && (r.bonus || kind == "achat")).collect();
    let collisions = (!pool.is_empty()).then(|| {
        let w: u128 = pool.iter().map(|r| r.weight as u128).sum();
        let wf = w as f64;
        let same_book: f64 = pool.iter().map(|r| (r.weight as f64 / wf).powi(2)).sum();
        let mut by_x: BTreeMap<u64, u128> = BTreeMap::new();
        let mut by_c: BTreeMap<u64, u64> = BTreeMap::new();
        for r in &pool {
            *by_x.entry(r.payout).or_default() += r.weight as u128;
            if let Some(c) = r.content {
                *by_c.entry(c).or_default() += 1;
            }
        }
        let same_multi: f64 = by_x.values().map(|&v| (v as f64 / wf).powi(2)).sum();
        Collisions {
            class: if kind == "achat" { "achat".into() } else { "bonus".into() },
            same_book,
            repeat_in_10: 1.0 - (-45.0 * same_book).exp(),
            same_multi,
            distinct_multis: by_x.len() as u64,
            identical_content: by_c.values().filter(|&&n| n > 1).map(|&n| n - 1).sum(),
        }
    });
    match &collisions {
        Some(c) => {
            let bad = c.repeat_in_10 > rules.repeat_in_10_max || c.same_multi > rules.same_multi_max || c.identical_content > 0;
            add(
                "doublons",
                "Doublons : même book, même multiplicateur",
                if bad { Warn } else { Pass },
                format!("revoir un {} identique en 10 : {} ; deux {} au même multi : {} ; {} multis distincts ; {} book(s) au contenu identique", c.class, pct(c.repeat_in_10, 2), if c.class == "achat" { "achats" } else { "bonus" }, pct(c.same_multi, 2), group(c.distinct_multis), c.identical_content),
                format!("≤ {} en 10 ; même multi ≤ {} ; aucun book identique", pct(rules.repeat_in_10_max, 0), pct(rules.same_multi_max, 0)),
                "Revoir en 10 = 1 − exp(−45 × Σ p²), p = probabilité de chaque book parmi les bonus.".into(),
            );
        }
        None => add("doublons", "Doublons : même book, même multiplicateur", NotRun, "aucun bonus".into(), "—".into(), String::new()),
    }

    // 7. Profil (hit rate) comparé au marché.
    let hit = hit_w as f64 / twf;
    if kind == "base" {
        let mut notes = vec![];
        let mut status = NotRun;
        if let Some(m) = market.filter(|m| m.n > 0 && m.hit_p25.is_some()) {
            status = if hit >= m.hit_p25.unwrap_or(0.0) { Pass } else { Warn };
            notes.push(format!("marché ({} pairs au max comparable, {}) : médiane {}, P25 {}, P75 {}", m.n, m.basis, m.hit_median.map_or("—".into(), |v| pct(v, 1)), pct(m.hit_p25.unwrap_or(0.0), 1), m.hit_p75.map_or("—".into(), |v| pct(v, 1))));
        }
        if let Some(t) = rules.studio_hit_target {
            if hit < t {
                status = Warn;
            } else if status == NotRun {
                status = Pass;
            }
            notes.push(format!("cible du studio {}", pct(t, 1)));
        }
        add(
            "profil",
            "Hit rate et profil des spins",
            status,
            format!("{} de gains (mort {} / sous la mise {} / ≥ mise {}) ; σ/prix {}", pct(hit, 2), pct(1.0 - hit, 1), pct(under_w as f64 / twf, 1), pct(ge_w as f64 / twf, 1), fr(sd, 2)),
            if notes.is_empty() { "≥ P25 du marché (aucune référence chargée)".into() } else { notes.join(" ; ") },
            "Repère du concepteur (capture) : « studio aim ≈ 29 % de gains pour un jeu ×12 500 ». Réglez `studio_hit_target` pour l'ajouter au contrôle.".into(),
        );
    }

    // 8. Max win.
    let max_price = max_p as f64 / price_cents;
    let max_one_in = one_in(max_w as f64 / twf);
    let mut mx_notes = vec![];
    if let Some(m) = market.and_then(|m| m.max_one_in_median) {
        mx_notes.push(format!("marché : max à {} (médiane des pairs)", one_in_s(Some(m))));
    }
    add(
        "max_win",
        "Max win : hauteur et fréquence",
        if max_price > rules.max_win_2star { Warn } else { Pass },
        format!("{} du prix, {}", xm(max_price), one_in_s(max_one_in)),
        format!("≤ {} du prix (barème Stake 2★){}", xm(rules.max_win_2star), if mx_notes.is_empty() { String::new() } else { format!(" ; {}", mx_notes.join(" ; ")) }),
        "Au-delà de 50 000× le prix, le barème 2★ de Stake n'est plus au vert (mise maximale abaissée).".into(),
    );

    // 9. Part du RTP des bonus.
    if let Some(sh) = bonus_share {
        let max = if kind == "ante" { rules.bonus_share_ante_max } else { rules.bonus_share_base_max };
        add(
            "part_bonus",
            if kind == "ante" { "Part du RTP venant des bonus (Ante)" } else { "Part du RTP venant des bonus (base)" },
            if sh <= max { Pass } else { Warn },
            format!("{} du RTP ; bonus {}", pct(sh, 1), one_in_s(one_in(bonus_w as f64 / twf))),
            format!("≤ {}", pct(max, 0)),
            "Un Ante qui double la fréquence des bonus à loi égale fait monter mécaniquement cette part.".into(),
        );
    }

    ModeDesign {
        name: input.name.clone(),
        cost,
        kind: kind.into(),
        books: rows.len() as u64,
        rtp: mean,
        hit,
        dead: 1.0 - hit,
        under_cost: under_w as f64 / twf,
        ge_cost: ge_w as f64 / twf,
        sd_per_price: sd,
        max_price,
        max_one_in,
        bonus_one_in: (has_plain && bonus_w > 0).then(|| twf / bonus_w as f64),
        bonus_share,
        board_share,
        wins_share,
        scatters,
        floors,
        bands,
        collisions,
        criteria: input.criteria.clone(),
        symbols: input.symbols.clone(),
        market: market.cloned(),
        checks,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(weight: u64, payout_x: f64, bonus: bool, group: &str, scatters: Option<u32>) -> BookRow {
        BookRow { weight, payout: (payout_x * 100.0).round() as u64, bonus, group: group.into(), scatters, content: None }
    }

    fn check<'a>(m: &'a ModeDesign, key: &str) -> &'a DesignCheck {
        m.checks.iter().find(|c| c.key == key).unwrap_or_else(|| panic!("contrôle {key} absent"))
    }

    #[test]
    fn symbol_tiers() {
        let r = DesignRules::default();
        for s in ["H1", "H5", "HP2", "P3"] {
            assert_eq!(tier_of(s, &r), Tier::High, "{s}");
        }
        for s in ["A", "K", "10", "L4", "LP1", "9"] {
            assert_eq!(tier_of(s, &r), Tier::Low, "{s}");
        }
        for s in ["W", "S", "M", "T", "BN", "H"] {
            assert_eq!(tier_of(s, &r), Tier::Special, "{s}");
        }
        let r2 = DesignRules { high_symbols: vec!["M".into()], low_symbols: vec!["H1".into()], ..Default::default() };
        assert_eq!(tier_of("M", &r2), Tier::High);
        assert_eq!(tier_of("H1", &r2), Tier::Low);
    }

    /// Mode de base fabriqué : bonus à 3/4/5 scatters dont la moyenne BAISSE de 4 à 5 dans « b ».
    fn base_with_inversion() -> ModeInput {
        let mut rows = vec![];
        for i in 0..600 {
            rows.push(row(1000, if i % 4 == 0 { 0.5 } else { 0.0 }, false, "sans_bonus", None));
        }
        for i in 0..40 {
            rows.push(row(2, 20.0 + i as f64, true, "a", Some(3)));
            rows.push(row(1, 60.0 + i as f64, true, "a", Some(4)));
            rows.push(row(2, 100.0 + i as f64, true, "b", Some(4)));
            rows.push(row(1, 80.0 + i as f64, true, "b", Some(5)));
        }
        ModeInput { name: "base".into(), cost: 1.0, rows, ..Default::default() }
    }

    #[test]
    fn scatter_inversion_inside_a_bonus_fails() {
        let m = analyze(&base_with_inversion(), &DesignRules::default(), None);
        let c = check(&m, "ordre_scatters");
        assert_eq!(c.status, DStatus::Fail, "{c:?}");
        assert!(c.observed.contains("b : 5 scatters"), "{}", c.observed);
        assert!(!c.observed.contains("a : "), "le bonus a est bien ordonné : {}", c.observed);
    }

    #[test]
    fn scatter_order_ok() {
        let mut inp = base_with_inversion();
        for r in inp.rows.iter_mut().filter(|r| r.group == "b" && r.scatters == Some(5)) {
            r.payout += 10_000;
        }
        let m = analyze(&inp, &DesignRules::default(), None);
        assert_eq!(check(&m, "ordre_scatters").status, DStatus::Pass, "{:?}", check(&m, "ordre_scatters"));
    }

    #[test]
    fn floor_too_close_to_median() {
        // Exemple du concepteur : plancher 1 000×, médiane 1 100×.
        let rows: Vec<BookRow> = (0..100).map(|i| row(1, 1000.0 + if i < 55 { i as f64 } else { 200.0 + 10.0 * i as f64 }, true, "hidden", None)).collect();
        let m = analyze(&ModeInput { name: "hidden".into(), cost: 1000.0, rows, ..Default::default() }, &DesignRules::default(), None);
        assert_eq!(m.kind, "achat");
        let c = check(&m, "plancher");
        assert_eq!(c.status, DStatus::Warn, "{c:?}");
        let f = &m.floors[0];
        assert!(f.has_floor && f.near_floor > 0.5 && f.ratio.unwrap() < 1.5, "{f:?}");
    }

    #[test]
    fn no_floor_when_minimum_is_tiny() {
        let rows: Vec<BookRow> = (0..100).map(|i| row(1, if i == 0 { 0.1 } else { 50.0 + i as f64 }, true, "x", None)).collect();
        let m = analyze(&ModeInput { name: "achat".into(), cost: 100.0, rows, ..Default::default() }, &DesignRules::default(), None);
        assert_eq!(check(&m, "plancher").status, DStatus::NotRun);
    }

    #[test]
    fn thin_band_and_hole() {
        let mut rows = vec![];
        for i in 0..50 {
            rows.push(row(10, 1.0 + i as f64 / 100.0, false, "0", None)); // [1 ; 2[ : 50 books
        }
        for i in 0..3 {
            rows.push(row(1, 12.0 + i as f64, false, "0", None)); // [10 ; 20[ : 3 books (mince)
        }
        rows.push(row(1, 700.0, false, "0", None)); // trous entre 20× et 500×
        rows.push(row(500, 0.0, false, "0", None));
        let m = analyze(&ModeInput { name: "base".into(), cost: 1.0, rows, ..Default::default() }, &DesignRules::default(), None);
        let c = check(&m, "books_par_tranche");
        assert_eq!(c.status, DStatus::Fail, "{c:?}");
        assert!(m.bands.iter().any(|b| b.flag == "mince" && b.books == 3));
        assert!(m.bands.iter().any(|b| b.flag == "trou"));
    }

    #[test]
    fn duplicates_and_bonus_share() {
        let mut rows = vec![row(900, 0.0, false, "0", None), row(80, 2.0, false, "0", None)];
        rows.push(BookRow { content: Some(7), ..row(10, 50.0, true, "fs", Some(3)) });
        rows.push(BookRow { content: Some(7), ..row(10, 50.0, true, "fs", Some(3)) });
        let m = analyze(&ModeInput { name: "base".into(), cost: 1.0, rows, ..Default::default() }, &DesignRules::default(), None);
        let c = m.collisions.as_ref().unwrap();
        assert!((c.same_book - 0.5).abs() < 1e-12 && (c.same_multi - 1.0).abs() < 1e-12 && c.identical_content == 1, "{c:?}");
        assert_eq!(check(&m, "doublons").status, DStatus::Warn);
        // Part du bonus : 2×10×50 / (80×2 + 2×10×50) = 1000 / 1160.
        assert!((m.bonus_share.unwrap() - 1000.0 / 1160.0).abs() < 1e-12);
        assert_eq!(check(&m, "part_bonus").status, DStatus::Warn);
    }

    #[test]
    fn wins_and_board_shares() {
        let mut inp = ModeInput { name: "base".into(), cost: 1.0, rows: vec![row(1, 0.0, false, "0", None), row(1, 1.0, false, "0", None)], has_boards: true, has_win_events: true, ..Default::default() };
        inp.board = [90, 10, 5];
        inp.wins[0] = [(95, 950), (5, 50), (0, 0)];
        let m = analyze(&inp, &DesignRules::default(), None);
        assert_eq!(check(&m, "symboles_plateau").status, DStatus::Warn);
        let c = check(&m, "connexions_hauts");
        assert_eq!(c.status, DStatus::Warn);
        assert!(c.observed.starts_with("hauts 5,0 %"), "{}", c.observed);
    }

    #[test]
    fn hit_against_market() {
        let inp = ModeInput { name: "base".into(), cost: 1.0, rows: vec![row(83, 0.0, false, "0", None), row(17, 2.0, false, "0", None)], ..Default::default() };
        let mk = MarketRef { n: 12, hit_median: Some(0.195), hit_p25: Some(0.18), hit_p75: Some(0.24), max_one_in_median: None, basis: "turnover".into() };
        let m = analyze(&inp, &DesignRules::default(), Some(&mk));
        assert_eq!(check(&m, "profil").status, DStatus::Warn);
        let m2 = analyze(&inp, &DesignRules { studio_hit_target: Some(0.15), ..Default::default() }, None);
        assert_eq!(check(&m2, "profil").status, DStatus::Pass);
    }
}
