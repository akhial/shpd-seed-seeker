// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import androidx.compose.animation.core.animateFloatAsState
import androidx.compose.animation.core.spring
import androidx.compose.foundation.clickable
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.material.icons.filled.Favorite
import androidx.compose.material.icons.filled.Info
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.FilledTonalButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.Surface
import androidx.compose.runtime.mutableIntStateOf
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.graphicsLayer
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.ColumnScope
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.height
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.text.selection.SelectionContainer
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material3.Card
import androidx.compose.material3.CardDefaults
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.font.FontFamily
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import dev.seedseeker.app.BuildConfig
import dev.seedseeker.app.engine.EngineInfo

private const val LICENSE_PATH = "third_party/shattered-pixel-dungeon/LICENSE.txt"

@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun AboutScreen(onBack: () -> Unit) {
    val context = LocalContext.current
    var showLicense by remember { mutableStateOf(false) }
    val licenseText = remember(context) {
        runCatching {
            context.assets.open(LICENSE_PATH).bufferedReader().use { it.readText() }
        }.getOrElse { "License text could not be loaded: ${it.message}" }
    }

    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("About & licenses") },
                navigationIcon = {
                    IconButton(onClick = onBack, shapes = IconButtonDefaults.shapes()) {
                        Icon(Icons.AutoMirrored.Filled.ArrowBack, contentDescription = "Back")
                    }
                },
                colors = TopAppBarDefaults.topAppBarColors(
                    containerColor = MaterialTheme.colorScheme.background,
                ),
            )
        },
    ) { scaffoldPadding ->
        Box(
            modifier = Modifier
                .fillMaxSize()
                .padding(scaffoldPadding),
            contentAlignment = Alignment.TopCenter,
        ) {
            LazyColumn(
                modifier = Modifier
                    .fillMaxWidth()
                    .widthIn(max = 680.dp)
                    .navigationBarsPadding(),
                contentPadding = PaddingValues(16.dp),
                verticalArrangement = Arrangement.spacedBy(12.dp),
            ) {
                // Every passage below is quoted verbatim from README.md, minus
                // its inline link markup. Keep it that way: the app's prose is
                // the project's own, not a second description of it that can
                // drift. Section titles are the README's own headings.
                item {
                    AboutHero()
                }

                // The README's opening line sits under its "# Seed Seeker"
                // heading, so it reads as a lede here rather than as a card
                // that would repeat the title above it.
                item {
                    Text(
                        "An extremely fast seed finder for Shattered Pixel Dungeon, written in Rust — with native apps for Android, Linux, macOS, and Windows.",
                        modifier = Modifier.fillMaxWidth().padding(horizontal = 12.dp, vertical = 2.dp),
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                        textAlign = TextAlign.Center,
                    )
                }

                item {
                    AboutSection("Acknowledgements", MaterialShapes.Heart, Icons.Filled.Favorite) {
                        Text(
                            "Seed Seeker reimplements the generation of Shattered Pixel Dungeon by Evan Debenham, itself based on Pixel Dungeon by Oleg Dolya.",
                        )
                        Spacer(Modifier.height(10.dp))
                        Text(
                            "Elektrochecker's shpd-seed-finder serves as an oracle for this project's parity tests.",
                        )
                    }
                }

                item {
                    AboutSection("License and identity", MaterialShapes.Cookie4Sided, Icons.Filled.Info) {
                        Text(
                            "This project is GPL-3.0-or-later. It contains a derived generation implementation and an unchanged item sprite atlas from Shattered Pixel Dungeon.",
                        )
                        Spacer(Modifier.height(10.dp))
                        AttributionLine("Pixel Dungeon", "© 2012–2015 Oleg Dolya / Watabou")
                        AttributionLine("Shattered Pixel Dungeon", "© 2014–2026 Evan Debenham")
                        AttributionLine("Upstream", "Shattered Pixel Dungeon v${EngineInfo.shpdVersion}")
                        AttributionLine("Release JAR SHA-256", EngineInfo.shpdCommit)
                        AttributionLine("Atlas SHA-256", "4774791518f960a4…7e8e7b5706")
                        AttributionLine("Icon SHA-256", "38df728d32842d9f…24d7eb9b72")
                        Spacer(Modifier.height(8.dp))
                        FilledTonalButton(onClick = { showLicense = !showLicense }, shapes = ButtonDefaults.shapes()) {
                            Text(if (showLicense) "Hide full license" else "Read full license")
                        }
                    }
                }

                if (showLicense) {
                    item {
                        Card(
                            modifier = Modifier.fillMaxWidth(),
                            shape = MaterialTheme.shapes.large,
                            colors = CardDefaults.cardColors(
                                containerColor = MaterialTheme.colorScheme.surfaceContainerLowest,
                            ),
                        ) {
                            SelectionContainer {
                                Text(
                                    licenseText,
                                    modifier = Modifier.padding(16.dp),
                                    style = MaterialTheme.typography.bodySmall,
                                    fontFamily = FontFamily.Monospace,
                                    color = MaterialTheme.colorScheme.onSurfaceVariant,
                                )
                            }
                        }
                    }
                }

                item {
                    Text(
                        "Seed Seeker ${BuildConfig.VERSION_NAME} · Shattered Pixel Dungeon " +
                            "v${EngineInfo.shpdVersion} profile",
                        modifier = Modifier
                            .fillMaxWidth()
                            .padding(vertical = 14.dp),
                        textAlign = TextAlign.Center,
                        style = MaterialTheme.typography.labelSmall,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
            }
        }
    }
}

@Composable
private fun AboutSection(
    title: String,
    polygon: androidx.graphics.shapes.RoundedPolygon,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    content: @Composable ColumnScope.() -> Unit,
) {
    Card(
        modifier = Modifier.fillMaxWidth().springEntrance(delayMillis = 80),
        shape = MaterialTheme.shapes.extraLarge,
        colors = CardDefaults.cardColors(containerColor = MaterialTheme.colorScheme.surfaceContainerLow),
    ) {
        Column(Modifier.padding(20.dp)) {
            SectionHeading(title, polygon, icon)
            Spacer(Modifier.height(12.dp))
            content()
        }
    }
}

/**
 * The brand, centre stage: the launcher mark on a sunburst. Tap it and it
 * spins a full turn, swells and throws sparkles — the one purely joyful
 * control in the app.
 */
@OptIn(ExperimentalMaterial3ExpressiveApi::class)
@Composable
private fun AboutHero() {
    var taps by remember { mutableIntStateOf(0) }
    val spin by animateFloatAsState(
        targetValue = taps * 360f,
        animationSpec = spring(dampingRatio = 0.55f, stiffness = 120f),
        label = "brand-spin",
    )
    Column(
        Modifier.fillMaxWidth().padding(top = 8.dp, bottom = 4.dp),
        horizontalAlignment = Alignment.CenterHorizontally,
    ) {
        ShapeBackdrop(
            MaterialShapes.Sunny,
            MaterialTheme.colorScheme.primaryContainer,
            Modifier
                .size(148.dp)
                .celebrate(taps, CelebrationColors, count = 22, reach = 90f)
                .popOnChange(taps, peak = 1.12f)
                .clip(CircleShape)
                .clickable(onClickLabel = "Spin the seal") { taps++ },
        ) {
            BrandMark(Modifier.size(92.dp).graphicsLayer { rotationZ = spin })
        }
        Spacer(Modifier.height(14.dp))
        Text("Seed Seeker", style = MaterialTheme.typography.headlineMedium, fontWeight = FontWeight.ExtraBold)
        Spacer(Modifier.height(6.dp))
        Surface(shape = CircleShape, color = MaterialTheme.colorScheme.tertiaryContainer) {
            Text(
                "v${BuildConfig.VERSION_NAME}",
                modifier = Modifier.padding(horizontal = 12.dp, vertical = 4.dp),
                style = MaterialTheme.typography.labelLarge,
                color = MaterialTheme.colorScheme.onTertiaryContainer,
            )
        }
    }
}

@Composable
private fun AttributionLine(label: String, value: String) {
    Column(Modifier.padding(vertical = 4.dp)) {
        Text(label, style = MaterialTheme.typography.labelSmall, color = MaterialTheme.colorScheme.primary)
        Text(value, style = MaterialTheme.typography.bodySmall, color = MaterialTheme.colorScheme.onSurfaceVariant)
    }
}
