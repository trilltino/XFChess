import { useCallback, useEffect, useState } from "react";
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

  const loadTournament = useCallback(async () => {
    const r = await apiClient.getTournament(tournamentId);
    report("tournament", r.ok);
    if (r.ok && r.data) setTournament(r.data);
    setLoading(false);
  }, [tournamentId]);

  const loadBracket = useCallback(async () => {
    const r = await apiClient.getTournamentBracket(tournamentId);
    report("matches", r.ok);
    if (r.ok && r.data) setBracket(r.data);
  }, [tournamentId]);

  const loadSideData = useCallback(async () => {
    const [escrow, recon, audit, ops, txs] = await Promise.all([
      apiClient.getEscrowBalance(tournamentId),
      apiClient.getRegistrationReconciliation(tournamentId),
      apiClient.getTournamentAuditLog(tournamentId, 20),
      apiClient.getTournamentOperations(tournamentId),
      apiClient.getTournamentTransactions(tournamentId),
    ]);
    if (escrow.ok) setEscrowSol(escrow.data?.balance_sol ?? null);
    if (recon.ok) setReconciliation(recon.data);
    report("audit", audit.ok);
    if (audit.ok) setAuditEntries(audit.data?.entries ?? []);
    report("operations", ops.ok);
    if (ops.ok) setOperations(ops.data?.operations ?? []);
    report("transactions", txs.ok);
    if (txs.ok) setTransactions(txs.data?.transactions ?? []);
  }, [tournamentId]);

  const reload = useCallback(async () => {
    await Promise.all([loadTournament(), loadBracket(), loadSideData()]);
  }, [loadTournament, loadBracket, loadSideData]);

  useEffect(() => {
    setLoading(true);
    void reload();
    const id = window.setInterval(() => void reload(), POLL_MS);
    return () => window.clearInterval(id);
  }, [reload]);

  return {
    tournament, loading, bracket, escrowSol, reconciliation,
    auditEntries, operations, transactions, loadErrors, reload,
  };
}
