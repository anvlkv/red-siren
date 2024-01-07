package com.anvlkv.redsiren.app

import android.util.Log
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.core.typegen.Line
import com.anvlkv.redsiren.core.typegen.TunerEV
import com.anvlkv.redsiren.core.typegen.TunerVM


@Composable
fun TunerFFT(
    layoutLine: Line,
    data: List<List<Float>>
) {
    val color = MaterialTheme.colorScheme.primary
    Canvas(
        modifier = Modifier.fillMaxSize(),
    ) {
        val path = Path()
        val step = (layoutLine.line[1][0] - layoutLine.line[0][0]) / data.size.toDouble()

        path.moveTo(layoutLine.line[0][0].dp.toPx(), layoutLine.line[0][1].dp.toPx())

        for (index in data.indices) {
            val x = layoutLine.line[0][0] + step * index.toDouble()
            val y = layoutLine.line[0][1] + data[index][1]
            path.lineTo(x.dp.toPx(), y.dp.toPx())
        }

        path.lineTo(layoutLine.line[1][0].dp.toPx(), layoutLine.line[1][1].dp.toPx())

        Log.d("fft draw", data.toString())

        drawPath(
            color = color,
            style = Stroke(1F * this.density),
            path = path,
        )
    }
}

@Composable
fun AppTuner(vm: TunerVM, ev: (ev: TunerEV) -> Unit) {
    Box(
        Modifier
            .fillMaxSize()
            .clipToBounds()
    ) {
        TunerFFT(layoutLine = vm.line, data = vm.fft)
        vm.pairs.forEach { pair ->
            InstrumentButton(layoutRect = pair.rect)
        }
    }

}

