import React from "react";
import { int, num, oneIn, oneInP, pct, sci, x } from "../fmt";
import { BandsChart } from "./charts";
import { Card, Nature, Status } from "./ui";

/** Indicateurs clés d'une loi (EXACT, ou MONTE CARLO pour une loi empirique). */
export function KeyStats({ s, nature = "EXACT", vol }: { s: any; nature?: string; vol?: any }) {
  if (!s) return null;
  const rows: [string, React.ReactNode, React.ReactNode?][] = [
    ["RTP", pct(s.rtp, 4)],
    ["Gain moyen", x(s.mean), "mises de base"],
    ["Hit rate", pct(s.hit_rate, 3), oneIn(s.hit_one_in)],
    ["Spins perdants", pct(s.zero_rate, 2)],
    ["P(gain ≥ prix)", pct(s.p_ge_price, 3)],
    ["Médiane", x(s.median)],
    ["Écart-type / prix", num(s.sd_per_price, 3), `σ = ${num(s.sd, 2)} mises`],
    ["VI 90 % / 95 %", `${num(s.vi90, 2)} / ${num(s.vi95, 2)}`],
    ["Skewness", num(s.skewness, 2), `kurtosis en excès ${num(s.excess_kurtosis, 1)}`],
    ["Max win", x(s.max_win?.value), `${oneIn(s.max_win?.one_in)} · ${num((s.max_win?.rtp ?? 0) * 100, 3)} pts RTP`],
    ["Attente médiane du max", s.max_win?.median_wait ? `${int(s.max_win.median_wait)} rounds` : "—"],
    ["CVaR 0,1 %", x(s.cvar_001), `ETL40 ${num(s.etl_40, 3)}`],
    ["Issues distinctes", int(s.outcomes), `${int(s.effective_outcomes)} effectives`],
    ["Issue la plus lourde", oneInP(s.heaviest_prob)],
  ];
  if (s.bonus) {
    rows.push(["Bonus", oneIn(s.bonus.one_in), `moyenne ${x(s.bonus.mean)} · médiane ${x(s.bonus.median)}`]);
    rows.push(["Répartition du RTP", `${pct(1 - s.bonus.bonus_share, 1)} base / ${pct(s.bonus.bonus_share, 1)} bonus`, `${num(s.bonus.rtp_base * 100, 3)} + ${num(s.bonus.rtp_bonus * 100, 3)} pts`]);
  }
  if (vol) rows.push(["Classe de volatilité", vol.class, `score composite ${num(vol.score, 3)}`]);
  return (
    <Card title="Indicateurs clés" right={<Nature n={nature} />}>
      <div className="grid g4" style={{ gap: 8 }}>
        {rows.map(([l, v, sub]) => (
          <div key={l} className="stat" style={{ padding: "8px 10px" }}>
            <div className="l">{l}</div>
            <div className="v" style={{ fontSize: 16 }}>{v}</div>
            {sub && <div className="s">{sub}</div>}
          </div>
        ))}
      </div>
    </Card>
  );
}

