package com.anvlkv.redsiren.ui.theme

import androidx.compose.material3.Typography
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.Font
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontStyle
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp
import com.anvlkv.redsiren.R

val appFontFamily = FontFamily(
    fonts = listOf(
        Font(
            resId = R.font.rosarivo_regular,
            style = FontStyle.Normal
        ),
        Font(
            resId = R.font.rosarivo_italic,
            style = FontStyle.Italic
        ),
    )
)

// Set of Material typography styles to start with
val typography = Typography(
    bodyMedium = TextStyle(
        fontFamily = appFontFamily,
        fontStyle = FontStyle.Italic,
        fontSize = 18.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.5.sp
    ),
    bodySmall = TextStyle(
        fontFamily = appFontFamily,
        fontSize = 16.sp,
        lineHeight = 19.2.sp,
        letterSpacing = 0.5.sp
    ),
    bodyLarge = TextStyle(
        fontFamily = appFontFamily,
        fontSize = 18.sp,
        lineHeight = 24.sp,
        letterSpacing = 0.5.sp
    ),
    titleLarge = TextStyle(
        fontFamily = appFontFamily,
        fontSize = 36.sp,
        lineHeight = 42.sp,
        letterSpacing = 0.sp
    ),
    titleMedium = TextStyle(
        fontFamily = appFontFamily,
        fontSize = 30.7.sp,
        fontStyle = FontStyle.Italic,
        lineHeight = 42.sp,
        letterSpacing = 0.sp
    ),
    /* Other default text styles to override
    labelSmall = TextStyle(
        fontFamily = FontFamily.Default,
        fontWeight = FontWeight.Medium,
        fontSize = 11.sp,
        lineHeight = 16.sp,
        letterSpacing = 0.5.sp
    )
    */
)