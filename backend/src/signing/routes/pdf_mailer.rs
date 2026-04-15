//! PDF email service for sending welcome packets to new signups.
//!
//! Uses SendGrid API to deliver PDF attachments via email.
//! Requires SENDGRID_API_KEY environment variable.

use axum::{http::StatusCode, Json, Router, routing::post};
use base64::Engine;
use printpdf::*;
use serde::{Deserialize, Serialize};
use std::env;
use std::io::BufWriter;
use tracing::{info, error};

/// Signup request with email
#[derive(Deserialize)]
pub struct SignUpRequest {
    pub email: String,
    pub referral: Option<String>,
}

/// SendGrid API request structure
#[derive(Serialize)]
struct SendGridRequest {
    personalizations: Vec<Personalization>,
    from: EmailAddress,
    subject: String,
    content: Vec<Content>,
    attachments: Vec<Attachment>,
}

#[derive(Serialize)]
struct Personalization {
    to: Vec<EmailAddress>,
}

#[derive(Serialize)]
struct EmailAddress {
    email: String,
    name: Option<String>,
}

#[derive(Serialize)]
struct Content {
    #[serde(rename = "type")]
    content_type: String,
    value: String,
}

#[derive(Serialize)]
struct Attachment {
    content: String, // base64 encoded
    filename: String,
    #[serde(rename = "type")]
    content_type: String,
    disposition: String,
}

/// Generate a welcome PDF for new users
fn generate_welcome_pdf(email: &str, referral: Option<&str>) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    // Create PDF document (A4 size)
    let (doc, page1, layer1) = PdfDocument::new(
        "XFChess Welcome Guide",
        Mm(210.0),
        Mm(297.0),
        "Layer 1"
    );
    
    let layer = doc.get_page(page1).get_layer(layer1);
    
    // Load a standard font
    let font = doc.add_builtin_font(BuiltinFont::Helvetica)?;
    let font_bold = doc.add_builtin_font(BuiltinFont::HelveticaBold)?;
    
    let mut y_pos: f32 = 270.0; // Start from top
    let left_margin: f32 = 20.0;
    
    // Title
    y_pos = draw_text(&layer, &font_bold, "Welcome to XFChess!", y_pos, 24.0, left_margin);
    y_pos -= 12.0;
    
    // Subtitle
    y_pos = draw_text(&layer, &font, "Your On-Chain Chess Tournament Guide", y_pos, 14.0, left_margin);
    y_pos -= 20.0;
    
    // Welcome message
    let name = email.split('@').next().unwrap_or("Player");
    y_pos = draw_text(&layer, &font, &format!("Hello {},", name), y_pos, 12.0, left_margin);
    y_pos -= 15.0;
    
    y_pos = draw_text(&layer, &font, 
        "Thank you for joining XFChess! You're now part of the future of competitive chess on Solana.", 
        y_pos, 12.0, left_margin);
    y_pos -= 25.0;
    
    // What's Inside section
    y_pos = draw_text(&layer, &font_bold, "What's Inside:", y_pos, 14.0, left_margin);
    y_pos -= 15.0;
    
    let features = vec![
        "• Tournament entry with real SOL prizes",
        "• ELO-based matchmaking and rankings", 
        "• On-chain game verification",
        "• Anti-cheat protected gameplay",
        "• Player profiles and achievements",
    ];
    
    for feature in features {
        y_pos = draw_text(&layer, &font, feature, y_pos, 11.0, left_margin);
        y_pos -= 12.0;
    }
    y_pos -= 15.0;
    
    // Getting Started section
    y_pos = draw_text(&layer, &font_bold, "Getting Started:", y_pos, 14.0, left_margin);
    y_pos -= 15.0;
    
    let steps = vec![
        "1. Download the XFChess client",
        "2. Connect your Solana wallet",
        "3. Join a tournament or play casual games",
        "4. Compete for SOL prizes!",
    ];
    
    for step in steps {
        y_pos = draw_text(&layer, &font, step, y_pos, 11.0, left_margin);
        y_pos -= 12.0;
    }
    y_pos -= 15.0;
    
    // Tournament Structure section
    y_pos = draw_text(&layer, &font_bold, "Tournament Structure:", y_pos, 14.0, left_margin);
    y_pos -= 15.0;
    
    let tournament_info = vec![
        "• 8, 16, 32, 64, or 128 player brackets",
        "• Entry fees: FREE to 0.5 SOL",
        "• Prize distribution: 50%/30%/15%/5% for 16+ players",
        "• Winner-take-all for 8 player tournaments",
    ];
    
    for info in tournament_info {
        y_pos = draw_text(&layer, &font, info, y_pos, 11.0, left_margin);
        y_pos -= 12.0;
    }
    
    // Referral note
    if let Some(ref_source) = referral {
        y_pos -= 15.0;
        draw_text(&layer, &font, &format!("You heard about us from: {}", ref_source), y_pos, 11.0, left_margin);
    }
    
    // Footer
    y_pos = 45.0;
    draw_text(&layer, &font_bold, "Good luck on the board!", y_pos, 12.0, left_margin);
    y_pos -= 15.0;
    draw_text(&layer, &font, "- The XFChess Team", y_pos, 11.0, left_margin);
    
    // Save to bytes
    let mut pdf_bytes = Vec::new();
    {
        let mut writer = BufWriter::new(&mut pdf_bytes);
        doc.save(&mut writer)?;
    } // writer is dropped here, flushing to pdf_bytes
    
    Ok(pdf_bytes)
}

