import React from "react";
import {
  Bar, BarChart, CartesianGrid, ComposedChart, Legend, Line, LineChart, ReferenceLine, ResponsiveContainer, Scatter, ScatterChart, Tooltip, XAxis, YAxis, ZAxis,
} from "recharts";
import { num, oneInP, pct, x } from "../fmt";

const C = { a: "#4da3ff", b: "#e0a83a", c: "#3fbf7f", d: "#c9718f", e: "#8d7cf0", grid: "#222731" };
export const SERIES = [C.a, C.b, C.c, C.d, C.e, "#5fc4c4", "#d7d7d7"];

/** Bandes de gain : probabilité (échelle log) et contribution au RTP. */
export function BandsChart({ bands, compare, height = 280 }: { bands: any[]; compare?: any[]; height?: number }) {
  const data = (bands ?? [])
    .filter((b) => b.prob > 0 || compare?.find((c) => c.label === b.label)?.prob > 0)
    .map((b) => {
      const c = compare?.find((k) => k.label === b.label);
      return { label: b.label.replace(" (pas de gain)", ""), prob: b.prob || null, rtp: b.rtp * 100, sim: c?.prob || null };
    });
  return (
    <div className="grid g2">
      <div>
        <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Probabilité par round (échelle log){compare ? " — exact vs simulé" : ""}</div>
        <ResponsiveContainer width="100%" height={height}>
          <BarChart data={data} layout="vertical" margin={{ left: 40, right: 10 }}>
            <CartesianGrid strokeDasharray="2 4" horizontal={false} />
            <XAxis type="number" scale="log" domain={["auto", 1]} allowDataOverflow tickFormatter={(v) => oneInP(v).replace("1 sur ", "1/")} />
            <YAxis type="category" dataKey="label" width={110} interval={0} />
            <Tooltip formatter={(v: any, n: any) => [`${pct(v, 5)} (${oneInP(v)})`, n === "prob" ? "exact" : "simulé"]} />
            <Bar dataKey="prob" fill={C.a} name="prob" />
            {compare && <Bar dataKey="sim" fill={C.b} name="sim" />}
          </BarChart>
        </ResponsiveContainer>
      </div>
      <div>
        <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>Contribution au RTP (points)</div>
        <ResponsiveContainer width="100%" height={height}>
          <BarChart data={data} layout="vertical" margin={{ left: 40, right: 10 }}>
            <CartesianGrid strokeDasharray="2 4" horizontal={false} />
            <XAxis type="number" />
            <YAxis type="category" dataKey="label" width={110} interval={0} />
            <Tooltip formatter={(v: any) => [`${num(v, 3)} points`, "RTP"]} />
            <Bar dataKey="rtp" fill={C.c} />
          </BarChart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}

export function HistogramChart({ hist, label, fmt, ours, height = 200 }: { hist: [number, number, number][]; label: string; fmt: (v: number) => string; ours?: number | null; height?: number }) {
  const data = (hist ?? []).map(([lo, hi, n]) => ({ k: `${fmt(lo)} – ${fmt(hi)}`, n, mid: (lo + hi) / 2 }));
  const ourIdx = ours !== null && ours !== undefined ? (hist ?? []).findIndex(([lo, hi]) => ours >= lo && ours <= hi) : -1;
  return (
    <ResponsiveContainer width="100%" height={height}>
      <BarChart data={data} margin={{ left: 0, right: 10, bottom: 30 }}>
        <CartesianGrid strokeDasharray="2 4" vertical={false} />
        <XAxis dataKey="k" angle={-30} textAnchor="end" interval={0} height={50} />
        <YAxis allowDecimals={false} />
        <Tooltip formatter={(v: any) => [v, label]} />
        <Bar dataKey="n" fill={C.a} />
        {ourIdx >= 0 && <ReferenceLine x={data[ourIdx].k} stroke={C.b} strokeWidth={2} label={{ value: "notre jeu", fill: C.b, fontSize: 11, position: "top" }} />}
      </BarChart>
    </ResponsiveContainer>
  );
}

export function ConvergenceChart({ points, theory, se1 }: { points: [number, number][]; theory: number; se1: number }) {
  // Bande ±1,96 σ/√n autour du RTP exact (σ exact par unité de prix).
  const data = (points ?? []).map(([n, r]) => ({ n, r: r * 100, hi: (theory + (1.96 * se1) / Math.sqrt(n)) * 100, lo: (theory - (1.96 * se1) / Math.sqrt(n)) * 100 }));
  return (
    <ResponsiveContainer width="100%" height={240}>
      <LineChart data={data} margin={{ left: 10, right: 20 }}>
        <CartesianGrid strokeDasharray="2 4" />
        <XAxis dataKey="n" type="number" scale="log" domain={["auto", "auto"]} tickFormatter={(v) => (v >= 1e6 ? `${num(v / 1e6, 0)} M` : `${num(v / 1e3, 0)} k`)} />
        <YAxis domain={["auto", "auto"]} tickFormatter={(v) => `${num(v, 1)} %`} width={60} />
        <Tooltip formatter={(v: any, n: any) => [`${num(v, 4)} %`, n === "r" ? "RTP cumulé" : n === "hi" ? "IC 95 % haut" : "IC 95 % bas"]} labelFormatter={(v) => `${num(v, 0)} spins`} />
        <ReferenceLine y={theory * 100} stroke={C.c} strokeDasharray="4 4" />
        <Line dataKey="hi" stroke="#3a4250" dot={false} strokeDasharray="3 3" />
        <Line dataKey="lo" stroke="#3a4250" dot={false} strokeDasharray="3 3" />
        <Line dataKey="r" stroke={C.a} dot={false} strokeWidth={2} />
      </LineChart>
    </ResponsiveContainer>
  );
}

export function MarketScatter({ rows, xKey, yKey, xLabel, yLabel, logX, logY, ours }: { rows: any[]; xKey: string; yKey: string; xLabel: string; yLabel: string; logX?: boolean; logY?: boolean; ours?: { x: number; y: number } | null }) {
  const pts = rows.filter((r) => r[xKey] > 0 && r[yKey] > 0).map((r) => ({ x: r[xKey], y: r[yKey], name: `${r.name} (${r.provider})`, z: 40 }));
  return (
    <ResponsiveContainer width="100%" height={300}>
      <ScatterChart margin={{ left: 10, right: 20, bottom: 20 }}>
        <CartesianGrid strokeDasharray="2 4" />
        <XAxis type="number" dataKey="x" name={xLabel} scale={logX ? "log" : "auto"} domain={["auto", "auto"]} tickFormatter={(v) => num(v, v < 10 ? 2 : 0)} label={{ value: xLabel, position: "insideBottom", offset: -10, fill: "#8d95a3", fontSize: 11 }} />
        <YAxis type="number" dataKey="y" name={yLabel} scale={logY ? "log" : "auto"} domain={["auto", "auto"]} tickFormatter={(v) => num(v, v < 10 ? 2 : 0)} label={{ value: yLabel, angle: -90, position: "insideLeft", fill: "#8d95a3", fontSize: 11 }} />
        <ZAxis dataKey="z" range={[30, 30]} />
        <Tooltip cursor={{ strokeDasharray: "3 3" }} content={({ payload }) => (payload && payload[0] ? <div className="card" style={{ padding: 8 }}>{payload[0].payload.name}<br />{xLabel} : {num(payload[0].payload.x, 3)}<br />{yLabel} : {num(payload[0].payload.y, 3)}</div> : null)} />
        <Scatter data={pts} fill={C.a} fillOpacity={0.75} />
        {ours && <Scatter data={[{ ...ours, name: "Notre loi", z: 80 }]} fill={C.b} shape="diamond" />}
      </ScatterChart>
    </ResponsiveContainer>
  );
}

export function ProvidersChart({ providers, metric, label, fmt }: { providers: any[]; metric: string; label: string; fmt: (v: number) => string }) {
  const data = providers.filter((p) => p[metric] !== null && p[metric] !== undefined).map((p) => ({ studio: p.studio, v: p[metric], n: p.slots }));
  return (
    <ResponsiveContainer width="100%" height={Math.max(160, data.length * 22 + 30)}>
      <BarChart data={data} layout="vertical" margin={{ left: 30, right: 20 }}>
        <CartesianGrid strokeDasharray="2 4" horizontal={false} />
        <XAxis type="number" tickFormatter={fmt} />
        <YAxis type="category" dataKey="studio" width={130} interval={0} />
        <Tooltip formatter={(v: any, _n: any, p: any) => [`${fmt(v)} (${p.payload.n} jeu(x))`, label]} />
        <Bar dataKey="v" fill={C.a} />
      </BarChart>
    </ResponsiveContainer>
  );
}

export function CompareBars({ rows, metrics }: { rows: { label: string; [k: string]: any }[]; metrics: { key: string; label: string; fmt: (v: number) => string }[] }) {
  return (
    <div className="grid g3">
      {metrics.map((m) => (
        <div key={m.key}>
          <div className="muted" style={{ fontSize: 11, marginBottom: 4 }}>{m.label}</div>
          <ResponsiveContainer width="100%" height={140}>
            <ComposedChart data={rows.map((r) => ({ label: r.label, v: r[m.key] }))} margin={{ left: 0, right: 8 }}>
              <CartesianGrid strokeDasharray="2 4" vertical={false} />
              <XAxis dataKey="label" />
              <YAxis tickFormatter={m.fmt} width={60} />
              <Tooltip formatter={(v: any) => [m.fmt(v), m.label]} />
              <Bar dataKey="v" fill={C.a} />
            </ComposedChart>
          </ResponsiveContainer>
        </div>
      ))}
    </div>
  );
}

export function SurvivalChart({ points }: { points: [number, number][] }) {
  return (
    <ResponsiveContainer width="100%" height={200}>
      <LineChart data={points.map(([k, p]) => ({ k, p: p * 100 }))} margin={{ left: 10, right: 20 }}>
        <CartesianGrid strokeDasharray="2 4" />
        <XAxis dataKey="k" type="number" scale="log" domain={["auto", "auto"]} />
        <YAxis domain={[0, 100]} tickFormatter={(v) => `${v} %`} />
        <Tooltip formatter={(v: any) => [`${num(v, 1)} %`, "encore en jeu"]} labelFormatter={(v) => `${v} rounds`} />
        <Line dataKey="p" stroke={C.b} dot strokeWidth={2} />
      </LineChart>
    </ResponsiveContainer>
  );
}

export function Legend2() {
  return <Legend />;
}

export { x as fmtX };
