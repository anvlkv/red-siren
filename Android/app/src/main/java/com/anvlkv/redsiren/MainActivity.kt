package com.anvlkv.redsiren

import android.animation.TimeAnimator
import android.content.res.Resources
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
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
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.anvlkv.redsiren.app.AppAbout
import com.anvlkv.redsiren.app.AppInstrument
import com.anvlkv.redsiren.app.AppIntro
import com.anvlkv.redsiren.shared.shared_types.Event
import com.anvlkv.redsiren.shared.shared_types.InstrumentEV
import com.anvlkv.redsiren.shared.shared_types.IntroEV
import com.anvlkv.redsiren.shared.shared_types.TunerEV
import com.anvlkv.redsiren.ui.theme.ApplyTheme
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import com.anvlkv.redsiren.shared.shared_types.Activity as CoreActivity

class MainActivity : ComponentActivity() {
    var core: Core? = null

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

    fun updateConfig(width: Double, height: Double, cutouts: Array<Double>) {
        val dpi = Resources.getSystem().displayMetrics.densityDpi.toDouble()

        coroutineScope.launch {
            core.update(Event.CreateConfigAndConfigureApp(width, height, dpi, cutouts.asList()))
        }
    }


    val introVm = core.view.intro
    val instrumentVm = core.view.instrument
    val tunerVm = core.view.tuning


    val introEv = fun(ev: IntroEV) {
        coroutineScope.launch {
            core.update(Event.IntroEvent(ev))
        }
    }

    val instrumentEv = fun(ev: InstrumentEV) {
        coroutineScope.launch {
            core.update(Event.InstrumentEvent(ev))
        }
    }

    val tunerEv = fun(ev: TunerEV) {
        coroutineScope.launch {
            core.update(Event.TunerEvent(ev))
        }
    }

    fun navigateTo(act: CoreActivity) {
        when (act) {
            is CoreActivity.Intro -> {
                navController.navigate("intro")
            }

            is CoreActivity.Play -> {
                navController.navigate("play")
            }

            is CoreActivity.Tune -> {
                navController.navigate("tune")
            }

            is CoreActivity.Listen -> {
                navController.navigate("listen")
            }

            is CoreActivity.About -> {
                navController.navigate("about")
            }
        }
    }

    LaunchedEffect(core.navigateTo) {
        if (core.navigateTo != null) {
            navigateTo(core.navigateTo!!)
            core.update(Event.ReflectActivity(core.navigateTo))
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



    BoxWithConstraints(
        modifier = Modifier
            .fillMaxSize()
    ) {
        val width = this.maxWidth
        val height = this.maxHeight

        LaunchedEffect(width, height) {

            updateConfig(width.value.toDouble(), height.value.toDouble(), safeAreas)
        }
        NavHost(navController = navController, startDestination = "intro") {
            composable("intro") {
                AppIntro(introVm, introEv)
            }
            composable("play") {
                AppInstrument(instrumentVm, instrumentEv)
            }
            composable("listen") {
                AppInstrument(instrumentVm, instrumentEv)
            }
            composable("tune") {
//                AppInstrument(tunerVm, tunerEv)
            }
            composable("about") {
                AppAbout(introVm, introEv)
            }
        }
    }
}


@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    RedSiren(viewModel())
}
