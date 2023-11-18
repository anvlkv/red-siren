package com.anvlkv.redsiren.app

import android.content.res.Resources
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.absoluteOffset
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.width
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.geometry.CornerRadius
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Fill
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.Dp
import com.anvlkv.redsiren.shared_types.InstrumentEV
import com.anvlkv.redsiren.shared_types.InstrumentVM
import com.anvlkv.redsiren.shared_types.Line
import com.anvlkv.redsiren.shared_types.Rect
import kotlin.math.min

@Composable
fun InstrumentButton(layoutRect: Rect) {
    val color = MaterialTheme.colorScheme.primary
    Canvas(
        modifier = Modifier
            .width(Dp(layoutRect.rect[1][0] - layoutRect.rect[0][0]))
            .height(Dp(layoutRect.rect[1][1] - layoutRect.rect[0][1]))
            .absoluteOffset(
                Dp(layoutRect.rect[0][0]),
                Dp(layoutRect.rect[0][1]),
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
        path.moveTo(layoutLine.line[0][0] * this.density, layoutLine.line[0][1] * this.density)
        path.lineTo(layoutLine.line[1][0] * this.density, layoutLine.line[1][1] * this.density)
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
    val density = Resources.getSystem().displayMetrics.density

    val r = min(
        layoutRect.rect[1][0] - layoutRect.rect[0][0],
        layoutRect.rect[1][1] - layoutRect.rect[0][1]
    ) * density

    Canvas(
        modifier = Modifier
            .width(Dp(layoutRect.rect[1][0] - layoutRect.rect[0][0]))
            .height(Dp(layoutRect.rect[1][1] - layoutRect.rect[0][1]))
            .absoluteOffset(
                Dp(layoutRect.rect[0][0]),
                Dp(layoutRect.rect[0][1]),
            )
    ) {
        drawRoundRect(color = backgroundColor, style = Fill, cornerRadius = CornerRadius(r, r))
        drawRoundRect(color = color, style = Stroke(1F * density), cornerRadius = CornerRadius(r, r))
    }
}

@Composable
fun AppInstrument(vm: InstrumentVM, ev: (ev: InstrumentEV) -> Unit) {
    Box (Modifier.fillMaxSize()) {
        InstrumentInboundString(layoutLine = vm.layout.inbound)
        InstrumentOutboundString(layoutLine = vm.layout.outbound)

        vm.layout.tracks.forEach { rect ->
            InstrumentTrack(layoutRect = rect)
        }

        vm.layout.buttons.forEach { rect ->
            InstrumentButton(layoutRect = rect)
        }
    }
}

