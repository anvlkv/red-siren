@file:Suppress("NAME_SHADOWING")

package com.anvlkv.redsiren

import android.util.Log
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.lifecycle.viewModelScope
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
import kotlinx.coroutines.launch
import java.util.Optional

open class Core : androidx.lifecycle.ViewModel() {
    var view: ViewModel by mutableStateOf(ViewModel.bincodeDeserialize(view()))
        private set

    var navigateTo: Optional<Activity> = Optional.empty()

    private val httpClient = HttpClient(CIO)

    init {
        viewModelScope.launch {
            update(Event.Start())
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

        }
    }
}

