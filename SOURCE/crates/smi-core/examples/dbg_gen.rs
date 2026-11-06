use smi_core::generator::*;
use smi_core::volatility::VolatilityConfig;
fn main() {
    for v in ["Medium", "High"] {
        let spec = GeneratorSpec { volatility: v.into(), max_win: 10_000.0, max_win_one_in: Some(2_000_000.0), ..Default::default() };
        let m = generate(&spec, &VolatilityConfig::default()).unwrap();
        println!("{v}: {:?}\n notes {:?}", m.params, m.notes);
    }
}
