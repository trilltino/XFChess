package com.trilltino.xfchess

import android.os.Build.VERSION
import android.os.Build.VERSION_CODES
import android.os.Bundle
import android.view.View
import android.view.WindowManager
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity
import com.solana.mobilewalletadapter.clientlib.ActivityResultSender

/**
 * Translated from rust-mobile/rust-android-examples' agdk-winit-wgpu-egui
 * MainActivity.java (the android-activity crate maintainers' own reference)
 * — not written from scratch, since GameActivity's Java-side contract
 * (static-init loadLibrary ordering, hideSystemUI, decor-fits-system-windows)
 * is exactly the kind of thing worth copying from a verified-working source
 * rather than re-deriving.
 */
class MainActivity : GameActivity() {

    lateinit var activityResultSender: ActivityResultSender

    companion object {
        init {
            // Must match both the `[lib] name` in the root Cargo.toml
            // (produces libxfchess.so) and AndroidManifest.xml's
            // `android.app.lib_name` meta-data value.
            System.loadLibrary("xfchess")
        }
    }

    private fun hideSystemUI() {
        if (VERSION.SDK_INT >= VERSION_CODES.P) {
            window.attributes.layoutInDisplayCutoutMode =
                WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_ALWAYS
        }
        val decorView: View = window.decorView
        val controller = WindowInsetsControllerCompat(window, decorView)
        controller.hide(WindowInsetsCompat.Type.systemBars())
        controller.hide(WindowInsetsCompat.Type.displayCutout())
        controller.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        hideSystemUI()

        // MWA registers an Activity Result launcher in this constructor. It
        // must happen before onStart, otherwise Android rejects registration.
        activityResultSender = ActivityResultSender(this)

        super.onCreate(savedInstanceState)
    }
}
