import React, { useEffect, useRef, useState } from "react";
import { call, follow, Job } from "../api";
import { ms, num } from "../fmt";

export function Page({ title, intro, actions, children }: { title: string; intro?: React.ReactNode; actions?: React.ReactNode; children: React.ReactNode }) {
  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{title}</h1>
          {intro && <p>{intro}</p>}
        </div>
        {actions && <div className="row">{actions}</div>}
      </div>
      <div className="stack">{children}</div>
    </div>
  );
}

export function Card({ title, sub, right, children, className }: { title?: React.ReactNode; sub?: React.ReactNode; right?: React.ReactNode; children: React.ReactNode; className?: string }) {
  return (
    <div className={`card ${className ?? ""}`}>
      {(title || right) && (
        <h2>
          <span>
            {title} {sub && <span className="sub">· {sub}</span>}
          </span>
          {right}
        </h2>
      )}
      {children}
    </div>
  );
}

export function Stat({ label, value, sub }: { label: string; value: React.ReactNode; sub?: React.ReactNode }) {
  return (
    <div className="stat">
      <div className="l">{label}</div>
      <div className="v">{value}</div>
      {sub && <div className="s">{sub}</div>}
    </div>
  );
}

const NATURE_CLASS: Record<string, string> = {
  EXACT: "b-exact",
  "EXACT FORMULA": "b-formula",
  EXACT_FORMULA: "b-formula",
  "MONTE CARLO": "b-mc",
  MONTE_CARLO: "b-mc",
  SPECIFICATION: "b-spec",
  CONFIGURATION: "b-spec",
  "IMPORTÉ": "b-imported",
  IMPORTED: "b-imported",
};

/** Étiquette de nature d'un chiffre (règle n°5 : théorie et simulation séparées). */
export function Nature({ n }: { n: string }) {
  const label = n.replace("_", " ");
  return <span className={`badge ${NATURE_CLASS[n] ?? "b-muted"}`}>{label === "IMPORTED" ? "IMPORTÉ" : label}</span>;
}

const STATUS_CLASS: Record<string, string> = {
  DRAFT: "b-muted",
  NEEDS_REVIEW: "b-warn",
  VALIDATION_FAILED: "b-bad",
  VALIDATED: "b-ok",
  READY_FOR_EXPORT: "b-ok",
  PASS: "b-ok",
  WARN: "b-warn",
  FAIL: "b-bad",
  NOT_RUN: "b-muted",
  OK: "b-ok",
  PARTIEL: "b-warn",
  "ÉCHEC": "b-bad",
  "EN COURS": "b-warn",
  "À VALIDER": "b-warn",
  "VALIDÉ": "b-ok",
  "REJETÉ": "b-bad",
};

const STATUS_LABEL: Record<string, string> = {
  NEEDS_REVIEW: "NEEDS REVIEW",
  VALIDATION_FAILED: "VALIDATION FAILED",
  READY_FOR_EXPORT: "READY FOR EXPORT",
  PASS: "OK",
  WARN: "AVERTISSEMENT",
  FAIL: "ÉCHEC",
  NOT_RUN: "NON EXÉCUTÉ",
};

export function Status({ s }: { s: string }) {
  if (!s) return null;
  return <span className={`badge ${STATUS_CLASS[s] ?? "b-muted"}`}>{STATUS_LABEL[s] ?? s}</span>;
}

export function Tabs({ tabs, on, set }: { tabs: [string, string][]; on: string; set: (k: string) => void }) {
  return (
    <div className="tabs">
      {tabs.map(([k, l]) => (
        <button key={k} className={on === k ? "on" : ""} onClick={() => set(k)}>
          {l}
        </button>
      ))}
    </div>
  );
}

export function Field({ label, children, style }: { label: string; children: React.ReactNode; style?: React.CSSProperties }) {
  return (
    <label className="f" style={style}>
      <span>{label}</span>
      {children}
    </label>
  );
}

