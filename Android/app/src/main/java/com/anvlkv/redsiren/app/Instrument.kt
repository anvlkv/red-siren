package com.anvlkv.redsiren.app

import android.content.res.Resources
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.shared.shared_types.InstrumentEV
import com.anvlkv.redsiren.shared.shared_types.InstrumentVM
import com.anvlkv.redsiren.shared.shared_types.Line
import com.anvlkv.redsiren.shared.shared_types.MenuPosition
import com.anvlkv.redsiren.shared.shared_types.Rect
import kotlin.math.min


@Composable
fun InstrumentButton(layoutRect: Rect) {
    val color = MaterialTheme.colorScheme.primary
    Canvas(
        modifier = Modifier
            .width((layoutRect.rect[1][0] - layoutRect.rect[0][0]).dp)
            .height((layoutRect.rect[1][1] - layoutRect.rect[0][1]).dp)
            .absoluteOffset(
                (layoutRect.rect[0][0]).dp,
                (layoutRect.rect[0][1]).dp,
            )
    ) {
        drawCircle(color = color, style = Fill)
    }
}

@Composable
fun InstrumentInboundString(layoutLine: Line) {
    InstrumentString(layoutLine)
}

@Composable
fun InstrumentOutboundString(layoutLine: Line) {
    InstrumentString(layoutLine)
}

@Composable
fun InstrumentString(
    layoutLine: Line
) {
    val color = MaterialTheme.colorScheme.primary
    Canvas(
        modifier = Modifier.fillMaxSize(),

        ) {
        val path = Path()
        path.moveTo(layoutLine.line[0][0].dp.toPx(), layoutLine.line[0][1].dp.toPx())
        path.lineTo(layoutLine.line[1][0].dp.toPx(), layoutLine.line[1][1].dp.toPx())
        drawPath(
            color = color,
            style = Stroke(1F * this.density),
            path = path,
        )
    }
}


@Composable
fun InstrumentTrack(layoutRect: Rect) {
    val color = MaterialTheme.colorScheme.primary
    val backgroundColor = MaterialTheme.colorScheme.background



    Canvas(
        modifier = Modifier
            .width((layoutRect.rect[1][0] - layoutRect.rect[0][0]).dp)
            .height((layoutRect.rect[1][1] - layoutRect.rect[0][1]).dp)
            .absoluteOffset(
                layoutRect.rect[0][0].dp,
                layoutRect.rect[0][1].dp,
            )
    ) {
        val r = min(
            layoutRect.rect[1][0] - layoutRect.rect[0][0],
            layoutRect.rect[1][1] - layoutRect.rect[0][1]
        ).toFloat() * this.density
        drawRoundRect(color = backgroundColor, style = Fill, cornerRadius = CornerRadius(r, r))
        drawRoundRect(color = color, style = Stroke(1F.dp.toPx()), cornerRadius = CornerRadius(r, r))
    }
}

@Composable
fun AppInstrument(vm: InstrumentVM, ev: (ev: InstrumentEV) -> Unit) {
    Box (
        Modifier
            .fillMaxSize()
            .clipToBounds()) {
        InstrumentInboundString(layoutLine = vm.layout.inbound)
        InstrumentOutboundString(layoutLine = vm.layout.outbound)

        vm.layout.tracks.forEach { rect ->
            InstrumentTrack(layoutRect = rect)
        }

        vm.layout.buttons.forEach { rect ->
            InstrumentButton(layoutRect = rect)
        }
    }

    val menuRect = when (val position = vm.layout.menu_position) {
        is MenuPosition.TopRight -> position.value
        is MenuPosition.TopLeft -> position.value
        is MenuPosition.BottomLeft -> position.value
        is MenuPosition.Center -> position.value
        else -> throw Error("unknown position")
    }

    val menuSize =
        DpSize((menuRect.rect[1][0] - menuRect.rect[0][0]).dp, (menuRect.rect[1][1] - menuRect.rect[0][1]).dp)
    val density = Resources.getSystem().displayMetrics.density

    Box(
        Modifier
            .size(menuSize)
            .graphicsLayer(
                translationX = (menuRect.rect[0][0] * density.toDouble()).toFloat(),
                translationY = (menuRect.rect[0][1] * density.toDouble()).toFloat(),
            )
    ) {
        Menu(false)
    }
}

