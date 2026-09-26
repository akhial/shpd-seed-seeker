// SPDX-License-Identifier: GPL-3.0-or-later
package dev.seedseeker.app.ui

import android.content.Intent
import android.net.Uri
import android.provider.Settings
import androidx.compose.animation.animateContentSize
import androidx.compose.foundation.interaction.MutableInteractionSource
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.PaddingValues
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.Spacer
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.navigationBarsPadding
import androidx.compose.foundation.layout.padding
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.layout.width
import androidx.compose.foundation.layout.widthIn
import androidx.compose.foundation.lazy.LazyColumn
import androidx.compose.foundation.selection.toggleable
import androidx.compose.material.icons.Icons
import androidx.compose.material.icons.automirrored.filled.ArrowBack
import androidx.compose.material.icons.filled.Check
import androidx.compose.material.icons.filled.Edit
import androidx.compose.material.icons.filled.Lock
import androidx.compose.material3.ExperimentalMaterial3Api
import androidx.compose.material3.ExperimentalMaterial3ExpressiveApi
import androidx.compose.material3.Icon
import androidx.compose.material3.IconButton
import androidx.compose.material3.IconButtonDefaults
import androidx.compose.material3.MaterialShapes
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Scaffold
import androidx.compose.material3.Surface
import androidx.compose.material3.Switch
import androidx.compose.material3.Text
import androidx.compose.material3.TopAppBar
import androidx.compose.material3.TopAppBarDefaults
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.painterResource
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.text.font.FontWeight
import androidx.compose.ui.unit.dp
import androidx.graphics.shapes.RoundedPolygon
import dev.seedseeker.app.R
import dev.seedseeker.app.catalog.ItemCatalog
import dev.seedseeker.app.model.ArcaneResinFilter
import dev.seedseeker.app.model.ItemKind
import dev.seedseeker.app.model.ItemRequirement

/** Appearance and Android preferences; query controls live in Search settings. */
@OptIn(ExperimentalMaterial3Api::class, ExperimentalMaterial3ExpressiveApi::class)
@Composable
fun SettingsScreen(
    compactChips: Boolean,
    onCompactChipsChange: (Boolean) -> Unit,
    onBack: () -> Unit,
) {
    val context = LocalContext.current
    Scaffold(
        containerColor = MaterialTheme.colorScheme.background,
        topBar = {
            TopAppBar(
                title = { Text("App settings", fontWeight = FontWeight.ExtraBold) },
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
                verticalArrangement = Arrangement.spacedBy(10.dp),
            ) {
                item {
                    SectionHeading("Appearance", MaterialShapes.Flower, Icons.Filled.Edit)
                }
                item {
                    Surface(
                        shape = MaterialTheme.shapes.extraLarge,
                        color = MaterialTheme.colorScheme.surfaceContainerLow,
                        modifier = Modifier.springEntrance(delayMillis = 40),
                    ) {
                        Column {
                            Row(
                                modifier = Modifier
                                    .fillMaxWidth()
                                    .toggleable(
                                        value = compactChips,
                                        role = Role.Switch,
                                        onValueChange = onCompactChipsChange,
                                    )
                                    .padding(horizontal = 20.dp, vertical = 16.dp),
                                verticalAlignment = Alignment.CenterVertically,
                            ) {
                                Column(Modifier.weight(1f)) {
                                    Text("Compact chips", style = MaterialTheme.typography.titleMedium)
                                    Text(
                                        "Shorter chips with smaller sprites and text, " +
                                            "so more of the board fits on a line.",
                                        style = MaterialTheme.typography.bodySmall,
                                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                                    )
                                }
                                Spacer(Modifier.width(12.dp))
                                Switch(
                                    checked = compactChips,
                                    onCheckedChange = null,
                                    thumbContent = if (compactChips) {
                                        { Icon(Icons.Filled.Check, contentDescription = null, modifier = Modifier.size(16.dp)) }
                                    } else null,
                                )
                            }
                            // A live sample of the board, springing between sizes as the switch flips.
                            ChipPreview(compactChips)
                        }
                    }
                }
                item {
                    SectionHeading(
                        "Background search",
                        MaterialShapes.Cookie6Sided,
                        Icons.Filled.Lock,
                        modifier = Modifier.padding(top = 14.dp),
                    )
                }
                item {
                    Text(
                        "Search continues with the screen locked. If battery optimization pauses it, " +
                            "allow unrestricted battery use for Seed Seeker in Android settings. " +
                            "This uses more battery. Interrupted searches continue when you reopen the app.",
                        modifier = Modifier.padding(horizontal = 4.dp),
                        style = MaterialTheme.typography.bodyMedium,
                        color = MaterialTheme.colorScheme.onSurfaceVariant,
                    )
                }
                item {
                    val interaction = remember { MutableInteractionSource() }
                    Surface(
                        onClick = {
                            runCatching {
                                context.startActivity(Intent(Settings.ACTION_IGNORE_BATTERY_OPTIMIZATION_SETTINGS))
                            }.onFailure {
                                context.startActivity(Intent(Settings.ACTION_APPLICATION_DETAILS_SETTINGS,
                                    Uri.parse("package:${context.packageName}")))
                            }
                        },
                        interactionSource = interaction,
                        modifier = Modifier.fillMaxWidth().pressScale(interaction, pressed = 0.97f),
                        shape = MaterialTheme.shapes.extraLarge,
                        color = MaterialTheme.colorScheme.secondaryContainer,
                        contentColor = MaterialTheme.colorScheme.onSecondaryContainer,
                    ) {
                        Row(
                            modifier = Modifier.padding(horizontal = 20.dp, vertical = 16.dp),
                            horizontalArrangement = Arrangement.spacedBy(12.dp),
                            verticalAlignment = Alignment.CenterVertically,
                        ) {
                            Column(Modifier.weight(1f)) {
                                Text("Battery settings", style = MaterialTheme.typography.titleMedium)
                                Text(
                                    "Opens Android's battery optimization controls",
                                    style = MaterialTheme.typography.bodySmall,
                                    color = MaterialTheme.colorScheme.onSecondaryContainer.copy(alpha = 0.8f),
                                )
                            }
                            ShapeBackdrop(MaterialShapes.Cookie4Sided, MaterialTheme.colorScheme.secondary, Modifier.size(40.dp)) {
                                Icon(
                                    painterResource(R.drawable.ic_open_in_new),
                                    contentDescription = "Opens Android settings",
                                    modifier = Modifier.size(20.dp),
                                    tint = MaterialTheme.colorScheme.onSecondary,
                                )
                            }
                        }
                    }
                }
            }
        }
    }
}

