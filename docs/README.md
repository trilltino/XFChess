# XFChess Early Access Landing Page

This directory contains the GitHub Pages site for XFChess early access signups.

## Files

- `index.html` - Main landing page with early access form
- `api.js` - Client-side API for form submissions (demo version)
- `README.md` - This file

## Features

### Landing Page
- Modern, responsive design using Tailwind CSS
- Hero section with value proposition
- Detailed fee structure table
- Early access signup form with validation
- Smooth scrolling navigation
- Mobile-optimized layout

### Form Data Collection
The current implementation uses localStorage for demonstration purposes. In production, you'll need to:

1. **Backend API** - Replace the localStorage implementation with a real backend
2. **Database** - Store submissions in a proper database
3. **Email notifications** - Send confirmation emails
4. **Analytics** - Track conversion rates and user behavior

### Admin Panel
Access the admin panel by adding `#admin` to the URL:
```
https://trilltino.github.io/XFChess/#admin
```

The admin panel provides:
- Total submission count
- Country breakdown
- CSV export functionality
- Latest submission timestamp

## Fee Structure

The page presents the following European pricing:

| Service | Fee Type | Amount |
|---------|----------|--------|
| Platform Access | Monthly Subscription | €5.99/month |
| Wager Processing | Transaction Fee | 2.5% of wager |
| Board Sales | Marketplace Fee | 5% of sale price |
| Club Entry | Tournament Fee | €2.50/tournament |
| Withdrawals | Network Fee | Solana gas only |

## Deployment

The site is automatically deployed to GitHub Pages via the workflow in `.github/workflows/deploy.yml`. The workflow:

1. Triggers on push to main branch
2. Uses the `docs` folder as the source
3. Deploys to `https://trilltino.github.io/XFChess/`

## Customization

### Branding
- Update colors in the CSS variables
- Replace the logo/icon in the navigation
- Modify the hero section text

### Form Fields
- Add/remove fields in the HTML form
- Update the validation logic
- Modify the CSV export headers

### Pricing
- Update the fee table in the HTML
- Adjust the pricing descriptions
- Add new fee categories as needed

## Production Setup

### Backend API
Replace the `api.js` localStorage implementation with calls to your backend:

```javascript
// Example backend integration
const response = await fetch('https://your-api.com/early-access', {
    method: 'POST',
    headers: {
        'Content-Type': 'application/json',
        'Authorization': 'Bearer YOUR_API_KEY'
    },
    body: JSON.stringify(data)
});
```

### Database Schema
Recommended database schema for submissions:

```sql
CREATE TABLE early_access_submissions (
    id SERIAL PRIMARY KEY,
    name VARCHAR(255) NOT NULL,
    email VARCHAR(255) UNIQUE NOT NULL,
    matches_per_month VARCHAR(50),
    avg_wager VARCHAR(50),
    country VARCHAR(2),
    why_interested TEXT,
    ip_address INET,
    user_agent TEXT,
    referrer TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);
```

### Email Integration
Set up automated emails:
- Welcome email on submission
- Early access invitation emails
- Update notifications

## Analytics Tracking

Add your analytics tracking code to the `<head>` section:

```html
<!-- Google Analytics -->
<script async src="https://www.googletagmanager.com/gtag/js?id=GA_MEASUREMENT_ID"></script>
<script>
  window.dataLayer = window.dataLayer || [];
  function gtag(){dataLayer.push(arguments);}
  gtag('js', new Date());
  gtag('config', 'GA_MEASUREMENT_ID');
</script>
```

## Performance Optimization

- Images are optimized and use modern formats
- CSS is minified via Tailwind
- JavaScript is loaded asynchronously
- Font Awesome icons are loaded from CDN
- Lazy loading can be added for large content sections

## Security Considerations

- Form validation on both client and server side
- Rate limiting for form submissions
- CSRF protection for backend API
- GDPR compliance for data collection
- Input sanitization to prevent XSS

## Monitoring

Set up monitoring for:
- Form submission success rate
- Page load times
- Error rates
- User engagement metrics

## Next Steps

1. **Backend Development** - Implement real API endpoints
2. **Database Setup** - Configure production database
3. **Email Service** - Set up transactional emails
4. **Analytics** - Implement tracking and reporting
5. **A/B Testing** - Test different messaging and pricing
6. **SEO Optimization** - Add meta tags and structured data

## Support

For questions about the early access system or to report issues, please contact the development team.
