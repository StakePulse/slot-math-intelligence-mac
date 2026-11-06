const nf = (d: number) => new Intl.NumberFormat("fr-FR", { minimumFractionDigits: d, maximumFractionDigits: d });

export const isNum = (v: any): v is number => typeof v === "number" && Number.isFinite(v);

export function num(v: any, d = 2): string {
  return isNum(v) ? nf(d).format(v) : "—";
}

export function int(v: any): string {
  return isNum(v) ? nf(0).format(Math.round(v)) : "—";
}

export function pct(v: any, d = 2): string {
  return isNum(v) ? `${nf(d).format(v * 100)} %` : "—";
}

export function x(v: any): string {
  if (!isNum(v)) return "—";
  if (v >= 100) return `${nf(0).format(v)}×`;
  if (v >= 10) return `${nf(1).format(v)}×`;
  return `${nf(2).format(v)}×`;
}

/** « 1 sur N » à partir de N. */
export function oneIn(n: any): string {
  if (!isNum(n) || n <= 0) return "jamais";
  return n < 100 ? `1 sur ${nf(2).format(n)}` : `1 sur ${nf(0).format(Math.round(n))}`;
}

/** « 1 sur N » à partir d'une probabilité. */
export function oneInP(p: any): string {
  return isNum(p) && p > 0 ? oneIn(1 / p) : "jamais";
}

export function sci(v: any, d = 2): string {
  if (!isNum(v)) return "—";
  if (v === 0) return "0";
  return v.toExponential(d).replace("e", "·10^").replace("+", "");
}

export function bytes(v: number): string {
  if (v > 1e6) return `${num(v / 1e6, 1)} Mo`;
  if (v > 1e3) return `${num(v / 1e3, 0)} ko`;
  return `${v} o`;
}

export function ms(v: any): string {
  if (!isNum(v)) return "—";
  return v > 60000 ? `${num(v / 60000, 1)} min` : v > 1000 ? `${num(v / 1000, 1)} s` : `${int(v)} ms`;
}
