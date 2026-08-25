import { useEffect, useMemo, useState } from 'react';
import { listTournaments, type TournamentSummaryResponse } from '../../lib/api';
import { SeoHead } from '../../components/SeoHead';
import { PAGE_METADATA } from '../../lib/seo/metadata';

// Was a static explainer (registration → brackets → payout, three tournament
// tiers, entry requirements). It now lists what the backend actually has, so
// the page is only worth loading when there is something to enter.

const LAMPORTS_PER_SOL = 1_000_000_000;

function sol(lamports: number): string {
    if (!lamports) return 'Free';
    const amount = lamports / LAMPORTS_PER_SOL;
    return `${amount < 0.01 ? amount.toFixed(4) : amount.toFixed(2)} SOL`;
}

/** "Sat 6 Sep · 19:00" in the viewer's own timezone. */
function formatWhen(unixSeconds: number): string {
    const d = new Date(unixSeconds * 1000);
    const day = d.toLocaleDateString(undefined, { weekday: 'short', day: 'numeric', month: 'short' });
    const time = d.toLocaleTimeString(undefined, { hour: '2-digit', minute: '2-digit' });
    return `${day} · ${time}`;
}

/** "in 2d 4h" / "in 35m" / "starting now". */
function formatCountdown(unixSeconds: number, now: number): string {
    const secs = unixSeconds - now;
    if (secs <= 0) return 'starting now';
    const d = Math.floor(secs / 86400);
    const h = Math.floor((secs % 86400) / 3600);
    const m = Math.floor((secs % 3600) / 60);
    if (d > 0) return `in ${d}d ${h}h`;
    if (h > 0) return `in ${h}h ${m}m`;
    return `in ${m}m`;
}

/** Group key: "Today", "Tomorrow", else "Saturday 6 September". */
function dayHeading(unixSeconds: number, now: number): string {
    const d = new Date(unixSeconds * 1000);
    const today = new Date(now * 1000);
    const startOfDay = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
    const diffDays = Math.round((startOfDay(d) - startOfDay(today)) / 86_400_000);
    if (diffDays === 0) return 'Today';
    if (diffDays === 1) return 'Tomorrow';
    return d.toLocaleDateString(undefined, { weekday: 'long', day: 'numeric', month: 'long' });
}

type Group = { heading: string; events: TournamentSummaryResponse[] };

export function Tournaments() {
    const [tournaments, setTournaments] = useState<TournamentSummaryResponse[] | null>(null);
    const [failed, setFailed] = useState(false);
    const [now, setNow] = useState(() => Math.floor(Date.now() / 1000));

    useEffect(() => {
        let mounted = true;
        const load = async () => {
            try {
                const rows = await listTournaments();
                if (mounted) {
                    setTournaments(rows);
                    setFailed(false);
                }
            } catch {
                if (mounted) {
                    setTournaments([]);
                    setFailed(true);
                }
            }
        };
        load();
        const refresh = setInterval(load, 30_000);
        const tick = setInterval(() => setNow(Math.floor(Date.now() / 1000)), 30_000);
        return () => {
            mounted = false;
            clearInterval(refresh);
            clearInterval(tick);
        };
    }, []);

    // Anything finished or cancelled is dropped. Scheduled events sort by
    // start time; live ones sort to the top; unscheduled ones sink to the
    // bottom under their own heading rather than being hidden.
    const { groups, unscheduled, live } = useMemo(() => {
        const rows = (tournaments ?? []).filter((t) => {
            const s = t.status.toLowerCase();
            return s !== 'completed' && s !== 'cancelled';
        });

        const live = rows.filter((t) => t.status.toLowerCase() === 'active');
        const pending = rows.filter((t) => t.status.toLowerCase() !== 'active');

        const scheduled = pending
            .filter((t) => t.scheduled_at && t.scheduled_at > 0)
            .sort((a, b) => (a.scheduled_at ?? 0) - (b.scheduled_at ?? 0));

        const groups: Group[] = [];
        for (const event of scheduled) {
            const heading = dayHeading(event.scheduled_at as number, now);
            const last = groups[groups.length - 1];
            if (last && last.heading === heading) last.events.push(event);
            else groups.push({ heading, events: [event] });
        }

        return {
            groups,
            live,
            unscheduled: pending.filter((t) => !t.scheduled_at || t.scheduled_at <= 0),
        };
    }, [tournaments, now]);

    const isEmpty = tournaments !== null && groups.length === 0 && live.length === 0 && unscheduled.length === 0;

    return (
        <main className="mp mp-wide">
            <SeoHead meta={PAGE_METADATA.tournaments} />
            <h1 className="mp-title">Tournaments</h1>

            {tournaments === null && <p className="mp-lede">Loading…</p>}

            {isEmpty && (
                <p className="mp-lede">
                    {failed
                        ? 'Cannot reach the tournament server right now. Try again shortly.'
                        : 'Nothing scheduled. Check back soon.'}
                </p>
            )}

            {live.length > 0 && (
                <section className="cal-day">
                    <h2 className="cal-heading">In progress</h2>
                    {live.map((t) => (
                        <EventRow key={t.tournament_id} t={t} now={now} live />
                    ))}
                </section>
            )}

            {groups.map((group) => (
                <section className="cal-day" key={group.heading}>
                    <h2 className="cal-heading">{group.heading}</h2>
                    {group.events.map((t) => (
                        <EventRow key={t.tournament_id} t={t} now={now} />
                    ))}
                </section>
            ))}

            {unscheduled.length > 0 && (
                <section className="cal-day">
                    <h2 className="cal-heading">Open, no start time yet</h2>
                    {unscheduled.map((t) => (
                        <EventRow key={t.tournament_id} t={t} now={now} />
                    ))}
                </section>
            )}
        </main>
    );
}

// Rows are not links: /tournament/:id (detail, standings, play) was removed
// along with the rest of the non-core pages, so there is nowhere to click
// through to. Entry happens in the desktop client.
function EventRow({ t, now, live }: { t: TournamentSummaryResponse; now: number; live?: boolean }) {
    const full = t.registered >= t.max_players;
    return (
        <div className="cal-row">
            <div className="cal-time">
                {live ? 'Live' : t.scheduled_at ? formatWhen(t.scheduled_at) : 'TBC'}
                {!live && t.scheduled_at && (
                    <span className="cal-countdown">{formatCountdown(t.scheduled_at, now)}</span>
                )}
            </div>

            <div className="cal-main">
                <span className="cal-name">
                    {t.name}
                    {t.is_private && <span className="cal-tag">Private</span>}
                </span>
                <span className="cal-sub">
                    {t.format === 'single_elimination' ? 'Knockout' : 'Swiss'}
                    {' · '}
                    {t.registered}/{t.max_players} players
                    {full && ' · full'}
                    {t.min_elo > 0 && ` · ${t.min_elo}+ Elo`}
                </span>
            </div>

            <div className="cal-money">
                <span className="cal-pot">{sol(t.prize_pool)}</span>
                <span className="cal-sub">{sol(t.entry_fee_lamports)} entry</span>
            </div>
        </div>
    );
}

export default Tournaments;
