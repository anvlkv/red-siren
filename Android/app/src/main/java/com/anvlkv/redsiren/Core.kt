@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren

import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import com.anvlkv.redsiren.shared.processEvent
import com.anvlkv.redsiren.shared.view
import com.anvlkv.redsiren.shared_types.Activity
import com.anvlkv.redsiren.shared_types.Effect
import com.anvlkv.redsiren.shared_types.Event
import com.anvlkv.redsiren.shared_types.NavigateOperation
import com.anvlkv.redsiren.shared_types.Request
import com.anvlkv.redsiren.shared_types.Requests
import com.anvlkv.redsiren.shared_types.ViewModel
import io.ktor.client.HttpClient
import io.ktor.client.engine.cio.CIO

open class Core(var navigate_to: (act: Activity) -> Unit) : androidx.lifecycle.ViewModel() {
    var view: ViewModel by mutableStateOf(ViewModel.bincodeDeserialize(view()))
        private set

    private val httpClient = HttpClient(CIO)

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



            is Effect.KeyValue -> {}

            is Effect.Navigate -> {
                when (val op = effect.value) {
                    is NavigateOperation.To -> {
                        this.navigate_to(op.value)
                    }
                }
            }
        }
    }
}

