package com.anvlkv.redsiren

import android.animation.TimeAnimator
import android.content.ContentResolver
import android.content.Context
import android.content.res.Resources
import android.os.Bundle
import android.provider.Settings
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Button
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.rememberNavController
import com.anvlkv.redsiren.app.AppIntro
import com.anvlkv.redsiren.core.typegen.Event
import com.anvlkv.redsiren.core.typegen.VisualEV
import com.anvlkv.redsiren.ui.theme.ApplyTheme
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import com.anvlkv.redsiren.core.typegen.Activity as CoreActivity


val Context.dataStore: DataStore<Preferences> by preferencesDataStore(name = "kv")

class MainActivity : ComponentActivity() {
    public var core: Core? = null

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val windowInsetsController = WindowCompat.getInsetsController(window, window.decorView)
        windowInsetsController.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        windowInsetsController.hide(WindowInsetsCompat.Type.systemBars())

        WindowCompat.setDecorFitsSystemWindows(window, false)
        setContent {
            ApplyTheme(content = {
                core = viewModel()
                core!!.store = this.baseContext.dataStore

                Surface {
                    RedSiren(core!!)
                }
            })
        }
    }
}


@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun RedSiren(core: Core) {
    val navController = rememberNavController()
    val coroutineScope = rememberCoroutineScope()

    val recordAudioPermissionState = rememberPermissionState(
        android.Manifest.permission.RECORD_AUDIO
    )

    val reqDef = remember {
        CompletableDeferred<Boolean>()
    }

    var permissionRequested by remember { mutableStateOf(false) }

    core.onRequestPermissions = fun(): CompletableDeferred<Boolean> {
        if (recordAudioPermissionState.status.isGranted) {
            reqDef.complete(true)
        }
        else {
            recordAudioPermissionState.launchPermissionRequest()
        }
        permissionRequested = true
        return reqDef
    }

    LaunchedEffect(recordAudioPermissionState.status) {
        if (permissionRequested) {
            reqDef.complete(recordAudioPermissionState.status.isGranted)
        }
    }


    var animator: TimeAnimator? by remember {
        mutableStateOf(null)
    }


    LaunchedEffect(core.animationSender) {
        if (core.animationSender != null) {
            val listener = fun(_: TimeAnimator, time: Long, _: Long) {
                core.animationSender?.trySend(time)?.getOrNull()
            }
            animator = TimeAnimator()
            animator!!.setTimeListener(listener)
            animator!!.start()
            Log.d("redsiren::android", "animation listener added")
        }
        else {
            animator?.cancel()
            animator = null
            Log.d("redsiren::android", "animation listener removed")
        }
    }



    val context = LocalContext.current
    val cutouts = context.display?.cutout

    val safeAreas = remember {
        cutouts?.let {
            arrayOf(
                it.safeInsetLeft.toDouble(),
                it.safeInsetTop.toDouble(),
                it.safeInsetRight.toDouble(),
                it.safeInsetBottom.toDouble()
            )
        } ?: run {
            arrayOf(0.0, 0.0, 0.0, 0.0)
        }
    }

    LaunchedEffect(safeAreas) {
        core.update(Event.Visual(VisualEV.SafeAreaResize(safeAreas[0], safeAreas[1], safeAreas[2], safeAreas[3])))
    }

    val dpi = Resources.getSystem().displayMetrics.densityDpi.toDouble()

    LaunchedEffect(dpi) {
        core.update(Event.Visual(VisualEV.SetDensity(dpi)))
    }

    val dark = isSystemInDarkTheme()

    LaunchedEffect(dark) {
        core.update(Event.Visual(VisualEV.SetDarkMode(dark)))
    }

    val reducedMotion = isReducedMotionEnabled(context.contentResolver)

    LaunchedEffect(reducedMotion) {
        core.update(Event.Visual(VisualEV.SetReducedMotion(reducedMotion)))
    }

    BoxWithConstraints(
        modifier = Modifier
            .fillMaxSize()
    ) {
        val width = this.maxWidth.value.toDouble()
        val height = this.maxHeight.value.toDouble()

        LaunchedEffect(width, height) {
            core.update(Event.Visual(VisualEV.Resize(width, height)))
        }
        AppIntro()

        Button(onClick = {
            coroutineScope.launch {
                core.update(Event.StartAudioUnit())
            }
        }) {
            Text(text = "Play")
        }
    }

    LaunchedEffect(core) {
        core.update(Event.InitialNavigation(CoreActivity.Intro()))
    }
}


fun isReducedMotionEnabled(resolver: ContentResolver): Boolean {
    val animationDuration = try {
        Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE)
    } catch (e: Settings.SettingNotFoundException) {
        1f
    }
    return animationDuration == 0f
}

@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    RedSiren(viewModel())
}
