//! Error handling utilities for state systems
//!
//! Provides wrappers and utilities for handling errors gracefully in state systems,
//! preventing panics from crashing the entire application.

use bevy::ecs::error::{BevyError, ErrorContext, FallbackErrorHandler};
use bevy::prelude::*;

/// Global fallback error handler for the Bevy world.
///
/// As of Bevy 0.19, panics raised inside systems, commands, observers and run
/// conditions are caught by the executor and routed here instead of unwinding
/// the whole app. We log the failure (and append a crash-report line so it ends
/// up alongside [`crate::core::crash`] reports) and then **return**, which tells
/// Bevy to recover and keep the schedule running.
///
/// This matters for XFChess specifically: a hard crash mid-match drops the P2P
/// session and lets the on-chain game run down to a timeout/forfeit. Logging and
/// continuing means a buggy rendering/UI system degrades the frame instead of
/// surrendering the match.
fn recovering_error_handler(error: BevyError, ctx: ErrorContext) {
    error!(
        "[RECOVERED] {} `{}` failed — logged and continuing instead of crashing: {}",
        ctx.kind(),
        ctx.name(),
        error,
    );

    crate::core::crash::record_recovered_error(&format!("{} `{}`", ctx.kind(), ctx.name()), &error);
    // Intentionally no panic/re-raise: returning here keeps the app alive.
}

/// Installs [`recovering_error_handler`] as the world's [`FallbackErrorHandler`].
///
/// Call this once while building the [`App`]. Systems that explicitly want to
/// abort can still do so by returning a `Severity::Panic` error or panicking
/// from within the handler itself.
pub fn install_recovering_error_handler(app: &mut App) {
    app.insert_resource(FallbackErrorHandler(recovering_error_handler));
}

/// Helper macro to safely unwrap Option with error logging
#[macro_export]
macro_rules! safe_unwrap {
    ($expr:expr, $msg:expr) => {
        match $expr {
            Some(val) => val,
            None => {
                error!("[ERROR_HANDLER] {}", $msg);
                return;
            }
        }
    };
}

/// Helper macro to safely unwrap Result with error logging
#[macro_export]
macro_rules! safe_unwrap_result {
    ($expr:expr, $msg:expr) => {
        match $expr {
            Ok(val) => val,
            Err(e) => {
                error!("[ERROR_HANDLER] {}: {:?}", $msg, e);
                return;
            }
        }
    };
}

/// Helper function to safely parse hex color strings
///
/// Returns the parsed color or a default fallback color if parsing fails.
/// Logs an error if the hex string is invalid.
pub fn safe_parse_hex_color(
    hex: &str,
    fallback: bevy::color::Srgba,
    context: &str,
) -> bevy::color::Srgba {
    match bevy::color::Srgba::hex(hex) {
        Ok(color) => color,
        Err(e) => {
            error!(
                "[ERROR_HANDLER] Failed to parse hex color '{}' in {}: {:?}",
                hex, context, e
            );
            warn!("[ERROR_HANDLER] Using fallback color: {:?}", fallback);
            fallback
        }
    }
}

/// Wrapper macro for observer functions to catch panics
///
/// Observers can panic if they access invalid entities or resources.
/// This macro wraps observer logic in a panic-catching block and logs errors.
///
/// Usage:
/// ```rust,ignore
/// pub fn on_piece_click(...) {
///     safe_observer!("on_piece_click", entity, {
///         // observer logic here
///     });
/// }
/// ```
#[macro_export]
macro_rules! safe_observer {
    ($observer_name:expr, $entity:expr, $body:block) => {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| $body)).unwrap_or_else(|_| {
            error!(
                "[OBSERVER] {} panicked on entity {:?} - continuing",
                $observer_name, $entity
            );
        });
    };
}
