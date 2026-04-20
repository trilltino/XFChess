import { useState } from 'react';
import { MessageSquare, ThumbsUp, Search, Plus } from 'lucide-react';

interface Thread {
    id: string;
    title: string;
    author: string;
    replies: number;
    views: number;
    likes: number;
    category: string;
    timestamp: string;
    isPinned?: boolean;
}

const categories = [
    { id: 'all', name: 'All Topics', color: '#ad5c2f' },
];

const mockThreads: Thread[] = [
    { id: '3', title: 'Tournament brackets now live - April 2026', author: 'Tournament_Mod', replies: 12, views: 892, likes: 45, category: 'tournaments', timestamp: '1 day ago', isPinned: true },
];

export function Forum() {
    const [activeCategory, setActiveCategory] = useState('all');
    const [searchQuery, setSearchQuery] = useState('');

    const filteredThreads = mockThreads.filter(thread => {
        const matchesCategory = activeCategory === 'all' || thread.category === activeCategory;
        const matchesSearch = thread.title.toLowerCase().includes(searchQuery.toLowerCase()) ||
                            thread.author.toLowerCase().includes(searchQuery.toLowerCase());
        return matchesCategory && matchesSearch;
    });

    const pinnedThreads = filteredThreads.filter(t => t.isPinned);
    const regularThreads = filteredThreads.filter(t => !t.isPinned);

    return (
        <main className="section" style={{ minHeight: '100vh', paddingTop: '100px' }}>
            <div className="section-label">COMMUNITY</div>
            <h2 style={{ fontSize: '2.5rem', marginBottom: '8px' }}>Forum<span className="accent">.</span></h2>
            <p style={{ color: 'var(--text-dim)', marginBottom: '40px', maxWidth: '600px' }}>
                Join the conversation. Discuss strategies, report issues, and connect with the XFChess community.
            </p>

            {/* Search and Actions */}
            <div style={{ display: 'flex', gap: '16px', marginBottom: '32px', flexWrap: 'wrap' }}>
                <div style={{ flex: '1', minWidth: '280px', position: 'relative' }}>
                    <Search size={18} style={{ position: 'absolute', left: '16px', top: '50%', transform: 'translateY(-50%)', color: 'var(--text-dim)' }} />
                    <input
                        type="text"
                        value={searchQuery}
                        onChange={(e) => setSearchQuery(e.target.value)}
                        style={{
                            width: '100%',
                            padding: '14px 16px 14px 44px',
                            borderRadius: '10px',
                            border: '1px solid var(--border)',
                            background: 'var(--glass)',
                            color: '#fff',
                            fontSize: '15px'
                        }}
                    />
                </div>
                <button className="btn btn-primary" style={{ width: 'auto', padding: '0 24px', display: 'flex', alignItems: 'center', gap: '8px' }}>
                    <Plus size={18} />
                    New Thread
                </button>
            </div>

            {/* Categories */}
            <div style={{ display: 'flex', gap: '12px', marginBottom: '32px', flexWrap: 'wrap' }}>
                {categories.map(cat => (
                    <button
                        key={cat.id}
                        onClick={() => setActiveCategory(cat.id)}
                        style={{
                            padding: '10px 20px',
                            borderRadius: '20px',
                            border: '1px solid',
                            borderColor: activeCategory === cat.id ? cat.color : 'var(--border)',
                            background: activeCategory === cat.id ? `${cat.color}20` : 'var(--glass)',
                            color: activeCategory === cat.id ? cat.color : 'var(--text-dim)',
                            fontSize: '14px',
                            fontWeight: 600,
                            cursor: 'pointer',
                            transition: 'all 0.2s'
                        }}
                    >
                        {cat.name}
                    </button>
                ))}
            </div>

            {/* Threads List */}
            <div style={{ display: 'flex', flexDirection: 'column', gap: '12px' }}>
                {pinnedThreads.map(thread => (
                    <ThreadCard key={thread.id} thread={thread} isPinned />
                ))}
                {regularThreads.map(thread => (
                    <ThreadCard key={thread.id} thread={thread} />
                ))}
            </div>

            {filteredThreads.length === 0 && (
                <div style={{ textAlign: 'center', padding: '60px', color: 'var(--text-dim)' }}>
                    <MessageSquare size={48} style={{ opacity: 0.3, marginBottom: '16px' }} />
                    <p>No threads found. Be the first to start a conversation!</p>
                </div>
            )}
        </main>
    );
}

function ThreadCard({ thread, isPinned }: { thread: Thread; isPinned?: boolean }) {
    const cat = categories.find(c => c.id === thread.category);

    return (
        <div style={{
            padding: '20px 24px',
            background: isPinned ? 'rgba(173, 92, 47, 0.08)' : 'var(--glass)',
            border: `1px solid ${isPinned ? 'rgba(173, 92, 47, 0.3)' : 'var(--border)'}`,
            borderRadius: '12px',
            display: 'flex',
            alignItems: 'center',
            gap: '20px',
            cursor: 'pointer',
            transition: 'all 0.2s'
        }} className="thread-card-hover">
            <div style={{ flex: '1' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '10px', marginBottom: '6px' }}>
                    {isPinned && (
                        <span style={{
                            padding: '2px 8px',
                            background: '#ad5c2f',
                            color: '#fff',
                            fontSize: '11px',
                            fontWeight: 700,
                            borderRadius: '4px',
                            textTransform: 'uppercase'
                        }}>Pinned</span>
                    )}
                    <span style={{
                        padding: '2px 10px',
                        background: `${cat?.color}20`,
                        color: cat?.color,
                        fontSize: '12px',
                        fontWeight: 600,
                        borderRadius: '4px'
                    }}>{cat?.name}</span>
                    <span style={{ color: 'var(--text-dim)', fontSize: '13px' }}>{thread.timestamp}</span>
                </div>
                <h3 style={{ fontSize: '1.1rem', fontWeight: 600, marginBottom: '4px' }}>{thread.title}</h3>
                <p style={{ color: 'var(--text-dim)', fontSize: '14px' }}>by {thread.author}</p>
            </div>

            <div style={{ display: 'flex', alignItems: 'center', gap: '24px', color: 'var(--text-dim)' }}>
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '14px' }}>
                    <MessageSquare size={16} />
                    {thread.replies}
                </div>
                <div style={{ display: 'flex', alignItems: 'center', gap: '6px', fontSize: '14px' }}>
                    <ThumbsUp size={16} />
                    {thread.likes}
                </div>
                <div style={{ fontSize: '14px' }}>
                    {thread.views.toLocaleString()} views
                </div>
            </div>
        </div>
    );
}
