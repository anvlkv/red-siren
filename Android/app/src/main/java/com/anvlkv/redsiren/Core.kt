@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren


import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
import com.anvlkv.redsiren.ffirs.AuCoreBridge
import com.anvlkv.redsiren.ffirs.auNew
import com.anvlkv.redsiren.ffirs.auRequest
import com.anvlkv.redsiren.ffirs.handleResponse
import com.anvlkv.redsiren.ffirs.logInit
import com.anvlkv.redsiren.ffirs.processEvent
import com.anvlkv.redsiren.ffirs.view
import com.anvlkv.redsiren.shared.shared_types.Activity
import com.anvlkv.redsiren.shared.shared_types.Effect
import com.anvlkv.redsiren.shared.shared_types.Event
import com.anvlkv.redsiren.shared.shared_types.NavigateOperation
import com.anvlkv.redsiren.shared.shared_types.PlayOperation
import com.anvlkv.redsiren.shared.shared_types.PlayOperationOutput
import com.anvlkv.redsiren.shared.shared_types.Request
import com.anvlkv.redsiren.shared.shared_types.Requests
import com.anvlkv.redsiren.shared.shared_types.ViewModel
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.launch
import java.util.Optional

open class Core : androidx.lifecycle.ViewModel() {
    var view: ViewModel by mutableStateOf(ViewModel.bincodeDeserialize(view()))

    var navigateTo: Optional<Activity> = Optional.empty()

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
                        this.navigateTo = Optional.of(op.value)
                        this.view = ViewModel.bincodeDeserialize(view())
                    }
                }
            }


            is Effect.KeyValue -> {}

            is Effect.Play -> {
                val response = playEffect(effect.value)
                val effects =
                    handleResponse(request.uuid.toByteArray(), response.bincodeSerialize())
                val requests = Requests.bincodeDeserialize(effects)
                for (request in requests) {
                    processEffect(request)
                }
            }
        }
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


