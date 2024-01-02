@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren


import android.content.Context
import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import androidx.datastore.core.DataStore
import androidx.datastore.dataStore
import androidx.datastore.preferences.core.Preferences
import androidx.datastore.preferences.core.edit
import androidx.datastore.preferences.core.stringPreferencesKey
import androidx.datastore.preferences.preferencesDataStore
import androidx.lifecycle.viewModelScope
import com.anvlkv.redsiren.ffirs.AuCoreBridge
import com.anvlkv.redsiren.ffirs.auNew
import com.anvlkv.redsiren.ffirs.auRequest
import com.anvlkv.redsiren.ffirs.handleResponse
import com.anvlkv.redsiren.ffirs.logInit
import com.anvlkv.redsiren.ffirs.processEvent
import com.anvlkv.redsiren.ffirs.view
import com.anvlkv.redsiren.shared.shared_types.Activity
import com.anvlkv.redsiren.shared.shared_types.AnimateOperation
import com.anvlkv.redsiren.shared.shared_types.AnimateOperationOutput
import com.anvlkv.redsiren.shared.shared_types.Effect
import com.anvlkv.redsiren.shared.shared_types.Event
import com.anvlkv.redsiren.shared.shared_types.KeyValueOperation
import com.anvlkv.redsiren.shared.shared_types.KeyValueOutput
import com.anvlkv.redsiren.shared.shared_types.NavigateOperation
import com.anvlkv.redsiren.shared.shared_types.PlayOperation
import com.anvlkv.redsiren.shared.shared_types.PlayOperationOutput
import com.anvlkv.redsiren.shared.shared_types.Request
import com.anvlkv.redsiren.shared.shared_types.Requests
import com.anvlkv.redsiren.shared.shared_types.ViewModel
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.Job
import kotlinx.coroutines.async
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.channels.ReceiveChannel
import kotlinx.coroutines.channels.SendChannel
import kotlinx.coroutines.coroutineScope
import kotlinx.coroutines.flow.first
import kotlinx.coroutines.flow.map
import kotlinx.coroutines.launch
import java.util.Optional


open class Core(val store: DataStore<Preferences>) : androidx.lifecycle.ViewModel() {

    var view: ViewModel by mutableStateOf(ViewModel.bincodeDeserialize(view()))
    var navigateTo: Activity? by mutableStateOf(null)
    var animationSender: SendChannel<Long>? by mutableStateOf(null)


    private val httpClient = HttpClient(CIO)

    var onRequestPermissions: (() -> CompletableDeferred<Boolean>)? = null

    init {
        viewModelScope.launch {
            update(Event.Start())
            logInit()
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

            is Effect.Navigate -> {
                when (val op = effect.value) {
                    is NavigateOperation.To -> {
                        this.navigateTo = op.value
                    }
                }
            }


            is Effect.KeyValue -> {
                when (val kv = effect.value) {
                    is KeyValueOperation.Read -> {

                        coroutineScope {
                            val key = stringPreferencesKey(kv.value)
                            val value = store.data.map { kv ->
                                kv[key] ?: ""
                            }
                            val entry = value.first()



                            var response = KeyValueOutput.Read(Optional.empty())

                            if (entry.isNotEmpty()) {
                                val data = entry.split(",").map {
                                    it.toByte()
                                }
                               response = KeyValueOutput.Read(Optional.of(data))
                            }

                            val effects =
                                handleResponse(request.uuid.toByteArray(), response.bincodeSerialize())
                            val requests = Requests.bincodeDeserialize(effects)
                            for (request in requests) {
                                processEffect(request)
                            }
                        }
                    }

                    is KeyValueOperation.Write -> {
                        coroutineScope {
                            val key = stringPreferencesKey(kv.field0)
                            val data = kv.field1
                            store.edit { kv ->
                                kv[key] = data.joinToString(",")
                            }
                            val response = KeyValueOutput.Write(true)

                            val effects =
                                handleResponse(request.uuid.toByteArray(), response.bincodeSerialize())
                            val requests = Requests.bincodeDeserialize(effects)
                            for (request in requests) {
                                processEffect(request)
                            }
                        }
                    }
                }
            }

            is Effect.Play -> {
                val response = playEffect(effect.value)
                val effects =
                    handleResponse(request.uuid.toByteArray(), response.bincodeSerialize())
                val requests = Requests.bincodeDeserialize(effects)
                for (request in requests) {
                    processEffect(request)
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

    private suspend fun playEffect(value: PlayOperation): PlayOperationOutput {
        when (value) {
            is PlayOperation.Permissions -> {
                var grant = false
                onRequestPermissions?.let { requestPermissions ->
                    val deferred = requestPermissions.invoke()
                    grant = deferred.await()
                }
                return PlayOperationOutput.Permission(grant)
            }

            is PlayOperation.InstallAU -> {
                installAu()
                return forward(value) ?: PlayOperationOutput.Success(false)
            }

            else -> {
                return forward(value) ?: PlayOperationOutput.Success(false)
            }
        }
    }

    private companion object {
        private var auBridge: AuCoreBridge? = null

        fun installAu() {
            auBridge = auNew()
        }

        suspend fun forward(op: PlayOperation): PlayOperationOutput? {
            return auBridge?.let {
                val out = auRequest(it, op.bincodeSerialize())
                PlayOperationOutput.bincodeDeserialize(out)
            }
        }
    }
}


