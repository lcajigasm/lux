package lux.android.host

import android.app.Activity
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.view.SurfaceView
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection
import android.view.inputmethod.BaseInputConnection

/// Kotlin host for Phase 13: `SurfaceView` + JNI. Real `InputConnection`,
/// not a keyboard-visibility hack. No terminal/LSP UI (GOALS v1.2.0).
class LuxActivity : Activity() {
    private lateinit var surface: SurfaceView

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        surface = SurfaceView(this)
        setContentView(surface)
        System.loadLibrary("lux")
    }

    override fun onCreateInputConnection(outAttrs: EditorInfo): InputConnection {
        outAttrs.inputType = EditorInfo.TYPE_CLASS_TEXT
        outAttrs.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI
        return BaseInputConnection(surface, true)
    }

    fun openSaf() {
        startActivityForResult(
            Intent(Intent.ACTION_OPEN_DOCUMENT).apply { type = "*/*" },
            1,
        )
    }

    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        val uri: Uri? = data?.data
        if (uri != null) {
            nativeOpenUri(uri.toString())
        }
    }

    private external fun nativeOpenUri(uri: String)
}
