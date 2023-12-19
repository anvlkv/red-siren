package com.anvlkv.redsiren.app

import android.Manifest
import androidx.compose.foundation.background
import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.fillMaxWidth
import androidx.compose.foundation.layout.padding
import androidx.compose.material3.Button
import androidx.compose.material3.ButtonDefaults
import androidx.compose.material3.ButtonDefaults.buttonColors
import androidx.compose.material3.Card
import androidx.compose.material3.MaterialTheme
import androidx.compose.material3.Text
import androidx.compose.runtime.Composable
import androidx.compose.runtime.rememberCoroutineScope
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.shadow
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.text.style.TextAlign
import androidx.compose.ui.unit.dp
import com.anvlkv.redsiren.MainActivity
import com.anvlkv.redsiren.conditional
import com.anvlkv.redsiren.shared.shared_types.Activity
import com.anvlkv.redsiren.shared.shared_types.Event
import com.anvlkv.redsiren.shared.shared_types.MenuPosition
import com.google.accompanist.permissions.ExperimentalPermissionsApi
import com.google.accompanist.permissions.isGranted
import com.google.accompanist.permissions.rememberPermissionState
import com.google.accompanist.permissions.shouldShowRationale
import kotlinx.coroutines.launch


@OptIn(ExperimentalPermissionsApi::class)
@Composable
fun Menu(expanded: Boolean, flip: Double?, position: MenuPosition) {
    val recordAudioPermissionState = rememberPermissionState(
        Manifest.permission.RECORD_AUDIO
    )
    val shape = MaterialTheme.shapes.extraLarge
    val textColor = MaterialTheme.colorScheme.background
    val backgroundColor = MaterialTheme.colorScheme.primary
    val buttonColors = buttonColors(MaterialTheme.colorScheme.background, backgroundColor)


    val activity = LocalContext.current as MainActivity

    val coroutineScope = rememberCoroutineScope()



    RedCard(modifier = Modifier.conditional(expanded, ifTrue = {
        shadow(8.dp, shape = shape)
    }), flip, position) {
        Text(
            color = textColor,
            text = "Red Siren",
            textAlign = TextAlign.Center,
            style = MaterialTheme.typography.titleLarge,
            modifier = Modifier.weight(0.5f)
        )

        val text = when {
            recordAudioPermissionState.status.shouldShowRationale -> "Red Siren is a noise chime. As an instrument activated by external sounds it requires permission to record audio. Please allow audio recording."

            !recordAudioPermissionState.status.isGranted -> "Red Siren is a noise chime. Please allow audio recording after you click Play or Tune"

            else -> null
        }

        Button(modifier = Modifier
            .fillMaxWidth()
            .weight(1f),
            shape = MaterialTheme.shapes.large,
            colors = buttonColors,
            onClick = {
                coroutineScope.launch {
                    activity.core!!.update(Event.Menu(Activity.Play()))
                }
            }) {
            Text(text = "Play", style = MaterialTheme.typography.titleLarge)
        }

        text?.let {
            Text(
                color = textColor,
                text = it,
                textAlign = TextAlign.Center,
                style = MaterialTheme.typography.bodyLarge,
                modifier = Modifier.weight(1f)
            )
        }

        Button(modifier = Modifier
            .fillMaxWidth()
            .weight(1f),
            shape = MaterialTheme.shapes.large,
            colors = buttonColors,
            onClick = {
                coroutineScope.launch {
                    activity.core!!.update(Event.Menu(Activity.Tune()))
                }
            }) {
            Text(text = "Tune", style = MaterialTheme.typography.titleLarge)
        }

        Button(modifier = Modifier
            .fillMaxWidth()
            .weight(1f),
            shape = MaterialTheme.shapes.large,
            colors = buttonColors,
            onClick = {
                coroutineScope.launch {
                    activity.core!!.update(Event.Menu(Activity.Listen()))
                }
            }) {
            Text(text = "Listen", style = MaterialTheme.typography.titleLarge)
        }

        Button(modifier = Modifier
            .fillMaxWidth()
            .weight(1f),
            shape = MaterialTheme.shapes.large,
            colors = buttonColors,
            onClick = {
                coroutineScope.launch {
                    activity.core!!.update(Event.Menu(Activity.About()))
                }
            }) {
            Text(text = "About", style = MaterialTheme.typography.titleLarge)
        }
    }
}