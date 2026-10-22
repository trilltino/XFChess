import { useCallback, useEffect, useRef, useState } from "react";
import { apiClient, type TournamentDetail } from "../../../services/api";
import type { TournamentOperation } from "../../../services/adminFlows";

export interface MatchRecord {
  match_index: number;
  /** 0-based, for brackets and Swiss alike. */
  round: number;
  board?: number;
  player_white: string | null;
  player_black: string | null;
  winner: string | null;
  game_id: number | null;
  status: string;
  next_match_for_winner: number | null;
}

export interface BracketData {
  tournament_id: number;
  status: string;
  max_players: number;
  players: string[];
  matches: (MatchRecord | null)[];
  current_round: number;
}

export interface Reconciliation {
  on_chain_player_count: number;
  local_player_count: number;
  reconciliation_status: string;
  on_chain_prize_pool_lamports?: number;
}

import { startPolling } from "../../../services/poll";

const POLL_MS = 10_000;

/** Loads everything the detail screen shows and keeps it fresh while open. */
export function useTournamentData(tournamentId: number) {
  const [tournament, setTournament] = useState<TournamentDetail | null>(null);
  const [loading, setLoading] = useState(true);
  const [bracket, setBracket] = useState<BracketData | null>(null);
  const [escrowSol, setEscrowSol] = useState<number | null>(null);
  const [reconciliation, setReconciliation] = useState<Reconciliation | null>(null);
  const [auditEntries, setAuditEntries] = useState<any[]>([]);
  const [operations, setOperations] = useState<TournamentOperation[]>([]);
  const [transactions, setTransactions] = useState<any[]>([]);
  const [loadErrors, setLoadErrors] = useState<Record<string, string>>({});

  const report = (name: string, ok: boolean) =>
    setLoadErrors(p => ({ ...p, [name]: ok ? "" : `Could not load ${name}. It will retry automatically.` }));

  const selection = useRef({ id: tournamentId, generation: 0 });
  if (selection.current.id !== tournamentId) selection.current = { id: tournamentId, generation: selection.current.generation + 1 };
  const pending = useRef<{ generation: number; task: Promise<void> } | null>(null);

  const reload = useCallback((active: () => boolean = () => true): Promise<void> => {
    const generation = selection.current.generation;
    if (pending.current?.generation === generation) return pending.current.task;
    const current = () => active() && selection.current.generation === generation && selection.current.id === tournamentId;
    const task = (async () => {
      const [detail, matches, escrow, recon, audit, ops, txs] = await Promise.all([
        apiClient.getTournament(tournamentId), apiClient.getTournamentBracket(tournamentId),
        apiClient.getEscrowBalance(tournamentId), apiClient.getRegistrationReconciliation(tournamentId),
        apiClient.getTournamentAuditLog(tournamentId, 20), apiClient.getTournamentOperations(tournamentId),
        apiClient.getTournamentTransactions(tournamentId),
      ]);
      if (!current()) return;
      report("tournament", detail.ok); report("matches", matches.ok);
      if (detail.ok && detail.data) setTournament(detail.data);
      if (matches.ok && matches.data) setBracket(matches.data);
      if (escrow.ok) setEscrowSol(escrow.data?.balance_sol ?? null);
      if (recon.ok) setReconciliation(recon.data);
      report("audit", audit.ok); if (audit.ok) setAuditEntries(audit.data?.entries ?? []);
      report("operations", ops.ok); if (ops.ok) setOperations(ops.data?.operations ?? []);
      report("transactions", txs.ok); if (txs.ok) setTransactions(txs.data?.transactions ?? []);
      setLoading(false);
    })().finally(() => { if (pending.current?.generation === generation) pending.current = null; });
    pending.current = { generation, task };
    return task;
  }, [tournamentId]);

  useEffect(() => {
    setLoading(true); setTournament(null); setBracket(null);
    setEscrowSol(null); setReconciliation(null); setAuditEntries([]); setOperations([]); setTransactions([]); setLoadErrors({});
    const stop = startPolling(reload, POLL_MS);
    return () => { stop(); selection.current.generation++; };
  }, [reload]);

  return {
    tournament, loading, bracket, escrowSol, reconciliation,
    auditEntries, operations, transactions, loadErrors, reload,
  };
}
