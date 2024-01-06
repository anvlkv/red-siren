package com.anvlkv.redsiren.app

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.core.typegen.InstrumentEV
import com.anvlkv.redsiren.core.typegen.InstrumentVM
import com.anvlkv.redsiren.core.typegen.Line
import com.anvlkv.redsiren.core.typegen.Rect
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

        Menu(false, flip = null, position = vm.layout.menu_position)
    }

}

