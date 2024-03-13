@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren


import android.content.ContentResolver
import android.provider.Settings
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.datastore.core.DataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.lifecycle.viewModelScope
import com.anvlkv.redsiren.core.typegen.Activity
import com.anvlkv.redsiren.core.typegen.AnimateOperation
import com.anvlkv.redsiren.core.typegen.AnimateOperationOutput
import com.anvlkv.redsiren.core.typegen.Effect
import com.anvlkv.redsiren.core.typegen.Event
import com.anvlkv.redsiren.core.typegen.PlayOperation
import com.anvlkv.redsiren.core.typegen.Request
import com.anvlkv.redsiren.core.typegen.Requests
import com.anvlkv.redsiren.core.typegen.ViewModel
import com.anvlkv.redsiren.core.handleResponse
import com.anvlkv.redsiren.core.logInit
import com.anvlkv.redsiren.core.initializeAndroidContext
import com.anvlkv.redsiren.core.processEvent
import com.anvlkv.redsiren.core.typegen.UnitResolve
import com.anvlkv.redsiren.core.view
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import java.util.Optional


open class Core : androidx.lifecycle.ViewModel() {

    var view: ViewModel by mutableStateOf(ViewModel.bincodeDeserialize(view()))
    var navigateTo: Activity? by mutableStateOf(null)
    var animationSender: SendChannel<Long>? by mutableStateOf(null)
    var store: DataStore<Preferences>? by mutableStateOf(null)

    private val httpClient = HttpClient(CIO)

    var onRequestPermissions: (() -> CompletableDeferred<Boolean>)? = null

    init {
        viewModelScope.launch {
            logInit()
            initializeAndroidContext()
        }
    }

    suspend fun update(event: Event) {
        val effects = processEvent(event.bincodeSerialize())

        val requests = Requests.bincodeDeserialize(effects)
        for (request in requests) {
            processEffect(request)
        }
    }

    private suspend fun processEffect(request: Request) {
        when (val effect = request.effect) {
            is Effect.Render -> {
                this.view = ViewModel.bincodeDeserialize(view())
            }

            is Effect.Play -> {
                Log.i("redsiren::android", "play op")
                when (effect.value) {
                    is PlayOperation.Permissions -> {
                        coroutineScope {
                            getRecordingPermission(request.uuid.toByteArray())
                        }
                    }
                }
            }

            is Effect.Animate -> {
                when (effect.value) {
                    is AnimateOperation.Start -> {
                        Log.i("redsiren::android", "starting animation loop")
                        val channel = Channel<Long>(Channel.CONFLATED)

                        animationSender = channel
                        coroutineScope {
                            animateStream(channel, request.uuid.toByteArray())
                        }
                    }
                    is AnimateOperation.Stop -> {
                        Log.i("redsiren::android", "stopping animation loop")
                        animationSender?.close()
                        animationSender = null
                    }
                }
            }
        }
    }

    private suspend fun getRecordingPermission(uuid: ByteArray) {
        val permission = onRequestPermissions!!().await()
        val response = UnitResolve.RecordingPermission(permission)
        val effects =
            handleResponse(uuid, response.bincodeSerialize())
        val requests = Requests.bincodeDeserialize(effects)
        for (request in requests) {
            processEffect(request)
        }
        Log.d("redsiren::android", "resolved permissions: $permission")
    }



    private suspend fun animateStream(channel: ReceiveChannel<Long>, uuid: ByteArray) {
        do {
            val ts = channel.receiveCatching().getOrNull() ?: break

            val response = AnimateOperationOutput.Timestamp(ts.toDouble())
            val effects =
                handleResponse(uuid, response.bincodeSerialize())
            val requests = Requests.bincodeDeserialize(effects)
            for (request in requests) {
                processEffect(request)
            }
            Log.d("redsiren::android", "animation stream tick")
        } while (true)

        val response = AnimateOperationOutput.Done()

        val effects =
            handleResponse(uuid, response.bincodeSerialize())
        val requests = Requests.bincodeDeserialize(effects)
        for (request in requests) {
            processEffect(request)
        }

        Log.i("redsiren::android", "animation stream loop exited")
    }
}
