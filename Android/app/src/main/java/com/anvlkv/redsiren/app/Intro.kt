package com.anvlkv.redsiren.app

import android.animation.TimeAnimator
import android.content.ContentResolver
import android.content.res.Resources
import android.provider.Settings
import android.util.Log
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.defaultMinSize
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
import androidx.compose.ui.unit.dp
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
    val density = Resources.getSystem().displayMetrics.density

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



    BoxWithConstraints(modifier = Modifier.fillMaxSize()) {
        val scaleX = this.maxWidth.value.toDouble() / vm.view_box.rect[1][0]
        val scaleY = this.maxHeight.value.toDouble() / vm.view_box.rect[1][1]

        Log.d("scaleX", scaleX.toString())
        Log.d("scaleY", scaleY.toString())

        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity.toFloat())
                .size(vm.view_box.rect[1][0].dp, vm.view_box.rect[1][1].dp)
                .align(Alignment.BottomEnd),
            contentAlignment = Alignment.BottomEnd
        ) {
            Box(
                modifier = Modifier
                    .graphicsLayer(
                        rotationZ = vm.flute_rotation[2].toFloat(),
                        // TODO: Here I lost patience...
                        // FIXME: landscape tablet shows flute with offset...
                        transformOrigin = TransformOrigin(
                            (vm.flute_rotation[0] / vm.view_box.rect[1][0] * scaleX).toFloat(),
                            (vm.flute_rotation[1] / vm.view_box.rect[1][1] * scaleY).toFloat()
                        ),
                    )
                    .graphicsLayer(
                        translationX = vm.flute_position[0].toFloat() * density * scaleX.toFloat(),
                        translationY = vm.flute_position[1].toFloat() * density * scaleY.toFloat()
                    )


            ) {
                InstrumentInboundString(layoutLine = vm.layout.inbound)
                InstrumentOutboundString(layoutLine = vm.layout.outbound)
            }
        }

        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity.toFloat())
                .size(vm.view_box.rect[1][0].dp, vm.view_box.rect[1][1].dp)
        ) {
            vm.layout.tracks.forEach { rect ->
                InstrumentTrack(layoutRect = rect)
            }
        }

        Box(
            modifier = Modifier
                .alpha(1 - vm.intro_opacity.toFloat())
                .absoluteOffset(vm.buttons_position[0].dp, vm.buttons_position[1].dp)
                .size(vm.view_box.rect[1][0].dp, vm.view_box.rect[1][1].dp)
        ) {
            vm.layout.buttons.forEach { rect ->
                InstrumentButton(layoutRect = rect)
            }
        }

        Box(
            modifier = Modifier
                .alpha(vm.intro_opacity.toFloat() + 0.1F)
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
                    .defaultMinSize(minWidth = 712.dp)
                    .align(Alignment.BottomStart)
                    .blur(1F.dp)
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
