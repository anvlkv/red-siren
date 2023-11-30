import "./Crypto.js";
import "./TextEncoder.js";
import { initSync, log_init } from "shared/shared";
import {
  EventVariantInstrumentEvent,
  InstrumentEVVariantPlayback,
  InstrumentVM,
  PlaybackEVVariantDataIn,
  ViewModel,
} from "shared_types/types/shared_types";
import { update, update_plain } from "./core";

export class RedSirenWorklet extends AudioWorkletProcessor {
  private vm: InstrumentVM | null = null;
  private core: any;
  constructor() {
    super();

    this.port.onmessage = this.onMessage.bind(this);
  }

  onRender = (vm: ViewModel) => {
    this.vm = vm.instrument;
  };

  onMessage(msg: MessageEvent) {
    try {
      switch (msg.data.type) {
        case "send-wasm-module":
          this.core = initSync(msg.data.wasmBytes);
          log_init();
          console.log("init");
          this.port.postMessage({
            type: "wasm-ready",
          });
          break;
        case "red-siren-ev":
          const ev = msg.data.ev as Uint8Array;
          console.log("event");
          update_plain(ev, this.onRender);
          break;
        default:
          console.log(msg);
          super.port.onmessage && super.port.onmessage(msg);
      }
    } catch (error) {
      console.error(error);
      this.port.postMessage({
        type: "error",
        error,
      });
    }
  }

  process(
    [[inputs]]: Float32Array[][],
    outputs: Float32Array[][],
    parameters: Record<string, Float32Array>
  ): boolean {
    if (!inputs || !this.vm) {
      console.warn("playing no vm")
      return true;
    }

    update(
      new EventVariantInstrumentEvent(new InstrumentEVVariantPlayback(
        new PlaybackEVVariantDataIn([inputs] as unknown as number[][])
      )),
      this.onRender
    );

    if (this.vm.audio_data.length && inputs) {
      for (let output of outputs) {
        for (let ch = 0; ch < output.length; ch++) {
          for (let s = 0; s < output[ch].length; s++) {
            if (this.vm.audio_data[ch] !== undefined) {
              output[ch][s] = this.vm.audio_data[ch][s];
            } else {
              output[ch][s] = this.vm.audio_data[0][s];
            }
          }
        }
      }
    }

    return true;
  }
}

registerProcessor("red-siren", RedSirenWorklet);
