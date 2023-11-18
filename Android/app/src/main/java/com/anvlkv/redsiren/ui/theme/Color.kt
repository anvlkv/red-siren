package com.anvlkv.redsiren.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color

val Red = Color(0xFFE30022)
val Black = Color(0xFF353839)
val Gray = Color(0xFF36454F)
val MutedRed = Color(0xFFE44D2E)

private val darkColors = darkColorScheme(
    primary = Red,
    background = Black,
    secondary = MutedRed,
    surface = Black,
)

private val lightColors = lightColorScheme(
    primary = Black,
    background = Red,
    secondary = Gray,
    surface = Red,
)

@Composable
fun ApplyTheme(
    useDarkTheme: Boolean = isSystemInDarkTheme(),
    content: @Composable () -> Unit
) {
    val colorScheme =
        if (useDarkTheme) {
            darkColors
        } else {
            lightColors
        }

    MaterialTheme(
        colorScheme = colorScheme,
        content = content
    )
}
