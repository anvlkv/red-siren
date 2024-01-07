@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren


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
import com.anvlkv.redsiren.core.typegen.KeyValueOperation
import com.anvlkv.redsiren.core.typegen.KeyValueOutput
import com.anvlkv.redsiren.core.typegen.NavigateOperation
import com.anvlkv.redsiren.core.typegen.PlayOperation
import com.anvlkv.redsiren.core.typegen.PlayOperationOutput
import com.anvlkv.redsiren.core.typegen.Request
import com.anvlkv.redsiren.core.typegen.Requests
import com.anvlkv.redsiren.core.typegen.ViewModel
import com.anvlkv.redsiren.ffirs.AuCoreBridge
import com.anvlkv.redsiren.ffirs.AuReceiver
import com.anvlkv.redsiren.ffirs.auNew
import com.anvlkv.redsiren.ffirs.auReceive
import com.anvlkv.redsiren.ffirs.auRequest
import com.anvlkv.redsiren.ffirs.handleResponse
import com.anvlkv.redsiren.ffirs.logInit
import com.anvlkv.redsiren.ffirs.processEvent
import com.anvlkv.redsiren.ffirs.view
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
                            var response = KeyValueOutput.Read(Optional.empty())
                            if (store != null) {
                                val key = stringPreferencesKey(kv.value)
                                val value = store!!.data.map { kv ->
                                    kv[key] ?: ""
                                }

                                val entry = value.first()

                                if (entry.isNotEmpty()) {
                                    val data = entry.split(",").map {
                                        it.toByte()
                                    }
                                    response = KeyValueOutput.Read(Optional.of(data))
                                }
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
                            store!!.edit { kv ->
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
                coroutineScope {
                    playEffect(effect.value) {response ->
                        launch {
                            val effects =
                                handleResponse(
                                    request.uuid.toByteArray(),
                                    response
                                )
                            val requests = Requests.bincodeDeserialize(effects)
                            for (request in requests) {
                                processEffect(request)
                            }

                            Log.d("redsiren::android", "launched response completed")
                        }
                    }

                    Log.d("redsiren::android", "play effect coroutine completed")
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

    private suspend fun playEffect(value: PlayOperation, onData: (ByteArray) -> Job) {
        when (value) {
            is PlayOperation.Permissions -> {
                var grant = false
                onRequestPermissions?.let { requestPermissions ->
                    val deferred = requestPermissions.invoke()
                    grant = deferred.await()
                }
                onData(PlayOperationOutput.Permission(grant).bincodeSerialize()).join()
            }

            is PlayOperation.InstallAU -> {
                installAu()
                forward(value)?.let {rec ->
                    auReceive(rec)?.let {
                        onData(it).join()
                    }
                }
            }

            else -> {
                forward(value)?.let {rec ->
                    while (true) {
                        val d = auReceive(rec) ?: break
                        onData(d).join()
                    }
                }
            }
        }

        Log.i("redsiren::android", "au effect completed")
    }

    private companion object {
        private var auBridge: AuCoreBridge? = null

        fun installAu() {
            auBridge = auNew()
        }

        fun forward(op: PlayOperation): AuReceiver? {
            return auBridge?.let {
                return auRequest(it, op.bincodeSerialize())
            }
        }
    }
}


