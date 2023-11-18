package com.anvlkv.redsiren.app

import android.animation.TimeAnimator
import android.content.ContentResolver
import android.content.res.Resources
import android.provider.Settings
import android.util.Log
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.draw.rotate
import androidx.compose.ui.draw.scale
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.Dp
import com.anvlkv.redsiren.R
import com.anvlkv.redsiren.shared_types.IntroEV
import com.anvlkv.redsiren.shared_types.IntroVM
import kotlinx.coroutines.launch

fun isReducedMotionEnabled(resolver: ContentResolver): Boolean {
    val animationDuration = try {
        Settings.Global.getFloat(resolver, Settings.Global.ANIMATOR_DURATION_SCALE)
    } catch (e: Settings.SettingNotFoundException) {
        1f
    }
    return animationDuration == 0f
}


@Composable
fun AppIntro(vm: IntroVM, ev: (ev: IntroEV) -> Unit) {
    val coroutineScope = rememberCoroutineScope()
    val sun = ImageVector.vectorResource(id = R.drawable.intro_sun)
    val waves = ImageVector.vectorResource(id = R.drawable.intro_shine)
    val sirenComp = ImageVector.vectorResource(id = R.drawable.intro_siren)

    val sunPainter = rememberVectorPainter(image = sun)
    val wavesPainter = rememberVectorPainter(image = waves)
    val sirenPainter = rememberVectorPainter(image = sirenComp)

    val reducedMotion = isReducedMotionEnabled(LocalContext.current.contentResolver)

    LaunchedEffect(Unit, reducedMotion, sirenPainter) {
        val listener = fun(_: TimeAnimator, time: Long, _: Long) {
            coroutineScope.launch {
                ev(IntroEV.TsNext(time.toDouble()))
            }
        }
        val animator = TimeAnimator()

        animator.setTimeListener(listener)
        ev(IntroEV.StartAnimation(0.0, reducedMotion))
        animator.start()
    }

    Box(modifier = Modifier.fillMaxSize()) {
        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity)
                .size(Dp(vm.view_box.rect[1][0]), Dp(vm.view_box.rect[1][1]))
                .align(Alignment.BottomEnd)

        ) {
            Box(
                modifier = Modifier
                    .graphicsLayer(
                        rotationZ = vm.flute_rotation[2],
                        // TODO: Here I lost patience...
                        // FIXME: landscape tablet shows flute with offset...
                        transformOrigin = TransformOrigin(
                            vm.flute_rotation[0] / vm.view_box.rect[1][0],
                            vm.flute_rotation[1] / vm.view_box.rect[1][1]
                        ),
                    )
                    .absoluteOffset(Dp(vm.flute_position[0]), Dp(vm.flute_position[1]))
            ) {
                InstrumentInboundString(layoutLine = vm.layout.inbound)
                InstrumentOutboundString(layoutLine = vm.layout.outbound)
            }
        }

        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity)
                .size(Dp(vm.view_box.rect[1][0]), Dp(vm.view_box.rect[1][1]))
        ) {
            vm.layout.tracks.forEach { rect ->
                InstrumentTrack(layoutRect = rect)
            }
        }

        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity)
                .absoluteOffset(Dp(vm.buttons_position[0]), Dp(vm.buttons_position[1]))
                .size(Dp(vm.view_box.rect[1][0]), Dp(vm.view_box.rect[1][1]))
        ) {
            vm.layout.buttons.forEach { rect ->
                InstrumentButton(layoutRect = rect)
            }
        }

        Box(
            modifier = Modifier
                .alpha(vm.intro_opacity)
                .fillMaxSize()
        ) {

            Canvas(
                modifier = Modifier
                    .fillMaxSize()
                    .align(Alignment.TopStart)
            ) {
                with(sunPainter) {
                    draw(intrinsicSize)
                }
            }



            Box(
                modifier = Modifier
                    .fillMaxSize()
                    .align(Alignment.BottomStart)
                    .blur(Dp(1F))
            ) {
                Image(
                    painter = wavesPainter,
                    contentDescription = "Sun reflecting on water",
                    modifier = Modifier.align(Alignment.BottomStart)
                )
            }

            Box(
                modifier = Modifier.fillMaxSize()

            ) {
                Image(
                    painter = sirenPainter,
                    contentDescription = "Siren playing on a flute",
                    modifier = Modifier.align(Alignment.BottomEnd)
                )
            }

        }
    }

}
