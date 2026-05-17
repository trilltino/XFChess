import React, { useState, useEffect } from "react";
import { apiClient } from "../services/api";

interface PgnViewerProps {
  gameId: string;
}

export const PgnViewer: React.FC<PgnViewerProps> = ({ gameId }) => {
  const [pgn, setPgn] = useState<string>("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string>("");

  useEffect(() => {
    if (!gameId) return;
    setLoading(true);
    setError("");
    apiClient
      .getGamePgn(gameId)
      .then((res) => {
        if (res.ok && res.data) {
          setPgn(res.data.pgn);
        } else {
          setError(res.error?.message || "Failed to load PGN");
        }
      })
      .catch((e) => setError(e.message || "Network error"))
      .finally(() => setLoading(false));
  }, [gameId]);

  const handleCopy = () => {
    navigator.clipboard.writeText(pgn);
  };

  return (
    <div className="bg-white rounded-lg shadow p-4">
      <div className="flex items-center justify-between mb-3">
        <h3 className="text-lg font-semibold text-gray-800">PGN Viewer</h3>
        <div className="flex gap-2">
          <button
            onClick={handleCopy}
            disabled={!pgn}
            className="px-3 py-1 text-sm bg-blue-600 text-white rounded hover:bg-blue-700 disabled:opacity-50"
          >
            Copy
          </button>
        </div>
      </div>

      {loading && <p className="text-gray-500 text-sm">Loading PGN…</p>}
      {error && <p className="text-red-600 text-sm">{error}</p>}

      {!loading && !error && (
        <textarea
          readOnly
          value={pgn || "No PGN available for this game."}
          className="w-full h-64 p-3 border border-gray-300 rounded font-mono text-sm resize-y focus:outline-none focus:ring-2 focus:ring-blue-500"
        />
      )}
    </div>
  );
};
