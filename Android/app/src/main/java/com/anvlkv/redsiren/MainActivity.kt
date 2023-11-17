package com.anvlkv.redsiren

import android.app.Activity
import android.content.res.Configuration
import android.os.Bundle
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.platform.LocalConfiguration
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.tooling.preview.Preview
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import androidx.navigation.NavHostController
import androidx.navigation.compose.NavHost
import androidx.navigation.compose.composable
import androidx.navigation.compose.rememberNavController
import com.anvlkv.redsiren.app.AppIntro
import com.anvlkv.redsiren.shared_types.Event
import com.anvlkv.redsiren.shared_types.InstrumentEV
import com.anvlkv.redsiren.shared_types.IntroEV
import com.anvlkv.redsiren.shared_types.TunerEV
import getScreenHeight
import getScreenWidth
import kotlinx.coroutines.launch
import com.anvlkv.redsiren.shared_types.Activity as CoreActivity

class MainActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val windowInsetsController =
            WindowCompat.getInsetsController(window, window.decorView)
        windowInsetsController.systemBarsBehavior =
            WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        windowInsetsController.hide(WindowInsetsCompat.Type.systemBars())


        setContent {
            RedSiren()
        }
    }
}


@Composable
fun RedSiren() {
    val navController = rememberNavController()
    val navigate_to = remember {
        fun(act: CoreActivity) {
            when (val act = act) {
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
    }
    val core = remember {
        Core(navigate_to = navigate_to)
    }




    RedSirenNavHost(navController = navController, core = core)
}

@Composable
fun RedSirenNavHost(
    navController: NavHostController,
    core: Core
) {
    val coroutineScope = rememberCoroutineScope()
    val width = getScreenWidth()
    val height = getScreenHeight()
    val devicePortrait = height > width
    val configuration = LocalConfiguration.current

    fun updateConfig(width: Float, height: Float) {
        coroutineScope.launch {
            val flip =
                (configuration.orientation == Configuration.ORIENTATION_LANDSCAPE && devicePortrait) || (
                        configuration.orientation == Configuration.ORIENTATION_PORTRAIT && !devicePortrait
                        )


            if (flip) {
                core.update(Event.CreateConfigAndConfigureApp(height, width))
            } else {
                core.update(Event.CreateConfigAndConfigureApp(width, height))
            }
        }
    }

    LaunchedEffect(width, height) {
        updateConfig(width.toFloat(), height.toFloat())
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


    val activity = (LocalContext.current as Activity)
    NavHost(navController = navController, startDestination = "intro") {
        composable("intro") {
            AppIntro(vm = introVm, ev = introEv)
        }
        composable("play") {

        }
        composable("listen") {

        }
        composable("tune") {

        }
    }
}

@Preview(showBackground = true)
@Composable
fun DefaultPreview() {
    RedSiren()
}
