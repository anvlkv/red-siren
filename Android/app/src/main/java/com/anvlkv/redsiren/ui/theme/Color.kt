package com.anvlkv.redsiren.ui.theme

import androidx.compose.material3.darkColorScheme
import androidx.compose.material3.lightColorScheme
import androidx.compose.ui.graphics.Color

val Red = Color(0xFFE30022)
val Black = Color(0xFF353839)
val Gray = Color(0xFF36454F)
val MutedRed = Color(0xFFE44D2E)

val darkColors = darkColorScheme(
    primary = Red,
    background = Black,
    secondary = MutedRed,
    surface = Black,
)

val lightColors = lightColorScheme(
    primary = Black,
    background = Red,
    secondary = Gray,
    surface = Red,
)

