package com.anvlkv.redsiren.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.blur
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.vectorResource
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.MainActivity
import com.anvlkv.redsiren.R


@Composable
fun AppIntro() {
    val activity = LocalContext.current as MainActivity

    val vm = activity.core!!.view.visual


    Box(modifier = Modifier.fillMaxSize()) {
        IntroDrawing(
            modifier = Modifier
                .alpha(vm.intro_opacity.toFloat())
                .fillMaxSize()
        )
    }
}


@Composable
fun IntroDrawing(modifier: Modifier) {
    val sun = ImageVector.vectorResource(id = R.drawable.intro_sun)
    val waves = ImageVector.vectorResource(id = R.drawable.intro_shine)
    val sirenComp = ImageVector.vectorResource(id = R.drawable.intro_siren)
    val sunPainter = rememberVectorPainter(image = sun)
    val wavesPainter = rememberVectorPainter(image = waves)
    val sirenPainter = rememberVectorPainter(image = sirenComp)

    Box(
        modifier = modifier
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

        Canvas(
            modifier = Modifier
                .align(Alignment.BottomStart)
                .blur(1F.dp)
                .fillMaxSize()
        ) {
            with(wavesPainter) {
                draw(intrinsicSize)
            }
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