/** A section's name beside a small icon on an expressive shape. */
@Composable
internal fun SectionHeading(
    text: String,
    polygon: RoundedPolygon,
    icon: androidx.compose.ui.graphics.vector.ImageVector,
    modifier: Modifier = Modifier,
    color: Color = MaterialTheme.colorScheme.primary,
) {
    Row(modifier.padding(horizontal = 2.dp), verticalAlignment = Alignment.CenterVertically) {
        ShapeBackdrop(polygon, color.copy(alpha = 0.18f), Modifier.size(30.dp)) {
            Icon(icon, contentDescription = null, tint = color, modifier = Modifier.size(16.dp))
        }
        Spacer(Modifier.width(10.dp))
        Text(text, style = MaterialTheme.typography.titleMedium, color = color)
    }
}

/** Two sample chips drawn at the chosen size, so the setting shows what it does. */
@Composable
private fun ChipPreview(compact: Boolean) {
    val sample = remember {
        listOfNotNull(
            ItemCatalog.findById("wand_fireblast")?.let { ItemRequirement(key = 1, item = it, upgrade = 3) },
            ItemRequirement(key = 2, item = null, kind = ItemKind.RING, upgrade = 2),
        )
    }
    Surface(
        shape = MaterialTheme.shapes.large,
        color = MaterialTheme.colorScheme.surfaceContainerLowest,
        modifier = Modifier
            .fillMaxWidth()
            .padding(start = 12.dp, end = 12.dp, bottom = 12.dp)
            .clearAndSetSemantics {},
    ) {
        RequirementBoard(
            requirements = sample,
            enabled = false,
            compact = compact,
            onChange = {},
            onEdit = { _, _ -> },
            onRemove = {},
            onAdd = {},
            arcaneResin = 0,
            arcaneResinFilter = ArcaneResinFilter(),
            onEditResin = {},
            onRemoveResin = {},
            modifier = Modifier.padding(12.dp).animateContentSize(LayoutSizeSpring),
        )
    }
}