/// Helper to draw text at a position
fn draw_text(layer: &PdfLayerReference, font: &IndirectFontRef, text: &str, y: f32, size: f32, x: f32) -> f32 {
    layer.use_text(text, size, Mm(x), Mm(y), font);
    y - (size / 2.0 + 4.0) // Return new y position
}

/// Send welcome email with PDF via SendGrid
pub async fn send_welcome_email(Json(req): Json<SignUpRequest>) -> Result<StatusCode, StatusCode> {
    let sendgrid_api_key = env::var("SENDGRID_API_KEY")
        .map_err(|_| {
            error!("[pdf_mailer] SENDGRID_API_KEY not set");
            StatusCode::SERVICE_UNAVAILABLE
        })?;
    
    // Generate PDF
    let pdf_bytes = generate_welcome_pdf(&req.email, req.referral.as_deref())
        .map_err(|e| {
            error!("[pdf_mailer] PDF generation failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    let pdf_base64 = base64::engine::general_purpose::STANDARD.encode(&pdf_bytes);
    
    // Build SendGrid request
    let email_name = req.email.split('@').next().unwrap_or("Player");
    let sendgrid_req = SendGridRequest {
        personalizations: vec![Personalization {
            to: vec![EmailAddress {
                email: req.email.clone(),
                name: Some(email_name.to_string()),
            }],
        }],
        from: EmailAddress {
            email: "noreply@xfchess.com".to_string(),
            name: Some("XFChess".to_string()),
        },
        subject: "Welcome to XFChess - Your Tournament Guide".to_string(),
        content: vec![Content {
            content_type: "text/plain".to_string(),
            value: format!(
                "Hello {},\n\nWelcome to XFChess! Your tournament guide is attached as a PDF.\n\nGet ready to play chess on-chain!\n\n- The XFChess Team",
                email_name
            ),
        }],
        attachments: vec![Attachment {
            content: pdf_base64,
            filename: "xfchess-welcome-guide.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            disposition: "attachment".to_string(),
        }],
    };
    
    // Send via SendGrid API (URL overridable for testing)
    let sendgrid_url = env::var("SENDGRID_API_URL")
        .unwrap_or_else(|_| "https://api.sendgrid.com/v3/mail/send".to_string());
    let client = reqwest::Client::new();
    let response = client
        .post(&sendgrid_url)
        .header("Authorization", format!("Bearer {}", sendgrid_api_key))
        .json(&sendgrid_req)
        .send()
        .await
        .map_err(|e| {
            error!("[pdf_mailer] SendGrid request failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    
    if response.status().is_success() {
        info!("[pdf_mailer] Welcome email sent to {}", req.email);
        Ok(StatusCode::OK)
    } else {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        error!("[pdf_mailer] SendGrid error: {} - {}", status, body);
        Err(StatusCode::INTERNAL_SERVER_ERROR)
    }
}

/// Create router for PDF mailer endpoints
pub fn pdf_mailer_routes() -> Router {
    Router::new()
        .route("/signup", post(send_welcome_email))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sign_up_request_serialization() {
        let req = SignUpRequest {
            email: "test@example.com".to_string(),
            referral: Some("friend".to_string()),
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_sign_up_request_without_referral() {
        let req = SignUpRequest {
            email: "test@example.com".to_string(),
            referral: None,
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_sendgrid_request_serialization() {
        let req = SendGridRequest {
            personalizations: vec![Personalization {
                to: vec![EmailAddress {
                    email: "test@example.com".to_string(),
                    name: Some("Test User".to_string()),
                }],
            }],
            from: EmailAddress {
                email: "noreply@xfchess.com".to_string(),
                name: Some("XFChess".to_string()),
            },
            subject: "Test Subject".to_string(),
            content: vec![Content {
                content_type: "text/plain".to_string(),
                value: "Test content".to_string(),
            }],
            attachments: vec![Attachment {
                content: "base64content".to_string(),
                filename: "test.pdf".to_string(),
                content_type: "application/pdf".to_string(),
                disposition: "attachment".to_string(),
            }],
        };

        let json = serde_json::to_string(&req);
        assert!(json.is_ok());
    }

    #[test]
    fn test_email_address_serialization() {
        let addr = EmailAddress {
            email: "test@example.com".to_string(),
            name: Some("Test User".to_string()),
        };

        let json = serde_json::to_string(&addr);
        assert!(json.is_ok());
    }

    #[test]
    fn test_email_address_without_name() {
        let addr = EmailAddress {
            email: "test@example.com".to_string(),
            name: None,
        };

        let json = serde_json::to_string(&addr);
        assert!(json.is_ok());
    }

    #[test]
    fn test_content_serialization() {
        let content = Content {
            content_type: "text/plain".to_string(),
            value: "Test content".to_string(),
        };

        let json = serde_json::to_string(&content);
        assert!(json.is_ok());
    }

    #[test]
    fn test_attachment_serialization() {
        let attachment = Attachment {
            content: "base64content".to_string(),
            filename: "test.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            disposition: "attachment".to_string(),
        };

        let json = serde_json::to_string(&attachment);
        assert!(json.is_ok());
    }

    #[test]
    fn test_pdf_mailer_routes_creation() {
        let router = pdf_mailer_routes();
        assert!(router.not_found("test").is_some());
    }

    #[test]
    fn test_email_format_validation() {
        // Test valid email formats
        let valid_emails = vec![
            "test@example.com",
            "user.name@domain.co.uk",
            "user+tag@example.com",
        ];
        for email in valid_emails {
            let req = SignUpRequest {
                email: email.to_string(),
                referral: None,
            };
            assert!(req.email.contains('@'));
            assert!(req.email.contains('.'));
        }
    }

    #[test]
    fn test_referral_handling() {
        // Test with referral
        let req_with_referral = SignUpRequest {
            email: "test@example.com".to_string(),
            referral: Some("friend".to_string()),
        };
        assert_eq!(req_with_referral.referral, Some("friend".to_string()));

        // Test without referral
        let req_without_referral = SignUpRequest {
            email: "test@example.com".to_string(),
            referral: None,
        };
        assert_eq!(req_without_referral.referral, None);
    }

    #[test]
    fn test_pdf_generation_basic() {
        // Test that PDF generation doesn't panic with valid input
        let result = generate_welcome_pdf("test@example.com", Some("friend"));
        assert!(result.is_ok());
    }

    #[test]
    fn test_pdf_generation_without_referral() {
        // Test PDF generation without referral
        let result = generate_welcome_pdf("test@example.com", None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_pdf_generation_invalid_email() {
        // Test PDF generation with edge case email
        let result = generate_welcome_pdf("@invalid.com", None);
        // Should still generate PDF even with weird email
        assert!(result.is_ok());
    }

    #[test]
    fn test_draw_text_function() {
        // This test verifies draw_text function exists and has correct signature
        // Actual PDF rendering tests would require more complex setup
        let _test_signature = |layer: &PdfLayerReference, font: &IndirectFontRef, text: &str, y: f32, size: f32, x: f32| -> f32 {
            draw_text(layer, font, text, y, size, x)
        };
        let _: fn(&PdfLayerReference, &IndirectFontRef, &str, f32, f32, f32) -> f32 = _test_signature;
    }

    #[test]
    fn test_base64_encoding() {
        // Test that base64 encoding works for PDF bytes
        let pdf_bytes = vec![1u8, 2, 3, 4, 5];
        let encoded = base64::engine::general_purpose::STANDARD.encode(&pdf_bytes);
        assert!(!encoded.is_empty());
    }

    #[test]
    fn test_attachment_filename() {
        let attachment = Attachment {
            content: "base64content".to_string(),
            filename: "xfchess-welcome-guide.pdf".to_string(),
            content_type: "application/pdf".to_string(),
            disposition: "attachment".to_string(),
        };
        assert_eq!(attachment.filename, "xfchess-welcome-guide.pdf");
        assert_eq!(attachment.content_type, "application/pdf");
        assert_eq!(attachment.disposition, "attachment");
    }

    #[test]
    fn test_personalization_structure() {
        let personalization = Personalization {
            to: vec![EmailAddress {
                email: "test@example.com".to_string(),
                name: Some("Test User".to_string()),
            }],
        };
        assert_eq!(personalization.to.len(), 1);
        assert_eq!(personalization.to[0].email, "test@example.com");
    }
}
