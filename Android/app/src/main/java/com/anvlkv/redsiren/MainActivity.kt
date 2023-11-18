package com.anvlkv.redsiren

import android.content.res.Resources
import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.Surface
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.lifecycle.viewmodel.compose.viewModel
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.anvlkv.redsiren.app.AppInstrument
import com.anvlkv.redsiren.app.AppIntro
import com.anvlkv.redsiren.shared_types.Event
import com.anvlkv.redsiren.shared_types.InstrumentEV
import com.anvlkv.redsiren.shared_types.IntroEV
import com.anvlkv.redsiren.shared_types.TunerEV
import com.anvlkv.redsiren.ui.theme.ApplyTheme
import kotlinx.coroutines.launch
import com.anvlkv.redsiren.shared_types.Activity as CoreActivity

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val windowInsetsController = WindowCompat.getInsetsController(window, window.decorView)
        windowInsetsController.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        windowInsetsController.hide(WindowInsetsCompat.Type.systemBars())


        setContent {
            ApplyTheme(content = {
                Surface {
                    RedSiren()
                }
            })
        }
    }
}


@Composable
fun RedSiren() {
    val navController = rememberNavController()


    RedSirenNavHost(navController)
}

@Composable
fun RedSirenNavHost(
    navController: NavHostController, core: Core = viewModel()
) {
    val coroutineScope = rememberCoroutineScope()


    fun updateConfig(width: Float, height: Float) {
        val density = Resources.getSystem().displayMetrics.density

        coroutineScope.launch {
            core.update(Event.CreateConfigAndConfigureApp(width, height, density))
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

    fun navigateTo (act: CoreActivity) {
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
        }
    }

    val navigationTarget = core.navigateTo

    LaunchedEffect(navigationTarget) {
        if (navigationTarget.isPresent) {
            val activity = navigationTarget.get()
            navigateTo(activity)
            core.update(Event.Activate(activity))
        }
    }



    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val width = this.maxWidth
        val height = this.maxHeight

        LaunchedEffect(width, height) {
            updateConfig(width.value, height.value)
        }
        NavHost(navController = navController, startDestination = "intro") {
            composable("intro") {
                AppIntro(vm = introVm, ev = introEv)
            }
            composable("play") {
                AppInstrument(vm = instrumentVm, ev = instrumentEv)
            }
            composable("listen") {

            }
            composable("tune") {

            }
        }
    }
}

@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    RedSiren()
}
