// The three watched agents, once.
//
// The island and the Settings window both drew these, and they drifted: the
// pill colours and the Settings dots were two copies of the same table, which
// is how a pill could be one colour while its own status row claimed another.
// One table, imported by both.

export interface AgentMeta {
  /** What the user calls it. A pill is named after its agent, not its session. */
  name: string;
  /** Pill and status-dot colour. */
  color: string;
  /** How to start it, for the Settings row when it is not connected. */
  how: string;
}

export const AGENTS_META: Record<string, AgentMeta> = {
  // Grey on purpose: OpenCode is the one that is usually just there. A blue
  // pill read as "something is happening" when most of the time nothing is.
  opencode: { name: "OpenCode", color: "#8A8F98", how: "opencode serve --port 54321" },
  hermes: { name: "Hermes", color: "#FFD700", how: "hermes gateway run" },
  freebuff: { name: "Freebuff", color: "#34D399", how: "run freebuff and send a message" },
};

/** Serde ids are kebab-case: opencode | hermes | freebuff | claude-code. */
export function agentName(id: string): string {
  return AGENTS_META[id]?.name ?? id;
}