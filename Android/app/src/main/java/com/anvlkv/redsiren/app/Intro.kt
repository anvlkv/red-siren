package com.anvlkv.redsiren.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.vector.ImageVector
import androidx.compose.ui.graphics.vector.rememberVectorPainter
import androidx.compose.ui.res.vectorResource
import com.anvlkv.redsiren.R
import com.anvlkv.redsiren.shared_types.IntroEV
import com.anvlkv.redsiren.shared_types.IntroVM


@Composable
fun AppIntro(vm: IntroVM, ev: (ev: IntroEV) -> Unit) {
    val sun = ImageVector.vectorResource(id = R.drawable.intro_sun)
    val waves = ImageVector.vectorResource(id = R.drawable.intro_shine)
    val sirenComp = ImageVector.vectorResource(id = R.drawable.intro_siren)

    val sunPainter = rememberVectorPainter(image = sun)
    val wavesPainter = rememberVectorPainter(image = waves)
    val sirenPainter = rememberVectorPainter(image = sirenComp)

    Box {

        Canvas(modifier = Modifier
            .fillMaxSize()
            .align(Alignment.TopStart)) {
            with(sunPainter) {
                draw(intrinsicSize)
            }
        }


        Canvas(modifier = Modifier
            .fillMaxSize()
            .align(Alignment.BottomStart)) {

            with(wavesPainter) {
                draw(intrinsicSize)
            }
        }

        Box(
            modifier = Modifier
                .fillMaxSize()

        ) {
            Image(
                painter = sirenPainter,
                contentDescription = "Siren playing on a flute",
                modifier = Modifier
                    .align(Alignment.BottomEnd)
            )
        }

    }
}
