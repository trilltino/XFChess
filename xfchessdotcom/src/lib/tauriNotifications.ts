/**
 * Native desktop notification helpers for the Tauri wrapper.
 *
 * These functions only work when the app is running inside the Tauri
 * desktop shell (not the browser). Use feature detection before calling.
 */

/** Returns true if we are running inside the Tauri desktop app. */
export function isTauri(): boolean {
  return typeof window !== 'undefined' && (window as any).__TAURI__ !== undefined;
}

/** Sends a native OS notification via the Tauri notification plugin. */
export async function sendNativeNotification(
  title: string,
  body: string,
): Promise<void> {
  if (!isTauri()) {
    // Fallback: use browser Notification API
    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification(title, { body });
    }
    return;
  }

  try {
    // @ts-ignore — optional dependency only present in Tauri desktop builds
    const tauri = await import('@tauri-apps/api/core');
    await tauri.invoke('show_notification', { title, body });
  } catch {
    // Tauri API not available in this build — fall back to browser
    if ('Notification' in window && Notification.permission === 'granted') {
      new Notification(title, { body });
    }
  }
}

/** Requests browser notification permission (for web fallback). */
export async function requestNotificationPermission(): Promise<boolean> {
  if (!('Notification' in window)) return false;
  const permission = await Notification.requestPermission();
  return permission === 'granted';
}

/** Shows a tournament-start notification. */
export async function notifyTournament(
  tournamentName: string,
  players: number,
  fee: string,
): Promise<void> {
  await sendNativeNotification(
    '🏆 Tournament Starting Soon',
    `${tournamentName} — ${players} players registered, ${fee} entry`,
  );
}

/** Shows a matchmaking match-found notification. */
export async function notifyMatchFound(): Promise<void> {
  await sendNativeNotification(
    '⚔️ Match Found',
    'An opponent has been found! Click to join the game.',
  );
}

/** Shows a game invite notification. */
export async function notifyGameInvite(fromUsername: string): Promise<void> {
  await sendNativeNotification(
    '🎮 Game Invite',
    `${fromUsername} invited you to a match.`,
  );
}

/** Shows a turn reminder notification. */
export async function notifyYourTurn(opponentName: string): Promise<void> {
  await sendNativeNotification(
    '⏳ Your Turn',
    `It's your move against ${opponentName}.`,
  );
}
