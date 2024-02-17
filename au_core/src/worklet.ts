import * as worklet from "../pkg";

class RedSirenProcessor extends AudioWorkletProcessor {
  constructor() {
    super();
    worklet.instantiate_unit();
    this.port.onmessage = this.onMessage.bind(this);
  }

  onMessage = (msg: MessageEvent) => {
    switch (msg.data.type) {
      case "update":
        worklet.unit_update(msg.data.value);
        break;
      default:
        console.warn("unknown event");
        break;
    }
  };

  process([input], [output]) {
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

    this.port.postMessage({
      type: "snoops_data",
      value: worklet.get_snoops_data(),
    });
    
    this.port.postMessage({
      type: "fft_data",
      value: worklet.get_fft_data(),
    });

    return true;
  }
}

registerProcessor("red-siren-processor", RedSirenProcessor);
