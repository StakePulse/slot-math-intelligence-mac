import { invoke } from "@tauri-apps/api/core";

/** Appel d'une commande Rust ; l'erreur renvoyée par le moteur est remontée telle quelle. */
export async function call<T = any>(cmd: string, args: Record<string, any> = {}): Promise<T> {
  return invoke<T>(cmd, args);
}

export type Job = {
  id: number;
  kind: string;
  label: string;
  status: "running" | "paused" | "done" | "error" | "cancelled";
  progress: number;
  message: string;
  result?: any;
  error?: string;
};

/** Suit une tâche longue jusqu'à sa fin. */
export async function follow(id: number, onTick: (j: Job) => void): Promise<Job> {
  for (;;) {
    const j = await call<Job>("job_status", { id });
    onTick(j);
    if (j.status === "done" || j.status === "error" || j.status === "cancelled") return j;
    await new Promise((r) => setTimeout(r, 300));
  }
}

export const SPIN_PRESETS = [100_000, 500_000, 1_000_000, 5_000_000, 10_000_000, 50_000_000, 100_000_000];
export const MAX_PRESETS = [100, 250, 500, 1000, 2000, 5000, 10000, 20000, 50000];
export const MECHANICS = [
  "Classic Reels", "Ways", "Paylines", "Megaways", "Cascading Reels", "Cluster Pays", "Expanding Wilds", "Sticky Wilds",
  "Multipliers", "Free Spins", "Bonus Games", "Scatter Features", "Buy Bonus", "Gamble Features", "Hold and Win", "Progressive Features",
];
export const VOL_CLASSES = ["Low", "Medium-Low", "Medium", "Medium-High", "High", "Very High"];
