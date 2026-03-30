import React from 'react';
import { Terminal } from 'lucide-react';

interface CodeViewerProps {
    title: string;
    code: string;
    language?: string;
}

const highlightCode = (code: string) => {
    let highlighted = code
        .replace(/&/g, '&amp;')
        .replace(/</g, '&lt;')
        .replace(/>/g, '&gt;');

    // Comments
    highlighted = highlighted.replace(/(\/\/.*)/g, '<span class="comment">$1</span>');

    // Strings
    highlighted = highlighted.replace(/(&quot;.*?&quot;)/g, '<span class="string">$1</span>');

    // Keywords
    const keywords = ['pub', 'fn', 'let', 'mut', 'if', 'else', 'match', 'return', 'struct', 'impl', 'use', 'mod', 'crate', 'true', 'false', 'type'];
    keywords.forEach(kw => {
        const reg = new RegExp(`\\b${kw}\\b`, 'g');
        highlighted = highlighted.replace(reg, `<span class="keyword">${kw}</span>`);
    });

    // Macros (require!, info!, warn!, etc.)
    highlighted = highlighted.replace(/(\b\w+!)/g, '<span class="macro">$1</span>');

    // Types (PascalCase)
    highlighted = highlighted.replace(/\b([A-Z][a-zA-Z0-9]*)\b/g, '<span class="type">$1</span>');

    // Functions (followed by paren)
    highlighted = highlighted.replace(/\b(\w+)(?=\s*\()/g, '<span class="function">$1</span>');

    return highlighted;
};

const CodeViewer: React.FC<CodeViewerProps> = ({ title, code, language = 'rust' }) => {
    const highlightedHtml = highlightCode(code);

    return (
        <div className="code-viewer-container">
            <div className="code-viewer-header">
                <div className="code-viewer-dots">
                    <span className="dot red" />
                    <span className="dot yellow" />
                    <span className="dot green" />
                </div>
                <div className="code-viewer-title">
                    <Terminal size={14} />
                    <span>{title}</span>
                </div>
                <div className="code-viewer-lang">{language}</div>
            </div>
            <div className="code-viewer-body">
                <pre>
                    <code dangerouslySetInnerHTML={{ __html: highlightedHtml }} />
                </pre>
            </div>
        </div>
    );
};

export default CodeViewer;
