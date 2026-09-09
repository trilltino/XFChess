export type OfflineFormat = "single_elimination" | "swiss";
export type OfflineStatus = "draft" | "published" | "active" | "completed" | "cancelled";

export interface OfflineParticipant {
  identity: string;
  name: string;
  rating?: number;
}

export interface OfflineMatch {
  index: number;
  round: number;
  board: number;
  white: string | null;
  black: string | null;
  result: "white" | "black" | "draw" | null;
}

export interface OfflineSwissStanding {
  identity: string;
  score: number;
  opponents: string[];
  rank: number;
}

export interface OfflineTournament {
  version: 1;
  id: string;
  name: string;
  format: OfflineFormat;
  status: OfflineStatus;
  participants: OfflineParticipant[];
  matches: OfflineMatch[];
  standings: OfflineSwissStanding[];
  currentRound: number;
  totalRounds: number;
  revision: number;
  updatedAt: number;
}

const now = () => Date.now();
const nextPowerOfTwo = (count: number) => 2 ** Math.ceil(Math.log2(Math.max(2, count)));

export function createOfflineTournament(input: {
  id: string;
  name: string;
  format: OfflineFormat;
  participants: OfflineParticipant[];
  totalRounds?: number;
}): OfflineTournament {
  if (input.participants.length < 2) throw new Error("At least two participants are required");
  if (new Set(input.participants.map(p => p.identity)).size !== input.participants.length) throw new Error("Participant identities must be unique");
  const matches = input.format === "single_elimination" ? createBracket(input.participants) : [];
  const totalRounds = input.totalRounds ?? Math.max(1, Math.ceil(Math.log2(input.participants.length)));
  return {
    version: 1,
    id: input.id,
    name: input.name,
    format: input.format,
    status: "draft",
    participants: input.participants,
    matches,
    standings: input.participants.map((p, index) => ({ identity: p.identity, score: 0, opponents: [], rank: index + 1 })),
    currentRound: 0,
    totalRounds,
    revision: 0,
    updatedAt: now(),
  };
}

function createBracket(participants: OfflineParticipant[]): OfflineMatch[] {
  const size = nextPowerOfTwo(participants.length);
  const seeded = [...participants].sort((a, b) => (b.rating ?? 0) - (a.rating ?? 0));
  const matches: OfflineMatch[] = [];
  let index = 0;
  let roundSize = size / 2;
  let round = 1;
  while (roundSize >= 1) {
    for (let board = 1; board <= roundSize; board++) {
      const match: OfflineMatch = { index: index++, round, board, white: null, black: null, result: null };
      if (round === 1) {
        match.white = seeded[(board - 1) * 2]?.identity ?? null;
        match.black = seeded[(board - 1) * 2 + 1]?.identity ?? null;
        if (!match.black) match.result = "white";
      }
      matches.push(match);
    }
    roundSize = Math.floor(roundSize / 2);
    round++;
  }
  return matches;
}

export function publishOfflineTournament(tournament: OfflineTournament): OfflineTournament {
  return update(tournament, { status: "published" });
}

export function startOfflineTournament(tournament: OfflineTournament): OfflineTournament {
  if (tournament.status !== "published" && tournament.status !== "draft") throw new Error("Tournament cannot be started");
  return update(tournament, { status: "active", currentRound: 1 });
}

export function recordOfflineMatch(tournament: OfflineTournament, matchIndex: number, result: OfflineMatch["result"]): OfflineTournament {
  if (tournament.status !== "active") throw new Error("Tournament is not active");
  if (!result) throw new Error("A result is required");
  const match = tournament.matches.find(m => m.index === matchIndex);
  if (!match || !match.white || !match.black || match.result) throw new Error("Match is not recordable");
  const matches = tournament.matches.map(m => m.index === matchIndex ? { ...m, result } : m);
  const winner = result === "white" ? match.white : result === "black" ? match.black : null;
  if (winner) advanceWinner(matches, match, winner);
  const final = matches.find(m => m.round === Math.max(...matches.map(x => x.round)));
  const completed = Boolean(final?.result && final.round === Math.max(...matches.map(x => x.round)));
  return update(tournament, { matches, status: completed ? "completed" : "active" });
}

function advanceWinner(matches: OfflineMatch[], source: OfflineMatch, winner: string) {
  const next = matches.find(m => m.round === source.round + 1 && m.board === Math.ceil(source.board / 2));
  if (!next) return;
  if (source.board % 2 === 1) next.white = winner; else next.black = winner;
}

export function recordOfflineSwissResult(tournament: OfflineTournament, white: string, black: string, result: "white" | "black" | "draw"): OfflineTournament {
  if (tournament.format !== "swiss" || tournament.status !== "active") throw new Error("Swiss tournament is not active");
  const standings = tournament.standings.map(s => ({ ...s, opponents: [...s.opponents] }));
  const whiteStanding = standings.find(s => s.identity === white);
  const blackStanding = standings.find(s => s.identity === black);
  if (!whiteStanding || !blackStanding || white === black || whiteStanding.opponents.includes(black)) throw new Error("Invalid Swiss pairing");
  whiteStanding.opponents.push(black); blackStanding.opponents.push(white);
  if (result === "white") whiteStanding.score += 1;
  else if (result === "black") blackStanding.score += 1;
  else { whiteStanding.score += 0.5; blackStanding.score += 0.5; }
  standings.sort((a, b) => b.score - a.score || a.identity.localeCompare(b.identity)).forEach((s, i) => { s.rank = i + 1; });
  return update(tournament, { standings });
}

function update(tournament: OfflineTournament, changes: Partial<OfflineTournament>): OfflineTournament {
  return { ...tournament, ...changes, revision: tournament.revision + 1, updatedAt: now() };
}
