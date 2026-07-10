// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui.theme

import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.darkColorScheme
import androidx.compose.runtime.Composable
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.text.TextStyle
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.sp

val Ink = Color(0xFF0C1212)
val DeepMoss = Color(0xFF121B1A)
val RaisedMoss = Color(0xFF1A2624)
val Mint = Color(0xFF7DE2B8)
val MintDark = Color(0xFF153D31)
val Amber = Color(0xFFFFC96B)
val Cream = Color(0xFFF2F5EA)
val Muted = Color(0xFFAAB8B0)
val ErrorCoral = Color(0xFFFFB4A9)

private val SeedSeekerColors = darkColorScheme(
    primary = Mint,
    onPrimary = Color(0xFF003829),
    primaryContainer = MintDark,
    onPrimaryContainer = Color(0xFFB7F4D8),
    secondary = Amber,
    onSecondary = Color(0xFF432C00),
    secondaryContainer = Color(0xFF3B2D13),
    onSecondaryContainer = Color(0xFFFFDEA1),
    background = Ink,
    onBackground = Cream,
    surface = DeepMoss,
    onSurface = Cream,
    surfaceVariant = RaisedMoss,
    onSurfaceVariant = Muted,
    outline = Color(0xFF53635D),
    outlineVariant = Color(0xFF2D3A37),
    error = ErrorCoral,
    onError = Color(0xFF690005),
)

private val SeedSeekerTypography
    @Composable get() = MaterialTheme.typography.copy(
        headlineLarge = TextStyle(
            fontFamily = FontFamily.SansSerif,
            fontWeight = FontWeight.Bold,
            fontSize = 32.sp,
            lineHeight = 38.sp,
            letterSpacing = (-0.6).sp,
        ),
        headlineSmall = MaterialTheme.typography.headlineSmall.copy(fontWeight = FontWeight.Bold),
        titleLarge = MaterialTheme.typography.titleLarge.copy(fontWeight = FontWeight.SemiBold),
        titleMedium = MaterialTheme.typography.titleMedium.copy(fontWeight = FontWeight.SemiBold),
        labelLarge = MaterialTheme.typography.labelLarge.copy(fontWeight = FontWeight.SemiBold),
    )

@Composable
fun SeedSeekerTheme(content: @Composable () -> Unit) {
    MaterialTheme(
        colorScheme = SeedSeekerColors,
        typography = SeedSeekerTypography,
        content = content,
    )
}
