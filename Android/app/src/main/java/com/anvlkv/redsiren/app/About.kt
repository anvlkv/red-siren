package com.anvlkv.redsiren.app

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.Row
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.lazy.grid.GridCells
import androidx.compose.foundation.lazy.grid.LazyVerticalGrid
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.alpha
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.MainActivity
import com.anvlkv.redsiren.shared.shared_types.Activity
import com.anvlkv.redsiren.shared.shared_types.Event
import com.anvlkv.redsiren.shared.shared_types.IntroEV
import com.anvlkv.redsiren.shared.shared_types.IntroVM
import kotlinx.coroutines.launch

@Composable
fun AppAbout(vm: IntroVM, ev: (ev: IntroEV) -> Unit) {
    val shape = MaterialTheme.shapes.extraLarge
    val textColor = MaterialTheme.colorScheme.background
    val backgroundColor = MaterialTheme.colorScheme.primary
    val buttonColors =
        ButtonDefaults.buttonColors(MaterialTheme.colorScheme.background, backgroundColor)

    val coroutineScope = rememberCoroutineScope()
    val gap = Arrangement.spacedBy(24.dp)
    val activity = LocalContext.current as MainActivity

    Box(modifier = Modifier.fillMaxSize()) {
        IntroDrawing(
            modifier = Modifier
                .fillMaxSize()
        )

        RedCard(
            modifier = Modifier.shadow(8.dp, shape = shape),
            flip = null,
            position = vm.layout.menu_position
        ) {
            Text(
                color = textColor,
                text = "About the Red Siren",
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.titleMedium,
            )

            Text(
                color = textColor,
                text = "Red siren is a noise chime.",
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.bodyLarge,
            )

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "Red",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "The color red and its many meanings.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "Siren",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "Siren - the mythical creature, but also the alarm.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "is",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "It exists right now.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "a",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "It's a choice, one of many, and therefore any.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "noise",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "Random or unwanted sounds.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Row (Modifier.fillMaxWidth(), horizontalArrangement = gap){
                Text(
                    color = textColor,
                    text = "chime",
                    textAlign = TextAlign.Right,
                    style = MaterialTheme.typography.bodyMedium,
                    modifier = Modifier.weight(0.2f)
                )
                Text(
                    color = textColor,
                    text = "The musical instrument.",
                    textAlign = TextAlign.Left,
                    style = MaterialTheme.typography.bodyLarge,
                    modifier = Modifier.weight(0.8f)
                )
            }

            Button(modifier = Modifier
                .fillMaxWidth()
                .weight(1f),
                shape = MaterialTheme.shapes.large,
                colors = buttonColors,
                onClick = {
                    coroutineScope.launch {
                        activity.core!!.update(Event.Menu(Activity.Intro()))
                    }
                }) {
                Text(text = "Clear", style = MaterialTheme.typography.titleLarge)
            }
        }
    }
}