import "fastestsmallesttextencoderdecoder/EncoderDecoderTogether.min.js";

import * as worklet from "../pkg";

class RedSirenProcessor extends AudioWorkletProcessor {
  private wasm_ready = false;
  constructor() {
    super();
    this.port.onmessage = this.onMessage.bind(this);
    try {
      worklet.instantiate_unit();
      this.wasm_ready = true;
    } catch (e) {
      console.debug("instantiate on wasm_ready", e);
    }
  }

  onMessage = (msg: MessageEvent) => {
    switch (msg.data.type) {
      case "wasm":
        worklet.initSync(msg.data.value);
        worklet.init_once();
        worklet.instantiate_unit();
        this.wasm_ready = true;
        this.port.postMessage({ type: "wasm_ready" });
        break;
      case "update":
        worklet.unit_update(msg.data.value);
        break;
      default:
        console.warn("unknown event");
        break;
    }
  };

  process([input], [output]) {
    if (this.wasm_ready) {
      if (input && input[0]) {
        const data = worklet.process_samples(input[0]);

        for (let ch = 0; ch < 2; ch++) {
          for (let fr = 0; fr < output[0].length; fr++) {
            const sample = data[fr * 2 + ch];
            if (output[ch]) {
              output[ch][fr] = sample;
            } else {
              output[0][fr] += sample;
              output[0][fr] /= ch + 1;
            }
          }
        }
      }

      const snoops_data = worklet.get_snoops_data();
      if (snoops_data){
        this.port.postMessage({
          type: "snoops_data",
          value: snoops_data,
        });
        
        console.log("send snoops")
      }

      const fft_data = worklet.get_snoops_data();
      if (fft_data) {
        this.port.postMessage({
          type: "fft_data",
          value: fft_data,
        });

        console.log("send fft")
      }
    }

    return true;
  }
}

registerProcessor("red-siren-processor", RedSirenProcessor);
