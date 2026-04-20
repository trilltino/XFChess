// Simple API endpoint for early access form submission
// This would typically be deployed as a serverless function
// For now, it stores data in localStorage for demonstration

class EarlyAccessAPI {
    constructor() {
        this.storageKey = 'xfchess_early_access_submissions';
    }

    // Store submission (in real app, this would send to a backend)
    async submitSubmission(data) {
        try {
            // In production, replace this with actual API call:
            // const response = await fetch('https://your-api.com/early-access', {
            //     method: 'POST',
            //     headers: { 'Content-Type': 'application/json' },
            //     body: JSON.stringify(data)
            // });
            
            // For demo purposes, store in localStorage
            const submissions = this.getSubmissions();
            const submission = {
                ...data,
                id: Date.now(),
                timestamp: new Date().toISOString(),
                userAgent: navigator.userAgent,
                referrer: document.referrer
            };
            
            submissions.push(submission);
            localStorage.setItem(this.storageKey, JSON.stringify(submissions));
            
            // Simulate API delay
            await new Promise(resolve => setTimeout(resolve, 1000));
            
            return { success: true, id: submission.id };
        } catch (error) {
            console.error('Submission error:', error);
            return { success: false, error: error.message };
        }
    }

    // Get all submissions (for admin view)
    getSubmissions() {
        try {
            const stored = localStorage.getItem(this.storageKey);
            return stored ? JSON.parse(stored) : [];
        } catch (error) {
            console.error('Error retrieving submissions:', error);
            return [];
        }
    }

    // Export submissions as CSV (for manual data collection)
    exportToCSV() {
        const submissions = this.getSubmissions();
        if (submissions.length === 0) {
            alert('No submissions to export');
            return;
        }

        const headers = ['ID', 'Timestamp', 'Name', 'Email', 'Matches/Month', 'Avg Wager', 'Country', 'Comments'];
        const csvContent = [
            headers.join(','),
            ...submissions.map(s => [
                s.id,
                s.timestamp,
                `"${s.name || ''}"`,
                `"${s.email || ''}"`,
                `"${s.matches_per_month || ''}"`,
                `"${s.avg_wager || ''}"`,
                `"${s.country || ''}"`,
                `"${s.why_interested || ''}"`
            ].join(','))
        ].join('\n');

        const blob = new Blob([csvContent], { type: 'text/csv' });
        const url = window.URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = `xfchess_early_access_${new Date().toISOString().split('T')[0]}.csv`;
        document.body.appendChild(a);
        a.click();
        document.body.removeChild(a);
        window.URL.revokeObjectURL(url);
    }

    // Get analytics summary
    getAnalytics() {
        const submissions = this.getSubmissions();
        if (submissions.length === 0) return null;

        const countryCounts = {};
        const wagerRanges = {};
        const matchRanges = {};

        submissions.forEach(s => {
            // Count by country
            countryCounts[s.country] = (countryCounts[s.country] || 0) + 1;
            
            // Count by wager range
            wagerRanges[s.avg_wager] = (wagerRanges[s.avg_wager] || 0) + 1;
            
            // Count by match frequency
            matchRanges[s.matches_per_month] = (matchRanges[s.matches_per_month] || 0) + 1;
        });

        return {
            totalSubmissions: submissions.length,
            countryBreakdown: countryCounts,
            wagerBreakdown: wagerRanges,
            matchFrequencyBreakdown: matchRanges,
            latestSubmission: submissions[submissions.length - 1]?.timestamp
        };
    }
}

// Make available globally
window.EarlyAccessAPI = EarlyAccessAPI;

// Add admin controls (password protected for demo)
if (window.location.hash === '#admin') {
    const api = new EarlyAccessAPI();
    
    // Create admin panel
    const adminPanel = document.createElement('div');
    adminPanel.innerHTML = `
        <div style="position: fixed; top: 20px; right: 20px; background: white; padding: 20px; border-radius: 8px; box-shadow: 0 4px 20px rgba(0,0,0,0.1); z-index: 1000; max-width: 400px;">
            <h3>Admin Panel</h3>
            <div id="admin-stats"></div>
            <button onclick="window.exportSubmissions()" style="margin-top: 10px; padding: 8px 16px; background: #007bff; color: white; border: none; border-radius: 4px; cursor: pointer;">Export CSV</button>
            <button onclick="this.parentElement.remove()" style="margin-left: 10px; padding: 8px 16px; background: #dc3545; color: white; border: none; border-radius: 4px; cursor: pointer;">Close</button>
        </div>
    `;
    document.body.appendChild(adminPanel);

    // Update stats
    function updateStats() {
        const analytics = api.getAnalytics();
        if (analytics) {
            document.getElementById('admin-stats').innerHTML = `
                <p><strong>Total Submissions:</strong> ${analytics.totalSubmissions}</p>
                <p><strong>Latest:</strong> ${new Date(analytics.latestSubmission).toLocaleString()}</p>
                <details>
                    <summary>Country Breakdown</summary>
                    ${Object.entries(analytics.countryBreakdown).map(([country, count]) => 
                        `<p>${country}: ${count}</p>`
                    ).join('')}
                </details>
            `;
        }
    }

    window.exportSubmissions = () => api.exportToCSV();
    updateStats();
}
