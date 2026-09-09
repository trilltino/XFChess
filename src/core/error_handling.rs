use bevy::ecs::error::{BevyError, ErrorContext, FallbackErrorHandler};
use bevy::prelude::*;

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

pub fn install_recovering_error_handler(app: &mut App) {
    app.insert_resource(FallbackErrorHandler(recovering_error_handler));
}

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
