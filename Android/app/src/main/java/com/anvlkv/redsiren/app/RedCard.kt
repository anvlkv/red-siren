package com.anvlkv.redsiren.app

import android.content.res.Resources
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.TransformOrigin
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.unit.DpSize
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.core.typegen.MenuPosition

@Composable
fun RedCard(
    modifier: Modifier,
    flip: Double?,
    position: MenuPosition,
    block: @Composable() (ColumnScope.() -> Unit)
) {
    val shape = MaterialTheme.shapes.extraLarge
    val backgroundColor = MaterialTheme.colorScheme.primary

    val menuRect = when (position) {
        is MenuPosition.TopRight -> position.value
        is MenuPosition.TopLeft -> position.value
        is MenuPosition.BottomLeft -> position.value
        is MenuPosition.Center -> position.value
        else -> throw Error("unknown position")
    }

    val menuSize = DpSize(
        (menuRect.rect[1][0] - menuRect.rect[0][0]).dp,
        (menuRect.rect[1][1] - menuRect.rect[0][1]).dp
    )
    val density = Resources.getSystem().displayMetrics.density

    Box(
        Modifier.graphicsLayer(
            translationX = (menuRect.rect[0][0] * density.toDouble()).toFloat(),
            translationY = (menuRect.rect[0][1] * density.toDouble()).toFloat(),
        )
    ) {
        Card(
            Modifier
                .graphicsLayer(
                    rotationY = flip?.toFloat() ?: 0F,
                    transformOrigin = TransformOrigin.Center
                )
                .size(menuSize)
                .then(modifier),
            shape = shape
        ) {
            Column(
                Modifier
                    .fillMaxSize()
                    .background(backgroundColor)
                    .padding(12.dp),
                verticalArrangement = Arrangement.spacedBy(24.dp),
                horizontalAlignment = Alignment.CenterHorizontally
            ) {
                if (flip == null || flip < 90 || flip > 270) {
                    block()
                }
            }
        }
    }

}