export function ClassesTable({ s, nature = "EXACT" }: { s: any; nature?: string }) {
  if (!s?.classes) return null;
  return (
    <Card title="Classes de résultat" sub="partition des rounds ; statistiques conditionnelles" right={<Nature n={nature} />}>
      <div className="tablewrap">
        <table>
          <thead>
            <tr>
              <th>Classe</th><th className="n">Probabilité</th><th className="n">Fréquence</th><th className="n">Points RTP</th><th className="n">Part du RTP</th>
              <th className="n">Moyenne</th><th className="n">Médiane</th><th className="n">P10 – P90</th><th className="n">P(0)</th><th className="n">Max</th><th className="n">Issues</th>
            </tr>
          </thead>
          <tbody>
            {s.classes.map((k: any) => (
              <tr key={k.key}>
                <td>{k.label}</td>
                <td className="n">{pct(k.prob, 4)}</td>
                <td className="n">{oneIn(k.one_in)}</td>
                <td className="n">{num(k.rtp * 100, 3)}</td>
                <td className="n">{pct(k.share_of_rtp, 1)}</td>
                <td className="n">{x(k.mean)}</td>
                <td className="n">{x(k.median)}</td>
                <td className="n">{x(k.p10)} – {x(k.p90)}</td>
                <td className="n">{k.p_no_win === null ? "—" : pct(k.p_no_win, 1)}</td>
                <td className="n">{x(k.max)}</td>
                <td className="n">{int(k.outcomes)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
}

export function CategoriesTable({ s, nature = "EXACT" }: { s: any; nature?: string }) {
  if (!s?.categories) return null;
  return (
    <Card title="Petits, moyens, gros, très gros gains" sub="seuils en multiples du prix (Réglages)" right={<Nature n={nature} />}>
      <table>
        <thead>
          <tr><th>Catégorie</th><th className="n">Probabilité</th><th className="n">Fréquence</th><th className="n">Points RTP</th><th className="n">Gain moyen</th></tr>
        </thead>
        <tbody>
          {s.categories.map((c: any) => (
            <tr key={c.key}>
              <td>{c.label}</td><td className="n">{pct(c.prob, 4)}</td><td className="n">{oneIn(c.one_in)}</td><td className="n">{num(c.rtp * 100, 3)}</td><td className="n">{x(c.mean)}</td>
            </tr>
          ))}
        </tbody>
      </table>
    </Card>
  );
}

export function BandsCard({ s, nature = "EXACT", compare, title = "Distribution des gains par bande" }: { s: any; nature?: string; compare?: any; title?: string }) {
  if (!s?.bands) return null;
  const visible = s.bands.filter((b: any) => b.prob > 0);
  return (
    <Card title={title} sub="gain du round en mises de base" right={<Nature n={nature} />}>
      <BandsChart bands={s.bands} compare={compare?.bands} />
      <div className="tablewrap" style={{ marginTop: 10 }}>
        <table>
          <thead>
            <tr><th>Bande</th><th className="n">Probabilité</th><th className="n">Fréquence</th><th className="n">/ 10 000</th><th className="n">Points RTP</th><th className="n">Moyenne</th><th className="n">P(≥ début)</th><th className="n">Issues</th>{compare && <th className="n">Simulé</th>}</tr>
          </thead>
          <tbody>
            {visible.map((b: any) => {
              const c = compare?.bands?.find((k: any) => k.label === b.label);
              return (
                <tr key={b.label}>
                  <td>{b.label}</td><td className="n">{pct(b.prob, 5)}</td><td className="n">{oneIn(b.one_in)}</td><td className="n">{num(b.per_10000, 2)}</td>
                  <td className="n">{num(b.rtp * 100, 3)}</td><td className="n">{x(b.mean)}</td><td className="n">{pct(b.survival, 4)}</td><td className="n">{int(b.outcomes)}</td>
                  {compare && <td className="n">{c ? pct(c.prob, 5) : "—"}</td>}
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      {s.gaps?.length > 0 && <div className="note warn" style={{ marginTop: 8 }}>Bandes vides sous le maximum (« trous ») : {s.gaps.join(" · ")}</div>}
    </Card>
  );
}

export function ThresholdsTable({ s }: { s: any }) {
  if (!s?.thresholds) return null;
  return (
    <Card title="Seuils, fréquences et attentes" right={<Nature n="EXACT_FORMULA" />}>
      <div className="tablewrap">
        <table>
          <thead>
            <tr><th>Gain ≥</th><th className="n">Probabilité</th><th className="n">Fréquence</th><th className="n">Attente médiane</th><th className="n">≥ 1 fois / 100</th><th className="n">/ 1 000</th><th className="n">/ 10 000</th><th className="n">Dépense / événement</th></tr>
          </thead>
          <tbody>
            {s.thresholds.map((t: any) => (
              <tr key={t.mult_price}>
                <td>{x(t.mult_price)} le prix</td><td className="n">{pct(t.prob, 5)}</td><td className="n">{oneIn(t.one_in)}</td><td className="n">{t.median_wait ? int(t.median_wait) : "—"}</td>
                <td className="n">{pct(t.at_least_once_100, 1)}</td><td className="n">{pct(t.at_least_once_1000, 1)}</td><td className="n">{pct(t.at_least_once_10000, 1)}</td><td className="n">{x(t.spend_per_event)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
      <div className="faint" style={{ marginTop: 6 }}>Attente médiane = ⌈ln 0,5 / ln(1−p)⌉ · « au moins une fois en n » = 1 − (1−p)ⁿ · rounds indépendants.</div>
    </Card>
  );
}

export function PercentilesRow({ s }: { s: any }) {
  if (!s?.percentiles) return null;
  return (
    <Card title="Percentiles du gain d'un round" right={<Nature n="EXACT" />}>
      <table>
        <thead><tr>{s.percentiles.map((p: any) => <th key={p.q} className="n">P{num(p.q * 100, p.q > 0.99 ? 3 : 0)}</th>)}</tr></thead>
        <tbody>
          <tr>{s.percentiles.map((p: any) => <td key={p.q} className="n">{x(p.value)}</td>)}</tr>
          <tr className="faint">{s.percentiles.map((p: any) => <td key={p.q} className="n">{x(p.value_price)} prix</td>)}</tr>
        </tbody>
      </table>
      <div className="faint" style={{ marginTop: 6 }}>Pq = plus petit gain x tel que P(X ≤ x) ≥ q.</div>
    </Card>
  );
}

export function StakeTable({ s }: { s: any }) {
  if (!s?.stake_checks) return null;
  return (
    <Card title="Barèmes Stake Engine (2★ / 3★)" sub="non bloquants chez Stake : abaissent la mise maximale" right={<Nature n="EXACT" />}>
      <div className="tablewrap">
        <table>
          <thead><tr><th>Barème</th><th>Contrôle</th><th className="n">Valeur</th><th className="n">Limite</th><th>Statut</th></tr></thead>
          <tbody>
            {s.stake_checks.map((k: any, i: number) => (
              <tr key={i}>
                <td>{k.scale}</td><td>{k.item}</td><td className="n">{k.value < 0.01 && k.value > 0 ? sci(k.value) : num(k.value, 4)}</td><td className="n">{k.limit === null ? "—" : num(k.limit, 4)}</td>
                <td>{k.pass === null ? <span className="faint">seuil non renseigné</span> : <Status s={k.pass ? "PASS" : "FAIL"} />}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </Card>
  );
}

export function ClosureCard({ s }: { s: any }) {
  if (!s?.closure) return null;
  const c = s.closure;
  return (
    <Card title="Clôture des décompositions" sub="le RTP par 5 sommations indépendantes" right={<Nature n="EXACT" />}>
      <table>
        <tbody>
          <tr><td>Σ p·x (issues)</td><td className="n">{pct(c.by_outcomes, 7)}</td></tr>
          <tr><td>Σ classes</td><td className="n">{pct(c.by_classes, 7)}</td></tr>
          <tr><td>Σ bandes</td><td className="n">{pct(c.by_bands, 7)}</td></tr>
          <tr><td>Σ catégories</td><td className="n">{pct(c.by_categories, 7)}</td></tr>
          <tr><td>Base + bonus</td><td className="n">{pct(c.by_base_plus_bonus, 7)}</td></tr>
          <tr><td><b>Écart maximal</b></td><td className="n"><b>{sci(c.max_abs_diff)}</b></td></tr>
        </tbody>
      </table>
    </Card>
  );
}

export function FullReport({ s, nature = "EXACT", vol, base, bonus, compare }: { s: any; nature?: string; vol?: any; base?: any; bonus?: any; compare?: any }) {
  if (!s) return null;
  return (
    <div className="stack">
      <KeyStats s={s} nature={nature} vol={vol} />
      <ClassesTable s={s} nature={nature} />
      <div className="grid g2">
        <CategoriesTable s={s} nature={nature} />
        <ClosureCard s={s} />
      </div>
      <BandsCard s={s} nature={nature} compare={compare} />
      <PercentilesRow s={s} />
      {base && <BandsCard s={base} title="Distribution du jeu de base (rounds gagnants sans bonus)" />}
      {bonus && <BandsCard s={bonus} title="Distribution des bonus (rounds avec bonus)" />}
      <ThresholdsTable s={s} />
      <StakeTable s={s} />
    </div>
  );
}