export function NumInput({ value, onChange, step, min, max, style, placeholder }: { value: number | null | undefined; onChange: (v: number | null) => void; step?: number; min?: number; max?: number; style?: React.CSSProperties; placeholder?: string }) {
  const [txt, setTxt] = useState(value === null || value === undefined ? "" : String(value));
  useEffect(() => {
    const cur = txt === "" ? null : Number(txt.replace(",", "."));
    if (cur !== value) setTxt(value === null || value === undefined ? "" : String(value));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [value]);
  return (
    <input
      type="text"
      inputMode="decimal"
      value={txt}
      placeholder={placeholder}
      style={style}
      onChange={(e) => {
        setTxt(e.target.value);
        const t = e.target.value.replace(",", ".").replace(/\s/g, "");
        if (t === "") onChange(null);
        else if (Number.isFinite(Number(t))) {
          let v = Number(t);
          if (min !== undefined) v = Math.max(min, v);
          if (max !== undefined) v = Math.min(max, v);
          onChange(v);
        }
      }}
      data-step={step}
    />
  );
}

export function Chips({ options, value, onChange, multi = true }: { options: string[]; value: string[]; onChange: (v: string[]) => void; multi?: boolean }) {
  return (
    <div className="chips">
      {options.map((o) => {
        const on = value.includes(o);
        return (
          <button key={o} className={`chip ${on ? "on" : ""}`} onClick={() => onChange(multi ? (on ? value.filter((v) => v !== o) : [...value, o]) : on ? [] : [o])}>
            {o}
          </button>
        );
      })}
    </div>
  );
}

export function Note({ kind, children }: { kind?: "warn" | "bad" | "ok"; children: React.ReactNode }) {
  return <div className={`note ${kind ?? ""}`}>{children}</div>;
}

export function Progress({ value }: { value: number }) {
  return (
    <div className="progress">
      <div style={{ width: `${Math.round(value * 1000) / 10}%` }} />
    </div>
  );
}

export function ErrorBox({ error }: { error: any }) {
  if (!error) return null;
  return <Note kind="bad"><span className="err">{String(error)}</span></Note>;
}

/** Lance une tâche longue côté Rust et suit sa progression (pause, reprise, annulation). */
export function useJob() {
  const [job, setJob] = useState<Job | null>(null);
  const [error, setError] = useState<string | null>(null);
  const started = useRef<number>(0);
  async function start(cmd: string, args: Record<string, any>): Promise<Job | null> {
    setError(null);
    setJob(null);
    started.current = Date.now();
    try {
      const id = await call<number>(cmd, args);
      const end = await follow(id, setJob);
      if (end.status === "error") setError(end.error ?? "erreur");
      return end;
    } catch (e: any) {
      setError(String(e));
      return null;
    }
  }
  async function control(action: "pause" | "resume" | "cancel") {
    if (job) await call("job_control", { id: job.id, action });
  }
  return { job, error, start, control, elapsed: () => Date.now() - started.current };
}

export function JobBar({ j }: { j: ReturnType<typeof useJob> }) {
  const job = j.job;
  if (!job) return j.error ? <ErrorBox error={j.error} /> : null;
  const running = job.status === "running" || job.status === "paused";
  return (
    <div className="card">
      <div className="row between" style={{ marginBottom: 8 }}>
        <span>
          <b>{job.label}</b> <span className="muted">· {job.message}</span>
        </span>
        <span className="row">
          <span className="muted num">{num(job.progress * 100, 1)} % · {ms(j.elapsed())}</span>
          {running && job.status === "running" && <button className="btn small" onClick={() => j.control("pause")}>Pause</button>}
          {running && job.status === "paused" && <button className="btn small" onClick={() => j.control("resume")}>Reprendre</button>}
          {running && <button className="btn small danger" onClick={() => j.control("cancel")}>Annuler</button>}
          {!running && <Status s={job.status === "done" ? "OK" : job.status === "cancelled" ? "PARTIEL" : "ÉCHEC"} />}
        </span>
      </div>
      <Progress value={job.progress} />
      <ErrorBox error={j.error} />
    </div>
  );
}

export function Empty({ children }: { children: React.ReactNode }) {
  return <div className="muted" style={{ padding: "18px 0" }}>{children}</div>;
}
