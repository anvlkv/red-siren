package com.anvlkv.redsiren.ui.theme

import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.MaterialTheme
import androidx.compose.runtime.Composable


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
        content = content,
        typography = typography
    )
}